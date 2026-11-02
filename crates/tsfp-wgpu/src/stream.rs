// SPDX-License-Identifier: GPL-3.0-or-later
//! Recorded render call streams (T2024): the [`RenderDevice`] and [`RenderHost`] calls of the
//! T1898 render code, plus the guest memory the draws read (vertex runs, index arrays) and the
//! texture images, in call order. A stream is what the wgpu replayer consumes, so the renderer
//! is developed without the game, the Bevy shell or the host.
//!
//! The text form is one event per line (`#` starts a comment), all words in hex:
//! `call <Name> <args..>`, `host <function-va> <args..>`, `mem <address> <hex bytes>`,
//! `texture <id> <width> <height> <hex rgba8 bytes>`, `present`. It carries no retail data
//! unless the recorder was fed retail memory, which the committed fixtures never are.
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::rc::Rc;

use tsfp_engine_render::device::{DeviceCall, HostArg, HostFunction, RenderDevice, RenderHost};
use tsfp_engine_render::mem::SparseMemory;
use tsfp_engine_render::memory::GuestMemory;

/// One step of a stream.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A [`RenderDevice`] call.
    Call(DeviceCall),
    /// A [`RenderHost`] call the device cares about (the vertex stream binding).
    Host(u32, Vec<u32>),
    /// Guest bytes that later draws read (applied to the replay image in order).
    Memory { address: u32, bytes: Vec<u8> },
    /// A texture image keyed by the texture record address `set_texture` receives.
    Texture {
        id: u32,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    /// `SetRenderTarget` (slice 5): the colour surface header address `target`, the depth surface header `depth`, then the words
    /// the NV2A surface methods carry (colour `data` = method 0x210 offset, `format` and `size` header words, `pitch`, depth
    /// `depth_data` = method 0x214) and `flags` (bit 0: the target is the device back buffer). Header words are the D3D pixel
    /// container fields, the same ones texture records have, so a texture whose Data word equals `data` samples this surface.
    Surface {
        target: u32,
        depth: u32,
        data: u32,
        format: u32,
        size: u32,
        pitch: u32,
        depth_data: u32,
        flags: u32,
    },
    /// A method the D3D8 library pushed itself (`lib <method> <value>`, slice 5): fog, combiner constants, enables and surface
    /// setup that never cross the RenderDevice boundary. The engine's own pushes stay `call PushMethod`.
    Lib { method: u32, value: u32 },
    /// End of a frame: the replayer resolves the image here.
    Present,
    /// Ordered native view-layer insertion before subsequent framebuffer feedback.
    ViewLayerBoundary,
}

/// An ordered list of events.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stream {
    pub events: Vec<Event>,
}

const HOST_FUNCTIONS: [HostFunction; 18] = [
    HostFunction::BindVertexStream,
    HostFunction::EffectBegin,
    HostFunction::EffectSetup,
    HostFunction::EffectStateSubmit,
    HostFunction::ConstantColour,
    HostFunction::OverlayQuad,
    HostFunction::TextureSelectFallback,
    HostFunction::DrawModelNode,
    HostFunction::DrawSceneObject,
    HostFunction::CharacterOverlay,
    HostFunction::PlayerRecordDraw,
    HostFunction::ObjectVisible,
    HostFunction::ResolveModelField,
    HostFunction::RotationFromAxisAngle,
    HostFunction::TransformVector,
    HostFunction::ParticleTrail,
    HostFunction::MatrixTranspose,
    HostFunction::TransformPointToVec4,
];

fn hex_words(words: &[u32]) -> String {
    let mut text = String::new();
    for word in words {
        let _ = write!(text, " {word:x}");
    }
    text
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

fn parse_bytes(text: &str) -> Option<Vec<u8>> {
    if text == "-" {
        return Some(Vec::new());
    }
    if text.len() % 2 != 0 {
        return None;
    }
    (0..text.len() / 2)
        .map(|index| u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).ok())
        .collect()
}

fn encode_call(call: &DeviceCall) -> String {
    use DeviceCall as C;
    match call {
        C::PushMethod { header, value } => format!("PushMethod{}", hex_words(&[*header, *value])),
        C::SetPixelConstants { index, vectors } => {
            let flat: Vec<u32> = vectors.iter().flatten().map(|f| f.to_bits()).collect();
            format!("SetPixelConstants {index:x}{}", hex_words(&flat))
        }
        C::DeferState { index, value } => format!("DeferState{}", hex_words(&[*index, *value])),
        C::StateHandler { index, value } => format!("StateHandler{}", hex_words(&[*index, *value])),
        C::DeferTextureStageWord { stage, word, value } => {
            format!(
                "DeferTextureStageWord{}",
                hex_words(&[*stage, *word, *value])
            )
        }
        C::SetTexture { stage, texture } => format!("SetTexture{}", hex_words(&[*stage, *texture])),
        C::SetVertexConstants {
            first_register,
            words,
        } => {
            format!("SetVertexConstants {first_register:x}{}", hex_words(words))
        }
        C::SetVertexConstantsUncached {
            first_register,
            words,
        } => {
            format!(
                "SetVertexConstantsUncached {first_register:x}{}",
                hex_words(words)
            )
        }
        C::SetVertexConstantOneVector {
            first_register,
            words,
        } => {
            format!(
                "SetVertexConstantOneVector {first_register:x}{}",
                hex_words(words)
            )
        }
        C::SetVertexConstantFourVectors {
            first_register,
            words,
        } => {
            format!(
                "SetVertexConstantFourVectors {first_register:x}{}",
                hex_words(words)
            )
        }
        C::LoadVertexProgram { program, address } => {
            format!("LoadVertexProgram{}", hex_words(&[*program, *address]))
        }
        C::SelectVertexDeclaration {
            declaration,
            address,
        } => {
            format!(
                "SelectVertexDeclaration{}",
                hex_words(&[*declaration, *address])
            )
        }
        C::SetPixelShader { program } => format!("SetPixelShader{}", hex_words(&[*program])),
        C::DrawVertices {
            primitive,
            start,
            count,
        } => {
            format!("DrawVertices{}", hex_words(&[*primitive, *start, *count]))
        }
        C::DrawIndexedVertices {
            primitive,
            count,
            indices,
        } => {
            format!(
                "DrawIndexedVertices{}",
                hex_words(&[*primitive, *count, *indices])
            )
        }
        C::SetScissor {
            count,
            flags,
            rects,
        } => {
            let mut text = format!("SetScissor{}", hex_words(&[*count, *flags]));
            for rect in rects {
                let _ = write!(text, " {} {} {} {}", rect[0], rect[1], rect[2], rect[3]);
            }
            text
        }
        C::SetVertexBindings { formats, offsets } => {
            let mut words = Vec::with_capacity(32);
            words.extend_from_slice(formats);
            words.extend_from_slice(offsets);
            format!("SetVertexBindings{}", hex_words(&words))
        }
        C::InsertFence => "InsertFence".into(),
        C::BlockUntilFence { fence } => format!("BlockUntilFence{}", hex_words(&[*fence])),
        C::LockVertexBuffer { buffer, flags } => {
            format!("LockVertexBuffer{}", hex_words(&[*buffer, *flags]))
        }
        C::BeginPush { dwords } => format!("BeginPush{}", hex_words(&[*dwords])),
        C::EndPush { cursor } => format!("EndPush{}", hex_words(&[*cursor])),
    }
}

fn decode_call(name: &str, args: &[&str]) -> Option<DeviceCall> {
    let words: Vec<u32> = args
        .iter()
        .map(|arg| u32::from_str_radix(arg, 16).ok())
        .collect::<Option<_>>()
        .or_else(|| (name == "SetScissor").then(Vec::new))?;
    decode_call_words(name, &words, args)
}

/// [`decode_call`] over already parsed words (`args` only matters for `SetScissor`, whose corners are decimal text; the binary
/// reader builds that call itself and never gets here with it).
pub(crate) fn decode_call_words(name: &str, words: &[u32], args: &[&str]) -> Option<DeviceCall> {
    use DeviceCall as C;
    let word = |index: usize| words.get(index).copied();
    Some(match name {
        "PushMethod" => C::PushMethod {
            header: word(0)?,
            value: word(1)?,
        },
        "SetPixelConstants" => C::SetPixelConstants {
            index: word(0)?,
            vectors: words[1..]
                .chunks_exact(4)
                .map(|c| [c[0], c[1], c[2], c[3]].map(f32::from_bits))
                .collect(),
        },
        "DeferState" => C::DeferState {
            index: word(0)?,
            value: word(1)?,
        },
        "StateHandler" => C::StateHandler {
            index: word(0)?,
            value: word(1)?,
        },
        "DeferTextureStageWord" => C::DeferTextureStageWord {
            stage: word(0)?,
            word: word(1)?,
            value: word(2)?,
        },
        "SetTexture" => C::SetTexture {
            stage: word(0)?,
            texture: word(1)?,
        },
        "SetVertexConstants" => C::SetVertexConstants {
            first_register: word(0)?,
            words: words[1..].to_vec(),
        },
        "SetVertexConstantsUncached" => C::SetVertexConstantsUncached {
            first_register: word(0)?,
            words: words[1..].to_vec(),
        },
        "SetVertexConstantOneVector" => C::SetVertexConstantOneVector {
            first_register: word(0)?,
            words: words[1..].to_vec(),
        },
        "SetVertexConstantFourVectors" => C::SetVertexConstantFourVectors {
            first_register: word(0)?,
            words: words[1..].to_vec(),
        },
        "SetVertexBindings" => C::SetVertexBindings {
            formats: words.get(0..16)?.try_into().ok()?,
            offsets: words.get(16..32)?.try_into().ok()?,
        },
        "LoadVertexProgram" => C::LoadVertexProgram {
            program: word(0)?,
            address: word(1)?,
        },
        "SelectVertexDeclaration" => C::SelectVertexDeclaration {
            declaration: word(0)?,
            address: word(1)?,
        },
        "SetPixelShader" => C::SetPixelShader { program: word(0)? },
        "DrawVertices" => C::DrawVertices {
            primitive: word(0)?,
            start: word(1)?,
            count: word(2)?,
        },
        "DrawIndexedVertices" => C::DrawIndexedVertices {
            primitive: word(0)?,
            count: word(1)?,
            indices: word(2)?,
        },
        "SetScissor" => {
            // count and flags are hex, the rectangle corners are signed decimal.
            let count = u32::from_str_radix(args.first()?, 16).ok()?;
            let flags = u32::from_str_radix(args.get(1)?, 16).ok()?;
            let numbers: Vec<i32> = args[2..]
                .iter()
                .map(|a| a.parse().ok())
                .collect::<Option<_>>()?;
            C::SetScissor {
                count,
                flags,
                rects: numbers
                    .chunks_exact(4)
                    .map(|c| [c[0], c[1], c[2], c[3]])
                    .collect(),
            }
        }
        "InsertFence" => C::InsertFence,
        "BlockUntilFence" => C::BlockUntilFence { fence: word(0)? },
        "LockVertexBuffer" => C::LockVertexBuffer {
            buffer: word(0)?,
            flags: word(1)?,
        },
        "BeginPush" => C::BeginPush { dwords: word(0)? },
        "EndPush" => C::EndPush { cursor: word(0)? },
        _ => return None,
    })
}

impl Stream {
    /// Serialise to the text form.
    pub fn to_text(&self) -> String {
        let mut text = String::from("# tsfp-wgpu stream v1 (T2024)\n");
        for event in &self.events {
            match event {
                Event::Call(call) => {
                    let _ = writeln!(text, "call {}", encode_call(call));
                }
                Event::Host(function, args) => {
                    let _ = writeln!(text, "host {function:x}{}", hex_words(args));
                }
                Event::Memory { address, bytes } => {
                    let body = if bytes.is_empty() {
                        "-".to_string()
                    } else {
                        hex_bytes(bytes)
                    };
                    let _ = writeln!(text, "mem {address:x} {body}");
                }
                Event::Texture {
                    id,
                    width,
                    height,
                    rgba,
                } => {
                    let _ = writeln!(
                        text,
                        "texture {id:x} {width:x} {height:x} {}",
                        hex_bytes(rgba)
                    );
                }
                Event::Surface {
                    target,
                    depth,
                    data,
                    format,
                    size,
                    pitch,
                    depth_data,
                    flags,
                } => {
                    let _ = writeln!(
                        text,
                        "call SetSurface{}",
                        hex_words(&[
                            *target,
                            *depth,
                            *data,
                            *format,
                            *size,
                            *pitch,
                            *depth_data,
                            *flags
                        ])
                    );
                }
                Event::Lib { method, value } => {
                    let _ = writeln!(text, "lib {method:x} {value:x}");
                }
                Event::Present => text.push_str("present\n"),
                Event::ViewLayerBoundary => text.push_str("view-layer-boundary\n"),
            }
        }
        text
    }

    /// Parse the text form. The error names the 1-based line.
    pub fn from_text(text: &str) -> Result<Self, String> {
        let mut events = Vec::new();
        for (number, line) in text.lines().enumerate() {
            if let Some(event) = Self::parse_line(number, line)? {
                events.push(event);
            }
        }
        Ok(Self { events })
    }

    /// Parse one line of the text form: `None` for blank lines and comments. `number` (0 based) only names the line in errors.
    pub fn parse_line(number: usize, line: &str) -> Result<Option<Event>, String> {
        {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return Ok(None);
            }
            let fail = || format!("line {}: cannot parse {line:?}", number + 1);
            let fields: Vec<&str> = line.split_whitespace().collect();
            let hex = |index: usize| {
                fields
                    .get(index)
                    .and_then(|f| u32::from_str_radix(f, 16).ok())
                    .ok_or_else(fail)
            };
            let event = match fields[0] {
                "call" if fields.get(1) == Some(&"SetSurface") => Event::Surface {
                    target: hex(2)?,
                    depth: hex(3)?,
                    data: hex(4)?,
                    format: hex(5)?,
                    size: hex(6)?,
                    pitch: hex(7)?,
                    depth_data: hex(8)?,
                    flags: hex(9)?,
                },
                "call" => Event::Call(
                    decode_call(fields.get(1).ok_or_else(fail)?, &fields[2..]).ok_or_else(fail)?,
                ),
                "host" => Event::Host(
                    hex(1)?,
                    fields[2..]
                        .iter()
                        .map(|f| u32::from_str_radix(f, 16).map_err(|_| fail()))
                        .collect::<Result<_, _>>()?,
                ),
                "mem" => Event::Memory {
                    address: hex(1)?,
                    bytes: parse_bytes(fields.get(2).ok_or_else(fail)?).ok_or_else(fail)?,
                },
                "texture" => Event::Texture {
                    id: hex(1)?,
                    width: hex(2)?,
                    height: hex(3)?,
                    rgba: parse_bytes(fields.get(4).ok_or_else(fail)?).ok_or_else(fail)?,
                },
                "lib" => Event::Lib {
                    method: hex(1)?,
                    value: hex(2)?,
                },
                "present" => Event::Present,
                "view-layer-boundary" if fields.len() == 1 => Event::ViewLayerBoundary,
                _ => return Err(fail()),
            };
            Ok(Some(event))
        }
    }

    /// Number of device calls of each kind (debug names), for the replay report.
    pub fn call_histogram(&self) -> BTreeMap<String, usize> {
        let mut map = BTreeMap::new();
        for event in &self.events {
            if let Event::Call(call) = event {
                let name = encode_call(call);
                let name = name.split_whitespace().next().unwrap_or("").to_string();
                *map.entry(name).or_insert(0) += 1;
            }
        }
        map
    }
}

/// A guest memory handle that can be cloned: the renderer holds one, the recorder another, and
/// every access borrows only for its own duration.
#[derive(Clone, Default)]
pub struct SharedMemory(Rc<RefCell<SparseMemory>>);

impl SharedMemory {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn read_bytes(&self, address: u32, length: u32) -> Vec<u8> {
        let memory = self.0.borrow();
        (0..length)
            .map(|offset| memory.read8(address.wrapping_add(offset)))
            .collect()
    }
}

impl GuestMemory for SharedMemory {
    fn read8(&self, address: u32) -> u8 {
        self.0.borrow().read8(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().write8(address, value);
    }
}

#[derive(Default)]
struct Inner {
    events: Vec<Event>,
    /// When set, events go here instead of `events` (the live renderer, `live.rs`).
    sink: Option<Box<dyn FnMut(Event)>>,
    /// Last `BindVertexStream` arguments: vertex data base address and stride.
    bound: Option<(u32, u32)>,
    fence: u32,
    lock_address: u32,
    push_cursor: u32,
}

impl Inner {
    fn push(&mut self, event: Event) {
        match self.sink.as_mut() {
            Some(sink) => sink(event),
            None => self.events.push(event),
        }
    }
}

/// Records a stream from the T1898 render code. Hand the renderer [`Recorder::device`],
/// [`Recorder::host`] and [`Recorder::memory`], run it, then [`Recorder::finish`].
#[derive(Clone, Default)]
pub struct Recorder {
    inner: Rc<RefCell<Inner>>,
    memory: SharedMemory,
}

impl Recorder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn memory(&self) -> SharedMemory {
        self.memory.clone()
    }
    pub fn device(&self) -> RecorderDevice {
        RecorderDevice {
            recorder: self.clone(),
        }
    }
    pub fn host(&self) -> RecorderHost {
        RecorderHost {
            recorder: self.clone(),
        }
    }
    /// The address `lock_vertex_buffer` answers (the base of the vertex ring in the image).
    pub fn set_lock_address(&self, address: u32) {
        self.inner.borrow_mut().lock_address = address;
    }
    pub fn push_texture(&self, id: u32, width: u32, height: u32, rgba: Vec<u8>) {
        assert_eq!(
            rgba.len() as u32,
            width * height * 4,
            "texture {id:#x} size"
        );
        self.emit(Event::Texture {
            id,
            width,
            height,
            rgba,
        });
    }
    pub fn present(&self) {
        self.emit(Event::Present);
    }
    pub fn finish(&self) -> Stream {
        Stream {
            events: std::mem::take(&mut self.inner.borrow_mut().events),
        }
    }

    /// Route events into `sink` (called in order, as they happen) instead of keeping a [`Stream`].
    pub fn set_sink(&self, sink: Box<dyn FnMut(Event)>) {
        self.inner.borrow_mut().sink = Some(sink);
    }

    /// Hand one event to the sink, or keep it for [`Recorder::finish`].
    pub fn emit(&self, event: Event) {
        self.inner.borrow_mut().push(event);
    }

    fn call(&self, call: DeviceCall) {
        self.emit(Event::Call(call));
    }

    /// Snapshot the guest bytes a draw reads, ahead of the draw event.
    fn snapshot(&self, address: u32, length: u32) {
        if length == 0 {
            return;
        }
        let bytes = self.memory.read_bytes(address, length);
        self.emit(Event::Memory { address, bytes });
    }
}

/// The [`RenderDevice`] face of a [`Recorder`].
pub struct RecorderDevice {
    recorder: Recorder,
}

impl RenderDevice for RecorderDevice {
    fn push_method(&mut self, header: u32, value: u32) {
        self.recorder.call(DeviceCall::PushMethod { header, value });
    }
    fn defer_state(&mut self, index: u32, value: u32) {
        self.recorder.call(DeviceCall::DeferState { index, value });
    }
    fn state_handler(&mut self, index: u32, value: u32) {
        self.recorder
            .call(DeviceCall::StateHandler { index, value });
    }
    fn defer_texture_stage_word(&mut self, stage: u32, word: u32, value: u32) {
        self.recorder
            .call(DeviceCall::DeferTextureStageWord { stage, word, value });
    }
    fn set_texture(&mut self, stage: u32, texture: u32) {
        self.recorder
            .call(DeviceCall::SetTexture { stage, texture });
    }
    fn set_vertex_bindings(&mut self, formats: [u32; 16], offsets: [u32; 16]) {
        self.recorder
            .call(DeviceCall::SetVertexBindings { formats, offsets });
    }
    fn set_vertex_constants(&mut self, first_register: u32, words: &[u32]) {
        self.recorder.call(DeviceCall::SetVertexConstants {
            first_register,
            words: words.to_vec(),
        });
    }
    fn set_vertex_constants_uncached(&mut self, first_register: u32, words: &[u32]) {
        self.recorder.call(DeviceCall::SetVertexConstantsUncached {
            first_register,
            words: words.to_vec(),
        });
    }
    fn set_vertex_constant_one_vector(&mut self, first_register: u32, words: &[u32]) {
        self.recorder.call(DeviceCall::SetVertexConstantOneVector {
            first_register,
            words: words.to_vec(),
        });
    }
    fn set_vertex_constant_four_vectors(&mut self, first_register: u32, words: &[u32]) {
        self.recorder
            .call(DeviceCall::SetVertexConstantFourVectors {
                first_register,
                words: words.to_vec(),
            });
    }
    fn load_vertex_program(&mut self, program: u32, address: u32) {
        self.recorder
            .call(DeviceCall::LoadVertexProgram { program, address });
    }
    fn select_vertex_declaration(&mut self, declaration: u32, address: u32) {
        self.recorder.call(DeviceCall::SelectVertexDeclaration {
            declaration,
            address,
        });
    }
    fn set_pixel_shader(&mut self, program: u32) {
        self.recorder.call(DeviceCall::SetPixelShader { program });
    }
    fn draw_vertices(&mut self, primitive: u32, start: u32, count: u32) {
        let bound = self.recorder.inner.borrow().bound;
        if let Some((base, stride)) = bound {
            self.recorder.snapshot(
                base.wrapping_add(start.wrapping_mul(stride)),
                count.wrapping_mul(stride),
            );
        }
        self.recorder.call(DeviceCall::DrawVertices {
            primitive,
            start,
            count,
        });
    }
    fn draw_indexed_vertices(&mut self, primitive: u32, count: u32, indices: u32) {
        let bound = self.recorder.inner.borrow().bound;
        self.recorder.snapshot(indices, count.wrapping_mul(2));
        if let Some((base, stride)) = bound {
            let highest = (0..count)
                .map(|index| self.recorder.memory.read16(indices.wrapping_add(index * 2)))
                .max()
                .map_or(0, u32::from);
            self.recorder
                .snapshot(base, (highest + 1).wrapping_mul(stride));
        }
        self.recorder.call(DeviceCall::DrawIndexedVertices {
            primitive,
            count,
            indices,
        });
    }
    fn set_scissor(&mut self, count: u32, flags: u32, rects: &[[i32; 4]]) {
        self.recorder.call(DeviceCall::SetScissor {
            count,
            flags,
            rects: rects.to_vec(),
        });
    }
    fn insert_fence(&mut self) -> u32 {
        self.recorder.call(DeviceCall::InsertFence);
        self.recorder.inner.borrow().fence
    }
    fn block_until_fence(&mut self, fence: u32) {
        self.recorder.call(DeviceCall::BlockUntilFence { fence });
    }
    fn lock_vertex_buffer(&mut self, buffer: u32, flags: u32) -> u32 {
        self.recorder
            .call(DeviceCall::LockVertexBuffer { buffer, flags });
        self.recorder.inner.borrow().lock_address
    }
    fn set_pixel_constants(&mut self, index: u32, vectors: &[[f32; 4]]) {
        self.recorder.call(DeviceCall::SetPixelConstants {
            index,
            vectors: vectors.to_vec(),
        });
    }
    fn begin_push(&mut self, dwords: u32) -> u32 {
        self.recorder.call(DeviceCall::BeginPush { dwords });
        self.recorder.inner.borrow().push_cursor
    }
    fn end_push(&mut self, cursor: u32) {
        self.recorder.call(DeviceCall::EndPush { cursor });
    }
}

/// The [`RenderHost`] face of a [`Recorder`]: keeps the vertex stream binding, answers 0 to
/// everything else.
pub struct RecorderHost {
    recorder: Recorder,
}

impl RenderHost for RecorderHost {
    fn call(&mut self, function: HostFunction, args: &[u32]) -> u32 {
        let mut inner = self.recorder.inner.borrow_mut();
        if function == HostFunction::BindVertexStream && args.len() == 2 {
            inner.bound = Some((args[0], args[1]));
        }
        if HOST_FUNCTIONS.contains(&function) {
            inner.push(Event::Host(function as u32, args.to_vec()));
        }
        0
    }
    fn call_buffers(
        &mut self,
        function: HostFunction,
        _args: &[HostArg],
        _buffers: &mut [Vec<u32>],
    ) -> u32 {
        self.recorder.emit(Event::Host(function as u32, Vec::new()));
        0
    }
    fn call_address(&mut self, target: u32, _args: &[u32]) -> u32 {
        self.recorder.emit(Event::Host(target, Vec::new()));
        0
    }
}

/// Replay-side vertex stream binding from a recorded host event.
pub fn is_bind_vertex_stream(function: u32) -> bool {
    function == HostFunction::BindVertexStream as u32
}

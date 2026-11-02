// SPDX-License-Identifier: GPL-3.0-or-later
//! The render attachment point (T1898): [`RenderDevice`] is the D3D8 library boundary of the
//! original (the `0x003D....` entry points the game calls) and [`RenderHost`] names the game
//! functions of other systems the render code still calls. Both are traits so a recorded-call
//! device, the guest-backed hybrid device and a future Vulkan or software backend can stand
//! behind the same Rust render code.
//!
//! Every method names the original entry in its doc comment. Calling conventions are
//! recorded there too. The names are INFERRED from the shapes, the XDK D3D8 names in
//! docs/d3d8-usage.md and docs/d3d8-shader-binding.md, not from symbols.
use alloc::vec::Vec;

/// Packed arguments of one scissor rectangle: left, top, right, bottom.
pub type ScissorRect = [i32; 4];

/// The D3D8 boundary as the render code sees it. Legacy calls carry raw guest dwords; native
/// operations such as vertex-array bindings expose the state they represent as typed values.
pub trait RenderDevice {
    /// `0x3D6C90` (fastcall: ECX header, EDX value). Emit one NV2A method header and one
    /// parameter into the push buffer.
    fn push_method(&mut self, header: u32, value: u32);
    /// The inlined deferred-state arm of `SetRenderState` for indices `0x5C..0x88`: OR the
    /// per-state dirty bit into the dirty mask `0x3E3AB8` and store `value` in the shadow
    /// `0x3E3CC0 + index * 4`.
    fn defer_state(&mut self, index: u32, value: u32);
    /// The 30 per-state handlers for indices `0x88..0xA6` (stdcall, one argument).
    /// [`crate::state::D3D8_STATE_HANDLERS`] lists the entry of each index.
    fn state_handler(&mut self, index: u32, value: u32);
    /// Texture stage control word `word` (0 or 1) of `stage`: set the dirty bit `1 << stage`
    /// in `0x3E3AB8` and store `value` at `0x3E3AC0 + stage * 0x80 + word * 4`.
    fn defer_texture_stage_word(&mut self, stage: u32, word: u32, value: u32);
    /// `0x3D4070` (stdcall: stage, texture) `SetTexture`.
    fn set_texture(&mut self, stage: u32, texture: u32);
    /// `0x3D58B0` (fastcall: ECX first register, EDX source, stack dword count) vertex
    /// shader constants, arbitrary dword count.
    fn set_vertex_constants(&mut self, first_register: u32, words: &[u32]);
    /// `0x3D57D0` (fastcall: ECX first register, EDX source, stack dword count) the uncached
    /// variant of the constant upload that the effect setups use.
    fn set_vertex_constants_uncached(&mut self, first_register: u32, words: &[u32]);
    /// `0x3D5670` (fastcall: ECX first register, EDX source) one vector. Unreachable from the
    /// ported callers (the run length is always a multiple of four vectors).
    fn set_vertex_constant_one_vector(&mut self, first_register: u32, words: &[u32]);
    /// `0x3D5720` (fastcall: ECX first register, EDX source) four vectors.
    fn set_vertex_constant_four_vectors(&mut self, first_register: u32, words: &[u32]);
    /// `0x3D59E0` (stdcall: program, address) load a vertex shader program.
    fn load_vertex_program(&mut self, program: u32, address: u32);
    /// `0x3D5630` (stdcall: declaration, address) select a vertex shader declaration.
    fn select_vertex_declaration(&mut self, declaration: u32, address: u32);
    /// `0x3D92E0` (stdcall: program) `SetPixelShader`.
    fn set_pixel_shader(&mut self, program: u32);
    /// `0x3D4FB0` (stdcall: primitive, start, count) `DrawVertices`.
    fn draw_vertices(&mut self, primitive: u32, start: u32, count: u32);
    /// `0x3D5050` (stdcall: primitive, count, indices) `DrawIndexedVertices`. `indices` is a
    /// guest address of 16 bit indices.
    fn draw_indexed_vertices(&mut self, primitive: u32, count: u32, indices: u32);
    /// `0x3D4470` (stdcall: count, flags, rectangles) scissor rectangles.
    fn set_scissor(&mut self, count: u32, flags: u32, rects: &[ScissorRect]);
    /// Native form of the engine's `0x1EB00` vertex-array push block. `formats[i]` and
    /// `offsets[i]` become NV2A methods `0x1760 + 4*i` and `0x1720 + 4*i` respectively.
    /// Backends can consume the typed state without constructing a guest push-buffer packet.
    fn set_vertex_bindings(&mut self, formats: [u32; 16], offsets: [u32; 16]);
    /// `0x3D3630` insert a fence, returns its value.
    fn insert_fence(&mut self) -> u32;
    /// `0x3D34B0` (stdcall: fence) block until the fence is reached.
    fn block_until_fence(&mut self, fence: u32);
    /// `0x3D4F30` (stdcall: buffer, flags) lock a vertex buffer, returns the data address.
    fn lock_vertex_buffer(&mut self, buffer: u32, flags: u32) -> u32;
    /// `0x3D9520` (stdcall: index, vectors, count) pixel shader constants `index..`, four
    /// floats each.
    fn set_pixel_constants(&mut self, index: u32, vectors: &[[f32; 4]]);
    /// `0x3D6660` (stdcall: dwords) `BeginPush`, returns the push buffer cursor.
    fn begin_push(&mut self, dwords: u32) -> u32;
    /// `0x3D6680` (stdcall: cursor) `EndPush`.
    fn end_push(&mut self, cursor: u32);
}

/// Game functions of other systems that the render code calls and that are not ported to
/// Rust yet. The discriminants are the original addresses. Each variant is an attachment
/// point: replacing it with a Rust call is how the neighbouring system joins the renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum HostFunction {
    /// `0x19240` set_stream_source_by_slot (cdecl: vertex buffer base, vertex stride) binds the vertex stream.
    BindVertexStream = 0x0001_9240,
    /// `0x21060` (no arguments) resets the id table `0x55FC98` to -1, first half of the effect
    /// start that the draw prepare step runs when a declaration is invalid or the batch has
    /// extra flags.
    EffectBegin = 0x0002_1060,
    /// `0x22740` (no arguments) second half of the effect start: sets four words at
    /// `0x56516C` and resets twelve sentinels.
    EffectSetup = 0x0002_2740,
    /// `0x210A0` (cdecl: batch extra flags) submits the cached effect state (registry name
    /// `xg_vertex_shader_buffer_length`).
    EffectStateSubmit = 0x0002_10A0,
    /// `0x1E6B0` (no arguments) the constant colour word `0x55FB14` of the current effect, result in EAX.
    ConstantColour = 0x0001_E6B0,
    /// `0x1F790` (cdecl, five arguments: -6, -1, 1, 0, 0 from the effect code) sets texture
    /// stage 0 to a special id with record select (the additive overlay of the effect code).
    OverlayQuad = 0x0001_F790,
    /// `0x1C620` (cdecl: index, -1, -1, 0, 0) texture stage 0 slot set with special ids, the
    /// fallback for indices the record table does not hold.
    TextureSelectFallback = 0x0001_C620,
    /// `0x18EA90` (cdecl: model, node index, arg2, arg3) draws one model node.
    DrawModelNode = 0x0018_EA90,
    /// `0x8F950` (cdecl: object model record, viewport, 0) model node draw with LOD swap by
    /// squared distance and the colour preset for the node.
    DrawSceneObject = 0x0008_F950,
    /// `0x1B90A0` (cdecl: object, viewport) weapon slot record callback hook, draws the
    /// laser fog when the argument is 1 (registry name).
    CharacterOverlay = 0x001B_90A0,
    /// `0x1265B0` (cdecl: player record + 0xC68, viewport) render object effect overlay: sets
    /// the state bytes and the scissor to the object bounds (registry name).
    PlayerRecordDraw = 0x0012_65B0,
    /// `0xA1AB0` (cdecl: object, 0) per player visibility of an object (field `+0x2B4`), result in EAX.
    ObjectVisible = 0x000A_1AB0,
    /// `0x8E670` (the argument travels in ESI) keyframe index lookup by elapsed time, cached
    /// per frame stamp (registry name), used to resolve a model record field.
    ResolveModelField = 0x0008_E670,
    /// `0x152770` (cdecl: out matrix buffer, axis buffer, angle bits) builds a 4x4 rotation
    /// matrix from an axis and an angle.
    RotationFromAxisAngle = 0x0015_2770,
    /// `0x152090` (cdecl: matrix buffer, vector address) rotates the vector at the address.
    TransformVector = 0x0015_2090,
    /// `0x10F810` (cdecl: emitter, particle) spawns the trail particle of a particle.
    ParticleTrail = 0x0010_F810,
    /// `0x1530E0` (cdecl: out matrix buffer, matrix address) 4x4 matrix transpose.
    MatrixTranspose = 0x0015_30E0,
    /// `0x151FC0` (cdecl: matrix address, vector3 address, out vector4 address) transforms a
    /// point by a 4x4 matrix.
    TransformPointToVec4 = 0x0015_1FC0,
}

/// An argument of [`RenderHost::call_buffers`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostArg {
    /// A plain dword.
    Word(u32),
    /// The guest address of the scratch copy of buffer `index`.
    Buffer(usize),
}

/// The cross-system calls the render code makes.
pub trait RenderHost {
    /// Call `function` with cdecl `args`, returns EAX. Float arguments travel as raw bits.
    fn call(&mut self, function: HostFunction, args: &[u32]) -> u32;
    /// Like [`RenderHost::call`] for functions that take pointers to local arrays: every
    /// buffer is copied to guest scratch memory, `HostArg::Buffer(index)` becomes its address,
    /// and the buffers are read back after the call.
    fn call_buffers(
        &mut self,
        function: HostFunction,
        args: &[HostArg],
        buffers: &mut [Vec<u32>],
    ) -> u32;
    /// Call the guest function at `target` (a function pointer read from guest memory, for
    /// example the handlers of an effect row) with cdecl `args`, returns EAX.
    fn call_address(&mut self, target: u32, args: &[u32]) -> u32;
}

/// One call on a [`RecordingDevice`].
#[derive(Clone, Debug, PartialEq)]
pub enum DeviceCall {
    PushMethod {
        header: u32,
        value: u32,
    },
    SetPixelConstants {
        index: u32,
        vectors: Vec<[f32; 4]>,
    },
    DeferState {
        index: u32,
        value: u32,
    },
    StateHandler {
        index: u32,
        value: u32,
    },
    DeferTextureStageWord {
        stage: u32,
        word: u32,
        value: u32,
    },
    SetTexture {
        stage: u32,
        texture: u32,
    },
    SetVertexConstants {
        first_register: u32,
        words: Vec<u32>,
    },
    SetVertexConstantsUncached {
        first_register: u32,
        words: Vec<u32>,
    },
    SetVertexConstantOneVector {
        first_register: u32,
        words: Vec<u32>,
    },
    SetVertexConstantFourVectors {
        first_register: u32,
        words: Vec<u32>,
    },
    LoadVertexProgram {
        program: u32,
        address: u32,
    },
    SelectVertexDeclaration {
        declaration: u32,
        address: u32,
    },
    SetPixelShader {
        program: u32,
    },
    DrawVertices {
        primitive: u32,
        start: u32,
        count: u32,
    },
    DrawIndexedVertices {
        primitive: u32,
        count: u32,
        indices: u32,
    },
    SetScissor {
        count: u32,
        flags: u32,
        rects: Vec<ScissorRect>,
    },
    SetVertexBindings {
        formats: [u32; 16],
        offsets: [u32; 16],
    },
    InsertFence,
    BlockUntilFence {
        fence: u32,
    },
    LockVertexBuffer {
        buffer: u32,
        flags: u32,
    },
    BeginPush {
        dwords: u32,
    },
    EndPush {
        cursor: u32,
    },
}

/// Records every call. `fence`, `lock_address` and `push_cursor` are the scripted answers.
#[derive(Clone, Debug, Default)]
pub struct RecordingDevice {
    pub calls: Vec<DeviceCall>,
    pub fence: u32,
    pub lock_address: u32,
    pub push_cursor: u32,
}

impl RenderDevice for RecordingDevice {
    fn push_method(&mut self, header: u32, value: u32) {
        self.calls.push(DeviceCall::PushMethod { header, value });
    }
    fn defer_state(&mut self, index: u32, value: u32) {
        self.calls.push(DeviceCall::DeferState { index, value });
    }
    fn state_handler(&mut self, index: u32, value: u32) {
        self.calls.push(DeviceCall::StateHandler { index, value });
    }
    fn defer_texture_stage_word(&mut self, stage: u32, word: u32, value: u32) {
        self.calls
            .push(DeviceCall::DeferTextureStageWord { stage, word, value });
    }
    fn set_texture(&mut self, stage: u32, texture: u32) {
        self.calls.push(DeviceCall::SetTexture { stage, texture });
    }
    fn set_vertex_constants(&mut self, first_register: u32, words: &[u32]) {
        self.calls.push(DeviceCall::SetVertexConstants {
            first_register,
            words: words.to_vec(),
        });
    }
    fn set_vertex_constants_uncached(&mut self, first_register: u32, words: &[u32]) {
        self.calls.push(DeviceCall::SetVertexConstantsUncached {
            first_register,
            words: words.to_vec(),
        });
    }
    fn set_vertex_constant_one_vector(&mut self, first_register: u32, words: &[u32]) {
        self.calls.push(DeviceCall::SetVertexConstantOneVector {
            first_register,
            words: words.to_vec(),
        });
    }
    fn set_vertex_constant_four_vectors(&mut self, first_register: u32, words: &[u32]) {
        self.calls.push(DeviceCall::SetVertexConstantFourVectors {
            first_register,
            words: words.to_vec(),
        });
    }
    fn load_vertex_program(&mut self, program: u32, address: u32) {
        self.calls
            .push(DeviceCall::LoadVertexProgram { program, address });
    }
    fn select_vertex_declaration(&mut self, declaration: u32, address: u32) {
        self.calls.push(DeviceCall::SelectVertexDeclaration {
            declaration,
            address,
        });
    }
    fn set_pixel_shader(&mut self, program: u32) {
        self.calls.push(DeviceCall::SetPixelShader { program });
    }
    fn draw_vertices(&mut self, primitive: u32, start: u32, count: u32) {
        self.calls.push(DeviceCall::DrawVertices {
            primitive,
            start,
            count,
        });
    }
    fn draw_indexed_vertices(&mut self, primitive: u32, count: u32, indices: u32) {
        self.calls.push(DeviceCall::DrawIndexedVertices {
            primitive,
            count,
            indices,
        });
    }
    fn set_scissor(&mut self, count: u32, flags: u32, rects: &[ScissorRect]) {
        self.calls.push(DeviceCall::SetScissor {
            count,
            flags,
            rects: rects.to_vec(),
        });
    }
    fn set_vertex_bindings(&mut self, formats: [u32; 16], offsets: [u32; 16]) {
        self.calls
            .push(DeviceCall::SetVertexBindings { formats, offsets });
    }
    fn insert_fence(&mut self) -> u32 {
        self.calls.push(DeviceCall::InsertFence);
        self.fence
    }
    fn block_until_fence(&mut self, fence: u32) {
        self.calls.push(DeviceCall::BlockUntilFence { fence });
    }
    fn lock_vertex_buffer(&mut self, buffer: u32, flags: u32) -> u32 {
        self.calls
            .push(DeviceCall::LockVertexBuffer { buffer, flags });
        self.lock_address
    }
    fn set_pixel_constants(&mut self, index: u32, vectors: &[[f32; 4]]) {
        self.calls.push(DeviceCall::SetPixelConstants {
            index,
            vectors: vectors.to_vec(),
        });
    }
    fn begin_push(&mut self, dwords: u32) -> u32 {
        self.calls.push(DeviceCall::BeginPush { dwords });
        self.push_cursor
    }
    fn end_push(&mut self, cursor: u32) {
        self.calls.push(DeviceCall::EndPush { cursor });
    }
}

/// Records host calls and answers `result` to every one.
#[derive(Clone, Debug, Default)]
pub struct RecordingHost {
    pub calls: Vec<(HostFunction, Vec<u32>)>,
    /// Calls made with local buffers: the function, the arguments and the buffers as passed.
    pub buffer_calls: Vec<(HostFunction, Vec<HostArg>, Vec<Vec<u32>>)>,
    /// Calls through function pointers: the target and the arguments.
    pub address_calls: Vec<(u32, Vec<u32>)>,
    pub result: u32,
}

impl RenderHost for RecordingHost {
    fn call(&mut self, function: HostFunction, args: &[u32]) -> u32 {
        self.calls.push((function, args.to_vec()));
        self.result
    }
    fn call_buffers(
        &mut self,
        function: HostFunction,
        args: &[HostArg],
        buffers: &mut [Vec<u32>],
    ) -> u32 {
        self.buffer_calls
            .push((function, args.to_vec(), buffers.to_vec()));
        self.result
    }
    fn call_address(&mut self, target: u32, args: &[u32]) -> u32 {
        self.address_calls.push((target, args.to_vec()));
        self.result
    }
}

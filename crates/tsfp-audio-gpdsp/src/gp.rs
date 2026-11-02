//! The GP effects DSP: a `dsp56300-emu` core wired to the Xbox GP peripheral
//! map, DMA engine and staged effects image.
//!
//! Memory map (the MCPX GP DSP): X RAM 0..0x1000, the mixbuffer aliased at X
//! 0x1400..0x1800 (the top 0x400 words of the X buffer), Y RAM 0..0x800,
//! P RAM 0..0x1000, peripherals at X 0xffff80 and up served by callbacks.

use crate::dma::{
    ACTION_START, ACTION_UNFREEZE, CONTROL_ACTION, Dma, DmaError, DmaHost, Register, Space,
};
use crate::{FIRMWARE_BYTES, PreparedImage, STATE_BASE, STATE_WORDS};
use dsp56300_emu::core::{
    CreateInfo, DspState, MemSpace, MemoryMap, MemoryRegion, PowerState, RegionKind,
};
use dsp56300_emu::jit::JitEngine;
use std::ffi::c_void;

pub const FRAME_WORDS: usize = 1024;
const XRAM: usize = 0x1000;
const YRAM: usize = 0x800;
const PRAM: usize = 0x1000;
const MIXBUFFER_AT: usize = 0xc00;
const INTERRUPT_START_FRAME: u32 = 2;
const INTERRUPT_DMA_EOL: u32 = 128;
/// Pending-start sentinel meaning no deferred range.
const PENDING_NONE: u32 = 0x8000;
const COMMAND_WORD: usize = 0x810;
/// Cycle budget of one run, matching the C adapter's instruction budget.
const RUN_CYCLES: i32 = 1_000_000;

#[derive(Debug, PartialEq, Eq)]
pub enum GpError {
    Dma(DmaError),
    PeripheralNotModeled,
    FifoNotConnected,
    BudgetExhausted,
    StateMismatch,
    Range,
}

impl GpError {
    /// NUL-terminated description for the C ABI.
    pub fn message(&self) -> &'static str {
        match self {
            GpError::Dma(_) => "DSP DMA rejected transfer\0",
            GpError::PeripheralNotModeled => "DSP peripheral access not modeled\0",
            GpError::FifoNotConnected => "DSP FIFO not connected\0",
            GpError::BudgetExhausted => "DSP instruction budget exhausted\0",
            GpError::StateMismatch => "DSP state did not match the staged image\0",
            GpError::Range => "GP span outside the verified domain\0",
        }
    }
}

/// Everything the peripheral callbacks reach; boxed so its address is stable.
struct Bus {
    dsp: *mut DspState,
    stage: Vec<u8>,
    dma: Dma,
    interrupts: u32,
    stop_on_command_clear: bool,
    /// The halt was requested only to stop at the command boundary, not by the firmware.
    stopped_for_command: bool,
    error: Option<GpError>,
}

struct BusHost<'a> {
    dsp: *mut DspState,
    stage: &'a mut Vec<u8>,
}

fn mem_space(space: Space) -> MemSpace {
    match space {
        Space::X => MemSpace::X,
        Space::Y => MemSpace::Y,
        Space::P => MemSpace::P,
    }
}

impl DmaHost for BusHost<'_> {
    fn mem_read(&mut self, space: Space, address: u32) -> u32 {
        // SAFETY: the DSP outlives the bus; callbacks only touch memory.
        unsafe { (*self.dsp).read_memory(mem_space(space), address) }
    }
    fn mem_write(&mut self, space: Space, address: u32, value: u32) {
        // SAFETY: as above; P writes also mark the JIT cache dirty.
        unsafe {
            (*self.dsp).write_memory(mem_space(space), address, value);
            if space == Space::P {
                (*self.dsp).pram_dirty.mark_dirty(address);
            }
        }
    }
    fn scratch(&mut self, bytes: &mut [u8], address: usize, write: bool) -> bool {
        let Some(window) = address
            .checked_add(bytes.len())
            .and_then(|end| self.stage.get_mut(address..end))
        else {
            return false;
        };
        if write {
            window.copy_from_slice(bytes);
        } else {
            bytes.copy_from_slice(window);
        }
        true
    }
}

impl Bus {
    fn command_clear(&self) -> bool {
        self.stage[COMMAND_WORD..COMMAND_WORD + 4] == [0; 4]
    }

    fn fail(&mut self, error: GpError) {
        self.error.get_or_insert(error);
        self.halt();
    }

    fn halt(&mut self) {
        // SAFETY: only the halt flags are touched, never registers or PC.
        unsafe {
            (*self.dsp).halt_requested = true;
            (*self.dsp).exit_requested = true;
        }
    }

    fn read(&mut self, address: u32) -> u32 {
        match address {
            0xffffb3 => 0,
            0xffffc5 => self.interrupts | if self.dma.eol { INTERRUPT_DMA_EOL } else { 0 },
            0xffffd4 => self.dma.read(Register::NextBlock),
            0xffffd5 => self.dma.read(Register::StartBlock),
            0xffffd6 => self.dma.read(Register::Control),
            0xffffd7 => self.dma.read(Register::Configuration),
            _ => {
                self.fail(GpError::PeripheralNotModeled);
                0
            }
        }
    }

    fn write(&mut self, address: u32, value: u32) {
        match address {
            0xffffc4 => {
                if value & 1 != 0 {
                    self.halt();
                }
            }
            0xffffc5 => {
                self.interrupts &= !value;
                if value & INTERRUPT_DMA_EOL != 0 {
                    self.dma.eol = false;
                }
            }
            0xffffd4 => self.dma_write(Register::NextBlock, value),
            0xffffd5 => self.dma_write(Register::StartBlock, value),
            0xffffd6 => self.dma_write(Register::Control, value),
            0xffffd7 => self.dma_write(Register::Configuration, value),
            // The pinned reference ignores these; they acknowledge nothing.
            0xffffb0..=0xffffb2 => {}
            _ => self.fail(GpError::PeripheralNotModeled),
        }
    }

    fn dma_write(&mut self, register: Register, value: u32) {
        let mut host = BusHost {
            dsp: self.dsp,
            stage: &mut self.stage,
        };
        let action = value & CONTROL_ACTION;
        let valid = (ACTION_START..=ACTION_UNFREEZE).contains(&action);
        if matches!(register, Register::Control) && !valid {
            return self.fail(GpError::Dma(DmaError::UnsupportedAction));
        }
        if let Err(error) = self.dma.write(&mut host, register, value) {
            self.fail(GpError::Dma(error));
        }
        if self.stop_on_command_clear && self.command_clear() {
            self.stopped_for_command = true;
            self.halt();
        }
    }
}

unsafe extern "C" fn peripheral_read(opaque: *mut c_void, address: u32) -> u32 {
    // SAFETY: opaque is the boxed Bus registered in the memory map.
    unsafe { (*opaque.cast::<Bus>()).read(address) }
}

unsafe extern "C" fn peripheral_write(opaque: *mut c_void, address: u32, value: u32) {
    // SAFETY: as above.
    unsafe { (*opaque.cast::<Bus>()).write(address, value) }
}

pub struct GpDsp {
    // Field order matters for drop: the DSP references the buffers and bus.
    dsp: Box<DspState>,
    jit: JitEngine,
    bus: Box<Bus>,
    xram: Box<[u32]>,
    _yram: Box<[u32]>,
    _pram: Box<[u32]>,
    map_start: Vec<u32>,
    map_bytes: Vec<u32>,
    pending: (u32, u32),
    failed: Option<&'static str>,
}

fn word(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

impl GpDsp {
    /// Boot the effects DSP from a prepared image and run the download command.
    /// `finish_frame` also runs the first frame to its halt, as the C adapter does.
    pub fn new(image: PreparedImage, finish_frame: bool) -> Result<GpDsp, GpError> {
        let mut xram = vec![0xcau32; XRAM].into_boxed_slice();
        let mut yram = vec![0xcau32; YRAM].into_boxed_slice();
        let mut pram = vec![0xcau32; PRAM].into_boxed_slice();
        let mut bus = Box::new(Bus {
            dsp: std::ptr::null_mut(),
            stage: image.stage,
            dma: Dma::default(),
            interrupts: INTERRUPT_START_FRAME,
            stop_on_command_clear: false,
            stopped_for_command: false,
            error: None,
        });
        let buffer = |base: &mut [u32], start, end, offset| MemoryRegion {
            start,
            end,
            kind: RegionKind::Buffer {
                base: base.as_mut_ptr(),
                offset,
            },
        };
        let map = MemoryMap {
            x_regions: vec![
                buffer(&mut xram, 0, 0x1000, 0),
                buffer(&mut xram, 0x1400, 0x1800, MIXBUFFER_AT as u32),
                MemoryRegion {
                    start: 0xffff80,
                    end: 0x100_0000,
                    kind: RegionKind::Callback {
                        opaque: (&mut *bus as *mut Bus).cast(),
                        read_fn: peripheral_read,
                        write_fn: peripheral_write,
                    },
                },
            ],
            y_regions: vec![buffer(&mut yram, 0, YRAM as u32, 0)],
            p_regions: vec![buffer(&mut pram, 0, PRAM as u32, 0)],
        };
        let mut dsp = Box::new(DspState::new(CreateInfo { memory_map: map }));
        bus.dsp = &mut *dsp;
        for (index, slot) in pram.iter_mut().take(0x800).enumerate() {
            let at = index * 4;
            *slot = match bus.stage.get(at..at + 4).filter(|_| at < FIRMWARE_BYTES) {
                Some(raw) => word(raw) & 0xff_ffff,
                None => 0,
            };
        }
        let mut gp = GpDsp {
            dsp,
            jit: JitEngine::new(PRAM),
            bus,
            xram,
            _yram: yram,
            _pram: pram,
            map_start: Vec::new(),
            map_bytes: Vec::new(),
            pending: (PENDING_NONE, 0),
            failed: None,
        };
        for map in gp_maps(&image.metadata) {
            gp.map_start.push(map.0);
            gp.map_bytes.push(map.1);
        }
        gp.bus.stop_on_command_clear = true;
        gp.run(true)?;
        for i in 0..STATE_WORDS {
            let staged = word(&gp.bus.stage[STATE_BASE + i * 4..]) & 0xff_ffff;
            if gp.dsp.read_memory(MemSpace::X, 0x80 + i as u32) != staged {
                return Err(GpError::StateMismatch);
            }
        }
        if finish_frame {
            gp.run(false)?;
        }
        Ok(gp)
    }

    fn halted(&self) -> bool {
        self.dsp.halt_requested
    }

    fn wake(&mut self) {
        self.dsp.halt_requested = false;
        self.dsp.exit_requested = false;
        self.dsp.power_state = PowerState::Normal;
        self.bus.interrupts |= INTERRUPT_START_FRAME;
    }

    /// Run until the DSP halts; with `command_only` stop once the staged
    /// command word has been cleared, waking the core whenever it halts.
    fn run(&mut self, command_only: bool) -> Result<(), GpError> {
        self.bus.stop_on_command_clear = command_only;
        loop {
            if let Some(error) = self.bus.error.take() {
                return Err(error);
            }
            if command_only && self.bus.command_clear() {
                if std::mem::take(&mut self.bus.stopped_for_command) {
                    // Like the C core, stop at the command boundary without halting the frame.
                    self.dsp.halt_requested = false;
                    self.dsp.exit_requested = false;
                }
                return Ok(());
            }
            if self.halted() {
                if !command_only {
                    return Ok(());
                }
                self.wake();
            }
            self.dsp.cycle_budget = 0;
            self.dsp.run(&mut self.jit, RUN_CYCLES);
            if !self.halted() && self.bus.error.is_none() {
                return Err(GpError::BudgetExhausted);
            }
        }
    }

    /// Run one audio frame over the 1024-word mixbuffer.
    pub fn frame(&mut self, input: &[u32; FRAME_WORDS]) -> Result<[u32; FRAME_WORDS], GpError> {
        self.usable()?;
        let result = self.frame_inner(input);
        self.record(&result);
        result
    }

    /// Sticky failure text (NUL-terminated) once a run has failed, as the C core keeps in `error`.
    pub fn error(&self) -> Option<&'static str> {
        self.failed
    }

    fn usable(&self) -> Result<(), GpError> {
        match self.failed {
            None => Ok(()),
            Some(_) => Err(GpError::Range),
        }
    }

    /// A failed run poisons the context; refused spans and ranges do not.
    fn record<T>(&mut self, result: &Result<T, GpError>) {
        if let Err(e) = result {
            if *e != GpError::Range {
                self.failed = Some(e.message());
            }
        }
    }

    fn frame_inner(&mut self, input: &[u32; FRAME_WORDS]) -> Result<[u32; FRAME_WORDS], GpError> {
        if input.iter().any(|&word| word > 0xff_ffff) {
            return Err(GpError::Range);
        }
        if !self.halted() {
            self.run(false)?;
        }
        self.xram[MIXBUFFER_AT..MIXBUFFER_AT + FRAME_WORDS].copy_from_slice(input);
        if self.halted() {
            self.wake();
        }
        self.run(false)?;
        let mut output = [0; FRAME_WORDS];
        output.copy_from_slice(&self.xram[MIXBUFFER_AT..MIXBUFFER_AT + FRAME_WORDS]);
        Ok(output)
    }

    fn span(&self, map: usize, offset: usize, len: usize) -> Option<u32> {
        self.usable().ok()?;
        let bytes = *self.map_bytes.get(map)? as usize;
        (offset <= bytes && len <= bytes - offset).then(|| self.map_start[map] + offset as u32)
    }

    /// Read effect-map bytes from live DSP X memory.
    pub fn read(&self, map: usize, offset: usize, out: &mut [u8]) -> Result<(), GpError> {
        let start = self.span(map, offset, out.len()).ok_or(GpError::Range)?;
        for (i, byte) in out.iter_mut().enumerate() {
            let pos = start + i as u32;
            let value = self.dsp.read_memory(MemSpace::X, 0x80 + pos / 4);
            *byte = (value >> ((pos & 3) * 8)) as u8;
        }
        Ok(())
    }

    /// Copy bytes into the staged image only (the original first RAM copy).
    pub fn stage(&mut self, map: usize, offset: usize, bytes: &[u8]) -> Result<(), GpError> {
        let start = self.span(map, offset, bytes.len()).ok_or(GpError::Range)?;
        let at = STATE_BASE + start as usize;
        self.bus.stage[at..at + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }

    /// Write whole 24-bit words straight into DSP X memory (the aligned MMIO domain).
    pub fn live_aligned(&mut self, map: usize, offset: usize, bytes: &[u8]) -> Result<(), GpError> {
        let start = self.span(map, offset, bytes.len()).ok_or(GpError::Range)?;
        if start % 4 != 0 || bytes.len() % 4 != 0 || bytes.chunks(4).any(|w| word(w) > 0xff_ffff) {
            return Err(GpError::Range);
        }
        for (i, w) in bytes.chunks(4).enumerate() {
            let address = 0x80 + start / 4 + i as u32;
            self.dsp.write_memory(MemSpace::X, address, word(w));
        }
        Ok(())
    }

    /// Stage bytes and either write them live byte-wise (immediate) or defer them to the next commit.
    pub fn write(
        &mut self,
        map: usize,
        offset: usize,
        bytes: &[u8],
        deferred: bool,
    ) -> Result<(), GpError> {
        let start = self.span(map, offset, bytes.len()).ok_or(GpError::Range)?;
        if bytes.is_empty() {
            return Ok(());
        }
        self.stage(map, offset, bytes)?;
        if deferred {
            return self.defer(map, offset, bytes.len());
        }
        for (i, &byte) in bytes.iter().enumerate() {
            let pos = start + i as u32;
            let address = 0x80 + pos / 4;
            let shift = (pos & 3) * 8;
            let old = self.dsp.read_memory(MemSpace::X, address);
            let value = ((old & !(0xff << shift)) | (u32::from(byte) << shift)) & 0xff_ffff;
            self.dsp.write_memory(MemSpace::X, address, value);
        }
        Ok(())
    }

    /// Track a deferred range with the original arithmetic: duplicate updates accumulate
    /// their byte counts and separated ranges include the gap (not a range union).
    pub fn defer(&mut self, map: usize, offset: usize, len: usize) -> Result<(), GpError> {
        let start = STATE_BASE as u32 + self.span(map, offset, len).ok_or(GpError::Range)?;
        let mut add = len as u32;
        let (old, bytes) = self.pending;
        if old != PENDING_NONE {
            let end = old.wrapping_add(bytes);
            if end < start {
                add = add.wrapping_add(start.wrapping_sub(bytes).wrapping_sub(old));
            } else if old > start.wrapping_add(add) {
                add = old - start;
            }
        }
        self.pending = (old.min(start), bytes.wrapping_add(add));
        if old == PENDING_NONE {
            self.pending.0 = start;
        }
        Ok(())
    }

    /// Pending (start, bytes) range awaiting `commit`; start is 0x8000 when none.
    pub fn pending(&self) -> (u32, u32) {
        self.pending
    }

    /// Issue the staged-state update command and run it to completion.
    pub fn commit(&mut self) -> Result<(), GpError> {
        self.usable()?;
        let result = self.commit_inner();
        self.record(&result);
        result
    }

    fn commit_inner(&mut self) -> Result<(), GpError> {
        let (start, bytes) = self.pending;
        if start == PENDING_NONE || bytes == 0 {
            return Ok(());
        }
        let top = (STATE_BASE + STATE_WORDS * 4) as u32;
        if start < STATE_BASE as u32 || start > top || bytes > top - start {
            return Err(GpError::Range);
        }
        if !self.bus.command_clear() {
            return Err(GpError::Range);
        }
        let put = |stage: &mut Vec<u8>, at: usize, value: u32| {
            stage[at..at + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(
            &mut self.bus.stage,
            0x800,
            (start / 4).wrapping_sub(2020 + 0x206),
        );
        put(&mut self.bus.stage, 0x808, start);
        put(&mut self.bus.stage, 0x80c, bytes / 4);
        put(&mut self.bus.stage, 0x810, 2);
        self.run(true)?;
        self.pending = (PENDING_NONE, 0);
        Ok(())
    }
}

/// Per-map (start offset into the state block, byte length), as the C adapter keeps them.
fn gp_maps(metadata: &tsfp_audio::effects_metadata::Metadata) -> Vec<(u32, u32)> {
    metadata
        .maps
        .iter()
        .take(metadata.map_count as usize)
        .map(|m| (m.state_offset - STATE_BASE as u32, m.state_bytes))
        .collect()
}

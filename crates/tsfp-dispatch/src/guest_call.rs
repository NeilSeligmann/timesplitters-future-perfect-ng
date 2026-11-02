// SPDX-License-Identifier: GPL-3.0-or-later
//! Nested host-to-guest calls on the calling guest thread's own stack, the Rust twin of
//! `src/host/recomp_guest_call.c` (T392 stdcall, T2310 cdecl and cdecl-with-ST0).
//!
//! Contract kept from the C oracle, step for step (order matters: a refused call touches nothing):
//! 1. too many arguments: refused;
//! 2. the address is resolved through the [`DispatchTable`] before the stack is touched;
//! 3. the `[return = 0][args...]` frame goes just below ESP (stdcall) or 64 bytes below it
//!    (cdecl), all or nothing through the guarded guest write;
//! 4. EAX..EBP are saved, ESP points at the frame, the function runs;
//! 5. balance: stdcall callee pops the whole frame (ESP back at the old top), cdecl callee pops
//!    only its return slot (ESP at frame + 4);
//! 6. EAX..EBP and ESP are restored (cdecl also the x87 top and control word), so a float result
//!    is popped and returned as ST0, or NaN when the callee did not push exactly one value.
use crate::table::DispatchTable;

/// C `GUEST_CALL_MAX_ARGS`.
pub const STDCALL_MAX_ARGS: usize = 4;
/// C `GUEST_CALL_CDECL_MAX_ARGS`.
pub const CDECL_MAX_ARGS: usize = 8;
/// Gap the cdecl frame leaves below the interrupted ESP.
pub const CDECL_HEADROOM_BYTES: u32 = 64;

/// The guest integer register file in the C `saved[8]` order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Registers {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
    pub ebx: u32,
    pub esi: u32,
    pub edi: u32,
    pub ebp: u32,
    pub esp: u32,
}

impl Registers {
    pub fn to_array(self) -> [u32; 8] {
        [
            self.eax, self.ecx, self.edx, self.ebx, self.esi, self.edi, self.ebp, self.esp,
        ]
    }

    pub fn from_array(words: [u32; 8]) -> Self {
        let [eax, ecx, edx, ebx, esi, edi, ebp, esp] = words;
        Self {
            eax,
            ecx,
            edx,
            ebx,
            esi,
            edi,
            ebp,
            esp,
        }
    }
}

/// The part of the lifted x87 model a cdecl call saves and restores.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FpuState {
    /// `g_fp_top`, the stack top index (masked with 7 when used).
    pub top: i32,
    /// `g_fp_control_word`.
    pub control_word: u16,
}

/// The current guest thread as the trampoline sees it. The host binding implements it over the
/// C thread-local register file; tests implement it over a fake.
pub trait GuestThread<F> {
    fn registers(&self) -> Registers;
    fn set_registers(&mut self, registers: Registers);
    fn fpu(&self) -> FpuState;
    fn set_fpu(&mut self, state: FpuState);
    /// `g_fp_stack[index]`, `index` in 0..8.
    fn fpu_register(&self, index: usize) -> f64;
    /// Guarded, all-or-nothing guest write (C `kernel_guest_write_bytes`).
    fn write_guest(&mut self, address: u32, bytes: &[u8]) -> bool;
    /// Runs a lifted function on this thread.
    fn invoke(&mut self, function: F);
}

/// Why a call was refused or did not balance. Every refusal leaves the thread untouched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuestCallError {
    TooManyArguments,
    NotFound,
    /// ESP is too low to hold the frame.
    StackUnderflow,
    /// The guarded write refused the frame (not writable stack).
    FrameNotWritable,
    /// The call ran but the callee popped the wrong amount; registers were restored.
    Unbalanced,
}

/// Result of a cdecl call that ran (balanced or not, as C still hands out EAX and ST0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CdeclOutcome {
    pub eax: u32,
    /// The value the callee pushed, NaN when the top did not move down by exactly one.
    pub st0: f64,
    pub balanced: bool,
}

/// The `[return = 0][args...]` frame as bytes. A fixed array, not a `Vec`: a guest stop leaves
/// the call by `siglongjmp` (C `host_run_stop`), so nothing live across `invoke` may need `Drop`.
fn frame_bytes(arguments: &[u32]) -> ([u8; 4 * (CDECL_MAX_ARGS + 1)], usize) {
    let mut bytes = [0u8; 4 * (CDECL_MAX_ARGS + 1)];
    let (slots, _) = bytes.as_chunks_mut::<4>();
    for (slot, word) in slots[1..].iter_mut().zip(arguments) {
        *slot = word.to_le_bytes();
    }
    (bytes, 4 * (arguments.len() + 1))
}

/// C `recomp_guest_call_stdcall`.
pub fn call_stdcall<F: Copy, T: GuestThread<F>>(
    thread: &mut T,
    table: &DispatchTable<F>,
    address: u32,
    arguments: &[u32],
) -> Result<(), GuestCallError> {
    if arguments.len() > STDCALL_MAX_ARGS {
        return Err(GuestCallError::TooManyArguments);
    }
    let function = table.lookup(address).ok_or(GuestCallError::NotFound)?;
    let (frame, length) = frame_bytes(arguments);
    let saved = thread.registers();
    let top = saved.esp;
    let base = top
        .checked_sub(length as u32)
        .ok_or(GuestCallError::StackUnderflow)?;
    if !thread.write_guest(base, &frame[..length]) {
        return Err(GuestCallError::FrameNotWritable);
    }
    thread.set_registers(Registers { esp: base, ..saved });
    thread.invoke(function);
    let balanced = thread.registers().esp == top;
    thread.set_registers(saved);
    if balanced {
        Ok(())
    } else {
        Err(GuestCallError::Unbalanced)
    }
}

/// C `guest_call_cdecl` (behind `recomp_guest_call_cdecl` and `recomp_guest_call_cdecl_st0`).
pub fn call_cdecl<F: Copy, T: GuestThread<F>>(
    thread: &mut T,
    table: &DispatchTable<F>,
    address: u32,
    arguments: &[u32],
) -> Result<CdeclOutcome, GuestCallError> {
    if arguments.len() > CDECL_MAX_ARGS {
        return Err(GuestCallError::TooManyArguments);
    }
    let function = table.lookup(address).ok_or(GuestCallError::NotFound)?;
    let (frame, length) = frame_bytes(arguments);
    let saved = thread.registers();
    let top = saved.esp;
    let needed = length as u32 + CDECL_HEADROOM_BYTES;
    if top < needed {
        return Err(GuestCallError::StackUnderflow);
    }
    let base = top - needed;
    if !thread.write_guest(base, &frame[..length]) {
        return Err(GuestCallError::FrameNotWritable);
    }
    let fpu = thread.fpu();
    thread.set_registers(Registers { esp: base, ..saved });
    thread.invoke(function);
    let after = thread.registers();
    let after_fpu = thread.fpu();
    let pushed_one = (fpu.top as u32).wrapping_add(7) & 7 == (after_fpu.top as u32) & 7;
    let st0 = if pushed_one {
        thread.fpu_register((after_fpu.top & 7) as usize)
    } else {
        f64::NAN
    };
    thread.set_registers(saved);
    thread.set_fpu(fpu);
    Ok(CdeclOutcome {
        eax: after.eax,
        st0,
        balanced: after.esp == base + 4,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// What a fake lifted function does. Pure data: no clock, no real stack.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Callee {
        /// Pops `pop` bytes (return slot included), sets EAX to the sum of `args` stack words,
        /// clobbers ECX..EBP.
        Sum { args: usize, pop: u32 },
        /// As `Sum` with 2 args and cdecl pop, and pushes `value` on the x87 stack and sets cw.
        Float { value: u32 },
    }

    struct FakeThread {
        registers: Registers,
        fpu: FpuState,
        fpu_stack: [f64; 8],
        memory: BTreeMap<u32, u8>,
        writable: core::ops::Range<u32>,
        writes: usize,
        seen_frames: Vec<Vec<u32>>,
    }

    impl FakeThread {
        fn new(esp: u32) -> Self {
            Self {
                registers: Registers {
                    eax: 0xA,
                    ecx: 0xC,
                    edx: 0xD,
                    ebx: 0xB,
                    esi: 0x51,
                    edi: 0xD1,
                    ebp: 0xBB,
                    esp,
                },
                fpu: FpuState {
                    top: 0,
                    control_word: 0x027F,
                },
                fpu_stack: [0.0; 8],
                memory: BTreeMap::new(),
                writable: 0x1000..0x2000,
                writes: 0,
                seen_frames: Vec::new(),
            }
        }

        fn word(&self, address: u32) -> u32 {
            u32::from_le_bytes(core::array::from_fn(|i| self.memory[&(address + i as u32)]))
        }
    }

    impl GuestThread<Callee> for FakeThread {
        fn registers(&self) -> Registers {
            self.registers
        }
        fn set_registers(&mut self, registers: Registers) {
            self.registers = registers;
        }
        fn fpu(&self) -> FpuState {
            self.fpu
        }
        fn set_fpu(&mut self, state: FpuState) {
            self.fpu = state;
        }
        fn fpu_register(&self, index: usize) -> f64 {
            self.fpu_stack[index]
        }
        fn write_guest(&mut self, address: u32, bytes: &[u8]) -> bool {
            let end = u64::from(address) + bytes.len() as u64;
            if address < self.writable.start || end > u64::from(self.writable.end) {
                return false;
            }
            self.writes += 1;
            for (offset, byte) in bytes.iter().enumerate() {
                self.memory.insert(address + offset as u32, *byte);
            }
            true
        }
        fn invoke(&mut self, function: Callee) {
            let (args, pop) = match function {
                Callee::Sum { args, pop } => (args, pop),
                Callee::Float { .. } => (2, 4),
            };
            let esp = self.registers.esp;
            let frame: Vec<u32> = (0..=args as u32).map(|i| self.word(esp + 4 * i)).collect();
            self.registers = Registers {
                eax: frame[1..]
                    .iter()
                    .fold(0u32, |sum, word| sum.wrapping_add(*word)),
                ecx: 1,
                edx: 2,
                ebx: 3,
                esi: 4,
                edi: 5,
                ebp: 6,
                esp: esp + pop,
            };
            self.seen_frames.push(frame);
            if let Callee::Float { value } = function {
                let top = (self.fpu.top - 1) & 7;
                self.fpu_stack[top as usize] = f64::from(value);
                self.fpu = FpuState {
                    top,
                    control_word: 0x0C7F,
                };
            }
        }
    }

    fn table() -> DispatchTable<Callee> {
        DispatchTable::new(vec![
            (0x0001_2000, Callee::Sum { args: 3, pop: 16 }),
            (0x0001_3000, Callee::Sum { args: 3, pop: 12 }),
            (0x0004_05C0, Callee::Float { value: 42 }),
            (0x0004_1000, Callee::Sum { args: 2, pop: 4 }),
            (0x0004_2000, Callee::Sum { args: 2, pop: 12 }),
        ])
        .unwrap()
    }

    #[test]
    fn stdcall_writes_frame_below_esp_and_restores_registers() {
        let mut thread = FakeThread::new(0x1800);
        let before = thread.registers;
        assert_eq!(
            call_stdcall(&mut thread, &table(), 0x12000, &[1, 2, 3]),
            Ok(())
        );
        assert_eq!(thread.seen_frames, vec![vec![0, 1, 2, 3]]);
        assert_eq!(thread.word(0x1800 - 16), 0, "return slot is zero");
        assert_eq!(thread.registers, before, "EAX..ESP restored");
    }

    #[test]
    fn stdcall_unbalanced_callee_reports_and_restores() {
        let mut thread = FakeThread::new(0x1800);
        let before = thread.registers;
        assert_eq!(
            call_stdcall(&mut thread, &table(), 0x13000, &[1, 2, 3]),
            Err(GuestCallError::Unbalanced)
        );
        assert_eq!(thread.registers, before);
    }

    #[test]
    fn refusals_touch_nothing() {
        let cases: [(u32, u32, &[u32], GuestCallError); 4] = [
            (
                0x1800,
                0x12000,
                &[1, 2, 3, 4, 5],
                GuestCallError::TooManyArguments,
            ),
            (0x1800, 0xDEAD_BEEF, &[], GuestCallError::NotFound),
            (8, 0x12000, &[1, 2, 3], GuestCallError::StackUnderflow),
            (
                0x1008,
                0x12000,
                &[1, 2, 3],
                GuestCallError::FrameNotWritable,
            ),
        ];
        for (esp, address, args, error) in cases {
            let mut thread = FakeThread::new(esp);
            let before = thread.registers;
            assert_eq!(
                call_stdcall(&mut thread, &table(), address, args),
                Err(error)
            );
            assert_eq!((thread.registers, thread.writes), (before, 0), "{error:?}");
        }
    }

    #[test]
    fn cdecl_returns_eax_leaves_headroom_and_restores() {
        let mut thread = FakeThread::new(0x1800);
        let before = (thread.registers, thread.fpu);
        let outcome = call_cdecl(&mut thread, &table(), 0x41000, &[40, 2]).unwrap();
        assert_eq!((outcome.eax, outcome.balanced), (42, true));
        assert!(outcome.st0.is_nan(), "no x87 push means NaN");
        assert_eq!(
            thread.word(0x1800 - 64 - 12 + 4),
            40,
            "frame sits 64 bytes below ESP"
        );
        assert_eq!((thread.registers, thread.fpu), before);
    }

    #[test]
    fn cdecl_unbalanced_still_returns_eax() {
        let mut thread = FakeThread::new(0x1800);
        let outcome = call_cdecl(&mut thread, &table(), 0x42000, &[1, 2]).unwrap();
        assert_eq!((outcome.eax, outcome.balanced), (3, false));
    }

    #[test]
    fn cdecl_st0_pops_the_pushed_float_and_restores_fpu() {
        let mut thread = FakeThread::new(0x1800);
        let outcome = call_cdecl(&mut thread, &table(), 0x405C0, &[7, 9]).unwrap();
        assert_eq!((outcome.st0, outcome.balanced), (42.0, true));
        assert_eq!(
            thread.fpu,
            FpuState {
                top: 0,
                control_word: 0x027F
            }
        );
    }

    #[test]
    fn cdecl_refusals() {
        let mut thread = FakeThread::new(0x1800);
        let nine = [0u32; 9];
        assert_eq!(
            call_cdecl(&mut thread, &table(), 0x41000, &nine),
            Err(GuestCallError::TooManyArguments)
        );
        // 3 words + 64 headroom = 76 > 75.
        let mut low = FakeThread::new(75);
        assert_eq!(
            call_cdecl(&mut low, &table(), 0x41000, &[1, 2]),
            Err(GuestCallError::StackUnderflow)
        );
        assert_eq!(low.writes, 0);
    }

    #[test]
    fn register_array_order_matches_c_saved_layout() {
        let registers = Registers::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!((registers.eax, registers.ebx, registers.esp), (1, 4, 8));
        assert_eq!(registers.to_array(), [1, 2, 3, 4, 5, 6, 7, 8]);
    }
}

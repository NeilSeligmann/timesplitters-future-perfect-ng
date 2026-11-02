//! Light-HRTF table selection, not HRTF DSP. Source contract: dsound_hrtf.c.
//! Structural refusals remain strict: pretending lock acquisition succeeded would
//! publish guest state without ownership. No C calls in the production path.
use crate::adapter::Refusal;

pub const ENTRY: u32 = 0x0040_6AB6;
pub const CRITICAL_SECTION: u32 = 0x0041_24B4;
pub const TABLE: u32 = 0x0041_2B5C;
pub const LIGHT_FUNCTIONS: [u32; 11] = [
    0x00409D06, 0x00409D40, 0x00409A0B, 0x00409AA1, 0x00409AEC, 0x00409BFD, 0x00409E28, 0x00409EDF,
    0x00409C8D, 0x00409F48, 4,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CriticalState {
    pub owner: u32,
    pub recursion: u32,
}

/// Single calling-thread kernel boundary. Implementations own locking and memory;
/// `mapped` must match kernel_guest_at, not just a read-permission test.
pub trait HrtfKernel {
    fn irql(&self) -> Option<u8>;
    fn mapped(&self, address: u32, bytes: u32) -> bool;
    fn enter(&mut self, address: u32) -> u32;
    fn critical_state(&self, address: u32) -> Option<CriticalState>;
    fn owner_token(&self) -> u32;
    fn write_word(&mut self, address: u32, value: u32) -> bool;
    fn leave(&mut self, address: u32) -> u32;
}

/// Select the original light table. Passive callers enter, verify ownership,
/// write eleven words, then propagate Leave's return. Elevated callers skip locks.
/// As in C, Enter's return and individual write verdicts are ignored. A failed
/// acquisition is a terminal refusal; the kernel retains any acquired lock.
pub fn use_light(kernel: &mut impl HrtfKernel) -> Result<u32, Refusal> {
    let irql = kernel
        .irql()
        .ok_or(Refusal("calling thread IRQL byte is unreadable"))?;
    if !kernel.mapped(TABLE, 44) || !kernel.mapped(CRITICAL_SECTION, 28) {
        return Err(Refusal("HRTF table or critical-section span is unmapped"));
    }
    if irql == 0 {
        kernel.enter(CRITICAL_SECTION);
        if !kernel
            .critical_state(CRITICAL_SECTION)
            .is_some_and(|state| state.owner == kernel.owner_token() && state.recursion != 0)
        {
            return Err(Refusal(
                "passive critical-section acquisition was not tracked",
            ));
        }
    }
    for (index, value) in LIGHT_FUNCTIONS.into_iter().enumerate() {
        kernel.write_word(TABLE + index as u32 * 4, value);
    }
    Ok(if irql == 0 {
        kernel.leave(CRITICAL_SECTION)
    } else {
        0
    })
}

//! T1871: the guest memory and engine call interface of the effect pool update callbacks and their helper family.
//!
//! The ports of `0xF4920`, `0xDCB80` ... work on entry addresses and foreign objects the way the original does: every field is a guest
//! word, every engine routine is one [`Guest::call`]. A host backs `read`/`write` with the real pool and objects; the goldens
//! (`tools/t1846_gold/eff_common.py`) back them with a synthetic world and scripted engine answers. Behaviour MEASURED against the original
//! under the Unicorn oracle (xemu level), names INFERRED. Document `docs/t-story-event-effect-update-t1871.md`.

/// Stand-in for the stack buffers the original passes by pointer (the port has no stack): a host maps this range as scratch memory.
pub const SCRATCH: u32 = 0x7E00_0000;
/// Size of an effect entry (`0x22C`) and the pool globals.
pub const ENTRY_BYTES: u32 = 0x22C;
pub const POOL_BASE: u32 = 0x0073_7BD4;
pub const CLASS_TABLE: u32 = 0x004D_EB28;
/// RNG state (64 bit LCG) and draw counter of `0x153B40`.
pub const RNG_STATE: u32 = 0x004E_7950;
pub const RNG_COUNTER: u32 = 0x0074_97C8;

pub trait Guest {
    fn read(&mut self, address: u32) -> u32;
    fn write(&mut self, address: u32, value: u32);
    /// One engine routine (cdecl, `args` in push order left to right; register arguments of usercall routines follow the stack ones).
    /// Returns eax.
    fn call(&mut self, va: u32, args: &[u32]) -> u32;
    /// An engine routine that returns its result in st(0) (rounded to f32 by the caller's store).
    fn call_f(&mut self, va: u32, args: &[u32]) -> f32;

    /// `[[pointer_address] + offset]`.
    fn at(&mut self, pointer_address: u32, offset: u32) -> u32 {
        let pointer = self.read(pointer_address);
        self.read(pointer.wrapping_add(offset))
    }
    fn readf(&mut self, address: u32) -> f32 {
        f32::from_bits(self.read(address))
    }
    fn writef(&mut self, address: u32, value: f32) {
        self.write(address, value.to_bits());
    }
    fn vec3(&mut self, address: u32) -> [f32; 3] {
        [
            self.readf(address),
            self.readf(address + 4),
            self.readf(address + 8),
        ]
    }
    fn set_vec3(&mut self, address: u32, value: [f32; 3]) {
        for (index, component) in value.into_iter().enumerate() {
            self.writef(address + 4 * index as u32, component);
        }
    }
    /// One byte of guest memory (little endian words).
    fn byte(&mut self, address: u32) -> u32 {
        (self.read(address & !3) >> (8 * (address & 3))) & 0xFF
    }
    /// `rep movsd`: copy `words` dwords.
    fn copy(&mut self, source: u32, destination: u32, words: u32) {
        for index in 0..words {
            let value = self.read(source.wrapping_add(4 * index));
            self.write(destination.wrapping_add(4 * index), value);
        }
    }
    /// `0x153B40`: 64 bit LCG, the low dword of the new state, draw counter incremented.
    fn rng(&mut self) -> u32 {
        let count = self.read(RNG_COUNTER).wrapping_add(1);
        self.write(RNG_COUNTER, count);
        let state = (u64::from(self.read(RNG_STATE + 4)) << 32) | u64::from(self.read(RNG_STATE));
        let next = state.wrapping_mul(0x343FD).wrapping_add(0x26_9EC3);
        self.write(RNG_STATE, next as u32);
        self.write(RNG_STATE + 4, (next >> 32) as u32);
        next as u32
    }
}

/// `[0x7497CC]`: state of the visual random generator `0x153BD0` (32 bit LCG).
pub const VISUAL_RNG_STATE: u32 = 0x0074_97CC;
/// `[0x475CBC]` (`1 / 32768` with a tiny bias, an f32 constant of the image): scale of the visual generator.
pub const VISUAL_RNG_SCALE: f32 = f32::from_bits(0x3800_0100);

/// `0x153BD0()`: the float visual generator, `((state >> 16) & 0x7FFF) * [0x475CBC]` returned in st(0) (an exact f64 product).
pub fn visual_rng(g: &mut dyn Guest) -> f64 {
    let state = g
        .read(VISUAL_RNG_STATE)
        .wrapping_mul(0x343FD)
        .wrapping_add(0x26_9EC3);
    g.write(VISUAL_RNG_STATE, state);
    f64::from((state >> 16) & 0x7FFF) * f64::from(VISUAL_RNG_SCALE)
}

/// A u32 as the original's `fild` of a signed dword plus `2^32` when negative (exact in f64).
pub fn unsigned_as_f64(word: u32) -> f64 {
    f64::from(word)
}

/// One byte store (little endian word read-modify-write).
pub fn write_byte(g: &mut dyn Guest, address: u32, value: u32) {
    let aligned = address & !3;
    let shift = 8 * (address & 3);
    let old = g.read(aligned);
    g.write(
        aligned,
        (old & !(0xFF << shift)) | ((value & 0xFF) << shift),
    );
}

/// `0x153BB0()`: the integer visual generator, `(state >> 16) & 0x7FFF` after one LCG step of `[0x7497CC]`.
pub fn visual_rng_int(g: &mut dyn Guest) -> u32 {
    let state = g
        .read(VISUAL_RNG_STATE)
        .wrapping_mul(0x343FD)
        .wrapping_add(0x26_9EC3);
    g.write(VISUAL_RNG_STATE, state);
    (state >> 16) & 0x7FFF
}

/// A signed byte (`movsx`).
pub fn sbyte(g: &mut dyn Guest, address: u32) -> i32 {
    i32::from(g.byte(address) as u8 as i8)
}

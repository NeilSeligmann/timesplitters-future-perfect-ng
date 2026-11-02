// SPDX-License-Identifier: GPL-3.0-or-later
//! Value encodings shared by the generated views. This file is path-included by
//! `crates/tsfp-ports` (see `views.rs` there), so it names its parent module's
//! `GuestMemory` with `super::` only.
use super::GuestMemory;

/// A typed guest pointer (address 0 is null). Zero cost; `T` is a compile-time tag only.
pub struct GuestPtr<T>(pub u32, core::marker::PhantomData<T>);
impl<T> GuestPtr<T> {
    pub const fn new(address: u32) -> Self {
        Self(address, core::marker::PhantomData)
    }
    pub const fn is_null(&self) -> bool {
        self.0 == 0
    }
}
impl<T> Clone for GuestPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for GuestPtr<T> {}
impl<T> core::fmt::Debug for GuestPtr<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "GuestPtr({:#x})", self.0)
    }
}
impl<T> PartialEq for GuestPtr<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

/// A value with a fixed little-endian guest encoding. Floats use their bit pattern so a round
/// trip is bit exact (NaN payloads and -0.0 included).
pub trait GuestValue: Copy {
    const SIZE: u32;
    fn read<M: GuestMemory>(m: &M, address: u32) -> Self;
    fn write<M: GuestMemory>(self, m: &mut M, address: u32);
}
macro_rules! int_value {
    ($ty:ty, $size:expr) => {
        impl GuestValue for $ty {
            const SIZE: u32 = $size;
            fn read<M: GuestMemory>(m: &M, a: u32) -> Self {
                let mut bytes = [0u8; $size];
                for (i, b) in bytes.iter_mut().enumerate() {
                    *b = m.read8(a.wrapping_add(i as u32));
                }
                <$ty>::from_le_bytes(bytes)
            }
            fn write<M: GuestMemory>(self, m: &mut M, a: u32) {
                for (i, b) in self.to_le_bytes().into_iter().enumerate() {
                    m.write8(a.wrapping_add(i as u32), b);
                }
            }
        }
    };
}
int_value!(u8, 1);
int_value!(u16, 2);
int_value!(i16, 2);
int_value!(u32, 4);
int_value!(i32, 4);
impl GuestValue for f32 {
    const SIZE: u32 = 4;
    fn read<M: GuestMemory>(m: &M, a: u32) -> Self {
        f32::from_bits(u32::read(m, a))
    }
    fn write<M: GuestMemory>(self, m: &mut M, a: u32) {
        self.to_bits().write(m, a)
    }
}
impl<T: GuestValue, const N: usize> GuestValue for [T; N] {
    const SIZE: u32 = T::SIZE * N as u32;
    fn read<M: GuestMemory>(m: &M, a: u32) -> Self {
        core::array::from_fn(|i| T::read(m, a.wrapping_add(i as u32 * T::SIZE)))
    }
    fn write<M: GuestMemory>(self, m: &mut M, a: u32) {
        for (i, v) in self.into_iter().enumerate() {
            v.write(m, a.wrapping_add(i as u32 * T::SIZE));
        }
    }
}
impl<T> GuestValue for GuestPtr<T> {
    const SIZE: u32 = 4;
    fn read<M: GuestMemory>(m: &M, a: u32) -> Self {
        Self::new(u32::read(m, a))
    }
    fn write<M: GuestMemory>(self, m: &mut M, a: u32) {
        self.0.write(m, a)
    }
}

/// Capacity of the scratch buffer behind `differs`; the generator rejects larger fields.
pub const BITS_BUFFER: usize = 256;

struct Capture {
    base: u32,
    bytes: [u8; BITS_BUFFER],
}
impl GuestMemory for Capture {
    fn read8(&self, address: u32) -> u8 {
        self.bytes[address.wrapping_sub(self.base) as usize]
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.bytes[address.wrapping_sub(self.base) as usize] = value;
    }
}

/// True when the guest bytes at `address` differ from the encoding of `value` (bit compare).
pub fn differs<M: GuestMemory, T: GuestValue>(m: &M, address: u32, value: T) -> bool {
    let mut capture = Capture {
        base: address,
        bytes: [0; BITS_BUFFER],
    };
    value.write(&mut capture, address);
    (0..T::SIZE).any(|i| capture.bytes[i as usize] != m.read8(address.wrapping_add(i)))
}

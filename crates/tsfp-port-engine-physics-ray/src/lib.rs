//! Typed ray memory and live return-register behavior for the native engine/physics lane.
#![no_std]

use tsfp_port_runtime::memory::GuestMemory;

fn arg<M: GuestMemory>(memory: &M, esp: u32, index: u32) -> u32 {
    memory.read32(esp.wrapping_add(4 + 4 * index))
}

fn float<M: GuestMemory>(memory: &M, address: u32) -> f32 {
    f32::from_bits(memory.read32(address))
}

/// Ray data read from the guest record.
pub struct Ray {
    pub origin: [f32; 3],
    pub direction: [f32; 3],
    pub inverse: [f32; 3],
    pub length: f32,
}

/// Reads the origin, direction, reciprocal direction and length of a guest ray record.
pub fn read_ray<M: GuestMemory>(memory: &M, address: u32) -> Ray {
    let triple = |offset| {
        core::array::from_fn(|k| float(memory, address.wrapping_add(offset + 4 * k as u32)))
    };
    Ray {
        origin: triple(0),
        direction: triple(12),
        inverse: triple(24),
        length: float(memory, address.wrapping_add(36)),
    }
}

/// Runs `0x47B30` ray initialization and returns its live EAX value.
///
/// The original leaves direction.z in EAX, then `ucomiss direction.z, 0; lahf` replaces AH.
pub fn ray_init<M: GuestMemory>(memory: &mut M, esp: u32) -> u32 {
    let ray = arg(memory, esp, 0);
    let origin = arg(memory, esp, 1);
    let direction = arg(memory, esp, 2);
    let length = arg(memory, esp, 3);
    for axis in 0..3u32 {
        let origin_word = memory.read32(origin.wrapping_add(4 * axis));
        let direction_word = memory.read32(direction.wrapping_add(4 * axis));
        memory.write32(ray.wrapping_add(4 * axis), origin_word);
        memory.write32(ray.wrapping_add(12 + 4 * axis), direction_word);
        let component = f32::from_bits(direction_word);
        let inverse = if component == 0.0 {
            0.0
        } else {
            1.0 / component
        };
        memory.write32(ray.wrapping_add(24 + 4 * axis), inverse.to_bits());
    }
    memory.write32(ray.wrapping_add(36), length);

    let eax = memory.read32(direction.wrapping_add(8));
    let z = f32::from_bits(eax);
    let lahf = if z.is_nan() {
        0x47
    } else if z > 0.0 {
        0x02
    } else if z < 0.0 {
        0x03
    } else {
        0x42
    };
    (eax & !0x0000_ff00) | (lahf << 8)
}

#[cfg(test)]
mod tests {
    use super::ray_init;
    use tsfp_port_runtime::memory::GuestMemory;

    struct Memory([u8; 256]);

    impl Default for Memory {
        fn default() -> Self {
            Self([0; 256])
        }
    }

    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0[address as usize]
        }

        fn write8(&mut self, address: u32, value: u8) {
            self.0[address as usize] = value;
        }
    }

    fn put32(memory: &mut Memory, address: u32, value: u32) {
        memory.write32(address, value);
    }

    #[test]
    fn ray_init_returns_direction_z_with_lahf_flags() {
        let mut memory = Memory::default();
        let (esp, ray, origin, direction) = (0x10, 0x60, 0x80, 0xA0);
        for (index, value) in [ray, origin, direction, 0x3F80_0000]
            .into_iter()
            .enumerate()
        {
            put32(&mut memory, esp + 4 + 4 * index as u32, value);
        }
        for (index, value) in [0x3F80_0000, 0xC000_0000, 0x0000_0000]
            .into_iter()
            .enumerate()
        {
            put32(&mut memory, origin + 4 * index as u32, value);
            put32(&mut memory, direction + 4 * index as u32, value);
        }

        assert_eq!(ray_init(&mut memory, esp), 0x0000_4200);
        assert_eq!(memory.read32(ray + 36), 0x3F80_0000);
        assert_eq!(memory.read32(ray + 24), 0x3F80_0000);
        assert_eq!(memory.read32(ray + 28), 0xBF00_0000);
        assert_eq!(memory.read32(ray + 32), 0);
    }
}

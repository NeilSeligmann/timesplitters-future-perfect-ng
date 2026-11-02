// SPDX-License-Identifier: GPL-3.0-or-later
//! Native register adapter for cutscene key-node lookup roots `0x60390` and `0x60410`.

use tsfp_port_runtime::abi::ExtRegisters;
use tsfp_port_runtime::memory::GuestMemory as RuntimeGuestMemory;

/// `0x60390(frame)`: return the frame data pointer through the four streaming windows,
/// or the resident header base.
pub fn frame_pointer<M: RuntimeGuestMemory>(m: &mut M, frame: u32) -> u32 {
    if frame != 0 {
        let windows = m.read32(0x6d7834);
        for slot in 0..4u32 {
            let index = m.read32(0x7de540 + slot * 4);
            let entry = windows.wrapping_add(index.wrapping_mul(8));
            let start = m.read32(entry);
            if (frame as i32) < (start as i32) {
                continue;
            }
            let size = m.read32(entry.wrapping_add(4));
            let start_again = m.read32(entry);
            if (frame as i32) >= (start_again.wrapping_add(size) as i32) {
                continue;
            }
            if m.read32(0x7de520 + slot * 4) != 0 {
                break;
            }
            let header = m.read32(0x6d7818);
            let stride = m.read32(header.wrapping_add(0x14));
            let offsets = m.read32(0x6d7838);
            let relative = m.read32(offsets.wrapping_add(frame.wrapping_mul(4)));
            let base = m.read32(0x7de530);
            return relative
                .wrapping_add(stride.wrapping_mul(slot))
                .wrapping_add(base);
        }
    }
    let header = m.read32(0x6d7818);
    m.read32(header.wrapping_add(4)).wrapping_add(header)
}

/// `0x60410(frame, node)`: update the two-slot node cache and return its key pointer.
/// On a cache hit the original leaves EDX untouched. On a cache miss it leaves the
/// loaded `0x6d783c` offset-table pointer in EDX after using it for the lookup.
pub fn key_node<M: RuntimeGuestMemory>(m: &mut M, frame: i32, node: u32) -> u32 {
    key_node_effects(m, frame, node).0
}

/// Same lookup with the register effects needed by the native lane adapter.
pub fn key_node_with_regs<M: RuntimeGuestMemory>(
    m: &mut M,
    frame: i32,
    node: u32,
    regs: &mut ExtRegisters,
) {
    let (eax, edx) = key_node_effects(m, frame, node);
    regs.eax = eax;
    if let Some(edx) = edx {
        regs.edx = edx;
    }
}

fn key_node_effects<M: RuntimeGuestMemory>(m: &mut M, frame: i32, node: u32) -> (u32, Option<u32>) {
    let streaming = m.read32(0x7de530);
    let frame = if streaming == 0 || frame < 0 {
        0
    } else {
        frame
    };
    let count = m.read32(0x7bb260) as i32;
    let frame = if frame >= count {
        count.wrapping_sub(1)
    } else {
        frame
    };
    let (first, second);
    if m.read32(0x4d19cc) == node {
        first = m.read32(0x4d19d0) as i32;
        if frame == first {
            return (m.read32(0x6d782c), None);
        }
        second = m.read32(0x4d19d4) as i32;
        if frame == second {
            return (m.read32(0x6d7830), None);
        }
    } else {
        first = -1000;
        second = -1000;
        m.write32(0x4d19d0, first as u32);
        m.write32(0x4d19d4, second as u32);
        m.write32(0x4d19cc, node);
    }
    let base = frame_pointer(m, frame as u32);
    let stride = m.read32(0x7bb284);
    let offsets = m.read32(0x6d783c);
    let index = stride.wrapping_mul(frame as u32).wrapping_add(node);
    let pointer = base.wrapping_add(m.read32(offsets.wrapping_add(index.wrapping_mul(4))));
    if (frame > first && first < second) || (frame < first && first < second) {
        m.write32(0x4d19d0, frame as u32);
        m.write32(0x6d782c, pointer);
    } else {
        m.write32(0x4d19d4, frame as u32);
        m.write32(0x6d7830, pointer);
    }
    (pointer, Some(offsets))
}

#[cfg(test)]
mod tests {
    use super::{key_node, key_node_with_regs};
    use std::collections::BTreeMap;
    use tsfp_port_runtime::abi::{ExtRegisters, Fpu};
    use tsfp_port_runtime::memory::GuestMemory;

    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);

    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }

        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    fn registers(edx: u32) -> ExtRegisters {
        ExtRegisters {
            eax: 0,
            ecx: 0,
            edx,
            esp: 0,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: Default::default(),
            xmm_hi: Default::default(),
        }
    }

    #[test]
    fn slow_path_preserves_the_original_offset_table_in_edx() {
        let mut m = Memory::default();
        m.write32(0x7de530, 1);
        m.write32(0x7bb260, 1);
        m.write32(0x6d7818, 0x1000);
        m.write32(0x1004, 0x100);
        m.write32(0x6d783c, 0x2000);
        m.write32(0x200c, 0x20);
        let mut regs = registers(0xaaaaaaaa);

        key_node_with_regs(&mut m, 0, 3, &mut regs);

        assert_eq!(regs.eax, 0x1120);
        assert_eq!(regs.edx, 0x2000);
    }

    #[test]
    fn cache_hit_leaves_the_callers_edx_unchanged() {
        let mut m = Memory::default();
        m.write32(0x7de530, 1);
        m.write32(0x7bb260, 1);
        m.write32(0x4d19cc, 3);
        m.write32(0x4d19d0, 0);
        m.write32(0x6d782c, 0x12345678);
        let mut regs = registers(0xabcdef01);

        key_node_with_regs(&mut m, 0, 3, &mut regs);

        assert_eq!(regs.eax, 0x12345678);
        assert_eq!(regs.edx, 0xabcdef01);
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
//! Captured ABI side effects for scene root 0x8F5E0 (T3702 item t2_div-scene_1).
#![cfg_attr(not(test), no_std)]
extern crate alloc;

/// Mirror the root ABI and recursive call stack while the renderer owns traversal.
#[macro_export]
macro_rules! draw_model_node_tree {
    ($renderer:expr, $memory:expr, $entry_esp:expr, $args:expr, $regs:expr) => {{
        let model = $args[0];
        let index = $args[1];
        let arg2 = $args[2];
        let arg3 = $args[3];
        let saved = $renderer.state_slot_pending(6);
        $memory
            .borrow_mut()
            .write32($entry_esp.wrapping_add(4), saved);
        let data = $renderer.mem.read32(model.wrapping_add(4));
        let count = $renderer.mem.read32(data);
        let nodes = data.wrapping_sub((count as i32).wrapping_mul(0xC8) as u32);
        let header = $renderer.mem.read32(model);
        let forced = $renderer.mem.read32(header.wrapping_add(0x28)) & 0x2000_0000 != 0;
        $renderer.state_slot_set(6, if forced { 0x901 } else { saved });

        let signed = index as i32;
        if signed >= 0 && signed < count as i32 {
            let mut snapshot = alloc::vec::Vec::with_capacity(count as usize);
            for i in 0..count {
                let node = nodes.wrapping_add(i.wrapping_mul(0xC8));
                let child = $renderer.mem.read8(node.wrapping_add(0x4B)) as i8;
                let sibling = $renderer.mem.read8(node.wrapping_add(0x4C)) as i8;
                snapshot.push(crate::scene::render_prep::ModelNode {
                    mask: $renderer.mem.read32(node.wrapping_add(0x5C)),
                    child: if child < 0 {
                        None
                    } else {
                        Some(child as usize)
                    },
                    sibling: if sibling < 0 {
                        None
                    } else {
                        Some(sibling as usize)
                    },
                    stop_children: $renderer.mem.read8(node.wrapping_add(0x48)) == 3,
                });
            }
            let mask = $renderer.mem.read32(model.wrapping_add(0x30));
            if let Ok(commands) =
                crate::scene::render_prep::model_commands(&snapshot, index as usize, mask)
            {
                let mut slots = alloc::vec::Vec::new();
                for command in commands {
                    match command {
                        crate::scene::render_prep::ModelCommand::Enter => {
                            let current = $renderer.state_slot_pending(6);
                            slots.push(current);
                            $renderer.state_slot_set(6, if forced { 0x901 } else { current });
                        }
                        crate::scene::render_prep::ModelCommand::Draw(node) => {
                            // Original 0x8F5E0 has three saved registers and a pushed EBP at
                            // this call site. Each recursive call adds four arguments and a
                            // return address, so its frame base advances by 0x34 bytes.
                            let depth = slots.len().saturating_sub(1) as u32;
                            let inherited_stack_word = $entry_esp
                                .wrapping_sub(16)
                                .wrapping_sub(depth.wrapping_mul(0x34));
                            $renderer.host.call(
                                HostFunction::DrawModelNode,
                                &[model, node as u32, arg2, arg3, inherited_stack_word],
                            );
                        }
                        crate::scene::render_prep::ModelCommand::Leave => {
                            if let Some(current) = slots.pop() {
                                $renderer.state_slot_set(6, current);
                            }
                        }
                    }
                }
            }
            if forced {
                $renderer.state_slot_set(6, saved);
            }
        }
        $regs.edx = 0;
    }};
}

#[cfg(test)]
mod scene {
    pub mod render_prep {
        #[derive(Clone, Copy)]
        pub struct ModelNode {
            pub mask: u32,
            pub child: Option<usize>,
            pub sibling: Option<usize>,
            pub stop_children: bool,
        }

        pub enum ModelCommand {
            Enter,
            Draw(usize),
            Leave,
        }

        pub fn model_commands(
            nodes: &[ModelNode],
            root: usize,
            mask: u32,
        ) -> Result<alloc::vec::Vec<ModelCommand>, ()> {
            fn visit(
                nodes: &[ModelNode],
                i: usize,
                mask: u32,
                out: &mut alloc::vec::Vec<ModelCommand>,
            ) {
                let Some(node) = nodes.get(i) else { return };
                out.push(ModelCommand::Enter);
                if node.mask == 0 || node.mask & mask != 0 {
                    out.push(ModelCommand::Draw(i));
                }
                if !node.stop_children {
                    if let Some(child) = node.child {
                        visit(nodes, child, mask, out);
                    }
                }
                if let Some(sibling) = node.sibling {
                    visit(nodes, sibling, mask, out);
                }
                out.push(ModelCommand::Leave);
            }
            let mut out = alloc::vec::Vec::new();
            visit(nodes, root, mask, &mut out);
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use core::cell::RefCell;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);

    impl Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0xCD)
        }

        fn read32(&self, address: u32) -> u32 {
            u32::from_le_bytes([
                self.read8(address),
                self.read8(address + 1),
                self.read8(address + 2),
                self.read8(address + 3),
            ])
        }

        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }

        fn write32(&mut self, address: u32, value: u32) {
            for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
                self.write8(address + offset as u32, byte);
            }
        }
    }

    struct SharedMemory<'a>(&'a RefCell<Memory>);

    impl SharedMemory<'_> {
        fn read8(&self, address: u32) -> u8 {
            self.0.borrow().read8(address)
        }

        fn read32(&self, address: u32) -> u32 {
            self.0.borrow().read32(address)
        }
    }

    #[derive(Default)]
    struct Host(Vec<Vec<u32>>);

    impl Host {
        fn call(&mut self, function: HostFunction, args: &[u32]) -> u32 {
            assert_eq!(function, HostFunction::DrawModelNode);
            self.0.push(args.to_vec());
            0
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum HostFunction {
        DrawModelNode,
    }

    struct Renderer<'a> {
        mem: SharedMemory<'a>,
        host: &'a mut Host,
        slot: u32,
    }

    impl Renderer<'_> {
        fn state_slot_pending(&self, slot: u32) -> u32 {
            assert_eq!(slot, 6);
            self.slot
        }

        fn state_slot_set(&mut self, slot: u32, value: u32) -> u32 {
            assert_eq!(slot, 6);
            let previous = self.slot;
            self.slot = value;
            previous
        }
    }

    #[test]
    fn recursive_draws_preserve_original_inherited_stack_word() {
        let model = 0x100;
        let header = 0x200;
        let data = 0x400;
        let nodes = data - 2 * 0xC8;
        let entry_esp = 0x8000u32;
        let mut memory = Memory::default();
        memory.write32(model, header);
        memory.write32(model + 4, data);
        memory.write32(model + 0x30, 1);
        memory.write32(data, 2);
        memory.write8(nodes + 0x4B, 1);
        memory.write8(nodes + 0x4C, 0xFF);
        memory.write8(nodes + 0x48, 0);
        memory.write32(nodes + 0x5C, 0);
        memory.write8(nodes + 0xC8 + 0x4B, 0xFF);
        memory.write8(nodes + 0xC8 + 0x4C, 0xFF);
        memory.write8(nodes + 0xC8 + 0x48, 0);
        memory.write32(nodes + 0xC8 + 0x5C, 0);

        let shared_memory = RefCell::new(memory);
        let mut host = Host::default();
        let mut renderer = Renderer {
            mem: SharedMemory(&shared_memory),
            host: &mut host,
            slot: 0x70,
        };
        let mut regs = Registers { edx: 0x38 };
        crate::draw_model_node_tree!(
            &mut renderer,
            shared_memory,
            entry_esp,
            [model, 0, 2, 3],
            &mut regs
        );

        assert_eq!(shared_memory.borrow().read32(entry_esp + 4), 0x70);
        assert_eq!(host.0.len(), 2);
        assert_eq!(host.0[0], [model, 0, 2, 3, entry_esp - 16]);
        assert_eq!(host.0[1], [model, 1, 2, 3, entry_esp - 16 - 0x34]);
        assert_eq!(regs.edx, 0);
    }

    struct Registers {
        edx: u32,
    }
}

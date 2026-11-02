// SPDX-License-Identifier: GPL-3.0-or-later
//! T2049: typed lookup of mission-linked records (0x9B190).
//! Contract and Tier 1 fabricated differential evidence: docs/t2049-manual-audit.md.
use crate::abi::{ExtRegisters, Xmm};
use crate::c2r::rt::RegsExt;
use crate::memory::GuestMemory;
use crate::views::generated_views::{TsfpMissionLinkNodeView, TsfpMissionPointRowView};

const LINK_NODES: u32 = 0x70e178;

/// Signed indexing is intentional: negative object IDs probe rows before the base.
fn signed_point_group<M: GuestMemory>(memory: &M, base: u32, object: i16) -> i16 {
    let row = TsfpMissionPointRowView(
        base.wrapping_add((i32::from(object) as u32).wrapping_mul(TsfpMissionPointRowView::SIZE)),
    );
    memory.read16(row.0.wrapping_add(TsfpMissionPointRowView::OFFSET_GROUP)) as i16
}

/// Caller-owned cell containing the signed first index of a link chain.
/// Read it at the point of use so a caller's update remains visible.
#[derive(Clone, Copy)]
struct GuestLinkHead(u32);

impl GuestLinkHead {
    fn first<M: GuestMemory>(self, memory: &M) -> LinkIndex {
        LinkIndex(memory.read32(self.0) as i32)
    }
}

/// Borrowed guest node. Keep field reads live: other services can mutate the table.
#[derive(Clone, Copy)]
struct GuestLinkNode(TsfpMissionLinkNodeView);

impl GuestLinkNode {
    fn at(index: LinkIndex) -> Self {
        Self(TsfpMissionLinkNodeView(LINK_NODES.wrapping_add(
            (index.0 as u32).wrapping_mul(TsfpMissionLinkNodeView::SIZE),
        )))
    }

    fn read<M: GuestMemory>(self, memory: &M) -> LinkNode {
        let address = self.0.0;
        LinkNode {
            next: LinkIndex(i32::from(
                memory.read16(address.wrapping_add(TsfpMissionLinkNodeView::OFFSET_NEXT)) as i16,
            )),
            object: memory.read16(address.wrapping_add(TsfpMissionLinkNodeView::OFFSET_OBJECT))
                as i16,
            result: memory.read16(address.wrapping_add(TsfpMissionLinkNodeView::OFFSET_RESULT))
                as i16,
            value_bits: memory
                .read32(address.wrapping_add(TsfpMissionLinkNodeView::OFFSET_VALUE_BITS)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkIndex(pub i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkNode {
    pub next: LinkIndex,
    pub object: i16,
    pub result: i16,
    /// Preserve float bits, including NaN payloads, across the lookup.
    pub value_bits: u32,
}

/// Attachment point for owned tables or a guest-backed typed view.
pub trait LinkTable {
    fn node(&self, index: LinkIndex) -> LinkNode;
    fn object_key(&self, object: i16) -> i16;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lookup {
    pub terminal: LinkIndex,
    pub found: Option<LinkNode>,
}

/// Walk until the object key matches or a negative link ends the chain.
/// The original requires a terminating chain; cyclic input remains invalid.
pub fn find_by_object_key(table: &impl LinkTable, mut index: LinkIndex, key: i32) -> Lookup {
    while index.0 >= 0 {
        let node = table.node(index);
        if i32::from(table.object_key(node.object)) == key {
            return Lookup {
                terminal: index,
                found: Some(node),
            };
        }
        index = node.next;
    }
    Lookup {
        terminal: index,
        found: None,
    }
}

struct GuestLinks<'a, M, P> {
    memory: &'a M,
    points: P,
}
impl<M: GuestMemory, P: crate::missions_system::PointTables> LinkTable for GuestLinks<'_, M, P> {
    fn node(&self, index: LinkIndex) -> LinkNode {
        GuestLinkNode::at(index).read(self.memory)
    }
    fn object_key(&self, object: i16) -> i16 {
        if object >= 0 && crate::missions_reader::mode() != 0 {
            self.points.main_row(object as u32).group
        } else {
            // Negative object indices retain the original signed-address contract.
            signed_point_group(self.memory, self.points.main_rows(), object)
        }
    }
}

// portreg: va=0x0009b190
/// cdecl(head_pointer, signed_key). No callees. Only a hit publishes the float.
/// Guest offsets are confined to this compatibility boundary; the search uses typed state.
pub fn lookup_guest<M: GuestMemory>(m: &mut M, r: &mut ExtRegisters) {
    let head = GuestLinkHead(m.read32(r.esp.wrapping_add(4)));
    let start = head.first(m);
    r.push(m, r.esi);
    r.push(m, r.edi);
    r.ecx = start.0 as u32;
    if start.0 >= 0 {
        r.edx = m.read32(r.esp.wrapping_add(16));
        let lookup = if crate::missions_reader::mode() == 2 {
            find_by_object_key(
                &GuestLinks {
                    memory: &*m,
                    points: crate::missions_wire::OwnedPoints(&*m),
                },
                start,
                r.edx as i32,
            )
        } else {
            find_by_object_key(
                &GuestLinks {
                    memory: &*m,
                    points: crate::missions_system::GuestTables(&*m),
                },
                start,
                r.edx as i32,
            )
        };
        if let Some(node) = lookup.found {
            r.ecx = (lookup.terminal.0 as u32).wrapping_mul(16);
            r.eax = i32::from(node.result) as u32;
            r.xmm[0] = Xmm::scalar(node.value_bits);
            m.write32(0x715f48, node.value_bits);
        } else {
            r.ecx = lookup.terminal.0 as u32;
            r.eax = u32::MAX;
        }
    } else {
        r.eax = u32::MAX;
    }
    r.edi = m.read32(r.esp);
    r.esi = m.read32(r.esp.wrapping_add(4));
    r.esp = r.esp.wrapping_add(8); // The C2R bridge owns the root return slot.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::vec::Vec;
    struct Table;
    impl LinkTable for Table {
        fn node(&self, index: LinkIndex) -> LinkNode {
            let n = index.0;
            LinkNode {
                next: LinkIndex(if n == 3 { -7 } else { n + 1 }),
                object: n as i16 - 1,
                result: -12,
                value_bits: 0x7fc12345,
            }
        }
        fn object_key(&self, object: i16) -> i16 {
            object * 7
        }
    }
    #[test]
    fn searches_signed_keys_and_retains_payload() {
        let hit = find_by_object_key(&Table, LinkIndex(0), -7);
        assert_eq!(hit.terminal, LinkIndex(0));
        assert_eq!(hit.found.unwrap().value_bits, 0x7fc12345);
        assert_eq!(hit.found.unwrap().result, -12);
        assert_eq!(
            find_by_object_key(&Table, LinkIndex(0), 14).terminal,
            LinkIndex(3)
        );
        assert_eq!(
            find_by_object_key(&Table, LinkIndex(0), 99),
            Lookup {
                terminal: LinkIndex(-7),
                found: None
            }
        );
        assert_eq!(
            find_by_object_key(&Table, LinkIndex(-2), 0),
            Lookup {
                terminal: LinkIndex(-2),
                found: None
            }
        );
    }

    #[test]
    fn guest_node_view_keeps_live_widths_order_and_wrapping_stride() {
        struct Observed(RefCell<Vec<(u8, u32)>>);
        impl GuestMemory for Observed {
            fn read8(&self, _: u32) -> u8 {
                panic!("unexpected byte read")
            }
            fn write8(&mut self, _: u32, _: u8) {
                panic!("unexpected byte write")
            }
            fn read16(&self, address: u32) -> u16 {
                self.0.borrow_mut().push((16, address));
                match address.wrapping_sub(LINK_NODES.wrapping_add(0xffff_fffeu32.wrapping_mul(16)))
                {
                    0 => 0xfffd,
                    4 => 0xfffe,
                    6 => 0xfff4,
                    _ => panic!("unexpected halfword address"),
                }
            }
            fn read32(&self, address: u32) -> u32 {
                self.0.borrow_mut().push((32, address));
                0x7fc1_2345
            }
        }
        let memory = Observed(RefCell::new(Vec::new()));
        let index = LinkIndex(-2);
        let base =
            LINK_NODES.wrapping_add(0xffff_fffeu32.wrapping_mul(TsfpMissionLinkNodeView::SIZE));
        assert_eq!(
            GuestLinkNode::at(index).read(&memory),
            LinkNode {
                next: LinkIndex(-3),
                object: -2,
                result: -12,
                value_bits: 0x7fc1_2345,
            }
        );
        assert_eq!(
            *memory.0.borrow(),
            [
                (16, base),
                (16, base.wrapping_add(4)),
                (16, base.wrapping_add(6)),
                (32, base.wrapping_add(12))
            ]
        );
    }

    #[test]
    fn signed_point_group_keeps_halfword_width_and_wrapping_rows() {
        struct Observed(RefCell<Vec<u32>>);
        impl GuestMemory for Observed {
            fn read8(&self, _: u32) -> u8 {
                panic!("unexpected byte read")
            }
            fn write8(&mut self, _: u32, _: u8) {
                panic!("unexpected byte write")
            }
            fn read16(&self, address: u32) -> u16 {
                self.0.borrow_mut().push(address);
                0xfff4
            }
        }
        let memory = Observed(RefCell::new(Vec::new()));
        assert_eq!(signed_point_group(&memory, 0x10, -2), -12);
        assert_eq!(signed_point_group(&memory, 0xffff_fff0, 2), -12);
        assert_eq!(*memory.0.borrow(), [0xffff_ffd8, 0x38]);
    }

    #[test]
    fn guest_head_reads_one_live_signed_dword() {
        struct Head(std::cell::Cell<u32>);
        impl GuestMemory for Head {
            fn read8(&self, _: u32) -> u8 {
                panic!("unexpected byte read")
            }
            fn write8(&mut self, _: u32, _: u8) {
                panic!("unexpected byte write")
            }
            fn read32(&self, address: u32) -> u32 {
                assert_eq!(address, 0x20);
                self.0.get()
            }
        }
        let memory = Head(std::cell::Cell::new(u32::MAX));
        assert_eq!(GuestLinkHead(0x20).first(&memory), LinkIndex(-1));
        memory.0.set(7);
        assert_eq!(GuestLinkHead(0x20).first(&memory), LinkIndex(7));
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: wraps=tsfp-ports/c2r/data_missions
// portreg: wraps=tsfp-ports/c2r/t2049_exec_001
// portreg: wraps=tsfp-ports/c2r/t2049_exec_002
// portreg: wraps=tsfp-ports/c2r/t1937_math
//! T2041 library reader A/B. Default off; no host admission or Tier 2 claim.
//! Tier 1 vs translated Rust, FABRICATED inputs (not independent lifted or live Tier 2).
//! Typed point reads; retained translated family ABI, readable search/stream roots.
use crate::missions_system::{
    self as tables, DrawStream, GuestDrawStream, GuestTables, PointTables,
};
use crate::{
    abi::{ExtRegisters, GuestCall, Xmm},
    c2r::rt::RegsExt,
    memory::GuestMemory,
};
use core::sync::atomic::{AtomicU8, Ordering};

static MODE: AtomicU8 = AtomicU8::new(0);
/// 0 retains existing readers, 1 uses GuestTables, 2 uses persistent OwnedPoints.
/// Owned mode requires the separately configured, write-through Missions authority.
/// Configure only between runs, as with the existing library owner selectors.
pub fn configure(mode: u8) -> Result<(), u8> {
    if mode > 2 {
        return Err(mode);
    }
    MODE.store(mode, Ordering::Relaxed);
    Ok(())
}
pub fn mode() -> u8 {
    MODE.load(Ordering::Relaxed)
}
pub const ROOTS: [u32; 8] = [
    0x9cc40, 0xf9080, 0x9cca0, 0x9a510, 0x9a420, 0x9a3c0, 0x9bef0, 0x9c0e0,
];

/// Value-level compatibility boundary for retained script/point readers.
/// Modelled main/second/third fields and links are intercepted; unknown words stay
/// guest-backed. Writes keep their width. No snapshots hide callee write-through.
pub struct Reads<'a, M, T>(pub &'a mut M, pub T);
impl<M: GuestMemory, T: PointTables> Reads<'_, M, T> {
    fn word(&self, a: u32) -> Option<u32> {
        match a {
            tables::DRAW_CURSOR => return Some(self.1.draw_cursor()),
            tables::MAIN_ROWS => return Some(self.1.main_rows()),
            tables::LINKS => return Some(self.1.links()),
            tables::SECOND_ROWS => return Some(self.1.second_rows()),
            tables::THIRD_ROWS => return Some(self.1.third_rows()),
            tables::MAIN_COUNT => return Some(self.1.main_count()),
            tables::LINK_COUNT => return Some(self.1.link_count()),
            tables::SECOND_COUNT => return Some(self.1.second_count()),
            _ => {}
        }
        if let Some((table, index, offset)) = self.row_address(a) {
            if offset == 0 {
                return Some(match table {
                    0 => self.1.main_key(index),
                    1 => self.1.second_key(index),
                    _ => self.1.third_row(index).key,
                });
            }
            if !matches!(offset, 4 | 8 | 12 | 16 | 20) {
                return None;
            }
            let row = match table {
                0 => self.1.main_row(index),
                1 => self.1.second_row(index),
                _ => self.1.third_row(index),
            };
            return Some(match offset {
                4 => row.flags,
                8 => u32::from(row.group as u16) | (u32::from(row.tag as u16) << 16),
                _ => row.position_bits[((offset - 12) / 4) as usize],
            });
        }
        let offset = a.wrapping_sub(self.1.links());
        if self.1.links() != 0
            && offset / tables::LINK_SIZE < self.1.link_count()
            && offset % 4 == 0
        {
            return Some(
                self.1
                    .link_word(offset / tables::LINK_SIZE, (offset % tables::LINK_SIZE) / 4),
            );
        }
        None
    }
    fn row_address(&self, a: u32) -> Option<(usize, u32, u32)> {
        for (table, (base, count)) in [
            (self.1.main_rows(), self.1.main_count().max(1)),
            (self.1.second_rows(), self.1.second_count().max(1)),
            (self.1.third_rows(), 20),
        ]
        .into_iter()
        .enumerate()
        {
            let offset = a.wrapping_sub(base);
            if (table == 0 || base != 0) && offset / tables::ROW_SIZE < count {
                return Some((table, offset / tables::ROW_SIZE, offset % tables::ROW_SIZE));
            }
        }
        None
    }
}
impl<M: GuestMemory, T: PointTables> GuestMemory for Reads<'_, M, T> {
    fn read8(&self, a: u32) -> u8 {
        let shift = (a & 3) * 8;
        // Table pointers need not be aligned. Resolve row-relative alignment first.
        let (word, shift) = if let Some((_, _, offset)) = self.row_address(a) {
            (self.word(a.wrapping_sub(offset & 3)), (offset & 3) * 8)
        } else {
            let offset = a.wrapping_sub(self.1.links());
            if self.1.links() != 0 && offset / tables::LINK_SIZE < self.1.link_count() {
                (self.word(a.wrapping_sub(offset & 3)), (offset & 3) * 8)
            } else {
                (self.word(a & !3), shift)
            }
        };
        word.map(|v| (v >> shift) as u8)
            .unwrap_or_else(|| self.0.read8(a))
    }
    fn read32(&self, a: u32) -> u32 {
        self.word(a).unwrap_or_else(|| {
            u32::from_le_bytes(core::array::from_fn(|i| {
                self.read8(a.wrapping_add(i as u32))
            }))
        })
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.write8(a, v);
    }
    fn write32(&mut self, a: u32, v: u32) {
        self.0.write32(a, v);
    }
}

// Borrow memory through a shared reborrow, while all writes use the same backing
// store. A table adapter itself cannot be held across a mutable borrow; this view
// constructs it afresh per read instead of storing raw pointers or copying rows.
struct Selected<'a, M>(&'a mut M, u8);
impl<M: GuestMemory> GuestMemory for Selected<'_, M> {
    fn read8(&self, a: u32) -> u8 {
        with_reads(&*self.0, self.1, |r| r.read8(a))
    }
    fn read32(&self, a: u32) -> u32 {
        with_reads(&*self.0, self.1, |r| r.read32(a))
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.write8(a, v);
    }
    fn write32(&mut self, a: u32, v: u32) {
        self.0.write32(a, v);
    }
}
struct Borrow<'a, M>(&'a M);
impl<M: GuestMemory> GuestMemory for Borrow<'_, M> {
    fn read8(&self, a: u32) -> u8 {
        self.0.read8(a)
    }
    fn write8(&mut self, _: u32, _: u8) {
        panic!("read-only table borrow")
    }
}
fn with_reads<M: GuestMemory, R>(m: &M, mode: u8, f: impl FnOnce(&dyn GuestMemory) -> R) -> R {
    let mut borrowed = Borrow(m);
    if mode == 2 {
        f(&Reads(&mut borrowed, crate::missions_wire::OwnedPoints(m)))
    } else {
        f(&Reads(&mut borrowed, GuestTables(m)))
    }
}

/// Explicit per-call A/B dispatch, following the system run(enabled) convention.
/// False means no reads, writes, calls or register changes.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    selected: u8,
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) -> bool {
    if selected == 0 || selected > 2 || !ROOTS.contains(&va) {
        return false;
    }
    // Renderer-only reads must not initialise or classify the Missions owner.
    if va == 0xf9080 {
        stream(m, r);
        return true;
    }
    let mut view = Selected(m, selected);
    match va {
        0x9cc40 => search(&mut view, guest, r),
        0x9cca0 => crate::c2r::data_missions::game_entry_id_to_ptr(&mut view, guest, r),
        0x9a510 => crate::c2r::data_missions::game_entry_ptr_to_id(&mut view, guest, r),
        0x9a420 => crate::c2r::t2049_exec_001::game_table_715f2c_family_select_by_id_range_and_lookup(&mut view, guest, r),
        0x9a3c0 => crate::c2r::data_missions::game_table_715f34_binary_search_key_to_id(&mut view, r),
        0x9bef0 => crate::c2r::data_missions::game_table_715f2c_collect_entries_within_radius(&mut view, r),
        0x9c0e0 => crate::c2r::data_missions::game_point_table_715f2c_find_nearest_active_entry_in_group_beyond_min_distance(&mut view, r),
        _ => unreachable!(),
    }
    true
}
/// Dispatch used before translated module fallbacks. Mode starts at zero.
pub fn selected<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) -> bool {
    run(va, mode(), m, guest, r)
}

fn search<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, r: &mut ExtRegisters) {
    r.push(m, r.edi);
    r.edi = m.read32(r.esp.wrapping_add(8));
    if (r.edi as i32) >= 0x2329 {
        r.push(m, r.edi);
        r.call(guest, 0x9a3c0);
        r.esp = r.esp.wrapping_add(4);
    } else {
        r.push(m, r.ebx);
        r.push(m, r.esi);
        r.esi = m.read32(tables::MAIN_COUNT);
        r.eax = (r.esi as i32 >> 1) as u32;
        r.edx = 0;
        if r.eax != u32::MAX {
            r.ebx = m.read32(tables::MAIN_ROWS);
            loop {
                r.ecx = m.read32(r.ebx.wrapping_add(r.eax.wrapping_mul(tables::ROW_SIZE)));
                if r.edi == r.ecx {
                    break;
                }
                if (r.edi as i32) < (r.ecx as i32) {
                    r.esi = r.eax;
                } else {
                    r.edx = r.eax;
                }
                r.ecx = r.eax;
                r.eax = ((r.esi.wrapping_sub(r.edx) as i32 >> 1) as u32).wrapping_add(r.edx);
                if r.eax == r.ecx {
                    r.eax = u32::MAX;
                    break;
                }
            }
        }
        r.esi = m.read32(r.esp);
        r.esp = r.esp.wrapping_add(4);
        r.ebx = m.read32(r.esp);
        r.esp = r.esp.wrapping_add(4);
    }
    r.edi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
}
fn stream<M: GuestMemory>(m: &mut M, r: &mut ExtRegisters) {
    // Load each argument just before its store: stack/output aliases are legal.
    for i in 0..3u32 {
        r.eax = GuestDrawStream(&mut *m).cursor();
        r.xmm[0] = Xmm::scalar(m.read32(r.esp.wrapping_add(4 + 4 * i)));
        if i != 0 {
            r.eax = r.eax.wrapping_add(4);
            GuestDrawStream(&mut *m).set_cursor(r.eax);
        }
        GuestDrawStream(&mut *m).write_word(r.eax, r.xmm[0].lanes[0]);
    }
    let next = GuestDrawStream(&mut *m).cursor().wrapping_add(4);
    GuestDrawStream(m).set_cursor(next);
}

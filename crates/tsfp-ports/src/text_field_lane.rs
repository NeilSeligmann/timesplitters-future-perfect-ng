// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-ports/c2r/ui_framework_2
//! T2197: hybrid lane for the name-entry text field. Four roots run the readable model of
//! `crates/tsfp-text-field` (`keyboard::select_table`, `widget::*`) over guest memory behind the
//! per-function A/B switch (`--rust-ports=0x7c1b0,...`), so the persona and Map Maker name pages can be
//! compared against the retained originals with `tools.rust_trace` (docs/t2197-text-field.md):
//!
//! * `0x7C1B0` key table select (EDX flags, ECX out pointer for the byte table, EAX label table);
//! * `0x7C410` delete at cursor (EBX widget, ECX preserved);
//! * `0x7C4D0` blank-name test (ECX string, EAX 1 when empty or all spaces);
//! * `0x7DB20` pad key press (cdecl widget): key byte from the selected grid slot, insertion, the font
//!   width rule, cursor advance and one-shot shift release, tick sounds and the widget dirty call.
//!
//! Numeric ring mode (flag 0x20) and the zero-padded numeric class are outside the name-field slice:
//! those calls run the retained T1878 translation (`c2r::ui_framework_2`), decided before any write.
//! Volatile registers that no caller reads (EDX after 0x7C1B0/0x7C410/0x7C4D0, ECX after 0x7C4D0) are
//! not reproduced. Registered through `lanes.json` (`python3 -m tools.lanes regen`).
use crate::{
    abi::{ExtRegisters, GuestCall},
    c2r::{rt::RegsExt, ui_framework_2},
    memory::GuestMemory,
    text_field::{
        keyboard::{FieldFlags, select_table},
        widget::{self, Insert, Widget, WidgetMemory},
    },
};

/// The text-field lane roots in registration order.
pub const ROOTS: [u32; 4] = [0x7c1b0, 0x7c410, 0x7c4d0, 0x7db20];

/// Retained callees of the pad key press.
const PLAY_KEY_TICK: u32 = 0x7d960;
const MARK_WIDGET_DIRTY: u32 = 0x746d0;
const FIND_WIDGET_NODE: u32 = 0x71940;
const SELECT_TEXT_FONT: u32 = 0x3fa10;
const MEASURE_TEXT_WIDTH: u32 = 0x405c0;

/// Guest memory seen through the widget model's access trait.
struct Guest<'a, M: GuestMemory>(&'a mut M);
impl<M: GuestMemory> WidgetMemory for Guest<'_, M> {
    fn read8(&self, address: u32) -> u8 {
        self.0.read8(address)
    }
    fn read32(&self, address: u32) -> u32 {
        self.0.read32(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.write8(address, value)
    }
    fn write32(&mut self, address: u32, value: u32) {
        self.0.write32(address, value)
    }
}

/// Dispatches a root; `false` when `va` is not one of the text-field roots.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !ROOTS.contains(&va) {
        return false;
    }
    let entry_esp = regs.esp;
    let edited = match va {
        0x7db20 => Some(Widget(m.read32(entry_esp.wrapping_add(4)))),
        0x7c410 => Some(Widget(regs.ebx)),
        _ => None,
    };
    let before = edited.map(|field| snapshot(&Guest(m), field));
    let handled = crate::anim_lane::guarded(va, m, guest, regs, |view, guest, regs| {
        match va {
            0x7c1b0 => select_key_table(view, regs),
            0x7c410 => delete_key(view, guest, regs),
            0x7c4d0 => regs.eax = widget::is_blank(&Guest(view), regs.ecx) as u32,
            _ => press_key(view, guest, regs),
        }
        true
    });
    regs.esp = entry_esp;
    if let (Some(field), Some(before)) = (edited, before) {
        trace_edit(va, field, before, snapshot(&Guest(m), field));
    }
    handled
}

/// Widget words of one edit for the T2197 key-mapping trace: flags, cursor, text length, grid slot,
/// selected key byte, limit, and the first eight text bytes.
fn snapshot(m: &impl WidgetMemory, field: Widget) -> [u32; 7] {
    let buffer = field.buffer(m);
    let table = select_table(FieldFlags(field.flags(m).0 & 0xffff));
    let slot = field.grid_index(m);
    let text =
        |at: u32| u32::from_le_bytes([0, 1, 2, 3].map(|i| m.read8(buffer.wrapping_add(at + i))));
    [
        field.flags(m).0,
        field.cursor(m) | field.text_len(m) << 16,
        slot | (m.read8(table.bytes.wrapping_add(slot)) as u32) << 16,
        field.limit(m),
        field.source(m),
        text(0),
        text(4),
    ]
}

/// One stderr line per pad key press or delete (`T2197 <va> <widget> <before x7> <after x7>`), the
/// measurement channel for the page key mapping and write-back (docs/t2197-text-field.md).
#[cfg(not(test))]
fn trace_edit(va: u32, field: Widget, before: [u32; 7], after: [u32; 7]) {
    unsafe extern "C" {
        fn write(fd: i32, buffer: *const core::ffi::c_void, count: usize) -> isize;
    }
    let mut line = [0u8; 256];
    let mut at = 0;
    for byte in b"T2197 edit" {
        line[at] = *byte;
        at += 1;
    }
    for value in [va, field.0]
        .iter()
        .chain(before.iter())
        .chain(after.iter())
    {
        line[at] = b' ';
        at += 1;
        for shift in (0..8).rev() {
            line[at] = b"0123456789abcdef"[((value >> (4 * shift)) & 0xf) as usize];
            at += 1;
        }
    }
    line[at] = b'\n';
    unsafe { write(2, line.as_ptr().cast(), at + 1) };
}
#[cfg(test)]
fn trace_edit(_: u32, _: Widget, _: [u32; 7], _: [u32; 7]) {}

/// 0x7C1B0: EAX = label table of the selected grid, the byte table address stored through ECX.
fn select_key_table<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let table = select_table(FieldFlags(regs.edx & 0xffff));
    if regs.ecx != 0 {
        m.write32(regs.ecx, table.bytes);
    }
    regs.eax = table.labels;
}

/// Calls the widget dirty hook 0x746D0(widget), cdecl.
fn mark_dirty<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    widget: Widget,
) {
    regs.push(m, widget.0);
    regs.call(guest, MARK_WIDGET_DIRTY);
    regs.esp = regs.esp.wrapping_add(4);
}

/// 0x7C410: delete at the cursor of the EBX widget, then the dirty call unless suppressed.
fn delete_key<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let field = Widget(regs.ebx);
    if !field.is_name_field(&Guest(m)) {
        return ui_framework_2::game_ui_text_input_delete_char_at_cursor_and_shift_left_or_zero_fill(m, guest, regs);
    }
    let (esi, edi) = (regs.esi, regs.edi);
    // The original keeps the text length in the stack slot of its pushed ECX and pops it on return,
    // so ECX leaves holding the length the delete started from.
    let length = field.text_len(&Guest(m));
    widget::delete_at_cursor(&mut Guest(m), field);
    regs.push(m, length);
    regs.push(m, esi);
    regs.esp = regs.esp.wrapping_add(4);
    regs.eax = field.flags(&Guest(m)).0;
    if widget::delete_marks_dirty(&Guest(m), field) {
        mark_dirty(m, guest, regs, field);
    }
    regs.esp = regs.esp.wrapping_add(4);
    (regs.ecx, regs.esi, regs.edi) = (length, esi, edi);
}

/// The font-width rule of 0x7DB20: with a positive scale, the measured width of the edited text must not
/// exceed `viewport_x * scale`. Measures through the retained font callees on the widget's font node.
fn too_wide<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    field: Widget,
) -> bool {
    let scale = field.width_scale(&Guest(m));
    regs.xmm[0] = crate::abi::Xmm::scalar(scale.to_bits());
    let zero = f32::from_bits(m.read32(widget::WIDTH_SCALE_ZERO));
    if !(scale > zero) {
        return false;
    }
    let entry = regs.esp;
    regs.push(m, 0xffff_fffe);
    regs.push(m, field.0);
    regs.call(guest, FIND_WIDGET_NODE);
    let font = widget::read_node_field32(&Guest(m), regs.eax, 0x1c);
    regs.push(m, font);
    regs.call(guest, SELECT_TEXT_FONT);
    let buffer = field.buffer(&Guest(m));
    regs.push(m, buffer);
    regs.call(guest, MEASURE_TEXT_WIDTH);
    let width = regs.fpu.st(0);
    regs.fpu.pop();
    regs.esp = entry;
    let viewport = f32::from_bits(m.read32(widget::VIEWPORT_X_SCALE)) as f64;
    let limit = crate::c2r::rt::fp_pc(&regs.fpu, viewport * scale as f64);
    width > limit
}

/// 0x7DB20(widget): one pad key press on the on-screen keyboard.
fn press_key<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = regs.esp;
    let argument = entry.wrapping_add(4);
    let field = Widget(m.read32(argument));
    if !field.is_name_field(&Guest(m)) {
        return ui_framework_2::game_ui_text_input_widget_step_character_by_pad_with_tick_sound_and_wrap(m, guest, regs);
    }
    let saved = [regs.ebx, regs.ebp, regs.esi, regs.edi];
    for value in saved {
        regs.push(m, value);
    }
    // The original passes its argument slot as 0x7C1B0's out pointer and then keeps the key byte there.
    // Preserve the original nested-call boundary: capture-based Tier 2 compares this call as
    // part of 0x7DB20, and the retained selector has the exact stack/output-pointer contract.
    let flags = field.flags(&Guest(m)).0 & 0xffff;
    regs.edx = flags;
    regs.ecx = argument;
    regs.call(guest, 0x7c1b0);
    regs.ecx = m.read32(argument);
    let slot = field.grid_index(&Guest(m));
    regs.eax = slot;
    let key = m.read8(regs.ecx.wrapping_add(slot));
    m.write8(argument, key);
    regs.ebx = (regs.ebx & !0xff) | key as u32;
    regs.esi = field.0;
    regs.call(guest, PLAY_KEY_TICK);

    if let Insert::Placed { tail } = widget::insert_at_cursor(&mut Guest(m), field, key) {
        if too_wide(m, guest, regs, field) {
            widget::undo_insert(&mut Guest(m), field, tail);
            regs.esp = entry;
            [regs.ebx, regs.ebp, regs.esi, regs.edi] = saved;
            return;
        }
        widget::commit_insert(&mut Guest(m), field);
        regs.call(guest, PLAY_KEY_TICK);
    }
    widget::finish_key(&mut Guest(m), field);
    mark_dirty(m, guest, regs, field);
    regs.esp = entry;
    [regs.ebx, regs.ebp, regs.esi, regs.edi] = saved;
}

macro_rules! export {
    ($name:ident, $va:expr) => {
        /// # Safety
        /// `cpu` is a live C rust_cpu object and offset maps the guest window.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(cpu: *mut crate::lane::LaneCpu, offset: usize) {
            let mut memory = unsafe { crate::memory::MappedGuest::new(offset) };
            crate::lane_inv::execute($va, &mut memory, unsafe { &mut *cpu });
        }
    };
}
export!(tsfp_lane_0007c1b0, 0x7c1b0);
export!(tsfp_lane_0007c410, 0x7c410);
export!(tsfp_lane_0007c4d0, 0x7c4d0);
export!(tsfp_lane_0007db20, 0x7db20);

/// Tier 1 differential: every root against the retained T1878 translation (itself Tier 1 against the
/// lifted original) on FABRICATED widget states, with a deterministic stub for the retained callees.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{Fpu, Xmm};
    use alloc::{collections::BTreeMap, vec::Vec};

    #[derive(Clone, Default, PartialEq, Debug)]
    struct Sparse(BTreeMap<u32, u8>);
    impl GuestMemory for Sparse {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    /// Callee stub: logs (va, esp, first argument) and gives each callee a visible, deterministic effect.
    struct Stub {
        log: Vec<(u32, u32, u32)>,
        memory: *mut Sparse,
    }
    impl GuestCall for Stub {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            // SAFETY: `memory` is the live guest memory of this single-threaded run.
            let m = unsafe { &mut *self.memory };
            if va == 0x7c1b0 {
                // Preserve the real selector behavior on both the lane and translated paths.
                return ui_framework_2::game_font_charset_select(m, regs);
            }
            let argument = m.read32(regs.esp);
            self.log.push((va, regs.esp, argument));
            match va {
                MARK_WIDGET_DIRTY => {
                    m.write32(
                        argument.wrapping_add(0x44),
                        m.read32(argument.wrapping_add(0x44)) | 0x20,
                    );
                    regs.eax = argument;
                }
                FIND_WIDGET_NODE => regs.eax = NODE,
                SELECT_TEXT_FONT => m.write32(0x6b93b4, argument),
                MEASURE_TEXT_WIDTH => {
                    let length = widget::string_length(&Guest(m), argument);
                    regs.fpu.push(length as f64 * 7.5);
                }
                _ => regs.eax = va,
            }
            regs.ecx = 0xdead_0000 | (va & 0xffff);
            regs.edx = 0xbeef_0000;
        }
    }

    const WIDGET: u32 = 0x1000;
    const BUFFER: u32 = 0x2000;
    const NODE: u32 = 0x2800;
    const STACK: u32 = 0x8000;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 16) as u32
        }
        fn below(&mut self, n: u32) -> u32 {
            self.next() % n.max(1)
        }
    }

    fn fabricate(rng: &mut Rng) -> (Sparse, ExtRegisters) {
        let mut m = Sparse::default();
        for address in 0x4d_8e74..0x4d_94e0 {
            let byte = [0u8, b' ', b'a', b'Z', b'0', 0xe9][rng.below(6) as usize];
            m.write8(address, byte);
        }
        let mut flags = 0;
        for bit in [0x2, 0x4, 0x8, 0x10, 0x80, 0x100, 0x1000, 0x4000] {
            if rng.below(3) == 0 {
                flags |= bit;
            }
        }
        if rng.below(16) == 0 {
            flags |= FieldFlags::NUMERIC_RING;
        }
        let limit = rng.below(14);
        let length = rng.below(limit + 2);
        for i in 0..length {
            m.write8(
                BUFFER + i,
                [b'a', b' ', b'B', b'7', 0xe9][rng.below(5) as usize],
            );
        }
        m.write8(BUFFER + length + 1, 0x55); // a byte past the terminator stays observable
        m.write32(
            WIDGET + 0x4,
            if rng.below(16) == 0 {
                widget::ZERO_PADDED_CLASS
            } else {
                0x4d_0000
            },
        );
        m.write32(WIDGET + 0x18c, rng.below(41));
        m.write32(WIDGET + 0x220, BUFFER);
        m.write32(WIDGET + 0x228, flags);
        m.write32(WIDGET + 0x22c, rng.below(length + 1));
        m.write32(WIDGET + 0x230, limit);
        let scale = [0.0f32, 0.5, 1.0, -1.0, f32::NAN][rng.below(5) as usize];
        m.write32(WIDGET + 0x238, scale.to_bits());
        m.write32(NODE + 0x1c, 0x3000);
        m.write32(widget::VIEWPORT_X_SCALE, 40.0f32.to_bits());
        m.write32(STACK + 4, WIDGET);
        let regs = ExtRegisters {
            eax: rng.next(),
            ecx: if rng.below(2) == 0 { 0 } else { 0x5000 },
            edx: flags | (rng.next() & 0xffff_0000),
            esp: STACK,
            ebx: WIDGET,
            ebp: 0x1111,
            esi: 0x2222,
            edi: 0x3333,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0x27f,
            },
            xmm: [Xmm { lanes: [0; 4] }; 5],
            xmm_hi: [Xmm { lanes: [0; 4] }; 3],
        };
        (m, regs)
    }

    type Side = (Sparse, ExtRegisters, Vec<(u32, u32, u32)>);

    fn run_side(va: u32, rust: bool, mut m: Sparse, mut regs: ExtRegisters) -> Side {
        if va == 0x7c4d0 {
            regs.ecx = BUFFER;
        }
        let pointer: *mut Sparse = &mut m;
        let mut stub = Stub {
            log: Vec::new(),
            memory: pointer,
        };
        // SAFETY: single-threaded test, the stub and the port take turns on the same memory.
        let memory = unsafe { &mut *pointer };
        if rust {
            match va {
                0x7c1b0 => select_key_table(memory, &mut regs),
                0x7c410 => delete_key(memory, &mut stub, &mut regs),
                0x7c4d0 => regs.eax = widget::is_blank(&Guest(memory), regs.ecx) as u32,
                _ => press_key(memory, &mut stub, &mut regs),
            }
        } else {
            match va {
                0x7c1b0 => ui_framework_2::game_font_charset_select(memory, &mut regs),
                0x7c410 => {
                    ui_framework_2::game_ui_text_input_delete_char_at_cursor_and_shift_left_or_zero_fill(memory, &mut stub, &mut regs)
                }
                0x7c4d0 => ui_framework_2::game_str_is_empty_or_spaces_ecx(memory, &mut regs),
                _ => ui_framework_2::game_ui_text_input_widget_step_character_by_pad_with_tick_sound_and_wrap(memory, &mut stub, &mut regs),
            }
        }
        let log = stub.log;
        // The frame below the entry ESP is scratch: drop it from the comparison.
        m.0.retain(|address, _| !(STACK - 0x80..STACK).contains(address));
        (m, regs, log)
    }

    #[test]
    fn roots_equal_the_translation() {
        let mut rng = Rng(0x2197_7c1b_0db2_0001);
        let mut name_field_presses = 0;
        for case in 0..24_000u32 {
            let va = ROOTS[(case % 4) as usize];
            let (m, regs) = fabricate(&mut rng);
            if va == 0x7db20 && Widget(WIDGET).is_name_field(&Guest(&mut m.clone())) {
                name_field_presses += 1;
            }
            let (rust_m, rust_r, rust_log) = run_side(va, true, m.clone(), regs);
            let (c2r_m, c2r_r, c2r_log) = run_side(va, false, m, regs);
            let differing: Vec<(u32, u8, u8)> = rust_m
                .0
                .keys()
                .chain(c2r_m.0.keys())
                .map(|&a| (a, rust_m.read8(a), c2r_m.read8(a)))
                .filter(|(_, rust, c2r)| rust != c2r)
                .collect();
            assert!(
                differing.is_empty(),
                "memory {va:#x} case {case}: (address, rust, translation) {differing:x?}"
            );
            assert_eq!(rust_log, c2r_log, "callees {va:#x} case {case}");
            let saved = |r: &ExtRegisters| {
                (
                    r.esp,
                    r.ebx,
                    r.ebp,
                    r.esi,
                    r.edi,
                    r.fpu.top,
                    r.fpu.st(0).to_bits(),
                )
            };
            assert_eq!(
                saved(&rust_r),
                saved(&c2r_r),
                "registers {va:#x} case {case}"
            );
            match va {
                0x7c1b0 | 0x7c4d0 => assert_eq!(rust_r.eax, c2r_r.eax, "eax {va:#x} case {case}"),
                0x7c410 => assert_eq!(
                    (rust_r.eax, rust_r.ecx),
                    (c2r_r.eax, c2r_r.ecx),
                    "eax/ecx case {case}"
                ),
                _ => assert_eq!(
                    rust_r.xmm[0].lanes[0], c2r_r.xmm[0].lanes[0],
                    "xmm0 case {case}"
                ),
            }
        }
        assert!(name_field_presses > 4_000, "{name_field_presses}");
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
//! T1901 mapmaker/modes: five Tier 0 behavioural candidates, unadmitted.
#[path = "../../tsfp-engine-core/src/system_slice.rs"]
pub mod boundary;
use boundary::{FunctionTier, Register, Services};

pub const FUNCTIONS: [FunctionTier; 5] = [
    FunctionTier {
        address: 0x3127d0,
        tier: 0,
        stack_cleanup: 0,
    },
    FunctionTier {
        address: 0x3127f0,
        tier: 0,
        stack_cleanup: 0,
    },
    FunctionTier {
        address: 0x313fe0,
        tier: 0,
        stack_cleanup: 0,
    },
    FunctionTier {
        address: 0x3248d0,
        tier: 0,
        stack_cleanup: 0,
    },
    FunctionTier {
        address: 0x325530,
        tier: 0,
        stack_cleanup: 0,
    },
];

/// Ask the mode validator about Story (category 0), returning 0 or 1.
/// Every nonzero validator result becomes 1, including values with only the sign bit set.
/// For example, a validator result of `0x8000_0000` yields `1` and records
/// exactly one call to `0x3265a0` with category `0`.
pub fn story_warning<H: Services>(h: &mut H) -> u32 {
    u32::from(h.call(0x3265a0, &[0]) != 0)
}
/// Ask the same validator about Assault (category 1), returning 0 or 1.
pub fn assault_warning<H: Services>(h: &mut H) -> u32 {
    u32::from(h.call(0x3265a0, &[1]) != 0)
}

fn label<H: Services>(h: &mut H, id: u32) -> u32 {
    h.call(0x3d7b0, &[id])
}

// Guest layout used by the Assault page. Field roles are inferred from the
// existing port; the numeric offsets and reload points preserve its ABI.
const EDITOR_GLOBAL: u32 = 0x7844a8;
const EDITOR_KIND: u32 = 0x248;
const PHASE_DESCRIPTOR_TABLE: u32 = 0x52e8c0;
const PHASE_DESCRIPTOR_STRIDE: u32 = 0x44;
const PHASE_ENABLED: u32 = 4;
const PHASE_STATE_BASE: u32 = 0x805c;
const PHASE_STATE_STRIDE: u32 = 0xb4;
const WIDGET_SELECTION: u32 = 0x18c;
const WIDGET_FLAGS: u32 = 0xc4;
const WIDGET_SELECT_ENABLED: u32 = 0x800000;

fn phase_descriptor<H: Services>(h: &H, editor: u32) -> u32 {
    let kind = h.read32(editor.wrapping_add(EDITOR_KIND));
    PHASE_DESCRIPTOR_TABLE.wrapping_add(kind.wrapping_mul(PHASE_DESCRIPTOR_STRIDE))
}
fn layout_y<H: Services>(h: &H, spacing: u32) -> f32 {
    let scale = h.read_float(0x7ba188);
    let half = scale * h.read_float(0x475d24);
    let other = scale * h.read_float(0x475d30);
    let mut y = spacing as i32 as f32;
    y += half;
    y += other;
    y -= h.read_float(0x7ba184);
    y * h.read_float(0x7ba030)
}

/// Build the Assault options page at `widget` using the caller-cleaned widget argument.
///
/// The service boundary preserves UI call order and reloads mutable state after calls.
/// The participant count is interpreted as a signed byte and clamped to at least one.
pub fn build_assault_options<H: Services>(h: &mut H, widget: u32) {
    let ticks = h.read32(0x4e8cec).wrapping_mul(300);
    h.write32(0x7de460, ticks);
    let participants = (h.read8(0x7de400) as i8).max(1) as u8;
    h.write8(0x7de400, participants);
    let title = label(h, 0xaef);
    h.call(0x75b70, &[0xa800, title, 40f32.to_bits(), 0, 0]);
    h.write32(widget.wrapping_add(0x218), 5);
    h.call(0x71b10, &[1, 0, 1, 1]);
    h.call(0x71c50, &[0x20000]);
    let spacing = h.call(0x22b90, &[]);
    let y = layout_y(h, spacing);
    let height = h.read_float(0x49f7d0) - y;
    h.call(0x78170, &[height.to_bits(), 0, 0, 0]);
    let preview = label(h, 0xaf1);
    h.call(0x79de0, &[preview, u32::MAX, 0x800, 0, 0]);
    h.call(0x7d4a0, &[0, 0]);
    h.call(0x716d0, &[0x39]);
    for (id, value) in [(0xaf3, 0), (0xaf4, 1)] {
        let text = label(h, id);
        h.call(0x71800, &[text, value]);
    }
    h.call(0x78590, &[0x784034]);
    h.call(0x783d0, &[]);
    h.call(0x784a0, &[0x40000000]);
    let start = label(h, 0xaf9);
    h.call(0x79d60, &[start, u32::MAX, 0, 0x40000000, 0x313f40, 0]);
    h.call(0x781a0, &[]);
    h.call(0x75e70, &[]);
    let spacing = h.call(0x22b90, &[]);
    let y = layout_y(h, spacing);
    let current = h.read32(0x7bb1e8);
    h.call(0x77320, &[current, y.to_bits()]);
}

/// Handle Assault phase selection and popup actions for `widget`.
///
/// An enabled, nonnegative selection toggles its phase word with wrapping `1 - old`;
/// the UI services can mutate the editor pointer between calls, so it is reloaded.
/// For example, an existing phase word of `2` becomes `0xffff_ffff`.
pub fn assault_phase_input<H: Services>(h: &mut H, page: u32, widget: u32) {
    let index = h.read32(widget.wrapping_add(WIDGET_SELECTION));
    if h.read32(widget.wrapping_add(WIDGET_FLAGS)) & WIDGET_SELECT_ENABLED != 0 && index as i32 >= 0
    {
        let editor = h.read32(EDITOR_GLOBAL);
        let descriptor = phase_descriptor(h, editor);
        if h.read32(descriptor.wrapping_add(PHASE_ENABLED)) != 0 {
            let state = editor
                .wrapping_add(PHASE_STATE_BASE)
                .wrapping_add(index.wrapping_mul(PHASE_STATE_STRIDE));
            h.write32(state, 1u32.wrapping_sub(h.read32(state)));
            h.call(0x3247c0, &[widget]);
            h.call(0x78980, &[widget]);
        }
    }
    if h.call(0x71880, &[widget, 0]) == 0 {
        return;
    }
    // T1688 records EDI=page in addition to the stack argument kind=1.
    match h.invoke(0x31fa00, &[1], &[(Register::Edi, page)]) {
        0 => {
            h.call(0x3247c0, &[widget]);
        }
        1 => {
            let editor = h.read32(EDITOR_GLOBAL);
            let descriptor = phase_descriptor(h, editor);
            let id = h.read32(descriptor.wrapping_add(0x30));
            let arg = h.read32(descriptor.wrapping_add(0x2c));
            let text = label(h, id);
            let destination = h.read32(EDITOR_GLOBAL);
            h.call(0x3c85d4, &[destination, text, arg]);
            let destination = h.read32(EDITOR_GLOBAL);
            h.call(0x7a270, &[widget, destination, 0]);
        }
        2 => {
            let text = label(h, 0xbba);
            let destination = h.read32(EDITOR_GLOBAL);
            h.call(0x3c85d4, &[destination, text, 10]);
            let destination = h.read32(EDITOR_GLOBAL);
            h.call(0x7a270, &[widget, destination, 0]);
        }
        _ => {}
    }
    h.call(0x78980, &[widget]);
}

/// Format the active Assault phase and refresh its node when `descriptor` matches.
/// The final service receives the selected node in EAX, not a stack argument.
pub fn format_assault_phase<H: Services>(h: &mut H, descriptor: u32) {
    const ACTIVE_PHASE_GLOBAL: u32 = 0x762f98;
    const ASSAULT_DESCRIPTOR_TYPE: u32 = 0x52e8b0;
    const RULES_BASE: u32 = 0x41e4;
    const RULE_STRIDE: u32 = 0x220;
    const NODE_BASE: u32 = 0x81a8;

    let active = h.read32(ACTIVE_PHASE_GLOBAL);
    if active == 0 || h.read32(descriptor.wrapping_add(4)) != ASSAULT_DESCRIPTOR_TYPE {
        return;
    }
    let rule = h
        .read32(EDITOR_GLOBAL)
        .wrapping_add(RULES_BASE)
        .wrapping_add(
            h.read32(active.wrapping_add(0xa4))
                .wrapping_mul(RULE_STRIDE),
        );
    if h.read32(active.wrapping_add(0xa0)) != 0 {
        let number = h.read32(active.wrapping_add(0xac));
        let text = label(h, 0xd6e);
        let destination = h.read32(ACTIVE_PHASE_GLOBAL).wrapping_add(0x20);
        h.call(0x3c85d4, &[destination, text, number]);
    }
    let node = h
        .read32(EDITOR_GLOBAL)
        .wrapping_add(NODE_BASE)
        .wrapping_add(
            h.read32(rule.wrapping_add(0x8c))
                .wrapping_mul(PHASE_STATE_STRIDE),
        );
    h.register_call(0x321810, Register::Eax, node);
}

#[cfg(test)]
mod tests {
    use super::*;
    use boundary::{
        Memory,
        test_support::{Host, call_digest},
    };
    #[test]
    fn warning_categories_and_full_word_normalization() {
        let mut h = Host::default();
        h.returns.insert(0x3265a0, [0, 0x80000000].into());
        assert_eq!(story_warning(&mut h), 0);
        assert_eq!(assault_warning(&mut h), 1);
        assert_eq!(h.calls[0].args, [0]);
        assert_eq!(h.calls[1].args, [1]);
    }
    #[test]
    fn warning_normalization_covers_every_result_bit() {
        for result in [0, u32::MAX].into_iter().chain((0..32).map(|bit| 1 << bit)) {
            for (category, warning) in [story_warning::<Host>, assault_warning::<Host>]
                .into_iter()
                .enumerate()
            {
                let mut h = Host::default();
                h.returns.insert(0x3265a0, [result].into());
                assert_eq!(warning(&mut h), u32::from(result != 0));
                assert_eq!(h.calls.len(), 1);
                assert_eq!(h.calls[0].target, 0x3265a0);
                assert_eq!(h.calls[0].args, [category as u32]);
            }
        }
    }
    #[test]
    fn builder_signed_clamp_and_service_order() {
        for input in [0, 1, 127, 128, 255] {
            let mut h = Host::default();
            h.write8(0x7de400, input);
            h.write32(0x4e8cec, u32::MAX);
            h.write32(0x7bb1e8, 0x777);
            build_assault_options(&mut h, 0x1000);
            assert_eq!(call_digest(&h.calls), 0x6f8499994e0ccf46);
            assert_eq!(h.read8(0x7de400), if input == 127 { 127 } else { 1 });
            assert_eq!(h.read32(0x7de460), u32::MAX.wrapping_mul(300));
            assert_eq!(h.read32(0x1218), 5);
            assert_eq!(h.calls.last().unwrap().target, 0x77320);
            assert_eq!(h.calls.last().unwrap().args[0], 0x777);
            assert!(
                h.calls
                    .iter()
                    .any(|c| c.target == 0x79d60 && c.args[4] == 0x313f40)
            );
        }
    }
    #[test]
    fn phase_toggle_wraps_and_popup_uses_reloaded_editor() {
        let mut h = Host::default();
        h.write32(0x7844a8, 0x10000);
        h.write32(0x10c4, 0x800000);
        h.write32(0x118c, 0);
        h.write32(0x52e8c4, 1);
        h.write32(0x1805c, 5);
        h.returns.insert(0x71880, [1].into());
        h.returns.insert(0x31fa00, [2].into());
        h.mutations
            .insert(0x3c85d4, [vec![(0x7844a8, 0x20000)]].into());
        assault_phase_input(&mut h, 0x2000, 0x1000);
        assert_eq!(h.read32(0x1805c), 1u32.wrapping_sub(5));
        let capacity = h.calls.iter().find(|c| c.target == 0x31fa00).unwrap();
        assert_eq!(capacity.args, [1]);
        assert_eq!(capacity.registers, [(Register::Edi, 0x2000)]);
        let popup = h.calls.iter().find(|c| c.target == 0x7a270).unwrap();
        assert_eq!(popup.args, [0x1000, 0x20000, 0]);
    }
    #[test]
    fn phase_toggle_preserves_wrapping_word_semantics() {
        for old in [0, 1, 2, i32::MAX as u32, 0x8000_0000, u32::MAX] {
            let mut h = Host::default();
            h.write32(0x7844a8, 0x10000);
            h.write32(0x10c4, 0x800000);
            h.write32(0x118c, 0);
            h.write32(0x52e8c4, 1);
            h.write32(0x1805c, old);
            assault_phase_input(&mut h, 0x2000, 0x1000);
            assert_eq!(h.read32(0x1805c), 1u32.wrapping_sub(old));
            assert_eq!(
                h.calls.iter().map(|call| call.target).collect::<Vec<_>>(),
                [0x3247c0, 0x78980, 0x71880]
            );
        }
    }
    #[test]
    fn formatter_gates_and_passes_node_in_eax() {
        let mut h = Host::default();
        format_assault_phase(&mut h, 0x1000);
        assert!(h.calls.is_empty());
        h.write32(0x762f98, 0x2000);
        h.write32(0x1004, 0x52e8b0);
        h.write32(0x7844a8, 0x10000);
        h.write32(0x20a4, 2);
        h.write32(0x10000 + 0x41e4 + 2 * 0x220 + 0x8c, 3);
        h.write32(0x20a0, 1);
        h.write32(0x20ac, 7);
        format_assault_phase(&mut h, 0x1000);
        assert_eq!(h.calls[1].args, [0x2020, 0, 7]);
        assert_eq!(
            h.calls.last().unwrap().registers,
            [(Register::Eax, 0x10000 + 0x81a8 + 3 * 0xb4)]
        );
    }
}

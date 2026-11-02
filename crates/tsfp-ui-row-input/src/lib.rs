// SPDX-License-Identifier: GPL-3.0-or-later
//! T3809, T3811: native input policy for rows of the original UI framework that the original moves only with its
//! pointer (sliders) or hides behind a group member (greyed entries with a help button). Pure Rust decisions; the
//! guest record access stays in `tsfp-ui-input` (`widgets::slider_step`, `native_page::help_member`).

use std::sync::Mutex;

pub mod command_queue;
pub mod page_stack;
pub mod repeat;
pub mod scroll;

/// Fields of a kind 5 text-entry button.
pub mod button {
    pub const KIND: u32 = 5;
    pub const TEXT: u32 = 0x48;
    pub const LIMIT: u32 = 0x98;
    pub const ARG_A4: u32 = 0xA4;
    pub const ARG_A8: u32 = 0xA8;
    pub const ARG_94: u32 = 0x94;
}

/// Guest globals used by native page actions.
pub mod page_global {
    pub const UI_MODE: u32 = 0x79_094C;
    pub const CURRENT_VIEWPORT: u32 = 0x7B_A9E0;
    pub const SOUND_MUTED: u32 = 0x70_18B4;
    pub const SOUND_BANK_READY: u32 = 0x7B_A16C;
    pub const SOUND_TICK_PTR: u32 = 0x7B_A178;
}

/// UI sounds of the activation and back paths.
pub mod sound {
    pub const ACCEPT: u32 = 0xD26;
    pub const BACK: u32 = 0xD27;
    pub const DENIED: u32 = 0xD2C;
}

/// Row `+0xB4` kind of a slider (`0x7D680`); its `+0xC8` value is a percent.
pub const KIND_SLIDER: u32 = 7;
/// Slider range: `0x7FE00` scales the pointer fraction by `[0x4785C8]` (100.0).
pub const SLIDER_MAX: i32 = 100;
/// Slider step without ticks: a tenth of the range.
pub const SLIDER_FREE_STEP: i32 = 10;

/// The slider value one step from `value` (native policy, not an original port). The original slider `0x7FE00` moves
/// only under a held pointer drag and the cursor step `0x7A7B0` cycles kind 1 rows only. A slider with ticks steps one
/// tick (`100 / ticks`) and snaps to the tick grid, a continuous one steps [`SLIDER_FREE_STEP`]. At an end the value
/// clamps, or with `wrap` goes round to the other end (activation: one button or click reaches every value).
pub fn slider_target(value: i32, ticks: u16, forward: bool, wrap: bool) -> i32 {
    let step = match ticks {
        0 => SLIDER_FREE_STEP,
        n => (SLIDER_MAX / i32::from(n)).max(1),
    };
    let value = value.clamp(0, SLIDER_MAX);
    let on_grid = value - value % step;
    let target = match forward {
        true => on_grid + step,
        false if on_grid < value => on_grid,
        false => on_grid - step,
    };
    match (wrap, forward) {
        (true, true) if value >= SLIDER_MAX => 0,
        (true, false) if value <= 0 => SLIDER_MAX,
        _ => target.clamp(0, SLIDER_MAX),
    }
}

/// One member of a row group as the help redirect sees it: usable (not greyed, not skipped) and with a handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupMember {
    pub usable: bool,
    pub has_handler: bool,
}

/// T3811: the column of the help button beside a greyed entry: the first member after column 0 that is usable and has a
/// handler. The Map Maker preview page (`0x314420`) builds an unavailable mode as [greyed name, "?" button `0x3126D0`].
pub fn help_column(members: &[GroupMember]) -> Option<usize> {
    (1..members.len()).find(|&column| members[column].usable && members[column].has_handler)
}

/// Row kinds an activation changes something on: the cases of `0x800A0` (2 option list, 4 check box, 5 button) and the
/// native slider (7). Kind 1 rows take their input from left and right.
pub const KIND_ACTIVATIONS: &[u32] = &[1, 2, 4, 5, KIND_SLIDER];

static UNHANDLED: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());

/// T3809 detector (I-T3809-1): an activation reached a row of `kind` on `page` (built by `builder`) that nothing
/// handled. The report line, once per builder and kind; None when that pair was reported before.
pub fn unhandled_activation(page: u32, builder: u32, kind: u32) -> Option<String> {
    let mut seen = UNHANDLED.lock().ok()?;
    if seen.contains(&(builder, kind)) {
        return None;
    }
    seen.push((builder, kind));
    Some(format!(
        "native input unhandled: activation of row kind {kind} on page {page:#x} builder {builder:#x} changed nothing \
         (no row, page or kind handler) (I-T3809-1)"
    ))
}

/// The (builder, kind) pairs reported so far.
pub fn unhandled_kinds() -> Vec<(u32, u32)> {
    UNHANDLED
        .lock()
        .map(|seen| seen.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_target_follows_ticks_and_snaps_off_grid_values() {
        assert_eq!(slider_target(50, 0, true, false), 60);
        assert_eq!(slider_target(37, 0, true, false), 40);
        assert_eq!(slider_target(37, 0, false, false), 30);
        assert_eq!(slider_target(0, 0, false, false), 0);
        assert_eq!(slider_target(0, 0, false, true), 100);
        assert_eq!(slider_target(100, 0, true, true), 0);
        assert_eq!(slider_target(100, 0, true, false), 100);
        assert_eq!(slider_target(50, 4, true, false), 75);
        assert_eq!(slider_target(99, 4, true, false), 100);
    }

    #[test]
    fn help_column_is_the_first_usable_member_with_a_handler() {
        let member = |usable, has_handler| GroupMember {
            usable,
            has_handler,
        };
        assert_eq!(
            help_column(&[member(false, false), member(true, true)]),
            Some(1)
        );
        assert_eq!(
            help_column(&[member(false, false), member(false, true)]),
            None
        );
        assert_eq!(
            help_column(&[
                member(false, false),
                member(true, false),
                member(true, true)
            ]),
            Some(2)
        );
        assert_eq!(
            help_column(&[member(true, true)]),
            None,
            "column 0 is the entry itself"
        );
    }

    #[test]
    fn unhandled_activations_report_once_per_builder_and_kind() {
        let first = unhandled_activation(0x6F_F29C, 0x31_2270, 7).unwrap();
        assert!(
            first.starts_with("native input unhandled: activation of row kind 7 on page 0x6ff29c")
        );
        assert!(first.ends_with("(I-T3809-1)"));
        assert_eq!(unhandled_activation(0x6F_F29C, 0x31_2270, 7), None);
        assert!(unhandled_activation(0x6F_F29C, 0x31_3010, 7).is_some());
        assert!(unhandled_kinds().contains(&(0x31_2270, 7)));
    }
}
/// Original page functions reached through the bindings.
pub mod original {
    /// Page veto before activation or back (`page`) -> nonzero = allowed.
    pub const PAGE_ACCEPTS: u32 = 0x7_3DE0;
    /// Activation of a row with a kind (`page`, `row`).
    pub const ROW_KIND_ACTIVATE: u32 = 0x8_00A0;
    /// Play a UI sound (`sound id`).
    pub const PLAY_SOUND: u32 = 0x2_1780;
    /// Parent context lookup (`page`, 1) -> context record.
    pub const PARENT_CONTEXT: u32 = 0x7_1AA0;
    /// Pop the context stack (`parent`).
    pub const POP_CONTEXT: u32 = 0x7_9C00;
    /// Page ready for input (`page`) -> nonzero.
    pub const PAGE_READY: u32 = 0x7_4760;
    /// Open a text entry from a push button, and the entry's done callback.
    pub const OPEN_TEXT_ENTRY: u32 = 0x7_D090;
    pub const TEXT_ENTRY_DONE: u32 = 0x3_89B0;
}

//! T2332: pause handlers on typed state, attached to native page commands.
//! The GuestUi store adapter is a TEMPORARY-SHIM under T2044 S3.
//! Eight offline callbacks have Tier 2 page-transition admission (T2332 receipts).
//! Cold alternate/header candidates remain Tier 0 opt-ins; no rendering claim.
use crate::native_page::{GuestUi, PageRef};
use tsfp_ui_model::pause_actions::{PauseAction, PauseActionInputs, PauseActionState, PauseEffect};

/// Owned pause-widget presentation state at the page-construction boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PauseWidgetState {
    pub flags: u32,
    pub fade: f32,
}
impl PauseWidgetState {
    pub fn close(&mut self) {
        self.flags |= 8;
        self.fade = 0.0;
    }
    pub fn mark_exiting(&mut self) {
        self.flags |= 0x40;
    }
}

/// Confirmation decisions are native; session/mission operations are neighbouring services.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirmation {
    Cancel,
    Quit,
    Restart,
    Alternate,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfirmationState {
    pub open_mask: u32,
}
pub trait ConfirmationServices {
    fn pop(&mut self);
    fn resume(&mut self);
    fn player_count(&mut self) -> i32;
    fn mark_exiting(&mut self, player: u32);
    fn prepare_exit(&mut self);
    fn quit(&mut self, alternate: bool);
    fn restart(&mut self);
    fn publish(&mut self, state: &ConfirmationState);
}
pub fn confirm(
    state: &mut ConfirmationState,
    action: Confirmation,
    services: &mut dyn ConfirmationServices,
) {
    if action == Confirmation::Cancel {
        state.open_mask = 0;
        services.publish(state);
        services.pop();
        return;
    }
    services.resume();
    // The original re-queries count at every iteration; retain this service contract.
    let mut player = 0;
    while player < services.player_count() {
        services.mark_exiting(player as u32);
        player += 1;
    }
    services.prepare_exit();
    if action == Confirmation::Restart {
        services.restart();
    } else {
        services.quit(action == Confirmation::Alternate);
        state.open_mask = 0;
        services.publish(state);
    }
}

/// Header-tab navigation on the original runtime descriptor table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageDirection {
    Previous,
    Next,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PageEntry {
    pub descriptor: u32,
    pub available: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageCycle {
    pub entries: [PageEntry; 12],
    pub current: u32,
    pub blocked: bool,
}
impl PageCycle {
    pub fn neighbour(
        &self,
        direction: PageDirection,
        mut available: impl FnMut(u32) -> bool,
    ) -> Option<(usize, u32)> {
        if self.blocked {
            return None;
        }
        let current = self
            .entries
            .iter()
            .position(|e| e.descriptor == self.current)?;
        match direction {
            PageDirection::Previous => (0..current).rev().find_map(|i| {
                let e = self.entries[i];
                (e.available != 0 && available(e.available)).then_some((i, e.descriptor))
            }),
            PageDirection::Next => (current + 1..12).find_map(|i| {
                let e = self.entries[i];
                (e.available != 0 && available(e.available)).then_some((i, e.descriptor))
            }),
        }
    }
}
fn cycle_page(g: &mut dyn GuestUi, page: PageRef, direction: PageDirection) {
    let slot = read(g, 0x7BA9E0);
    let bit = 1u32 << (slot & 31);
    let cycle = PageCycle {
        entries: std::array::from_fn(|i| PageEntry {
            descriptor: read(g, 0x5247C4 + i as u32 * 12),
            available: read(g, 0x5247C8 + i as u32 * 12),
        }),
        current: read(g, page.0 + 4),
        blocked: (read(g, 0x789640) | read(g, 0x7894FC)) & bit != 0,
    };
    // Original invokes the directional query twice, including sound/fade effects.
    let mut destination = None;
    for _ in 0..2 {
        let Some((index, descriptor)) =
            cycle.neighbour(direction, |va| g.call(va, &[]).unwrap_or(0) != 0)
        else {
            return;
        };
        if read(g, 0x7BA16C) != 0 {
            let table = read(g, 0x7BA178);
            let sound = read(g, table + 8);
            g.call(0x21780, &[sound]);
        }
        let extreme = match direction {
            PageDirection::Previous => 0x2E49F0,
            PageDirection::Next => 0x2E4A30,
        };
        let edge = g.call(extreme, &[]).unwrap_or(0) as i32;
        let (adjacent, animation, timer) = match direction {
            PageDirection::Previous => (index as i32, 1, 0x25C),
            PageDirection::Next => (index as i32, 2, 0x260),
        };
        let widget = read(g, 0x761C48 + read(g, 0x7BA9E0) * 4);
        g.write32(widget + 0x264, if adjacent == edge { animation } else { 0 });
        g.write32(widget + timer, 1.0f32.to_bits());
        destination = Some(descriptor);
    }
    let context = read(g, 0x761BBC + read(g, 0x7BA9E0) * 4);
    g.call(0x79860, &[context, destination.unwrap(), u32::MAX]);
}

/// Tier 2 page-topology callbacks, native by default with per-handler A/B.
pub const ADMITTED_HANDLERS: &[u32] = &[
    0x2EBB80, 0x2EC1A0, 0x2EC1F0, 0x2EBC30, 0x2EC0F0, 0x2EBBB0, 0x2EBBC0, 0x2EC090,
];
/// Tier 0 translated/compiled/tested candidates, not exercised by these traces.
pub const COLD_HANDLERS: &[u32] = &[0x2EBD50, 0x2EBCD0, 0x2F2F50, 0x2F2FA0];
pub fn enabled_for(handler: u32) -> bool {
    match std::env::var("TSFP_UI_INGAME_HANDLERS") {
        Ok(v) => selected_handler(&v, handler),
        Err(_) => ADMITTED_HANDLERS.contains(&handler),
    }
}
fn selected_handler(v: &str, handler: u32) -> bool {
    v == "1"
        || v.split(',').any(|s| {
            u32::from_str_radix(s.trim().strip_prefix("0x").unwrap_or(s.trim()), 16) == Ok(handler)
        })
}
fn read(g: &dyn GuestUi, a: u32) -> u32 {
    g.read32(a).unwrap_or(0)
}
fn import(g: &dyn GuestUi) -> PauseActionState {
    PauseActionState {
        confirm_open_mask: read(g, 0x789640),
        audio_video_open_mask: read(g, 0x7894FC),
        confirm_lines: [read(g, 0x762A18), read(g, 0x762A1C)],
        confirm_yes: read(g, 0x761C5C),
        confirm_no: read(g, 0x761C60),
        leave_match_pending: read(g, 0x762A20) != 0,
        controls_dirty: read(g, 0x78963C),
    }
}
fn publish(g: &mut dyn GuestUi, before: &PauseActionState, after: &PauseActionState) {
    for (address, old, new) in [
        (0x789640, before.confirm_open_mask, after.confirm_open_mask),
        (
            0x7894FC,
            before.audio_video_open_mask,
            after.audio_video_open_mask,
        ),
        (0x762A18, before.confirm_lines[0], after.confirm_lines[0]),
        (0x762A1C, before.confirm_lines[1], after.confirm_lines[1]),
        (0x761C5C, before.confirm_yes, after.confirm_yes),
        (0x761C60, before.confirm_no, after.confirm_no),
        (
            0x762A20,
            u32::from(before.leave_match_pending),
            u32::from(after.leave_match_pending),
        ),
        (0x78963C, before.controls_dirty, after.controls_dirty),
    ] {
        if old != new {
            g.write32(address, new);
        }
    }
}
fn resume(g: &mut dyn GuestUi) {
    let slot = read(g, 0x7BA9E0);
    // Capture widget before page construction, exactly as the original.
    let widget = read(g, 0x761C48 + slot * 4);
    let context = read(g, 0x761BBC + slot * 4);
    g.call(0x79B70, &[context, 0]);
    let mut state = PauseWidgetState {
        flags: read(g, widget),
        fade: f32::from_bits(read(g, widget + 0xC)),
    };
    state.close();
    g.write32(widget + 0xC, state.fade.to_bits());
    g.write32(widget, state.flags);
}
struct GuestConfirmation<'a> {
    guest: &'a mut dyn GuestUi,
    page: PageRef,
}
impl ConfirmationServices for GuestConfirmation<'_> {
    fn pop(&mut self) {
        let parent = read(self.guest, self.page.0 + 0x2C);
        crate::page_stack::pop(self.guest, PageRef(parent));
    }
    fn resume(&mut self) {
        resume(self.guest);
    }
    fn player_count(&mut self) -> i32 {
        self.guest.call(0x1CDAE0, &[]).unwrap_or(0) as i32
    }
    fn mark_exiting(&mut self, player: u32) {
        let widget = read(self.guest, 0x761C48 + player * 4);
        let mut state = PauseWidgetState {
            flags: read(self.guest, widget),
            fade: f32::from_bits(read(self.guest, widget + 0xC)),
        };
        state.mark_exiting();
        self.guest.write32(widget, state.flags);
    }
    fn prepare_exit(&mut self) {
        self.guest.call(0x668F0, &[]);
    }
    fn quit(&mut self, alternate: bool) {
        let destination = if alternate {
            let game = read(self.guest, 0x7B0C7C);
            let level = read(self.guest, game);
            self.guest.call(0x25EE60, &[level]).unwrap_or(0)
        } else {
            0
        };
        self.guest.call(0x653B0, &[destination]);
    }
    fn restart(&mut self) {
        self.guest.call(0x65470, &[]);
    }
    fn publish(&mut self, state: &ConfirmationState) {
        self.guest.write32(0x789640, state.open_mask);
    }
}

pub fn dispatch(g: &mut dyn GuestUi, page: PageRef, handler: u32) -> bool {
    if !enabled_for(handler) {
        return false;
    }
    dispatch_selected(g, page, handler)
}
fn dispatch_selected(g: &mut dyn GuestUi, page: PageRef, handler: u32) -> bool {
    if matches!(handler, 0x2F2F50 | 0x2F2FA0) {
        cycle_page(
            g,
            page,
            if handler == 0x2F2F50 {
                PageDirection::Previous
            } else {
                PageDirection::Next
            },
        );
        log(handler);
        return true;
    }
    let confirmation = match handler {
        0x2EBBB0 => Some(Confirmation::Cancel),
        0x2EBBC0 => Some(Confirmation::Quit),
        0x2EC090 => Some(Confirmation::Restart),
        0x2EBCD0 => Some(Confirmation::Alternate),
        _ => None,
    };
    if let Some(action) = confirmation {
        let mut state = ConfirmationState {
            open_mask: read(g, 0x789640),
        };
        confirm(
            &mut state,
            action,
            &mut GuestConfirmation { guest: g, page },
        );
    } else if let Some(action) = PauseAction::from_handler(handler) {
        // Online/leave-session are outside this offline page frontier.
        if matches!(action, PauseAction::Online | PauseAction::LeaveSession) {
            return false;
        }
        if action == PauseAction::Resume {
            resume(g);
        } else {
            let mut before = import(g);
            let mut state = before;
            let inputs = PauseActionInputs {
                current_slot: read(g, 0x7BA9E0),
                page_slot: read(g, page.0 + 0x38),
                mode: (read(g, 0x7DE454) >> 8) as u8,
                flags: read(g, 0x7DE458),
                ..Default::default()
            };
            let effects = state.activate(action, &inputs);
            // Controls copies the snapshot before clearing dirty; audio sets the mask
            // before calling the loader. Keep those observable service orders.
            for effect in effects {
                match effect {
                    PauseEffect::SnapshotControls { slot } => {
                        let root = read(g, 0x7B0C7C);
                        let source = read(g, root + 0x1500);
                        let words: [u32; 31] =
                            std::array::from_fn(|i| read(g, source + i as u32 * 4));
                        for (i, word) in words.into_iter().enumerate() {
                            g.write32(0x7BA364 + slot * 0x1E0 + i as u32 * 4, word);
                        }
                    }
                    PauseEffect::LoadAudioSettings => {
                        publish(g, &before, &state);
                        before = state;
                        g.call(0x2C3300, &[]);
                    }
                    PauseEffect::PushPage { slot, page } => {
                        publish(g, &before, &state);
                        let slot = if action == PauseAction::AudioVideo {
                            read(g, 0x7BA9E0)
                        } else {
                            slot
                        };
                        let context = read(g, 0x761BBC + slot * 4);
                        g.call(0x79B70, &[context, page]);
                    }
                    _ => unreachable!("excluded network/resume effects"),
                }
            }
        }
    } else {
        return false;
    }
    log(handler);
    true
}

fn log(handler: u32) {
    if std::env::var("TSFP_UI_NATIVE_LOG").is_ok_and(|v| v == "1") {
        eprintln!("ui-frontier: native in-game handler {handler:#x}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Services {
        events: Vec<String>,
        count: i32,
    }
    impl ConfirmationServices for Services {
        fn pop(&mut self) {
            self.events.push("pop".into());
        }
        fn resume(&mut self) {
            self.events.push("resume".into());
        }
        fn player_count(&mut self) -> i32 {
            self.events.push("count".into());
            self.count
        }
        fn mark_exiting(&mut self, p: u32) {
            self.events.push(format!("exit {p}"));
        }
        fn prepare_exit(&mut self) {
            self.events.push("prepare".into());
        }
        fn quit(&mut self, a: bool) {
            self.events.push(format!("quit {a}"));
        }
        fn restart(&mut self) {
            self.events.push("restart".into());
        }
        fn publish(&mut self, s: &ConfirmationState) {
            self.events.push(format!("mask {}", s.open_mask));
        }
    }
    #[test]
    fn cancel_clears_all_slots_before_pop() {
        let mut state = ConfirmationState { open_mask: 15 };
        let mut s = Services::default();
        confirm(&mut state, Confirmation::Cancel, &mut s);
        assert_eq!(s.events, ["mask 0", "pop"]);
    }
    #[test]
    fn confirms_preserve_exit_order_and_restart_mask() {
        for action in [
            Confirmation::Quit,
            Confirmation::Alternate,
            Confirmation::Restart,
        ] {
            let mut state = ConfirmationState { open_mask: 15 };
            let mut s = Services {
                count: 2,
                ..Default::default()
            };
            confirm(&mut state, action, &mut s);
            assert_eq!(
                &s.events[..7],
                [
                    "resume", "count", "exit 0", "count", "exit 1", "count", "prepare"
                ]
            );
            if action == Confirmation::Restart {
                assert_eq!(state.open_mask, 15);
                assert_eq!(s.events[7], "restart");
            } else {
                assert_eq!(state.open_mask, 0);
                assert_eq!(s.events[8], "mask 0");
            }
        }
    }
    #[test]
    fn page_cycle_respects_masks_and_skips_unavailable_tabs() {
        let mut cycle = PageCycle {
            entries: std::array::from_fn(|i| PageEntry {
                descriptor: 100 + i as u32,
                available: i as u32 + 1,
            }),
            current: 105,
            blocked: false,
        };
        assert_eq!(
            cycle.neighbour(PageDirection::Previous, |v| v == 3),
            Some((2, 102))
        );
        assert_eq!(
            cycle.neighbour(PageDirection::Next, |v| v == 9),
            Some((8, 108))
        );
        assert_eq!(cycle.neighbour(PageDirection::Next, |_| false), None);
        cycle.blocked = true;
        assert_eq!(
            cycle.neighbour(PageDirection::Next, |_| panic!(
                "blocked queries must not run"
            )),
            None
        );
        cycle.blocked = false;
        cycle.current = 999;
        assert_eq!(cycle.neighbour(PageDirection::Previous, |_| true), None);
    }
    #[derive(Default)]
    struct Guest {
        words: std::collections::HashMap<u32, u32>,
        calls: Vec<(u32, Vec<u32>)>,
    }
    impl GuestUi for Guest {
        fn read32(&self, a: u32) -> Option<u32> {
            Some(*self.words.get(&a).unwrap_or(&0))
        }
        fn write32(&mut self, a: u32, v: u32) -> bool {
            self.words.insert(a, v);
            true
        }
        fn call(&mut self, va: u32, args: &[u32]) -> Option<u32> {
            self.calls.push((va, args.to_vec()));
            if va == 0x79B70 && args[1] == 0 {
                self.words.insert(0x761C48 + 4, 0x9000);
                self.words.insert(0x3000, 0x20);
            }
            Some(0)
        }
    }
    #[test]
    fn resume_retains_pre_open_widget_and_reads_post_open_flags() {
        let mut g = Guest::default();
        g.words.extend([
            (0x7BA9E0, 1),
            (0x761C48 + 4, 0x3000),
            (0x761BBC + 4, 0x4000),
            (0x3000, 1),
        ]);
        assert!(dispatch_selected(&mut g, PageRef(0x2000), 0x2EBB80));
        assert_eq!(g.words[&0x3000], 0x28);
        assert_eq!(g.words[&0x300C], 0);
        assert!(!g.words.contains_key(&0x900C));
        assert_eq!(g.calls, [(0x79B70, vec![0x4000, 0])]);
    }
    #[test]
    fn controls_adapter_copies_31_words_to_current_slot_but_opens_page_slot() {
        let mut g = Guest::default();
        g.words.extend([
            (0x7BA9E0, 1),
            (0x2038, 2),
            (0x7B0C7C, 0x10000),
            (0x11500, 0x20000),
            (0x761BBC + 8, 0x4000),
            (0x78963C, 99),
        ]);
        let target = 0x7BA364 + 0x1E0;
        for i in 0..32 {
            g.words.insert(0x20000 + i * 4, i + 20);
        }
        g.words.insert(target + 31 * 4, 123);
        assert!(dispatch_selected(&mut g, PageRef(0x2000), 0x2EC1A0));
        for i in 0..31 {
            assert_eq!(g.words[&(target + i * 4)], i + 20);
        }
        assert_eq!(g.words[&(target + 31 * 4)], 123);
        assert_eq!(g.words[&0x78963C], 0);
        assert_eq!(g.calls, [(0x79B70, vec![0x4000, 0x524A40])]);
    }
    #[test]
    fn admission_is_scoped_to_trace_receipts_and_can_be_bisected() {
        assert_eq!(ADMITTED_HANDLERS.len(), 8);
        for &va in ADMITTED_HANDLERS {
            assert!(!COLD_HANDLERS.contains(&va));
            assert!(!selected_handler("0", va));
            assert!(selected_handler("1", va));
            assert!(selected_handler(&format!("{va:#x}"), va));
        }
        for &va in COLD_HANDLERS {
            assert!(!ADMITTED_HANDLERS.contains(&va));
        }
        assert!(!selected_handler("0x2ec090", 0x2EBB80));
    }
}

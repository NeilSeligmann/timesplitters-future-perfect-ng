// SPDX-License-Identifier: GPL-3.0-or-later
//! T3772: the profile selection page (builder `0x194980`) answers each player on its own controller port.
//!
//! A two player game shows one instance of the page per player (page record `+0x38` is the player): in the owner run
//! of 2026-10-26 slot 3 was player 1's profile list and slot 4 player 2's "press START" panel (MEASURED,
//! `docs/t3772-profile-page-ports.md`). The original update `0x192770` reads each player's pad through its port map, so
//! player 1's pad moves player 1's list and a pad that does not play yet joins with START. The generic native page queue
//! has no port and drove the top instance (player 2's panel) with every device: player 1 could not select a profile and
//! the START of the second pad, held back from the guest, joined nobody.
//!
//! Here every command carries the port it came from (router slot n is guest port n, T3771): [`PortInput`] turns the
//! held intents of each port into press edges, [`target`] picks the page instance of the player on that port (or, for a
//! START, the instance whose player waits for a controller) and [`service`] runs it at the guest frame boundary
//! through [`crate::native_page::service_on`], whose profile branches are the ports of `0x192770`
//! ([`crate::profile_page`]).
use std::collections::VecDeque;
use std::sync::Mutex;

use crate::input::{IntentSet, MenuIntent, PageEdgeGate};
use crate::native_page::{
    self, CommandQueue, GuestUi, NativeCommand, Outcome, PAGE_SLOTS, PageRef,
};
use crate::page_intents::native_page_commands;
use crate::profile_page::{self, BUILDER};

/// Controller ports (and router slots).
pub const PORTS: usize = 4;

/// Live press edges of each port while the profile page owns input.
#[derive(Clone, Debug, Default)]
pub struct PortInput {
    gates: [PageEdgeGate; PORTS],
    before: [IntentSet; PORTS],
    repeat_at: [[u64; 4]; PORTS],
}

impl PortInput {
    /// One input frame. `owner`: the page owning input (fed every frame, so a press held across a page change acts on
    /// neither page, T3414), `held`: the intents held on each port. `None` unless the profile page owns input; then
    /// the commands of this frame, each with its port: steps with key repeat, Accept, Delete, Back and START (Join) on
    /// their press edge.
    pub fn commands(
        &mut self,
        owner: Option<u32>,
        held: [IntentSet; PORTS],
        now: u64,
    ) -> Option<Vec<(u8, NativeCommand)>> {
        let mut commands = Vec::new();
        for port in 0..PORTS {
            let held = self.gates[port].filter(owner, held[port]);
            let before = std::mem::replace(&mut self.before[port], held);
            if owner != Some(BUILDER) {
                continue;
            }
            let edge = |intent| held.contains(intent) && !before.contains(intent);
            let mut port_commands =
                native_page_commands(held, before, None, now, &mut self.repeat_at[port]);
            if edge(MenuIntent::Back) {
                port_commands.push(NativeCommand::Back);
            }
            if edge(MenuIntent::Start) {
                port_commands.push(NativeCommand::Join(port as u8));
            }
            commands.extend(port_commands.into_iter().map(|c| (port as u8, c)));
        }
        (owner == Some(BUILDER)).then_some(commands)
    }
}

/// Live profile page instances (slot order) with their player and save menu state.
fn instances(g: &dyn GuestUi) -> Vec<(PageRef, u32, u32)> {
    (0..PAGE_SLOTS)
        .map(PageRef::slot)
        .filter(|p| p.live(g) && p.builder(g) == BUILDER)
        .filter_map(|p| profile_page::page_player(g, p).map(|(player, state)| (p, player, state)))
        .collect()
}

/// The page instance `command` from port `port` acts on. START (Join) goes to the instance whose player waits for a
/// controller (the lowest such player), every other command to the instance of the player on that port. Port 0 falls
/// back to the lowest player that is past the join when no player is mapped to it (one player games keep their
/// pre-T3772 behaviour whatever `0x74E914` holds). `None`: no instance answers that port.
pub fn target(g: &dyn GuestUi, port: u8, command: NativeCommand) -> Option<PageRef> {
    let pages = instances(g);
    if let NativeCommand::Join(_) = command {
        return pages
            .iter()
            .filter(|(page, ..)| profile_page::waits_for_join(g, *page))
            .min_by_key(|(_, player, _)| *player)
            .map(|(page, ..)| *page);
    }
    let joined = |(page, _, _): &&(PageRef, u32, u32)| !profile_page::waits_for_join(g, *page);
    pages
        .iter()
        .filter(joined)
        .find(|(_, player, _)| profile_page::player_port(g, *player) == Some(port))
        .or_else(|| {
            let mapped = pages
                .iter()
                .any(|(_, player, _)| profile_page::player_port(g, *player) == Some(port));
            (port == 0 && !mapped)
                .then(|| {
                    pages
                        .iter()
                        .filter(joined)
                        .min_by_key(|(_, player, _)| *player)
                })
                .flatten()
        })
        .map(|(page, ..)| *page)
}

/// Commands waiting for the guest frame, each with its port (input thread to guest thread).
static QUEUE: Mutex<VecDeque<(u8, NativeCommand)>> = Mutex::new(VecDeque::new());

fn queue() -> std::sync::MutexGuard<'static, VecDeque<(u8, NativeCommand)>> {
    QUEUE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Queue commands for [`service`].
pub fn push(commands: impl IntoIterator<Item = (u8, NativeCommand)>) {
    queue().extend(commands);
}

/// Drop the waiting commands (the page left).
pub fn clear() {
    queue().clear();
}

/// Guest frame boundary: run the queued commands on their page instance. A command waits while its page is not ready
/// for input; one activation, back, delete or join per frame (any may replace the page). `out` gets
/// `(port, command, outcome)`.
pub fn service(g: &mut dyn GuestUi, out: &mut Vec<(u8, NativeCommand, Outcome)>) {
    scripted_join(g);
    loop {
        let Some((port, command)) = queue().front().copied() else {
            return;
        };
        let Some(page) = target(g, port, command) else {
            queue().pop_front();
            out.push((port, command, Outcome::NoRow));
            continue;
        };
        let (consumed, terminal) = run(g, page, command, |outcome| {
            out.push((port, command, outcome))
        });
        if !consumed {
            return;
        }
        queue().pop_front();
        if terminal {
            return;
        }
    }
}

/// Diagnostic for headless evidence runs, which have no pads (like `TSFP_UI_AUTOSELECT`, not a variant):
/// `TSFP_PROFILE_JOIN_PORT=N` presses START on port N once, the first time a player waits for a controller. The press
/// is scripted (FABRICATED input); the join it drives is the native path a real pad takes.
fn scripted_join(g: &dyn GuestUi) {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let Some(port) = std::env::var("TSFP_PROFILE_JOIN_PORT")
        .ok()
        .and_then(|value| value.parse::<u8>().ok())
    else {
        return;
    };
    let waiting = instances(g)
        .iter()
        .any(|(page, ..)| profile_page::waits_for_join(g, *page));
    if waiting && !DONE.swap(true, std::sync::atomic::Ordering::AcqRel) {
        eprintln!(
            "tsfp-ui: TSFP_PROFILE_JOIN_PORT: scripted START on port {port} (FABRICATED press)"
        );
        push([(port, NativeCommand::Join(port))]);
    }
}

/// One command on `page` through the native page service. Returns (consumed, ends the frame).
fn run(
    g: &mut dyn GuestUi,
    page: PageRef,
    command: NativeCommand,
    mut report: impl FnMut(Outcome),
) -> (bool, bool) {
    let single = CommandQueue::from(VecDeque::from([command]));
    let mut done = Vec::new();
    native_page::service_on(g, Some(page), &single, &mut done);
    // Not ready yet (page fade): the press waits for the page like the original's held button does.
    let Some((_, outcome)) = done
        .first()
        .copied()
        .filter(|(_, o)| *o != Outcome::NotReady)
    else {
        return (false, false);
    };
    report(outcome);
    let terminal = !matches!(
        command,
        NativeCommand::Step(_) | NativeCommand::Focus(_) | NativeCommand::Scroll(_)
    );
    (true, terminal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu_state::layout::page;
    use std::collections::HashMap;

    /// A fake guest with the two instances of a two player profile page: player 0 (port 0) on its list in slot 3,
    /// player 1 waiting for a controller in slot 4. Ports 0 and 1 hold a pad.
    #[derive(Default)]
    struct TwoPlayers {
        mem: HashMap<u32, u32>,
        calls: Vec<(u32, Vec<u32>)>,
    }

    const PLAYER_STATE: u32 = 0x74_F9EC;
    const PLAYER_PORT: u32 = 0x74_E914;
    const PAGE_PLAYER: u32 = 0x38;
    const PAGE_FADE: u32 = 0x1B8;
    const FADE_READY: u32 = 0x47_5CD4;
    const PAD_PRESENT: u32 = 0x4_2710;
    const PAD_MAP_SET: u32 = 0x4_2D00;
    const PAGE_READY: u32 = 0x7_4760;

    impl GuestUi for TwoPlayers {
        fn read32(&self, va: u32) -> Option<u32> {
            Some(*self.mem.get(&va).unwrap_or(&0))
        }
        fn write32(&mut self, va: u32, value: u32) -> bool {
            self.mem.insert(va, value);
            true
        }
        fn call(&mut self, va: u32, args: &[u32]) -> Option<u32> {
            self.calls.push((va, args.to_vec()));
            Some(match va {
                PAD_PRESENT => u32::from(args.first().is_some_and(|port| *port < 2)),
                0x7_1A60 => 0,
                _ => 1,
            })
        }
    }

    fn two_players() -> TwoPlayers {
        let mut g = TwoPlayers::default();
        for (slot, player, state, port) in [(3, 0, 2, 0u32), (4, 1, 1, u32::MAX)] {
            let p = PageRef::slot(slot);
            g.mem.insert(p.0, 0x4E_8D54);
            g.mem.insert(p.0 + page::BUILDER, BUILDER);
            g.mem.insert(p.0 + page::STATE, 2);
            g.mem.insert(p.0 + PAGE_PLAYER, player);
            g.mem.insert(p.0 + PAGE_FADE, 1.0f32.to_bits());
            g.mem.insert(PLAYER_STATE + 4 * player, state);
            g.mem.insert(PLAYER_PORT + 4 * player, port);
        }
        g.mem.insert(FADE_READY, 0.5f32.to_bits());
        g.mem.insert(0x52_36C4, 2);
        g
    }

    fn held(port: usize, intent: MenuIntent) -> [IntentSet; PORTS] {
        let mut held = [IntentSet::default(); PORTS];
        held[port] = held[port].with(intent);
        held
    }

    #[test]
    fn each_port_drives_its_own_player_and_start_joins_the_waiting_one() {
        let g = two_players();
        let (p1, p2) = (PageRef::slot(3), PageRef::slot(4));
        // The old resolver (top instance) picked player 2's panel for every device.
        assert_eq!(
            native_page::find_page(&g, BUILDER).map(|(_, p)| p),
            Some(p2)
        );
        assert_eq!(target(&g, 0, NativeCommand::Accept), Some(p1));
        assert_eq!(
            target(&g, 0, NativeCommand::Step(crate::widgets::Step::Down)),
            Some(p1)
        );
        assert_eq!(target(&g, 1, NativeCommand::Join(1)), Some(p2));
        // Port 1 does not play yet: its A, steps and back act on nothing.
        assert_eq!(target(&g, 1, NativeCommand::Accept), None);
    }

    #[test]
    fn start_on_the_second_pad_joins_player_two_through_the_original_join_path() {
        let mut g = two_players();
        push([(1, NativeCommand::Join(1))]);
        let mut out = Vec::new();
        service(&mut g, &mut out);
        assert_eq!(out.len(), 1, "{out:?}");
        assert!(
            matches!(
                out[0],
                (1, NativeCommand::Join(1), Outcome::Activated { .. })
            ),
            "{out:?}"
        );
        assert_eq!(g.mem[&(PLAYER_PORT + 4)], 1, "player 2 plays on port 1");
        assert_eq!(
            g.mem[&(PLAYER_STATE + 4)],
            2,
            "player 2 reaches its profile list"
        );
        assert!(g.calls.contains(&(PAD_MAP_SET, vec![1, 1])));
        // Now port 1 drives player 2's instance and port 0 still player 1's.
        assert_eq!(target(&g, 1, NativeCommand::Accept), Some(PageRef::slot(4)));
        assert_eq!(target(&g, 0, NativeCommand::Accept), Some(PageRef::slot(3)));
        // A second START from port 0 (already playing) joins nobody.
        g.mem.insert(PLAYER_STATE + 4, 1);
        g.mem.insert(PLAYER_PORT + 4, u32::MAX);
        let page = PageRef::slot(4);
        assert_eq!(profile_page::join(&mut g, page, 0), Some(Outcome::Denied));
        // An absent port cannot join either.
        assert_eq!(profile_page::join(&mut g, page, 2), Some(Outcome::Denied));
        assert_eq!(g.mem[&(PLAYER_STATE + 4)], 1);
        let _ = PAGE_READY;
    }

    #[test]
    fn live_edges_carry_their_port_and_a_held_press_does_not_cross_pages() {
        let mut input = PortInput::default();
        let none = [IntentSet::default(); PORTS];
        // START held while the Players page owned input: swallowed on the profile page.
        assert_eq!(
            input.commands(Some(0x2D_2350), held(1, MenuIntent::Start), 0),
            None
        );
        assert_eq!(
            input.commands(Some(BUILDER), held(1, MenuIntent::Start), 1),
            Some(vec![])
        );
        assert_eq!(input.commands(Some(BUILDER), none, 2), Some(vec![]));
        assert_eq!(
            input.commands(Some(BUILDER), held(1, MenuIntent::Start), 3),
            Some(vec![(1, NativeCommand::Join(1))])
        );
        assert_eq!(
            input.commands(Some(BUILDER), held(0, MenuIntent::Accept), 4),
            Some(vec![(0, NativeCommand::Accept)])
        );
        assert_eq!(
            input.commands(Some(BUILDER), held(1, MenuIntent::Back), 5),
            Some(vec![(1, NativeCommand::Back)])
        );
        assert_eq!(
            input.commands(Some(BUILDER), held(1, MenuIntent::Down), 6),
            Some(vec![(1, NativeCommand::Step(crate::widgets::Step::Down))])
        );
    }

    /// T3781: when the profile page closes (the main menu takes input) the per-port routing lets go: no commands, the
    /// generic page input keeps the frame (`None`), and the port gates swallow a press held across the change.
    #[test]
    fn routing_resets_when_the_two_player_page_closes() {
        const MAIN_MENU: u32 = 0x2D_0BE0;
        let mut input = PortInput::default();
        let accept = held(1, MenuIntent::Accept);
        assert_eq!(
            input.commands(Some(BUILDER), held(1, MenuIntent::Down), 0),
            Some(vec![])
        );
        assert_eq!(input.commands(Some(MAIN_MENU), accept, 1), None);
        assert_eq!(input.commands(None, accept, 2), None);
        // Back on the profile page later: the A held since the main menu does not act there.
        assert_eq!(input.commands(Some(BUILDER), accept, 3), Some(vec![]));
    }

    #[test]
    fn one_player_games_keep_the_keyboard_and_first_pad_on_player_one() {
        let mut g = two_players();
        // One instance only, and 0x74E914 not mapping port 0 (whatever the title left there).
        g.mem.remove(&PageRef::slot(4).0);
        g.mem.insert(PLAYER_PORT, u32::MAX);
        assert_eq!(target(&g, 0, NativeCommand::Accept), Some(PageRef::slot(3)));
        assert_eq!(target(&g, 1, NativeCommand::Accept), None);
    }
}

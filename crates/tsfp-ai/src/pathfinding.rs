// SPDX-License-Identifier: GPL-3.0-or-later
//! T1904 readable route boundaries; each original function is verification Tier 0.
use crate::memory::GuestMemory;
/// 26d690. Route entries are 12 bytes; control types 12/13 redirect the next step.
/// Actor+7c points to extension; extension+aa0 is current step. Address arithmetic wraps.
///
/// The caller supplies the actor and route records through `GuestMemory`. A normal
/// entry advances by one 12-byte slot; control entries may redirect that result.
pub fn next_route_step<M: GuestMemory>(m: &M, actor: u32, route: u32) -> u32 {
    let ext = m.read32(actor.wrapping_add(0x7c));
    let index = m.read32(ext.wrapping_add(0xaa0));
    let next = route.wrapping_add(index.wrapping_add(1).wrapping_mul(12));
    match m.read32(next) {
        12 => route.wrapping_add(12),
        13 => route.wrapping_add(
            index
                .wrapping_sub(
                    m.read32(
                        route
                            .wrapping_add(index.wrapping_mul(12))
                            .wrapping_add(0x10),
                    ),
                )
                .wrapping_add(1)
                .wrapping_mul(12),
        ),
        _ => next,
    }
}
/// Typed projection of actor-extension fields used by 26d8b0.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RouteActor {
    pub state: u32,          // +20
    pub requested_node: i32, // +ac8
    pub target_node: i32,    // +684
    pub counter_9ac: u32,
    pub counter_9a8: u32,
    pub progress_b30: f32,
    pub mode_9d0: u32,
}
/// Cross-system services remain explicit; the caller owns character and node records.
pub trait NavigationHost {
    /// 267ff0, arguments (0, actor, requested node).
    fn enter_route(&mut self, actor: u32, requested_node: i32);
    /// 2a1c10, custom EBX state / EDI actor predicate.
    fn check_state(&mut self, actor: u32, state: u32) -> bool;
}
/// 26d8b0. Store the requested node even when the current-player fallback is used.
///
/// # Example
///
/// ```
/// use tsfp_ai::pathfinding::{enter_state_six, NavigationHost, RouteActor};
/// struct Host;
/// impl NavigationHost for Host {
///     fn enter_route(&mut self, _: u32, _: i32) {}
///     fn check_state(&mut self, _: u32, _: u32) -> bool { false }
/// }
/// let mut actor = RouteActor { state: 1, ..Default::default() };
/// enter_state_six(7, &mut actor, -1, 42, &mut Host);
/// assert_eq!((actor.requested_node, actor.target_node, actor.mode_9d0), (-1, 42, 6));
/// ```
pub fn enter_state_six<H: NavigationHost>(
    actor_id: u32,
    actor: &mut RouteActor,
    requested_node: i32,
    player_node: i32,
    host: &mut H,
) {
    if actor.state != 1 {
        return;
    }
    actor.requested_node = requested_node;
    actor.target_node = if requested_node < 0 {
        player_node
    } else {
        requested_node
    };
    actor.counter_9ac = 0;
    actor.counter_9a8 = 0;
    actor.progress_b30 = 0.0;
    actor.mode_9d0 = 6;
    host.enter_route(actor_id, requested_node);
}
/// 2a5c00. Only message 4 invokes the predicate; original result is 0 or -1.
pub fn message_check<H: NavigationHost>(actor: u32, message: u32, state: u32, host: &mut H) -> i32 {
    if message == 4 && host.check_state(actor, state) {
        -1
    } else {
        0
    }
}
/// RNG and x87/CRT conversion services for 2a1880. Host supplies original constants
/// and rounding rather than assuming a fabricated random distribution.
pub trait WeightedHost {
    fn random_word(&mut self) -> u32;
    fn unsigned_correction(&self) -> f64;
    fn random_scale(&self) -> f64;
    fn bias(&self) -> f64;
    fn round_precision(&self, value: f64) -> f64;
    fn ftol_adjust(&mut self, value: f64) -> i32;
}
/// 2a1880. End is the original EBX exclusive bound. Returns absolute index or 0.
/// Nonnegative slice indices express the valid caller domain; sums and comparisons
/// retain original wrapping/signed behavior, including zero/negative weights.
pub fn weighted_index<H: WeightedHost>(
    weights: &[i32],
    start: usize,
    end: usize,
    host: &mut H,
) -> usize {
    assert!(start <= weights.len() && end <= weights.len());
    let sum = if start < end {
        weights[start..end]
            .iter()
            .fold(0i32, |s, w| s.wrapping_add(*w))
    } else {
        0
    };
    let word = host.random_word();
    let mut r = (word as i32) as f64;
    if (word as i32) < 0 {
        r = host.round_precision(r + host.unsigned_correction());
    }
    r = host.round_precision(r * host.random_scale());
    r = host.round_precision(r * sum.wrapping_sub(1) as f64);
    r = host.round_precision(r + host.bias());
    let threshold = host.ftol_adjust(r).wrapping_add(1);
    if start >= end {
        return 0;
    }
    let mut cumulative = 0i32;
    for (i, weight) in weights.iter().enumerate().take(end).skip(start) {
        cumulative = cumulative.wrapping_add(*weight);
        if threshold <= cumulative {
            return i;
        }
    }
    0
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Memory;
    #[test]
    fn route_control_steps_and_wrapping() {
        let mut m = Memory(Default::default());
        m.write32(0x17c, 0x200);
        m.write32(0xca0, 2);
        let next = 0x1000 + 36;
        m.write32(next, 7);
        assert_eq!(next_route_step(&m, 0x100, 0x1000), next);
        m.write32(next, 12);
        assert_eq!(next_route_step(&m, 0x100, 0x1000), 0x100c);
        m.write32(next, 13);
        m.write32(0x1000 + 24 + 16, 2);
        assert_eq!(next_route_step(&m, 0x100, 0x1000), 0x100c);
        m.write32(0xca0, u32::MAX);
        m.write32(0x1000, 0);
        assert_eq!(next_route_step(&m, 0x100, 0x1000), 0x1000);
    }
    #[derive(Default)]
    struct Host {
        calls: Vec<(u32, i32)>,
    }
    impl NavigationHost for Host {
        fn enter_route(&mut self, a: u32, n: i32) {
            self.calls.push((a, n));
        }
        fn check_state(&mut self, a: u32, s: u32) -> bool {
            self.calls.push((a, s as i32));
            s == 7
        }
    }
    #[test]
    fn transitions_and_messages() {
        let mut a = RouteActor::default();
        let mut h = Host::default();
        enter_state_six(5, &mut a, -1, 42, &mut h);
        assert!(h.calls.is_empty());
        a.state = 1;
        enter_state_six(5, &mut a, -1, 42, &mut h);
        assert_eq!(a.target_node, 42);
        assert_eq!(a.requested_node, -1);
        assert_eq!(a.mode_9d0, 6);
        assert_eq!(h.calls, [(5, -1)]);
        assert_eq!(message_check(5, 3, 7, &mut h), 0);
        assert_eq!(h.calls.len(), 1);
        assert_eq!(message_check(5, 4, 7, &mut h), -1);
    }
    struct Random {
        threshold: i32,
        calls: usize,
    }
    impl WeightedHost for Random {
        fn random_word(&mut self) -> u32 {
            self.calls += 1;
            u32::MAX
        }
        fn unsigned_correction(&self) -> f64 {
            4294967296.0
        }
        fn random_scale(&self) -> f64 {
            1.0 / 4294967296.0
        }
        fn bias(&self) -> f64 {
            0.5
        }
        fn round_precision(&self, v: f64) -> f64 {
            v
        }
        fn ftol_adjust(&mut self, _: f64) -> i32 {
            self.threshold - 1
        }
    }
    #[test]
    fn weighted_absolute_indices_and_empty_rng() {
        let mut h = Random {
            threshold: 1,
            calls: 0,
        };
        for (threshold, expected) in [(1, 1), (2, 1), (3, 3), (5, 3), (6, 0)] {
            h.threshold = threshold;
            assert_eq!(weighted_index(&[99, 2, 0, 3], 1, 4, &mut h), expected);
        }
        assert_eq!(weighted_index(&[], 0, 0, &mut h), 0);
        assert_eq!(h.calls, 6);
    }
    #[test]
    fn route_step_grid_preserves_slot_and_redirect_rules() {
        for index in [0u32, 1, 2, 7, 31, u32::MAX] {
            let mut m = Memory(Default::default());
            let route = 0x1000u32;
            m.write32(0x17c, 0x200);
            m.write32(0xca0, index);
            let next = route.wrapping_add(index.wrapping_add(1).wrapping_mul(12));
            m.write32(next, 3);
            assert_eq!(next_route_step(&m, 0x100, route), next);
            m.write32(next, 12);
            assert_eq!(next_route_step(&m, 0x100, route), route + 12);
        }
    }
    #[test]
    fn state_entry_grid_preserves_gate_and_requested_node() {
        for state in [0, 1, 2, u32::MAX] {
            for requested in [-100, -1, 0, 1, 100] {
                let mut actor = RouteActor {
                    state,
                    counter_9ac: 9,
                    counter_9a8: 8,
                    progress_b30: 7.0,
                    ..Default::default()
                };
                let before = actor.clone();
                let mut host = Host::default();
                enter_state_six(5, &mut actor, requested, 42, &mut host);
                if state == 1 {
                    assert_eq!(actor.requested_node, requested);
                    assert_eq!(
                        actor.target_node,
                        if requested < 0 { 42 } else { requested }
                    );
                    assert_eq!(
                        (
                            actor.counter_9ac,
                            actor.counter_9a8,
                            actor.progress_b30,
                            actor.mode_9d0
                        ),
                        (0, 0, 0.0, 6)
                    );
                    assert_eq!(host.calls, [(5, requested)]);
                } else {
                    assert_eq!(actor, before);
                    assert!(host.calls.is_empty());
                }
            }
        }
    }
}

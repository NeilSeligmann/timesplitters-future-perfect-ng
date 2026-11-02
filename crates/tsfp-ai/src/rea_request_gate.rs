//! Raw-state model for the request ID gate at 0x45190.

use crate::rea_request_pool::RawRequestPool;

pub const REQUEST_ID_CAPACITY: usize = 100;
const IMMEDIATE_ACCEPT_FLAGS: u32 = 0x510;
const REQUEST_TOKEN_WRAP: u32 = 0x03ff_ffff;

/// Raw serial state read and advanced by 0x45710.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawRequestModeState {
    pub token: u32,
}

impl Default for RawRequestModeState {
    fn default() -> Self {
        Self { token: 1 }
    }
}

impl RawRequestModeState {
    /// Model 0x45710 from the table dword at +0x08 and request dword +0x04.
    ///
    /// The current token is returned in the low bits; the global advances and
    /// wraps to 1 after 0x03FFFFFF. Flag meanings remain unknown.
    pub fn mode_flags_45710(&mut self, table_dword: u32, request_dword_4: u32) -> u32 {
        let mut flags = self.token;
        let incremented = self.token.wrapping_add(1);
        self.token = if incremented <= REQUEST_TOKEN_WRAP {
            incremented
        } else {
            1
        };

        if table_dword & 0x0000_1000 != 0 {
            flags |= 0x0200_0000;
        }
        if table_dword & 0x0000_0040 != 0 {
            flags |= 0x1000_0000;
        }
        if table_dword & 0x0000_0400 != 0 {
            flags |= 0x0400_0000;
        }
        if table_dword & 0x0000_0010 != 0 {
            flags |= 0x4000_0000;
        }
        if request_dword_4 != 0 {
            flags |= 0x2000_0000;
        }
        flags
    }
}

/// The storage and mode comparison used by 0x45190, with unknown semantics retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawRequestIdGate {
    pub previous_mode: u32,
    pub ids: [u32; REQUEST_ID_CAPACITY],
    pub len: usize,
}

impl Default for RawRequestIdGate {
    fn default() -> Self {
        Self {
            previous_mode: u32::MAX,
            ids: [0; REQUEST_ID_CAPACITY],
            len: 0,
        }
    }
}

impl RawRequestIdGate {
    /// Model 0x45190 after the caller has validated the request ID and loaded
    /// table dword `[0x4CA9B4 + id * 0x14 + 8]`.
    ///
    /// Table flags intersecting `0x510` return true without touching list state.
    /// Otherwise, a changed mode resets the list to this ID; an unchanged mode
    /// rejects duplicates and full lists, appending a new ID when capacity remains.
    pub fn admit_45190(&mut self, request_id: u32, table_flags: u32, current_mode: u32) -> bool {
        if table_flags & IMMEDIATE_ACCEPT_FLAGS != 0 {
            return true;
        }

        if current_mode != self.previous_mode {
            self.previous_mode = current_mode;
            self.ids[0] = request_id;
            self.len = 1;
            return true;
        }

        if self.ids[..self.len].contains(&request_id) || self.len >= REQUEST_ID_CAPACITY {
            return false;
        }

        self.ids[self.len] = request_id;
        self.len += 1;
        true
    }
}

/// Model 0x45820's validated request routing around the raw gate and 25-slot pool.
///
/// Results from 0x27F80 and 0x45710 are explicit inputs; 0x28240 is an explicit
/// callback. The function preserves the original raw EAX return values.
pub fn route_45820(
    request: &mut [u32; 12],
    request_id: i32,
    id_resolves: bool,
    mode_flags: u32,
    table_flags: u32,
    current_mode: u32,
    gate: &mut RawRequestIdGate,
    pool: &mut RawRequestPool,
    mut process_28240: impl FnMut(&[u32; 12], u32) -> u32,
) -> i32 {
    if !(0..=0x1648).contains(&request_id) || !id_resolves {
        return -1;
    }

    if mode_flags & 0x4000_0000 != 0 {
        request[4] |= 0x20;
        request[2] = 0;
    }

    if mode_flags & 0x2000_0000 != 0 {
        return pool
            .insert_45410(request, mode_flags)
            .map(|result| result as i32)
            .unwrap_or(-2);
    }

    if !gate.admit_45190(request_id as u32, table_flags, current_mode) {
        return -3;
    }

    if process_28240(request, mode_flags) != 0 {
        -1
    } else {
        mode_flags as i32
    }
}

#[cfg(test)]
mod tests {
    use super::{
        REQUEST_ID_CAPACITY, REQUEST_TOKEN_WRAP, RawRequestIdGate, RawRequestModeState, route_45820,
    };
    use crate::rea_request_pool::RawRequestPool;

    #[test]
    fn matching_flags_accept_without_mutating_tracking_state() {
        let mut gate = RawRequestIdGate::default();
        let before = gate.clone();
        assert!(gate.admit_45190(42, 0x10, 0));
        assert_eq!(gate, before);
        assert!(gate.admit_45190(42, 0x500, 0));
        assert_eq!(gate, before);
    }

    #[test]
    fn mode_change_resets_the_list_and_same_mode_rejects_duplicate() {
        let mut gate = RawRequestIdGate::default();
        assert!(gate.admit_45190(7, 0, 3));
        assert_eq!(gate.previous_mode, 3);
        assert_eq!(&gate.ids[..gate.len], &[7]);
        assert!(!gate.admit_45190(7, 0, 3));
        assert!(gate.admit_45190(8, 0, 3));
        assert!(gate.admit_45190(7, 0, 4));
        assert_eq!(&gate.ids[..gate.len], &[7]);
        assert_eq!(gate.len, 1);
    }

    #[test]
    fn unchanged_mode_appends_until_exact_capacity_then_rejects() {
        let mut gate = RawRequestIdGate::default();
        assert!(gate.admit_45190(0, 0, 1));
        for id in 1..REQUEST_ID_CAPACITY as u32 {
            assert!(gate.admit_45190(id, 0, 1));
        }
        assert_eq!(gate.len, REQUEST_ID_CAPACITY);
        assert!(!gate.admit_45190(1000, 0, 1));
        assert!(!gate.admit_45190(0, 0, 1));
    }

    #[test]
    fn mode_flags_45710_maps_raw_table_bits_and_request_word() {
        let mut mode = RawRequestModeState::default();
        assert_eq!(mode.mode_flags_45710(0x1450, 1), 0x7600_0001);
        assert_eq!(mode.token, 2);
        assert_eq!(mode.mode_flags_45710(0, 0), 2);
        assert_eq!(mode.token, 3);
    }

    #[test]
    fn mode_flags_45710_wraps_global_token_to_one() {
        let mut mode = RawRequestModeState {
            token: REQUEST_TOKEN_WRAP,
        };
        assert_eq!(mode.mode_flags_45710(0, 0), REQUEST_TOKEN_WRAP);
        assert_eq!(mode.token, 1);
        assert_eq!(mode.mode_flags_45710(0, 0), 1);
        assert_eq!(mode.token, 2);
    }

    #[test]
    fn route_rejects_invalid_or_unresolved_ids_before_mutating_request() {
        let mut request = [7; 12];
        let original = request;
        let mut gate = RawRequestIdGate::default();
        let mut pool = RawRequestPool::default();
        assert_eq!(
            route_45820(
                &mut request,
                -1,
                true,
                0x4000_0000,
                0,
                0,
                &mut gate,
                &mut pool,
                |_, _| panic!("invalid request must not reach 0x28240"),
            ),
            -1
        );
        assert_eq!(request, original);
        assert_eq!(
            route_45820(
                &mut request,
                7,
                false,
                0,
                0,
                0,
                &mut gate,
                &mut pool,
                |_, _| panic!("unresolved request must not reach 0x28240"),
            ),
            -1
        );
    }

    #[test]
    fn route_applies_40000000_writes_then_returns_mode_flags_on_zero_status() {
        let mut request = [0; 12];
        request[4] = 0x8000;
        let mut gate = RawRequestIdGate::default();
        let mut pool = RawRequestPool::default();
        let result = route_45820(
            &mut request,
            7,
            true,
            0x4000_0000,
            0x10,
            1,
            &mut gate,
            &mut pool,
            |seen, flags| {
                assert_eq!(seen[4], 0x8020);
                assert_eq!(seen[2], 0);
                assert_eq!(flags, 0x4000_0000);
                0
            },
        );
        assert_eq!(result, 0x4000_0000u32 as i32);
    }

    #[test]
    fn route_20000000_uses_pool_and_preserves_full_error() {
        let mut request = [0; 12];
        request[1] = 5;
        let mut gate = RawRequestIdGate::default();
        let mut pool = RawRequestPool::default();
        assert_eq!(
            route_45820(
                &mut request,
                9,
                true,
                0x2000_0000,
                0,
                1,
                &mut gate,
                &mut pool,
                |_, _| panic!("pool route must not reach 0x28240"),
            ),
            0x2000_0000
        );
        assert_eq!(pool.slots[1].dwords[12], 0x2000_0000);
        assert_eq!(pool.slots[1].dwords[15], 2);

        for slot in &mut pool.slots {
            slot.dwords[15] = 4;
        }
        assert_eq!(
            route_45820(
                &mut request,
                9,
                true,
                0x2000_0000,
                0,
                1,
                &mut gate,
                &mut pool,
                |_, _| panic!("full pool must not reach 0x28240"),
            ),
            -2
        );
    }

    #[test]
    fn route_maps_gate_reject_and_28240_status_to_original_returns() {
        let mut request = [0; 12];
        let mut gate = RawRequestIdGate::default();
        let mut pool = RawRequestPool::default();
        assert!(gate.admit_45190(4, 0, 1));
        assert_eq!(
            route_45820(
                &mut request,
                4,
                true,
                0,
                0,
                1,
                &mut gate,
                &mut pool,
                |_, _| panic!("duplicate gate rejection must not reach 0x28240"),
            ),
            -3
        );
        assert_eq!(
            route_45820(
                &mut request,
                5,
                true,
                0x20,
                0x10,
                1,
                &mut gate,
                &mut pool,
                |_, _| 1,
            ),
            -1
        );
        assert_eq!(
            route_45820(
                &mut request,
                6,
                true,
                0x20,
                0x10,
                1,
                &mut gate,
                &mut pool,
                |_, _| 0,
            ),
            0x20
        );
    }
}

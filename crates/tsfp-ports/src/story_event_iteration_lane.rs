// SPDX-License-Identifier: GPL-3.0-or-later
//! T2491: typed cursor for the Story event active-list state pass (`0x237E70`).
//! The active list and count are captured at pass entry; slot contents remain live.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolIdentity {
    pub pool_base: u32,
    pub active_list_base: u32,
    pub entry_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventPassCursor {
    pub pool_identity: PoolIdentity,
    pub next_position: u32,
}

impl EventPassCursor {
    pub const fn at_pass_entry(pool_identity: PoolIdentity) -> Self {
        Self {
            pool_identity,
            next_position: 0,
        }
    }

    /// Returns the selected position and advances before its callback can run.
    pub fn next_position(&mut self, current: PoolIdentity) -> Option<u32> {
        if current != self.pool_identity || self.next_position >= self.pool_identity.entry_count {
            return None;
        }
        let selected = self.next_position;
        self.next_position = self.next_position.wrapping_add(1);
        Some(selected)
    }
}

#[cfg(not(test))]
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

fn enabled_from(value: Option<&[u8]>) -> bool {
    value == Some(b"rust")
}

/// Default-off A/B switch; the lifted 0x237E70 loop remains the fallback.
#[cfg(not(test))]
pub fn enabled() -> bool {
    unsafe {
        let value = getenv(c"TSFP_STORY_EVENT_ITERATION".as_ptr());
        !value.is_null() && enabled_from(Some(core::ffi::CStr::from_ptr(value).to_bytes()))
    }
}
#[cfg(test)]
pub fn enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_is_stable_and_advances_before_observation() {
        let identity = PoolIdentity {
            pool_base: 0x1000,
            active_list_base: 0x2000,
            entry_count: 3,
        };
        let mut cursor = EventPassCursor::at_pass_entry(identity);
        assert_eq!(cursor.next_position(identity), Some(0));
        assert_eq!(cursor.next_position(identity), Some(1));
        assert_eq!(cursor.next_position(identity), Some(2));
        assert_eq!(cursor.next_position(identity), None);
        let changed = PoolIdentity {
            active_list_base: 0x3000,
            ..identity
        };
        assert_eq!(cursor.next_position(changed), None);
    }

    #[test]
    fn switch_is_opt_in() {
        assert!(!enabled_from(None));
        assert!(!enabled_from(Some(b"1")));
        assert!(enabled_from(Some(b"rust")));
    }
}

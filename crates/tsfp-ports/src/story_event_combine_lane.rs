// SPDX-License-Identifier: GPL-3.0-or-later
//! T2473: typed Story active-event link-combination gate.

/// Low-word definition inputs and per-instance link latches consumed at 0x237F05.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkCombination {
    pub mode: u32,
    pub link_a_word: u16,
    pub link_b_word: u16,
    pub a_latched: bool,
    pub b_latched: bool,
}

pub fn enabled() -> bool {
    #[cfg(not(test))]
    unsafe {
        unsafe extern "C" {
            fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
        }
        let value = getenv(c"TSFP_STORY_EVENT_COMBINE".as_ptr());
        !value.is_null() && core::ffi::CStr::from_ptr(value).to_bytes() == b"rust"
    }
    #[cfg(test)]
    {
        true
    }
}

/// True means the event is gated (the lifted branch reaches 0x237FCA).
pub const fn is_gated(input: LinkCombination) -> bool {
    let a_ready = input.link_a_word == 0 && input.a_latched;
    let b_ready = input.link_b_word == 0 && input.b_latched;
    match input.mode {
        0 => a_ready,
        1 => a_ready && b_ready,
        2 => a_ready || b_ready,
        3 => {
            if a_ready {
                true
            } else {
                input.link_b_word != 0 || !input.b_latched
            }
        }
        // The original jump table tail-jumps for unsupported values; callers
        // keep that dispatch on the lifted path.
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lifted(x: LinkCombination) -> bool {
        let a = x.link_a_word as u32 == 0;
        let b = x.link_b_word as u32 == 0;
        match x.mode {
            0 => a && x.a_latched,
            1 => a && x.a_latched && b && x.b_latched,
            2 => (a && x.a_latched) || (b && x.b_latched),
            3 => {
                (a && x.a_latched)
                    || (!a && (x.link_b_word != 0 || !x.b_latched))
                    || (a && !x.a_latched && (x.link_b_word != 0 || !x.b_latched))
            }
            _ => false,
        }
    }

    #[test]
    fn tier1_weak_fabricated_differential_3000_cases() {
        let mut s = 0x2473_91u32;
        for _ in 0..3000 {
            s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            let x = LinkCombination {
                mode: (s >> 8) % 4,
                link_a_word: (s >> 16) as u16,
                link_b_word: s as u16,
                a_latched: s & 1 != 0,
                b_latched: s & 2 != 0,
            };
            assert_eq!(is_gated(x), lifted(x));
        }
    }

    #[test]
    fn mutating_required_latch_changes_and_gate_result() {
        let x = LinkCombination {
            mode: 0,
            link_a_word: 0,
            link_b_word: 0,
            a_latched: true,
            b_latched: false,
        };
        assert!(is_gated(x));
        assert!(!is_gated(LinkCombination {
            a_latched: false,
            ..x
        }));
    }
}

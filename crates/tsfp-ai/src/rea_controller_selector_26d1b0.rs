//! Readable return routing for the selector at XBE address 0x26D1B0.
//!
//! The selector's nested 0x59320 lookups remain an explicit input. Their table
//! payload and semantic meaning are not part of this measured return router.

/// Apply the observed 0x26D1B0 selector return table.
///
/// Selectors 2, 3, and 16 return 2.0; 4..=7 and 17 return the nested lookup
/// result; 8..=15 and values outside the 2..=17 dispatch range return 0.0.
/// Verification: Tier 0 (focused smoke tests only; no differential or replay).
pub fn route_controller_selector_26d1b0(selector: u32, nested_lookup_result: f32) -> f32 {
    match selector {
        2 | 3 | 16 => 2.0,
        4..=7 | 17 => nested_lookup_result,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::route_controller_selector_26d1b0;

    #[test]
    fn preserves_measured_constant_nested_and_zero_selector_groups() {
        assert_eq!(route_controller_selector_26d1b0(2, 13.0), 2.0);
        assert_eq!(route_controller_selector_26d1b0(3, 13.0), 2.0);
        assert_eq!(route_controller_selector_26d1b0(16, 13.0), 2.0);

        for selector in [4, 5, 6, 7, 17] {
            assert_eq!(route_controller_selector_26d1b0(selector, 13.0), 13.0);
        }

        for selector in [8, 9, 10, 11, 12, 13, 14, 15] {
            assert_eq!(route_controller_selector_26d1b0(selector, 13.0), 0.0);
        }
    }

    #[test]
    fn out_of_range_selectors_take_zero_default() {
        assert_eq!(route_controller_selector_26d1b0(0, 13.0), 0.0);
        assert_eq!(route_controller_selector_26d1b0(1, 13.0), 0.0);
        assert_eq!(route_controller_selector_26d1b0(18, 13.0), 0.0);
        assert_eq!(route_controller_selector_26d1b0(u32::MAX, 13.0), 0.0);
    }
}

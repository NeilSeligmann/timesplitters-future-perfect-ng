//! Typed classification of the measured navigation-node ID table domains.
//!
//! The bounded resolver read at `0x1596F0` routes IDs `[8000, 8350)` to one
//! special table and `[8350, 8370)` to another; other IDs take ordinary graph
//! handling, except raw `-1` which is a distinct sentinel. This describes table
//! selection only. It does not establish table membership, pointer validity, or
//! sentinel resolution behavior. See
//! `docs/t1662-rea-routes.md`, “Special node ID domains”.

use crate::navigation_scoring::NodeId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecialNodeTable {
    /// Root `0x715F34`, indexed from ID 8000.
    First,
    /// Root `0x715F38`, indexed from ID 8350.
    Second,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigationNodeDomain {
    /// Raw ID `-1` (`u32::MAX`) follows a distinct sentinel path.
    SentinelMinusOne,
    /// The resolver follows its ordinary graph-table path for this ID.
    Ordinary,
    /// A special-table ID and its zero-based index.
    Special { table: SpecialNodeTable, index: u16 },
}

/// Classifies the table domain selected by the bounded `0x1596F0` ID checks.
///
/// Table selection is not a presence check: the original subsequently tests
/// the selected entry and resolves a present record. The `-1` sentinel is
/// classified separately, but its resolution outcome remains unspecified here.
pub fn classify_navigation_node(id: NodeId) -> NavigationNodeDomain {
    match id.0 {
        u32::MAX => NavigationNodeDomain::SentinelMinusOne,
        8000..8350 => NavigationNodeDomain::Special {
            table: SpecialNodeTable::First,
            index: (id.0 - 8000) as u16,
        },
        8350..8370 => NavigationNodeDomain::Special {
            table: SpecialNodeTable::Second,
            index: (id.0 - 8350) as u16,
        },
        _ => NavigationNodeDomain::Ordinary,
    }
}

#[cfg(test)]
mod tests {
    use super::{NavigationNodeDomain, SpecialNodeTable, classify_navigation_node};
    use crate::navigation_scoring::NodeId;

    #[test]
    fn selects_first_special_table_at_measured_boundaries() {
        assert_eq!(
            classify_navigation_node(NodeId(8000)),
            NavigationNodeDomain::Special {
                table: SpecialNodeTable::First,
                index: 0,
            }
        );
        assert_eq!(
            classify_navigation_node(NodeId(8349)),
            NavigationNodeDomain::Special {
                table: SpecialNodeTable::First,
                index: 349,
            }
        );
    }

    #[test]
    fn selects_second_special_table_at_measured_boundaries() {
        assert_eq!(
            classify_navigation_node(NodeId(8350)),
            NavigationNodeDomain::Special {
                table: SpecialNodeTable::Second,
                index: 0,
            }
        );
        assert_eq!(
            classify_navigation_node(NodeId(8369)),
            NavigationNodeDomain::Special {
                table: SpecialNodeTable::Second,
                index: 19,
            }
        );
    }

    #[test]
    fn ordinary_domain_is_selected_below_and_after_special_ranges() {
        assert_eq!(
            classify_navigation_node(NodeId(7999)),
            NavigationNodeDomain::Ordinary
        );
        assert_eq!(
            classify_navigation_node(NodeId(8370)),
            NavigationNodeDomain::Ordinary
        );
    }

    #[test]
    fn raw_minus_one_uses_a_distinct_sentinel_domain() {
        assert_eq!(
            classify_navigation_node(NodeId(u32::MAX)),
            NavigationNodeDomain::SentinelMinusOne
        );
    }
}

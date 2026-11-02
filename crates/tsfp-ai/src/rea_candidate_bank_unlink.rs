//! Typed link-update plan observed in the 0x22C candidate-bank cleanup path.
//!
//! This models the row-neighbor and bank-endpoint writes only. The surrounding
//! routine also releases opaque references and runs callbacks; those effects
//! remain behind a caller-owned boundary.

/// Raw signed neighbor indices stored at bank-row offsets +0x10 and +0x14.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateBankRowLinks {
    pub link_10: i32,
    pub link_14: i32,
}

/// Exact writes required to unlink one row from the observed bank chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateBankUnlinkPlan {
    /// Write `link_14` to neighbor `link_10` at that row's +0x14.
    pub neighbor_10_link_14: Option<(usize, i32)>,
    /// When `link_10` is negative, update global `[0x7388FC]` instead.
    pub endpoint_7388fc: Option<i32>,
    /// Write `link_10` to neighbor `link_14` at that row's +0x10.
    pub neighbor_14_link_10: Option<(usize, i32)>,
    /// When `link_14` is negative, update global `[0x7368AC]` instead.
    pub endpoint_7368ac: Option<i32>,
    /// The removed row's two links are both reset to the measured -1 marker.
    pub cleared_row_links: CandidateBankRowLinks,
}

/// The two type-indexed endpoint values touched by the original unlink path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateBankEndpoints {
    pub endpoint_7368ac: i32,
    pub endpoint_7388fc: i32,
}

/// Invalid row reference found before applying any unlink writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateBankUnlinkError {
    RemovedRowOutOfBounds { row: usize },
    NeighborOutOfBounds { row: usize, neighbor: i32 },
}

/// Plan the observed unlink writes for one row in the 0x22C bank.
///
/// `link_10`/`link_14` deliberately retain their raw names: static code proves
/// reciprocal neighbor writes and endpoint selection, but not which endpoint
/// is semantically head or tail. Verification: Tier 0 unit test only.
pub fn plan_candidate_bank_unlink(links: CandidateBankRowLinks) -> CandidateBankUnlinkPlan {
    let neighbor_10_link_14 = if links.link_10 >= 0 {
        Some((links.link_10 as usize, links.link_14))
    } else {
        None
    };
    let endpoint_7388fc = if links.link_10 < 0 {
        Some(links.link_14)
    } else {
        None
    };
    let neighbor_14_link_10 = if links.link_14 >= 0 {
        Some((links.link_14 as usize, links.link_10))
    } else {
        None
    };
    let endpoint_7368ac = if links.link_14 < 0 {
        Some(links.link_10)
    } else {
        None
    };

    CandidateBankUnlinkPlan {
        neighbor_10_link_14,
        endpoint_7388fc,
        neighbor_14_link_10,
        endpoint_7368ac,
        cleared_row_links: CandidateBankRowLinks {
            link_10: -1,
            link_14: -1,
        },
    }
}

/// Apply the observed unlink writes to Rust-owned rows and one type's endpoints.
///
/// Every nonnegative row index is checked before mutation so malformed state
/// cannot leave a partially updated chain. Callback and resource cleanup are
/// outside this function and must be supplied by a separate typed boundary.
pub fn apply_candidate_bank_unlink(
    rows: &mut [CandidateBankRowLinks],
    row_index: usize,
    endpoints: &mut CandidateBankEndpoints,
) -> Result<CandidateBankUnlinkPlan, CandidateBankUnlinkError> {
    let links = rows
        .get(row_index)
        .copied()
        .ok_or(CandidateBankUnlinkError::RemovedRowOutOfBounds { row: row_index })?;
    let plan = plan_candidate_bank_unlink(links);
    for neighbor in [plan.neighbor_10_link_14, plan.neighbor_14_link_10]
        .into_iter()
        .flatten()
    {
        if neighbor.0 >= rows.len() {
            return Err(CandidateBankUnlinkError::NeighborOutOfBounds {
                row: row_index,
                neighbor: neighbor.0 as i32,
            });
        }
    }

    if let Some((neighbor, value)) = plan.neighbor_10_link_14 {
        rows[neighbor].link_14 = value;
    } else if let Some(value) = plan.endpoint_7388fc {
        endpoints.endpoint_7388fc = value;
    }
    if let Some((neighbor, value)) = plan.neighbor_14_link_10 {
        rows[neighbor].link_10 = value;
    } else if let Some(value) = plan.endpoint_7368ac {
        endpoints.endpoint_7368ac = value;
    }
    rows[row_index] = plan.cleared_row_links;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::{
        CandidateBankEndpoints, CandidateBankRowLinks, CandidateBankUnlinkError,
        apply_candidate_bank_unlink, plan_candidate_bank_unlink,
    };

    #[test]
    fn unlink_updates_reciprocal_links_or_the_corresponding_endpoint() {
        let middle = plan_candidate_bank_unlink(CandidateBankRowLinks {
            link_10: 3,
            link_14: 8,
        });
        assert_eq!(middle.neighbor_10_link_14, Some((3, 8)));
        assert_eq!(middle.endpoint_7388fc, None);
        assert_eq!(middle.neighbor_14_link_10, Some((8, 3)));
        assert_eq!(middle.endpoint_7368ac, None);
        assert_eq!(
            middle.cleared_row_links,
            CandidateBankRowLinks {
                link_10: -1,
                link_14: -1
            }
        );

        let first = plan_candidate_bank_unlink(CandidateBankRowLinks {
            link_10: -1,
            link_14: 8,
        });
        assert_eq!(first.endpoint_7388fc, Some(8));
        assert_eq!(first.neighbor_14_link_10, Some((8, -1)));
        assert_eq!(first.endpoint_7368ac, None);

        let last = plan_candidate_bank_unlink(CandidateBankRowLinks {
            link_10: 3,
            link_14: -1,
        });
        assert_eq!(last.neighbor_10_link_14, Some((3, -1)));
        assert_eq!(last.endpoint_7368ac, Some(3));
        assert_eq!(last.endpoint_7388fc, None);

        let only = plan_candidate_bank_unlink(CandidateBankRowLinks {
            link_10: -1,
            link_14: -1,
        });
        assert_eq!(only.endpoint_7388fc, Some(-1));
        assert_eq!(only.endpoint_7368ac, Some(-1));
        assert_eq!(only.neighbor_10_link_14, None);
        assert_eq!(only.neighbor_14_link_10, None);
    }

    #[test]
    fn apply_unlink_mutates_typed_rows_and_endpoints() {
        let mut rows = [
            CandidateBankRowLinks {
                link_10: -1,
                link_14: 1,
            },
            CandidateBankRowLinks {
                link_10: 0,
                link_14: 2,
            },
            CandidateBankRowLinks {
                link_10: 1,
                link_14: -1,
            },
        ];
        let mut endpoints = CandidateBankEndpoints {
            endpoint_7368ac: 0,
            endpoint_7388fc: 2,
        };

        apply_candidate_bank_unlink(&mut rows, 1, &mut endpoints).unwrap();
        assert_eq!(rows[0].link_14, 2);
        assert_eq!(rows[2].link_10, 0);
        assert_eq!(
            rows[1],
            CandidateBankRowLinks {
                link_10: -1,
                link_14: -1
            }
        );
        assert_eq!(
            endpoints,
            CandidateBankEndpoints {
                endpoint_7368ac: 0,
                endpoint_7388fc: 2
            }
        );

        apply_candidate_bank_unlink(&mut rows, 0, &mut endpoints).unwrap();
        assert_eq!(endpoints.endpoint_7388fc, 2);
        assert_eq!(rows[2].link_10, -1);
        apply_candidate_bank_unlink(&mut rows, 2, &mut endpoints).unwrap();
        assert_eq!(endpoints.endpoint_7368ac, -1);
        assert_eq!(endpoints.endpoint_7388fc, -1);
    }

    #[test]
    fn invalid_neighbor_keeps_the_bank_unchanged() {
        let mut rows = [CandidateBankRowLinks {
            link_10: -1,
            link_14: 9,
        }];
        let original_rows = rows;
        let mut endpoints = CandidateBankEndpoints {
            endpoint_7368ac: 0,
            endpoint_7388fc: 0,
        };
        let original_endpoints = endpoints;

        assert_eq!(
            apply_candidate_bank_unlink(&mut rows, 0, &mut endpoints),
            Err(CandidateBankUnlinkError::NeighborOutOfBounds {
                row: 0,
                neighbor: 9,
            })
        );
        assert_eq!(rows, original_rows);
        assert_eq!(endpoints, original_endpoints);
    }
}

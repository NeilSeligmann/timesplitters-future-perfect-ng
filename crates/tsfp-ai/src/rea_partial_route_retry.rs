//! The caller-specific single-replan policy at `0x15D500`.
//!
//! This is separate from `0x26EA00` point-follow, where the same `0x1596F0`
//! result `-2` takes an alert/abort path. The typed service seam records the
//! measured call order; it does not identify the runtime caller or actor store.
//! Contract and evidence limits: `docs/t1662-rea-routes.md`, “Partial-route
//! retry budget”.

use crate::navigation_node_domain::{NavigationNodeDomain, classify_navigation_node};
use crate::navigation_scoring::NodeId;

/// A nonnegative search result with its measured table-selection domain.
/// Keeping the original ID alongside the domain preserves the ABI value while
/// making special-table IDs explicit to later Rust-owned navigation code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedNavigationNode {
    pub id: NodeId,
    pub domain: NavigationNodeDomain,
}

impl ResolvedNavigationNode {
    fn from_search_result(result: i32) -> Self {
        let id = NodeId(result as u32);
        Self {
            id,
            domain: classify_navigation_node(id),
        }
    }
}

/// Narrow host services used by the bounded `0x15D500` retry policy.
pub trait PartialRouteRetryWorld {
    /// Calls `0x1596F0` with the caller's route state and trace-mode argument.
    fn search_route(&mut self, trace: bool) -> i32;

    /// Projects the measured `0x160B00(0/1)` switch call.
    fn set_search_switch(&mut self, enabled: bool);

    /// Clears the two reverse-trace counters before the single retry.
    fn clear_reverse_trace_counters(&mut self);

    /// Runs the terminal `0x15D490` stop path after an unresolved retry.
    fn stop_actor(&mut self);
}

/// Caller-visible result after the initial search or its one permitted retry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartialRouteOutcome {
    /// The search returned the measured arrival sentinel `-1`.
    Arrived,
    /// The search selected a nonnegative navigation node ID.
    NextNode(ResolvedNavigationNode),
    /// The one permitted retry remained below `-1`; the stop path ran.
    NoRouteAfterRetry,
    /// An initial result other than the measured retry/arrival/node cases.
    OtherSearchResult(i32),
}

/// Result of one caller invocation, including whether the one retry ran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartialRouteSearch {
    pub outcome: PartialRouteOutcome,
    pub retried: bool,
}

fn classify_search_result(result: i32) -> PartialRouteOutcome {
    match result {
        -1 => PartialRouteOutcome::Arrived,
        0.. => PartialRouteOutcome::NextNode(ResolvedNavigationNode::from_search_result(result)),
        other => PartialRouteOutcome::OtherSearchResult(other),
    }
}

/// Implements the measured `0x15D500` order with at most one replan attempt.
///
/// Only an initial `-2` triggers a retry. Before retrying, the caller disables
/// its search switch and clears its reverse-trace counters; it restores the
/// switch after the retry. A retry result below `-1` invokes the terminal stop
/// path. Other initial results pass through unchanged.
pub fn search_with_one_replan<W: PartialRouteRetryWorld>(world: &mut W) -> PartialRouteSearch {
    let first = world.search_route(true);
    if first != -2 {
        return PartialRouteSearch {
            outcome: classify_search_result(first),
            retried: false,
        };
    }

    world.set_search_switch(false);
    world.clear_reverse_trace_counters();
    let retry = world.search_route(false);
    world.set_search_switch(true);

    let outcome = if retry < -1 {
        world.stop_actor();
        PartialRouteOutcome::NoRouteAfterRetry
    } else {
        classify_search_result(retry)
    };
    PartialRouteSearch {
        outcome,
        retried: true,
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::{
        PartialRouteOutcome, PartialRouteRetryWorld, PartialRouteSearch, ResolvedNavigationNode,
        search_with_one_replan,
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Call {
        Search(bool),
        Switch(bool),
        ClearCounters,
        StopActor,
    }

    struct TestWorld {
        results: [i32; 2],
        next: usize,
        calls: Vec<Call>,
    }

    impl TestWorld {
        fn with_results(first: i32, second: i32) -> Self {
            Self {
                results: [first, second],
                next: 0,
                calls: Vec::new(),
            }
        }
    }

    impl PartialRouteRetryWorld for TestWorld {
        fn search_route(&mut self, trace: bool) -> i32 {
            self.calls.push(Call::Search(trace));
            let result = self.results[self.next];
            self.next += 1;
            result
        }

        fn set_search_switch(&mut self, enabled: bool) {
            self.calls.push(Call::Switch(enabled));
        }

        fn clear_reverse_trace_counters(&mut self) {
            self.calls.push(Call::ClearCounters);
        }

        fn stop_actor(&mut self) {
            self.calls.push(Call::StopActor);
        }
    }

    #[test]
    fn only_initial_minus_two_retries_and_restores_switch_before_return() {
        let mut world = TestWorld::with_results(-2, 17);

        assert_eq!(
            search_with_one_replan(&mut world),
            PartialRouteSearch {
                outcome: PartialRouteOutcome::NextNode(ResolvedNavigationNode {
                    id: crate::navigation_scoring::NodeId(17),
                    domain: crate::navigation_node_domain::NavigationNodeDomain::Ordinary,
                }),
                retried: true,
            }
        );
        assert_eq!(
            world.calls,
            [
                Call::Search(true),
                Call::Switch(false),
                Call::ClearCounters,
                Call::Search(false),
                Call::Switch(true),
            ]
        );
    }

    #[test]
    fn unresolved_retry_stops_actor_after_restoring_switch() {
        let mut world = TestWorld::with_results(-2, -3);

        assert_eq!(
            search_with_one_replan(&mut world),
            PartialRouteSearch {
                outcome: PartialRouteOutcome::NoRouteAfterRetry,
                retried: true,
            }
        );
        assert_eq!(
            world.calls,
            [
                Call::Search(true),
                Call::Switch(false),
                Call::ClearCounters,
                Call::Search(false),
                Call::Switch(true),
                Call::StopActor,
            ]
        );
    }

    #[test]
    fn other_initial_results_pass_through_without_replan() {
        for (result, expected) in [
            (-1, PartialRouteOutcome::Arrived),
            (
                0,
                PartialRouteOutcome::NextNode(ResolvedNavigationNode {
                    id: crate::navigation_scoring::NodeId(0),
                    domain: crate::navigation_node_domain::NavigationNodeDomain::Ordinary,
                }),
            ),
            (
                21,
                PartialRouteOutcome::NextNode(ResolvedNavigationNode {
                    id: crate::navigation_scoring::NodeId(21),
                    domain: crate::navigation_node_domain::NavigationNodeDomain::Ordinary,
                }),
            ),
            (-3, PartialRouteOutcome::OtherSearchResult(-3)),
        ] {
            let mut world = TestWorld::with_results(result, 99);

            assert_eq!(
                search_with_one_replan(&mut world),
                PartialRouteSearch {
                    outcome: expected,
                    retried: false,
                }
            );
            assert_eq!(world.calls, [Call::Search(true)]);
        }
    }

    #[test]
    fn selected_special_table_nodes_keep_the_original_id_and_domain() {
        use crate::navigation_node_domain::{NavigationNodeDomain, SpecialNodeTable};

        for (id, domain) in [
            (
                8000,
                NavigationNodeDomain::Special {
                    table: SpecialNodeTable::First,
                    index: 0,
                },
            ),
            (
                8350,
                NavigationNodeDomain::Special {
                    table: SpecialNodeTable::Second,
                    index: 0,
                },
            ),
        ] {
            let mut world = TestWorld::with_results(id, 99);
            assert_eq!(
                search_with_one_replan(&mut world).outcome,
                PartialRouteOutcome::NextNode(ResolvedNavigationNode {
                    id: crate::navigation_scoring::NodeId(id as u32),
                    domain,
                })
            );
        }
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use std::{cell::RefCell, rc::Rc, vec, vec::Vec};

const ID: OwnerIdentity = OwnerIdentity {
    player: 2,
    generation: 7,
    side: Side::Right,
};

// A synthetic owner proves protocol order and borrow boundaries. This is not an
// ammo differential test: production must use the existing typed ammo reducers.
struct State {
    guard: TransitionGuard,
    hand: HandSnapshot,
    loaded: i32,
    reserve: i32,
    realization_flag: bool,
    events: Vec<&'static str>,
}

#[derive(Clone)]
struct Owner(Rc<RefCell<State>>);

impl OwnerAccess for Owner {
    fn begin(&mut self, target: Record) -> Result<Begin, OwnerError> {
        let mut state = self.0.borrow_mut();
        let hand = state.hand;
        state.guard.begin(hand, target)
    }

    fn commit(
        &mut self,
        ticket: Ticket,
        stage: Stage,
        mutation: Mutation,
    ) -> Result<(), OwnerError> {
        let mut state = self.0.borrow_mut();
        let State {
            guard,
            hand,
            loaded,
            reserve,
            realization_flag,
            events,
        } = &mut *state;
        guard.commit(ticket, stage, mutation, || match mutation {
            Mutation::ClearChild => {
                hand.child = Child(0);
                events.push("clear");
            }
            Mutation::UnloadPrevious => {
                if hand.record != Record::Absent {
                    *reserve += *loaded;
                    *loaded = 0;
                }
                events.push("unload");
            }
            Mutation::PublishRecord => {
                hand.record = ticket.target();
                *realization_flag = false;
                events.push("record");
            }
            Mutation::PublishChild(child) => {
                hand.child = child;
                events.push("child");
            }
            Mutation::Reload => {
                *loaded += *reserve;
                *reserve = 0;
                events.push("reload");
            }
            Mutation::Finish => events.push("finish"),
            Mutation::Fail => events.push("fail"),
            _ => {}
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Point {
    Release,
    Create,
    Initialize,
    Attach,
}

struct Presentation {
    state: Rc<RefCell<State>>,
    child: Child,
    fail: Option<Point>,
    invalidate: Option<(Point, bool)>,
    released: Vec<Child>,
}

impl Presentation {
    fn enter(&mut self, point: Point, name: &'static str) -> Result<(), EffectError> {
        // This borrow panics if the runner retained the owner's RefCell borrow
        // across callbacks. The production analogue is the non-reentrant lock.
        let mut state = self.state.borrow_mut();
        state.events.push(name);
        if let Some((at, new_generation)) = self.invalidate {
            if at == point {
                let identity = OwnerIdentity {
                    generation: ID.generation + u64::from(new_generation),
                    ..ID
                };
                state.guard.invalidate(identity);
                state.hand = HandSnapshot {
                    record: Record::Present(42),
                    child: Child(99),
                };
                let hand = state.hand;
                // Even a replacement transition in the SAME lifetime must make
                // this runner stale, rather than letting it complete over it.
                state.guard.begin(hand, Record::Present(43)).unwrap();
            }
        }
        if self.fail == Some(point) {
            Err(EffectError::Unsupported)
        } else {
            Ok(())
        }
    }
}

impl Effects for Presentation {
    fn release_child(&mut self, ticket: Ticket, child: Child) -> Result<(), EffectError> {
        assert_eq!(self.state.borrow().hand, ticket.before());
        self.released.push(child);
        self.enter(Point::Release, "release")
    }

    fn create_child(&mut self, ticket: Ticket) -> Result<Child, EffectError> {
        {
            let state = self.state.borrow();
            assert_eq!(state.hand.record, ticket.target());
            assert_eq!(state.hand.child, Child(0));
            assert_eq!(state.loaded, 0);
            assert!(!state.realization_flag);
        }
        self.enter(Point::Create, "create")?;
        Ok(self.child)
    }

    fn initialize_child(&mut self, _ticket: Ticket, child: Child) -> Result<(), EffectError> {
        assert_eq!(self.state.borrow().hand.child, child);
        self.enter(Point::Initialize, "initialize")
    }

    fn attach_child(&mut self, _ticket: Ticket, child: Child) -> Result<(), EffectError> {
        assert_eq!(self.state.borrow().hand.child, child);
        self.enter(Point::Attach, "attach")
    }
}

fn fixture(child: Child) -> (Owner, Presentation) {
    let state = Rc::new(RefCell::new(State {
        guard: TransitionGuard::new(ID),
        hand: HandSnapshot {
            record: Record::Present(1),
            child,
        },
        loaded: 3,
        reserve: 7,
        realization_flag: true,
        events: Vec::new(),
    }));
    (
        Owner(state.clone()),
        Presentation {
            state,
            child: Child(123),
            fail: None,
            invalidate: None,
            released: Vec::new(),
        },
    )
}

#[test]
fn ordinary_transition_publishes_at_callback_boundaries_and_transfers_once() {
    let (mut owner, mut effects) = fixture(Child(55));
    let completed = realize(&mut owner, &mut effects, Record::Present(2)).unwrap();
    assert!(matches!(
        completed,
        Completion::Replaced {
            created_child: Some(ChildReceipt {
                child: Child(123),
                published: true
            }),
            ..
        }
    ));
    let state = owner.0.borrow();
    assert_eq!(
        state.events,
        vec![
            "release",
            "clear",
            "unload",
            "record",
            "create",
            "child",
            "initialize",
            "attach",
            "reload",
            "finish"
        ]
    );
    assert_eq!((state.loaded, state.reserve), (10, 0));
    assert_eq!(effects.released, vec![Child(55)]);
}

#[test]
fn absent_and_special_targets_unload_but_never_create_or_reload() {
    for target in [
        Record::Absent,
        Record::Present(0x29),
        Record::Present(0x38),
        Record::Present(0x3d),
    ] {
        let (mut owner, mut effects) = fixture(Child(55));
        let completed = realize(&mut owner, &mut effects, target).unwrap();
        assert!(matches!(
            completed,
            Completion::Replaced {
                created_child: None,
                ..
            }
        ));
        let state = owner.0.borrow();
        assert_eq!(
            state.hand,
            HandSnapshot {
                record: target,
                child: Child(0)
            }
        );
        assert_eq!((state.loaded, state.reserve), (0, 10));
        assert_eq!(
            state.events,
            vec!["release", "clear", "unload", "record", "finish"]
        );
    }
}

#[test]
fn identical_owned_record_is_unchanged_even_with_a_null_child() {
    let (mut owner, mut effects) = fixture(Child(0));
    let before = owner.0.borrow().hand;
    assert_eq!(
        realize(&mut owner, &mut effects, Record::Present(1)),
        Ok(Completion::Unchanged(before))
    );
    assert!(owner.0.borrow().events.is_empty());
}

#[test]
fn raw_zero_skips_release_while_max_is_passed_without_normalization() {
    for raw in [0, u32::MAX] {
        let (mut owner, mut effects) = fixture(Child(raw));
        realize(&mut owner, &mut effects, Record::Absent).unwrap();
        assert_eq!(
            effects.released,
            if raw == 0 { vec![] } else { vec![Child(raw)] }
        );
    }
}

#[test]
fn unsupported_child_initialization_preserves_exact_published_raw_receipt() {
    for raw in [0, u32::MAX] {
        let (mut owner, mut effects) = fixture(Child(55));
        effects.child = Child(raw);
        effects.fail = Some(Point::Initialize);
        let failure = realize(&mut owner, &mut effects, Record::Present(2)).unwrap_err();
        assert_eq!(failure.stage, Stage::Initializing);
        assert_eq!(
            failure.created_child,
            Some(ChildReceipt {
                child: Child(raw),
                published: true
            })
        );
        assert_eq!(failure.cause, Cause::Effect(EffectError::Unsupported));
        let state = owner.0.borrow();
        assert_eq!(
            state.hand,
            HandSnapshot {
                record: Record::Present(2),
                child: Child(raw)
            }
        );
        assert_eq!((state.loaded, state.reserve), (0, 10));
        assert!(!state.events.contains(&"reload"));
        assert!(!state.events.contains(&"attach"));
    }
}

#[test]
fn stale_creation_returns_unpublished_receipt_for_explicit_resource_disposition() {
    for new_generation in [false, true] {
        let (mut owner, mut effects) = fixture(Child(55));
        effects.invalidate = Some((Point::Create, new_generation));
        let failure = realize(&mut owner, &mut effects, Record::Present(2)).unwrap_err();
        assert_eq!(failure.cause, Cause::Owner(OwnerError::Stale));
        assert_eq!(
            failure.created_child,
            Some(ChildReceipt {
                child: Child(123),
                published: false
            })
        );
        assert_eq!(failure.failure_commit_error, Some(OwnerError::Stale));
        let state = owner.0.borrow();
        assert_eq!(
            state.hand,
            HandSnapshot {
                record: Record::Present(42),
                child: Child(99)
            }
        );
        assert_eq!(
            state.events,
            vec!["release", "clear", "unload", "record", "create"]
        );
    }
}

#[test]
fn stale_after_attachment_returns_published_receipt_and_never_reloads_new_owner() {
    let (mut owner, mut effects) = fixture(Child(55));
    effects.invalidate = Some((Point::Attach, true));
    let failure = realize(&mut owner, &mut effects, Record::Present(2)).unwrap_err();
    assert_eq!(
        failure.created_child,
        Some(ChildReceipt {
            child: Child(123),
            published: true
        })
    );
    assert_eq!(failure.cause, Cause::Owner(OwnerError::Stale));
    assert!(!owner.0.borrow().events.contains(&"reload"));
    assert_eq!(owner.0.borrow().hand.child, Child(99));
}

#[test]
fn failed_creation_keeps_record_publication_and_rejects_implicit_retry() {
    let (mut owner, mut effects) = fixture(Child(55));
    effects.fail = Some(Point::Create);
    let failure = realize(&mut owner, &mut effects, Record::Present(2)).unwrap_err();
    assert_eq!(failure.created_child, None);
    assert_eq!(owner.0.borrow().hand.record, Record::Present(2));
    assert_eq!(owner.0.borrow().hand.child, Child(0));
    let again = realize(&mut owner, &mut effects, Record::Present(2)).unwrap_err();
    assert_eq!(again.cause, Cause::Owner(OwnerError::Busy));
    assert_eq!(
        owner
            .0
            .borrow()
            .events
            .iter()
            .filter(|&&e| e == "create")
            .count(),
        1
    );
}

#[test]
fn release_failure_preserves_original_owner_and_stops_before_unload() {
    let (mut owner, mut effects) = fixture(Child(55));
    effects.fail = Some(Point::Release);
    let before = owner.0.borrow().hand;
    let failure = realize(&mut owner, &mut effects, Record::Present(2)).unwrap_err();
    assert_eq!(failure.stage, Stage::Releasing);
    assert_eq!(owner.0.borrow().hand, before);
    assert_eq!(owner.0.borrow().events, vec!["release", "fail"]);
}

#[test]
fn duplicate_and_out_of_order_commits_never_run_mutations() {
    let mut guard = TransitionGuard::new(ID);
    let before = HandSnapshot {
        record: Record::Present(1),
        child: Child(7),
    };
    let Begin::Active(ticket) = guard.begin(before, Record::Absent).unwrap() else {
        panic!()
    };
    assert_eq!(
        guard.commit(ticket, Stage::Started, Mutation::Reload, || panic!()),
        Err(OwnerError::InvalidMutation)
    );
    guard
        .commit(ticket, Stage::Started, Mutation::ClaimRelease, || {})
        .unwrap();
    assert_eq!(
        guard.commit(ticket, Stage::Started, Mutation::ClaimRelease, || panic!()),
        Err(OwnerError::WrongStage)
    );
    guard
        .commit(ticket, Stage::Releasing, Mutation::ClearChild, || {})
        .unwrap();
    assert_eq!(
        guard.commit(ticket, Stage::Releasing, Mutation::ClearChild, || panic!()),
        Err(OwnerError::WrongStage)
    );
    guard.invalidate(OwnerIdentity {
        generation: 8,
        ..ID
    });
    assert_eq!(
        guard.commit(
            ticket,
            Stage::Released,
            Mutation::UnloadPrevious,
            || panic!()
        ),
        Err(OwnerError::Stale)
    );
}

#[test]
fn serial_exhaustion_cannot_reuse_an_old_transition_identity() {
    let mut guard = TransitionGuard::new(ID);
    guard.serial = u64::MAX;
    let before = HandSnapshot {
        record: Record::Absent,
        child: Child(0),
    };
    assert_eq!(
        guard.begin(before, Record::Present(2)),
        Err(OwnerError::SerialExhausted)
    );
}

#[test]
fn same_generation_invalidation_cannot_revive_a_ticket_with_identical_inputs() {
    let mut guard = TransitionGuard::new(ID);
    let before = HandSnapshot {
        record: Record::Present(1),
        child: Child(7),
    };
    let Begin::Active(old) = guard.begin(before, Record::Absent).unwrap() else {
        panic!()
    };
    guard.invalidate(ID);
    let Begin::Active(new) = guard.begin(before, Record::Absent).unwrap() else {
        panic!()
    };
    assert_eq!(old.identity(), new.identity());
    assert_ne!(old.serial(), new.serial());
    assert_eq!(
        guard.commit(old, Stage::Started, Mutation::ClaimRelease, || panic!()),
        Err(OwnerError::Stale)
    );
    guard
        .commit(new, Stage::Started, Mutation::ClaimRelease, || {})
        .unwrap();
}

// SPDX-License-Identifier: GPL-3.0-or-later
//! T2411 staged weapon realization, not yet connected to the production runtime.
//!
//! Source contract: `docs/t1678-rea-hands.md` E11 and the retained `0x1BE870`
//! replacement block. Pose work and `0x1BC890` callbacks precede this boundary.
//! The block releases/clears a nonzero child, unloads, publishes the new record,
//! clears the adjacent realization flag, and (ordinary records only) creates,
//! publishes, initializes and attaches a child before reload. Targets -1, 0x29,
//! 0x38 and 0x3d skip creation AND reload. This differs from the existing
//! `WeaponHands::realise_hand`, which always creates/reloads. That API is unchanged.
//!
//! This crate owns the transition protocol, not another inventory or guest image.
//! [`OwnerAccess`] adapts the existing Rust owner. Its methods must release all
//! locks/borrows before returning. [`Effects`] executes between those methods,
//! never inside an owner mutation closure. A reentrant callback may invalidate
//! the lifetime/transition; the next checked commit stops the obsolete runner.
//! Already committed state and external effects are never rolled back. Failure
//! blocks implicit retry; resource cleanup is deliberately left to its owner.
//! The adapter serializes each commit. Resource lifetimes and concurrent disposal
//! remain presentation-backend contracts: this protocol does not claim complete
//! thread safety or exclude concurrent retirement between a claim and its effect.
//! A panicking/interrupted callback leaves its claimed stage busy, preventing an
//! implicit retry; recovering resources after such an interruption is also the
//! backend's responsibility. Explicit error returns preserve returned receipts.
//!
//! The adapter still needed: an upward hook from the active object leaf, resolving
//! immutable definitions and target from Rust-owned selection, checking the native
//! lifetime, applying mutations to its actual hand/reserve, and publishing precise
//! compatibility writes before presentation callbacks. `UnloadPrevious` must use
//! the existing unclamped typed reducer; `Reload` must use the typed flag-0 reducer.
//! `PublishRecord` also clears the adjacent realization flag. Resource creation,
//! initialization and attachment need an explicitly temporary retained boundary.
//! No counter arithmetic, guest-state refresh, root routing, or production API is
//! supplied here. This is a protocol prerequisite with synthetic tests, not Tier 2
//! evidence, an adoption verdict, or a fix to the old locked-callback API.
#![no_std]
// The runtime lock is the sole unsafe boundary; protocol and reducers stay safe.
#![deny(unsafe_code)]

pub mod owned;

/// Record identity; absent is distinct from every definition index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Record {
    Absent,
    Present(u16),
}

impl Record {
    pub const fn creates_child(self) -> bool {
        !matches!(self, Self::Absent | Self::Present(0x29 | 0x38 | 0x3d))
    }
}

/// The raw resource receipt. Zero and MAX remain distinct, including on failure.
/// The measured release predicate is nonzero; this type invents no null policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Child(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnerIdentity {
    pub player: u16,
    pub generation: u64,
    pub side: Side,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandSnapshot {
    pub record: Record,
    pub child: Child,
}

/// A single transition's immutable inputs, issued from the authoritative owner.
/// The snapshot is not a replacement owner and must never be restored on failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    identity: OwnerIdentity,
    serial: u64,
    before: HandSnapshot,
    target: Record,
}

impl Ticket {
    pub const fn identity(self) -> OwnerIdentity {
        self.identity
    }
    pub const fn serial(self) -> u64 {
        self.serial
    }
    pub const fn before(self) -> HandSnapshot {
        self.before
    }
    pub const fn target(self) -> Record {
        self.target
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Started,
    Releasing,
    Released,
    Unloaded,
    RecordPublished,
    Creating,
    ChildPublished,
    Initializing,
    Initialized,
    Attaching,
    Attached,
    Reloaded,
    Complete,
    Failed,
}

/// One atomic owner mutation. Effect claims only advance protocol metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mutation {
    ClaimRelease,
    ClearChild,
    SkipRelease,
    UnloadPrevious,
    PublishRecord,
    ClaimCreate,
    PublishChild(Child),
    ClaimInitialize,
    Initialized,
    ClaimAttach,
    Attached,
    Reload,
    Finish,
    Fail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerError {
    Stale,
    Busy,
    WrongStage,
    InvalidMutation,
    SerialExhausted,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Begin {
    Unchanged(HandSnapshot),
    Active(Ticket),
}

fn next_stage(ticket: Ticket, stage: Stage, mutation: Mutation) -> Result<Stage, OwnerError> {
    use Mutation as M;
    use Stage as S;
    Ok(match (stage, mutation) {
        (S::Started, M::ClaimRelease) if ticket.before.child.0 != 0 => S::Releasing,
        (S::Started, M::SkipRelease) if ticket.before.child.0 == 0 => S::Released,
        (S::Releasing, M::ClearChild) => S::Released,
        (S::Released, M::UnloadPrevious) => S::Unloaded,
        (S::Unloaded, M::PublishRecord) => S::RecordPublished,
        (S::RecordPublished, M::ClaimCreate) if ticket.target.creates_child() => S::Creating,
        (S::Creating, M::PublishChild(_)) => S::ChildPublished,
        (S::ChildPublished, M::ClaimInitialize) => S::Initializing,
        (S::Initializing, M::Initialized) => S::Initialized,
        (S::Initialized, M::ClaimAttach) => S::Attaching,
        (S::Attaching, M::Attached) => S::Attached,
        (S::Attached, M::Reload) => S::Reloaded,
        (S::Reloaded, M::Finish) => S::Complete,
        (S::RecordPublished, M::Finish) if !ticket.target.creates_child() => S::Complete,
        (s, M::Fail) if !matches!(s, S::Complete | S::Failed) => S::Failed,
        _ => return Err(OwnerError::InvalidMutation),
    })
}

/// Per-hand protocol metadata alongside the actual inventory, not a second hand.
/// Every ownership-changing reentry must invalidate the active transition before
/// changing its record/child or lifetime. Reserve-only operations can remain live.
pub struct TransitionGuard {
    identity: OwnerIdentity,
    serial: u64,
    active: Option<(Ticket, Stage)>,
}

impl TransitionGuard {
    pub const fn new(identity: OwnerIdentity) -> Self {
        Self {
            identity,
            serial: 0,
            active: None,
        }
    }

    /// Explicit retirement/supersession, including same-generation supersession.
    /// Serial numbers never reset, so an old ticket cannot match a later attempt.
    /// Calling this does not dispose of resources or undo previous publications.
    pub fn invalidate(&mut self, identity: OwnerIdentity) {
        self.identity = identity;
        self.active = None;
    }

    /// Call while holding the owner, supplying its actual current fields. Resolve
    /// fallible definition inputs before beginning. A failed transition is busy
    /// until explicitly invalidated; this prevents accidental effect retries.
    pub fn begin(&mut self, before: HandSnapshot, target: Record) -> Result<Begin, OwnerError> {
        if self
            .active
            .is_some_and(|(_, stage)| stage != Stage::Complete)
        {
            return Err(OwnerError::Busy);
        }
        if before.record == target {
            return Ok(Begin::Unchanged(before));
        }
        self.serial = self
            .serial
            .checked_add(1)
            .ok_or(OwnerError::SerialExhausted)?;
        let ticket = Ticket {
            identity: self.identity,
            serial: self.serial,
            before,
            target,
        };
        self.active = Some((ticket, Stage::Started));
        Ok(Begin::Active(ticket))
    }

    pub fn validate(&self, ticket: Ticket, expected: Stage) -> Result<(), OwnerError> {
        let Some((active, stage)) = self.active else {
            return Err(OwnerError::Stale);
        };
        if self.identity != ticket.identity || active != ticket {
            return Err(OwnerError::Stale);
        }
        if stage != expected {
            return Err(OwnerError::WrongStage);
        }
        Ok(())
    }

    /// Atomically validate, mutate actual typed owner state, and advance. `apply`
    /// must be infallible and contain no external callbacks, guest calls or locks.
    /// Invalid/stale requests never invoke it. No owner borrow escapes the call.
    pub fn commit(
        &mut self,
        ticket: Ticket,
        expected: Stage,
        mutation: Mutation,
        apply: impl FnOnce(),
    ) -> Result<(), OwnerError> {
        self.validate(ticket, expected)?;
        let next = next_stage(ticket, expected, mutation)?;
        apply();
        self.active = Some((ticket, next));
        Ok(())
    }
}

/// Implement over the existing retained Rust owner using short scoped access.
/// Each call releases every lock/borrow before returning; never stash a guard in
/// `self`. `begin` reads owned fields, not guest copies. `commit` must validate
/// generation, transition identity AND expected stage atomically with mutation.
/// Prefer [`TransitionGuard::commit`]; failures must not partially mutate state.
/// `Fail` changes only protocol status and preserves all previous publications.
pub trait OwnerAccess {
    fn begin(&mut self, target: Record) -> Result<Begin, OwnerError>;
    fn commit(
        &mut self,
        ticket: Ticket,
        expected: Stage,
        mutation: Mutation,
    ) -> Result<(), OwnerError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectError {
    Unsupported,
    Failed(u32),
}

/// Presentation boundary. Methods may reenter/retire the owner. They never receive
/// a mutable hand or owner borrow. Creation returns a raw receipt BEFORE child
/// publication and initialization; it must not initialize/attach internally.
/// No null-result policy is assumed: return the actual raw result, or an explicit
/// error if no receipt was produced. Initialization may reject an unsupported
/// receipt (including zero/MAX); the already-published receipt remains published.
pub trait Effects {
    fn release_child(&mut self, ticket: Ticket, child: Child) -> Result<(), EffectError>;
    fn create_child(&mut self, ticket: Ticket) -> Result<Child, EffectError>;
    fn initialize_child(&mut self, ticket: Ticket, child: Child) -> Result<(), EffectError>;
    fn attach_child(&mut self, ticket: Ticket, child: Child) -> Result<(), EffectError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChildReceipt {
    pub child: Child,
    /// True only after this runner's child-publication commit succeeded. Even a
    /// published resource may need attention following later lifetime retirement.
    pub published: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    Owner(OwnerError),
    Effect(EffectError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Failure {
    pub ticket: Option<Ticket>,
    pub stage: Stage,
    pub cause: Cause,
    /// A returned resource must not be lost if the creating callback retired or
    /// superseded the owner. Its resource owner decides disposal; no auto-cleanup.
    pub created_child: Option<ChildReceipt>,
    /// A stale transition cannot even be marked failed; expose that fact without
    /// replacing the original cause or touching the superseding owner's state.
    pub failure_commit_error: Option<OwnerError>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    Unchanged(HandSnapshot),
    Replaced {
        ticket: Ticket,
        created_child: Option<ChildReceipt>,
    },
}

/// Run one hand. Effects are claimed before invocation and never retried here.
/// Mutations/effects execute at most once in source order; failures stop the run.
/// Right/left call ordering remains the actor caller's responsibility.
pub fn realize<O: OwnerAccess, E: Effects>(
    owner: &mut O,
    effects: &mut E,
    target: Record,
) -> Result<Completion, Failure> {
    let ticket = match owner.begin(target) {
        Ok(Begin::Unchanged(snapshot)) => return Ok(Completion::Unchanged(snapshot)),
        Ok(Begin::Active(ticket)) => ticket,
        Err(error) => {
            return Err(Failure {
                ticket: None,
                stage: Stage::Started,
                cause: Cause::Owner(error),
                created_child: None,
                failure_commit_error: None,
            });
        }
    };
    let mut runner = Runner {
        owner,
        effects,
        ticket,
        stage: Stage::Started,
        created_child: None,
    };
    match runner.run() {
        Ok(()) => Ok(Completion::Replaced {
            ticket,
            created_child: runner.created_child,
        }),
        Err(cause) => {
            let stage = runner.stage;
            let failure_commit_error = runner.owner.commit(ticket, stage, Mutation::Fail).err();
            Err(Failure {
                ticket: Some(ticket),
                stage,
                cause,
                created_child: runner.created_child,
                failure_commit_error,
            })
        }
    }
}

struct Runner<'a, O, E> {
    owner: &'a mut O,
    effects: &'a mut E,
    ticket: Ticket,
    stage: Stage,
    created_child: Option<ChildReceipt>,
}

impl<O: OwnerAccess, E: Effects> Runner<'_, O, E> {
    fn commit(&mut self, mutation: Mutation) -> Result<(), Cause> {
        let next = next_stage(self.ticket, self.stage, mutation).map_err(Cause::Owner)?;
        self.owner
            .commit(self.ticket, self.stage, mutation)
            .map_err(Cause::Owner)?;
        self.stage = next;
        Ok(())
    }

    fn run(&mut self) -> Result<(), Cause> {
        if self.ticket.before.child.0 != 0 {
            self.commit(Mutation::ClaimRelease)?;
            self.effects
                .release_child(self.ticket, self.ticket.before.child)
                .map_err(Cause::Effect)?;
            self.commit(Mutation::ClearChild)?;
        } else {
            self.commit(Mutation::SkipRelease)?;
        }
        self.commit(Mutation::UnloadPrevious)?;
        self.commit(Mutation::PublishRecord)?;
        if self.ticket.target.creates_child() {
            self.commit(Mutation::ClaimCreate)?;
            let child = self
                .effects
                .create_child(self.ticket)
                .map_err(Cause::Effect)?;
            self.created_child = Some(ChildReceipt {
                child,
                published: false,
            });
            self.commit(Mutation::PublishChild(child))?;
            self.created_child = Some(ChildReceipt {
                child,
                published: true,
            });
            self.commit(Mutation::ClaimInitialize)?;
            self.effects
                .initialize_child(self.ticket, child)
                .map_err(Cause::Effect)?;
            self.commit(Mutation::Initialized)?;
            self.commit(Mutation::ClaimAttach)?;
            self.effects
                .attach_child(self.ticket, child)
                .map_err(Cause::Effect)?;
            self.commit(Mutation::Attached)?;
            self.commit(Mutation::Reload)?;
        }
        self.commit(Mutation::Finish)
    }
}

#[cfg(test)]
extern crate std;
#[cfg(test)]
mod tests;

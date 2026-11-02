// SPDX-License-Identifier: GPL-3.0-or-later
//! Authoritative subsystem lock and per-hand realization metadata.
// UnsafeCell access is confined to `locked`; the protocol has no unsafe access.
#![allow(unsafe_code)]
use super::OwnedTransition;
use crate::{OwnerError, Side};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};
use tsfp_weapon_owner::weapon_inventory::{
    WeaponInventoryStore, WeaponPlayerHandle, WeaponSubsystemState,
};

struct State<const N: usize> {
    subsystem: WeaponSubsystemState<N>,
    transitions: [[Option<OwnedTransition>; 2]; N],
    selection_handoffs: [Option<SelectionHandoff>; N],
}

impl<const N: usize> Default for State<N> {
    fn default() -> Self {
        Self {
            subsystem: WeaponSubsystemState::default(),
            transitions: core::array::from_fn(|_| [None, None]),
            selection_handoffs: [None; N],
        }
    }
}

/// Shared runtime owner. No callback may run inside a mutable access closure.
/// Legacy mutable access conservatively invalidates every in-flight ticket:
/// callers can replace the entire subsystem through that interface. Use `read`
/// for inspection and `access` for checked realization commits.
pub struct LockedInventoryStore<const N: usize> {
    held: AtomicBool,
    state: UnsafeCell<Option<State<N>>>,
}

/// Latest changed selection request for one player lifetime. Q9 observes a
/// single desired slot: a later request replaces the pending value instead of
/// adding another queue entry. This is staged state; runtime consumption is a
/// separate integration boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionHandoff {
    pub handle: WeaponPlayerHandle,
    pub previous: Option<u8>,
    pub desired: u8,
}

// SAFETY: every access to state is guarded by the atomic lock. References cannot
// escape the scoped closure. State contains only owned Send data.
unsafe impl<const N: usize> Sync for LockedInventoryStore<N> {}

struct Unlock<'a>(&'a AtomicBool);
impl Drop for Unlock<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl<const N: usize> LockedInventoryStore<N> {
    pub const fn new() -> Self {
        Self {
            held: AtomicBool::new(false),
            state: UnsafeCell::new(None),
        }
    }

    fn locked<R>(&self, wait: bool, f: impl FnOnce(&mut State<N>) -> R) -> Option<R> {
        // Keep the unsafe Sync argument checked if owned state changes later.
        fn assert_send<T: Send>() {}
        assert_send::<State<N>>();
        while self
            .held
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            if !wait {
                return None;
            }
            core::hint::spin_loop();
        }
        let _unlock = Unlock(&self.held);
        // SAFETY: exclusive access lasts until _unlock drops, including unwinding.
        let state = unsafe { &mut *self.state.get() }.get_or_insert_with(Default::default);
        Some(f(state))
    }

    fn mutate<R>(state: &mut State<N>, f: impl FnOnce(&mut WeaponSubsystemState<N>) -> R) -> R {
        for transition in state.transitions.iter_mut().flatten().flatten() {
            transition.invalidate();
        }
        f(&mut state.subsystem)
    }

    /// Existing blocking mutation boundary; never call it while holding access.
    pub fn with<R>(&self, f: impl FnOnce(&mut WeaponSubsystemState<N>) -> R) -> R {
        self.locked(true, |state| Self::mutate(state, f)).unwrap()
    }

    pub fn try_with<R>(&self, f: impl FnOnce(&mut WeaponSubsystemState<N>) -> R) -> Option<R> {
        self.locked(false, |state| Self::mutate(state, f))
    }

    /// Nonblocking immutable inspection does not invalidate a transition.
    pub fn read<R>(&self, f: impl FnOnce(&WeaponSubsystemState<N>) -> R) -> Option<R> {
        self.locked(false, |state| f(&state.subsystem))
    }

    pub fn access(&self, handle: WeaponPlayerHandle, side: Side) -> RuntimeAccess<'_, N> {
        RuntimeAccess {
            owner: self,
            handle,
            side,
        }
    }

    /// Stage one already-normalized selection index in the Rust-owned player
    /// inventory. Changed requests coalesce in the owner's single latest slot;
    /// a same-value request makes no state change. No callbacks run under lock.
    pub fn stage_selection(
        &self,
        handle: WeaponPlayerHandle,
        normalized_desired: u8,
    ) -> Result<Option<SelectionHandoff>, OwnerError> {
        self.locked(false, |state| {
            let slot = handle.player().slot();
            let Some(inventory) = state.subsystem.players.get_handle(handle) else {
                return Err(OwnerError::Stale);
            };
            let previous = inventory.desired_entry;
            if previous == Some(normalized_desired) {
                return Ok(None);
            }

            for transition in state.transitions[slot].iter_mut().flatten() {
                transition.invalidate();
            }
            state
                .subsystem
                .players
                .get_handle_mut(handle)
                .ok_or(OwnerError::Stale)?
                .desired_entry = Some(normalized_desired);
            let handoff = SelectionHandoff {
                handle,
                previous,
                desired: normalized_desired,
            };
            state.selection_handoffs[slot] = Some(handoff);
            Ok(Some(handoff))
        })
        .ok_or(OwnerError::Busy)?
    }

    /// Read the latest pending handoff only while its original player lifetime
    /// remains live and still owns the staged desired value.
    pub fn pending_selection(&self, handle: WeaponPlayerHandle) -> Option<SelectionHandoff> {
        self.locked(false, |state| {
            let slot = handle.player().slot();
            let handoff = state.selection_handoffs.get(slot).copied().flatten()?;
            let inventory = state.subsystem.players.get_handle(handle)?;
            (handoff.handle == handle && inventory.desired_entry == Some(handoff.desired))
                .then_some(handoff)
        })
        .flatten()
    }
}

impl<const N: usize> Default for LockedInventoryStore<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Reacquires the runtime lock for each protocol step, never for an effect.
pub struct RuntimeAccess<'a, const N: usize> {
    owner: &'a LockedInventoryStore<N>,
    handle: WeaponPlayerHandle,
    side: Side,
}

impl<const N: usize>
    tsfp_weapon_realization_protocol::OwnerAccess<WeaponInventoryStore<N>, OwnedTransition>
    for RuntimeAccess<'_, N>
{
    type Error = OwnerError;

    fn with<R>(
        &mut self,
        operation: impl FnOnce(&mut WeaponInventoryStore<N>, &mut OwnedTransition) -> R,
    ) -> Result<R, OwnerError> {
        self.owner
            .locked(false, |state| {
                if state.subsystem.players.get_handle(self.handle).is_none() {
                    return Err(OwnerError::Stale);
                }
                let side = match self.side {
                    Side::Left => 0,
                    Side::Right => 1,
                };
                let slot = &mut state.transitions[self.handle.player().slot()][side];
                if slot
                    .as_ref()
                    .is_none_or(|transition| transition.handle != self.handle)
                {
                    *slot = Some(OwnedTransition::new(self.handle, self.side));
                }
                Ok(operation(
                    &mut state.subsystem.players,
                    slot.as_mut().unwrap(),
                ))
            })
            .ok_or(OwnerError::Busy)?
    }
}

#[cfg(test)]
mod tests;

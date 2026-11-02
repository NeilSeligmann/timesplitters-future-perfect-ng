// SPDX-License-Identifier: GPL-3.0-or-later
//! Write-only publication of committed realization fields (TEMPORARY-SHIM, T2411).
//! The sink resolves and validates its destinations before starting the runner.
//! It must neither read guest state nor invoke callbacks or acquire owner locks.
use super::Projection;
use crate::owned::Publication;
use crate::{Mutation, Side, Ticket};
use tsfp_weapon_owner::weapon_hands::{HandSide, WeaponRecordId};
use tsfp_weapon_owner::weapon_inventory::WeaponInventory;

pub mod memory;

/// Infallible publications under the short owner access. Implementations preserve
/// signed ammo values and distinguish absent children from raw zero/MAX receipts.
/// Guest address/sentinel encoding belongs to the validated runtime sink.
pub trait Sink {
    fn child(&mut self, side: Side, child: Option<u32>);
    fn record(&mut self, side: Side, record: Option<WeaponRecordId>);
    fn clear_realization_flag(&mut self);
    fn ammo(&mut self, side: Side, loaded: &[i16], saved: &[i16], reserve: &[i16]);
}

pub struct WriteProjection<S>(pub S);

impl<S: Sink> Projection for WriteProjection<S> {
    fn publish(
        &mut self,
        owner: &WeaponInventory,
        ticket: Ticket,
        mutation: Mutation,
        publication: Publication,
    ) {
        let side = ticket.identity().side;
        let hand = owner.hands.hand(match side {
            Side::Left => HandSide::Left,
            Side::Right => HandSide::Right,
        });
        match mutation {
            Mutation::ClearChild | Mutation::PublishChild(_) => self.0.child(side, hand.child),
            Mutation::PublishRecord => {
                self.0.record(side, hand.record);
                if publication.clear_realization_flag {
                    self.0.clear_realization_flag();
                }
            }
            Mutation::UnloadPrevious | Mutation::Reload => {
                self.0
                    .ammo(side, &hand.loaded, &hand.saved_rounds, &owner.reserve);
            }
            // Claims, completion and failure change protocol metadata only.
            _ => {}
        }
    }
}

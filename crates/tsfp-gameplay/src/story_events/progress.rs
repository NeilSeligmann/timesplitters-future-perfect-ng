//! Static slot progress tracking (T1842): the body of `0x2282B0` for slot ids below 0x2D, the
//! actor registry calls (`0x227170`, `0x2271F0`, `0x2279D0`, `0x227920`) are in [`SlotManager`].
//!
//! `0x2282B0(id)` runs for every slot of the creation order list each frame (`0x228870`):
//! 1. Not in mode 3 and `id < 0x2D`:
//!    a. With the tracking selector `+0x48` not 0xFF and the slot neither failed nor finished,
//!       count the registered actors whose health (`+0x1CC`) is not above zero (selector 0 is the
//!       only value that counts, any other selector counts nothing). When the count differs from
//!       the last one (`+0x2C`) the remaining number is `+0x30 - count`: zero finishes the slot
//!       (`0x2274C0`), otherwise slots with flag bit 0 post the remaining count to every local
//!       player. Flagged slots then play effect 0xD6C.
//!    b. With a dependency id at `+0x10`, a living local player whose character id equals one of
//!       the dependency ids (`0x9CC40`, up to five) finishes the slot.
//! 2. Any slot with a callback calls it with the slot and code 3 and `[0x78CED0] = id`.
//!
//! MEASURED finding: the selector is written by `0x227A30` only, whose single caller `0x2339FF`
//! belongs to the mode 3 group setup, and step 1 is skipped in mode 3. The kill-count branch
//! (1a) is therefore unreachable in the retail image. It is ported anyway, behind the same
//! guards, so a world that switches tracking on (`SlotManager::enable_tracking`) sees the
//! decoded behaviour.

use super::engine::Engine;
use super::slots::{STATIC_SLOT_COUNT, STATUS_FAILED, STATUS_FINISHED, SlotCallback, SlotEvent};
use super::world::{IdVariant, World};

/// Effect id started after a progress change (`mov [..], 0xd6c` before `0x45820`).
pub const PROGRESS_EFFECT: u32 = 0xD6C;

impl Engine {
    /// `0x2282B0`.
    pub(crate) fn slot_frame(&mut self, id: u32, world: &mut dyn World) {
        if self.context.mode_id != 3 && id < STATIC_SLOT_COUNT {
            self.static_slot_progress(id, world);
        }
        // Slots without a callback return before touching `[0x78CED0]` (`test eax, eax / je`).
        let Some(callback) = self
            .slots
            .get(id)
            .map(|slot| slot.callback)
            .filter(|c| *c != SlotCallback::None)
        else {
            return;
        };
        self.slots.set_current(Some(id));
        if let SlotCallback::World(code) = callback {
            world.slot_poll(id, code);
        }
        self.slots.set_current(None);
    }

    fn static_slot_progress(&mut self, id: u32, world: &mut dyn World) {
        let Some(slot) = self.slots.get(id) else {
            return;
        };
        let (selector, status) = (slot.tracking, slot.status);
        if selector != 0xFF && status != STATUS_FINISHED && status != STATUS_FAILED {
            self.track_dead_actors(id, world);
        }
        if self
            .slots
            .get(id)
            .is_none_or(|slot| slot.dependencies.is_empty())
        {
            return;
        }
        let dependencies = self
            .slots
            .get(id)
            .map(|slot| slot.dependencies.clone())
            .unwrap_or_default();
        for player in 0..world.player_count() {
            let Some(health) = world.player_health(player) else {
                continue;
            };
            if health.partial_cmp(&0.0).is_none_or(|order| order.is_le()) {
                continue;
            }
            for &dependency in &dependencies {
                if world.player_matches(player, IdVariant::CharacterId, &[dependency]) {
                    self.finish_slot(id, world);
                }
            }
        }
    }

    fn track_dead_actors(&mut self, id: u32, world: &mut dyn World) {
        let Some(slot) = self.slots.get(id) else {
            return;
        };
        let dead = if slot.tracking == 0 {
            slot.tracker.as_ref().map_or(Some(0), |tracker| {
                tracker.actors.iter().filter(|&&actor| actor != 0).try_fold(
                    0_u32,
                    |dead, &actor| {
                        let health = world.actor_health(actor)?;
                        // The measured comparison counts health <= 0 as dead. An observed NaN
                        // compares false here; only a missing Option is an incomplete snapshot.
                        Some(dead + u32::from(health <= 0.0))
                    },
                )
            })
        } else {
            Some(0)
        };
        let Some(dead) = dead else {
            // An incomplete snapshot cannot authoritatively change objective progress.
            return;
        };
        if dead == slot.last_count {
            return;
        }
        let remaining = slot.total as i64 - i64::from(dead);
        let flagged = slot.flags & 1 != 0;
        let (one, many) = (slot.progress_one, slot.progress_many);
        if let Some(slot) = self.slots.get_mut(id) {
            slot.last_count = dead;
        }
        if remaining == 0 {
            self.finish_slot(id, world);
        } else if flagged && remaining > 0 {
            let label = if remaining > 1 { many } else { one };
            if label != 0 {
                for player in 0..world.player_count() {
                    world.slot_event(&SlotEvent::Progress {
                        slot: id,
                        player: player as u32,
                        remaining: remaining as u32,
                        label,
                    });
                }
            }
        }
        if flagged {
            world.slot_event(&SlotEvent::ProgressSound { slot: id });
        }
    }
}

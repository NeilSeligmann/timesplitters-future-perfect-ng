//! The edge of the animation system (owner rule 11, T1668): what the neighbours (object, AI, missions, weapons, player, cutscenes,
//! effects, render) may ask of an animation object, expressed as one struct and two traits over the ported routines. This is the shape the
//! 209 incoming game edges of `engine/anim` (see `docs/t-attach-engine-anim.md`) collapse into: 17 query entry points, a handful of commands and the per
//! frame step. Tier 0 for the facade itself (it only delegates; every delegate has its own tier in `docs/t1668-anim-tiers.md`).
use crate::layers::{self, Body, ClipWorld, SpeedWorld};
use crate::queries::{self, Event};

/// The services the animation system needs from the rest of the game: the clip table (`[0x7E3954]`), the policy words (`0x7DE660`), the
/// resident event lists (`0x56FC0`) and the global speed / slow motion switches (`[0x7DE458]`, `[0x7DE4A4]`, player object `0xC6140`).
pub trait AnimWorld: ClipWorld + SpeedWorld {
    /// Event list of a resident animation id (`0x56FC0`), `None` when it is not resident.
    fn events(&self, id: u32) -> Option<&[Event]>;
}

/// An animation object seen from outside: the body (layers, blend progress, speeds) plus the world services.
pub struct AnimObject<'a, W: AnimWorld> {
    pub body: &'a mut Body,
    pub world: &'a W,
    /// `[[body]+0x20]`: the model type (8 for characters).
    pub model_type: Option<u32>,
}

/// Read only queries: the 17 entry points that AI, missions, weapons and the player call (counts in the attachment report).
pub trait AnimQueries {
    fn plays(&self, id: u32) -> bool;
    fn current_plays(&self, id: u32) -> bool;
    fn previous_plays(&self, id: u32) -> bool;
    fn policy_hit(&self, masks: (u32, u32)) -> bool;
    fn any_policy_hit(&self, masks: (u32, u32)) -> bool;
    fn time(&self) -> f32;
    fn progress(&self) -> Option<f64>;
    fn event_in_window(&self, mask: u32) -> bool;
    fn key_number(&self) -> i32;
}

/// State changing commands.
pub trait AnimCommands {
    fn set_time(&mut self, time: f32);
    fn set_speed(&mut self, speed: f32);
    fn reset(&mut self);
}

impl<W: AnimWorld> AnimObject<'_, W> {
    fn active_events(&self) -> Option<&[Event]> {
        self.world.events(self.body.layer_id(self.body.active_layer()))
    }
    fn keys(&self) -> (i32, i32) {
        let layer = self.body.active_layer();
        queries::key_numbers(self.body.layer_time(layer), self.body.float(layer + 0xC))
    }
}

impl<W: AnimWorld> AnimQueries for AnimObject<'_, W> {
    fn plays(&self, id: u32) -> bool {
        layers::active_plays(self.body, id)
    }
    fn current_plays(&self, id: u32) -> bool {
        layers::current_plays(self.body, id)
    }
    fn previous_plays(&self, id: u32) -> bool {
        layers::previous_plays(self.body, id)
    }
    fn policy_hit(&self, masks: (u32, u32)) -> bool {
        layers::active_policy_hit(self.body, self.world, masks)
    }
    fn any_policy_hit(&self, masks: (u32, u32)) -> bool {
        layers::any_policy_hit(self.body, self.world, masks)
    }
    fn time(&self) -> f32 {
        layers::active_time(self.body)
    }
    fn progress(&self) -> Option<f64> {
        layers::normalised_position(self.body, self.world)
    }
    fn event_in_window(&self, mask: u32) -> bool {
        let (current, previous) = self.keys();
        queries::event_in_window(self.active_events(), mask, current, previous)
    }
    fn key_number(&self) -> i32 {
        self.keys().0
    }
}

impl<W: AnimWorld> AnimCommands for AnimObject<'_, W> {
    fn set_time(&mut self, time: f32) {
        layers::set_active_time(self.body, self.world, time);
    }
    fn set_speed(&mut self, speed: f32) {
        layers::set_speed(self.body, self.model_type, self.world, speed);
    }
    fn reset(&mut self) {
        layers::reset_current(self.body, self.world);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct World {
        clips: layers::MapWorld,
        events: BTreeMap<u32, Vec<Event>>,
    }
    impl ClipWorld for World {
        fn duration(&self, slot: usize) -> Option<f32> {
            self.clips.duration(slot)
        }
        fn policy(&self, slot: usize) -> (u32, u32) {
            self.clips.policy(slot)
        }
    }
    impl SpeedWorld for World {
        fn slow_motion_allowed(&self) -> bool {
            false
        }
        fn player_type(&self) -> Option<u32> {
            None
        }
        fn slow_motion_scale(&self) -> f32 {
            1.0
        }
    }
    impl AnimWorld for World {
        fn events(&self, id: u32) -> Option<&[Event]> {
            self.events.get(&id).map(Vec::as_slice)
        }
    }

    #[test]
    fn facade_delegates_to_the_ported_routines() {
        let mut clips = layers::MapWorld::default();
        clips.clips.insert(1, (2.0, (4, 0)));
        let world = World { clips, events: BTreeMap::from([(7, vec![Event { frame: 30, flags: 1 }])]) };
        let mut body = Body([0; layers::BODY_DWORDS]);
        body.0[layers::CURRENT / 4] = 7;
        body.0[layers::CURRENT / 4 + 1] = 1;
        let mut object = AnimObject { body: &mut body, world: &world, model_type: Some(8) };
        object.set_time(0.6);
        assert_eq!(object.time(), 0.6);
        assert_eq!(object.key_number(), 36);
        assert!(object.plays(7) && !object.plays(8) && object.current_plays(7));
        assert!(object.policy_hit((4, 0)) && !object.policy_hit((2, 0)));
        object.set_speed(-1.0);
        assert_eq!(object.body.float(0x108), 0.0, "type 8 models never run backwards");
        assert!(!object.event_in_window(1), "key 36 with previous 36: no window");
    }
}

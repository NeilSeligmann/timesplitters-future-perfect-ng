//! Starting or cross fading an animation layer, `0x5A360` (T1668). MEASURED against the ORIGINAL under Unicorn
//! (`tools/t1668_start_gold.py`, `tests/start_golden.rs`, 1500 fabricated cases, the key sampler stubbed).
//!
//! Signature (cdecl, 7 stack args): `0x5A360(id, blend_time: f32, speed: f32, scale: f32, request, current, previous)`.
//! `current` and `previous` are two 0x58 byte layer states (22 dwords, [`Layer`]); `request` holds the blend request
//! (`+0xC` blend time, `+0x14` blend progress, both reset here). `id` is an animation id (the index of `0x7E03C0`, which maps
//! it to a clip slot, see [`crate::loaders::AnimSlots`]).
//! * `blend_time == 0` (either zero): `current` becomes the new animation from its start and `previous` is reset to animation 0.
//! * otherwise (also NaN): if `previous` plays something else (id not 0 and not `id`) it is first copied into `current` (the
//!   outgoing layer fades out), unless the new clip loops and has policy bit 0x200 (then nothing happens at all); `previous` then
//!   becomes the new animation.
//!
//! Layer fields (byte offsets): `+0` id, `+4` clip slot, `+8` time, `+0xC` time again (start), `+0x10..0x18` root position of key 0,
//! `+0x1C..0x24` copy of it, `+0x28..0x30` zero, `+0x34` speed, `+0x38..0x48` zero, `+0x50` zero (wrap flag), `+0x54` flags (low bits
//! `0xFFFFF01B` kept). The other dwords are untouched.

pub const LAYER_DWORDS: usize = 22;

/// A 0x58 byte animation layer state kept as raw dwords with named accessors for the fields this routine writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer(pub [u32; LAYER_DWORDS]);

impl Layer {
    pub fn id(&self) -> u32 {
        self.0[0]
    }
    fn set_float(&mut self, dword: usize, value: f32) {
        self.0[dword] = value.to_bits();
    }
    /// Initialises the layer to play `id` from its start (`0x5A3C4..0x5A457` / `0x5A4E3..0x5A55B`).
    fn begin(&mut self, id: u32, slot: u32, speed: f32, start_time: f32, root: [f32; 3]) {
        self.0[0] = id;
        self.0[1] = slot;
        self.set_float(13, speed);
        self.0[21] &= 0xFFFF_F01B;
        self.set_float(3, start_time);
        self.set_float(2, start_time);
        for dword in 14..=18 {
            self.set_float(dword, 0.0);
        }
        self.0[20] = 0;
        for (i, value) in root.into_iter().enumerate() {
            self.set_float(4 + i, value);
        }
    }
    /// Common tail (`0x5A560..0x5A5A3`): previous root = current root, zero the three delta cells.
    fn finish(&mut self) {
        for i in 0..3 {
            self.0[7 + i] = self.0[4 + i];
            self.set_float(10 + i, 0.0);
        }
    }
}

/// The blend request block: only `+0xC` and `+0x14` are written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request(pub [u32; 6]);

/// What the routine needs from the world.
pub trait StartEnv {
    /// `0x7E03C0[id]`: clip slot of an animation id.
    fn slot_of(&self, id: u32) -> u32;
    /// `clip+0x18` of the clip in a slot (`None` when the slot is empty).
    fn duration(&self, slot: u32) -> Option<f32>;
    /// Policy word 0 of a slot (`0x7DE660 + 8 * slot`).
    fn policy0(&self, slot: u32) -> u32;
    /// First three floats of the node 0 key 0 sample (the root position).
    fn root_at_start(&self, slot: u32, scale: f32) -> [f32; 3];
}

/// Initial time: the (negative) duration of a clip with negative duration, else 0 (the original compares `0 > duration`).
fn start_time(duration: Option<f32>) -> f32 {
    match duration {
        Some(d) if 0.0 > d => d,
        _ => 0.0,
    }
}

/// `0x5A360`. Returns `false` when the routine leaves everything untouched (looping clip with policy 0x200 while a different
/// animation plays in `previous`).
#[allow(clippy::too_many_arguments)]
pub fn start_animation(
    env: &impl StartEnv,
    id: u32,
    blend_time: f32,
    speed: f32,
    scale: f32,
    request: &mut Request,
    current: &mut Layer,
    previous: &mut Layer,
) -> bool {
    let slot = env.slot_of(id);
    if blend_time == 0.0 {
        request.0[3] = blend_time.to_bits();
        request.0[5] = 0.0f32.to_bits();
        current.begin(
            id,
            slot,
            speed,
            start_time(env.duration(slot)),
            env.root_at_start(slot, scale),
        );
        previous.0[0] = 0;
        previous.0[1] = env.slot_of(0);
    } else {
        if previous.id() != 0 && previous.id() != id {
            let policy0 = env.policy0(slot);
            if policy0 & 1 != 0 && policy0 & 0x200 != 0 {
                return false;
            }
            current.0 = previous.0;
        }
        request.0[3] = blend_time.to_bits();
        request.0[5] = 0.0f32.to_bits();
        previous.begin(
            id,
            slot,
            speed,
            start_time(env.duration(slot)),
            env.root_at_start(slot, scale),
        );
    }
    current.finish();
    previous.finish();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    struct World;
    impl StartEnv for World {
        fn slot_of(&self, id: u32) -> u32 {
            id + 1
        }
        fn duration(&self, _: u32) -> Option<f32> {
            Some(2.0)
        }
        fn policy0(&self, _: u32) -> u32 {
            0
        }
        fn root_at_start(&self, _: u32, _: f32) -> [f32; 3] {
            [1.0, 2.0, 3.0]
        }
    }

    #[test]
    fn immediate_start_resets_previous_and_blend_copies_it() {
        let (mut request, mut current, mut previous) =
            (Request([7; 6]), Layer([5; 22]), Layer([9; 22]));
        assert!(start_animation(
            &World,
            4,
            0.0,
            1.5,
            1.0,
            &mut request,
            &mut current,
            &mut previous
        ));
        assert_eq!(
            (current.0[0], current.0[1], previous.0[0], previous.0[1]),
            (4, 5, 0, 1)
        );
        assert_eq!(f32::from_bits(current.0[7]), 1.0);
        let old = current.clone();
        previous.0[0] = 3;
        assert!(start_animation(
            &World,
            6,
            0.5,
            1.0,
            1.0,
            &mut request,
            &mut current,
            &mut previous
        ));
        assert_eq!(current.0[0], 3); // copied from the old previous (id 3)
        assert_eq!(previous.0[0], 6);
        assert_ne!(old, current);
    }
}

//! Arcade objective runtime and the level 0x64 (Map Maker) struct (T1845, item D).
//!
//! Static reading of `build/default.xbe` (SHA-256 `3cfd001a...1816bc`), no xemu run. Control flow and constants are MEASURED;
//! field meanings are INFERRED unless stated. Details and the unmodelled parts: `docs/t-story-event-arcade-runtime.md`.
//!
//! * [`level64`]: the "struct" the level table gives level 0x64 is a 0x154 byte BSS block at `0x78CC00` that is zeroed once
//!   (`0x23F8DA`) and whose first dword is the save-profile progress mask; no code stores `+0x110`, so the level hook is null.
//! * [`ObjectiveTable`]: the run-time table at `[0x74C3B0]` built by `0x2338D0` from the level struct and evaluated every frame
//!   by `0x232270` (mode 3). Pass 1 recomputes the 16-bit state word of every entry, pass 2 turns changes of that word into
//!   notifications for the per-level hook `[table + 0x544]` chosen by `0x232EA0` ([`objective_hook_for_level`]).

/// Number of actor slots a kind 0 or kind 2 entry watches (slot record `+0x54 .. +0x78`, stride 4).
pub const ACTOR_SLOTS: usize = 10;
/// Entry stride of the table at `[0x74C3B0]` (header dword = entry count, entries start at `+4`).
pub const ENTRY_STRIDE: usize = 0xA8;
/// Mask applied to the state words before they are compared bit by bit: ten actor bits plus the finished bit 15.
pub const STATE_MASK: u16 = 0x83FF;
/// Bit 15 of the state word: the slot of the entry is in status 5 (finished).
pub const FINISHED_BIT: u16 = 0x8000;
/// Event id the table hook receives when an entry has finished (`hook(entry, 0xF, 0)`).
pub const EVENT_FINISHED: u32 = 0xF;
/// The random value mixed into bits 10 to 15 of a changed finished entry is `(int)(raw / 2^32 * 31 + 0.5)` (byte).
pub const RANDOM_SCALE: f32 = 31.0;

/// Level 0x64 (Map Maker) facts (MEASURED).
pub mod level64 {
    /// Address the level table (`0x515FFC`) gives for both structs of level 0x64.
    pub const STRUCT_ADDRESS: u32 = 0x78_CC00;
    /// Size of the block: `0x23F8DA` clears `0x55` dwords starting there.
    pub const STRUCT_SIZE: u32 = 0x154;
    /// `+0x110` of the struct (`0x78CD10`): inside the zeroed block.
    pub const HOOK_FIELD: u32 = 0x110;
    /// Key looked up in the save buffer `[0x784014]` (`0x32A5B0`) to fill the first dword.
    pub const PROGRESS_KEY: u32 = 0xC7_7668;
    /// Functions that store the looked up progress mask into the first dword.
    pub const PROGRESS_WRITERS: [u32; 5] = [0x19A17E, 0x2C8423, 0x2C9FC1, 0x2D711F, 0x35506B];
    /// Function that zeroes the block (once, at start up).
    pub const CLEAR_SITE: u32 = 0x23F8DA;
    /// Readers that take the block as a level struct: arcade set-up `0x33AA80`, `0x338210`, `0x33FD90`, `0x33C4F0`.
    pub const STRUCT_READERS: [u32; 4] = [0x33AA80, 0x338210, 0x33FD90, 0x33C4F0];

    /// The level hook `[0x78CD10]`: never written (MEASURED: no direct reference to `0x78CD10`, no store through a pointer
    /// in the readers, the only writes are the clear and the first dword), so it stays 0 and the frame update skips it.
    pub fn hook() -> Option<u32> {
        None
    }
}

/// The table hook `[table + 0x544]` selected by `0x232EA0` for a level id (MEASURED: byte map over ids `0x1B..=0x64`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectiveHook {
    /// Level 0x1D: `0x208030`, switch over the event kind 0 to 4 (props "WALL" and "DOR1").
    Level1D,
    /// Level 0x1B: `0x222140`.
    Level1B,
    /// Level 0x30: `0x389B0`, the shared no-op.
    Level30NoOp,
    /// Level 0x3B: `0x225A50`.
    Level3B,
    /// Level 0x64 (Map Maker): `0x232E50`, see [`map_maker_hook`].
    MapMaker,
}

impl ObjectiveHook {
    pub fn address(self) -> u32 {
        match self {
            ObjectiveHook::Level1D => 0x208030,
            ObjectiveHook::Level1B => 0x222140,
            ObjectiveHook::Level30NoOp => 0x389B0,
            ObjectiveHook::Level3B => 0x225A50,
            ObjectiveHook::MapMaker => 0x232E50,
        }
    }
}

/// `0x232EA0`: the objective hook of a level, `None` for every id outside `0x1B..=0x64` and the ids the map leaves at 0.
pub fn objective_hook_for_level(level_id: u32) -> Option<ObjectiveHook> {
    match level_id {
        0x1B => Some(ObjectiveHook::Level1B),
        0x1D => Some(ObjectiveHook::Level1D),
        0x30 => Some(ObjectiveHook::Level30NoOp),
        0x3B => Some(ObjectiveHook::Level3B),
        0x64 => Some(ObjectiveHook::MapMaker),
        _ => None,
    }
}

/// The Map Maker table hook `0x232E50(entry_slot, event)`: for event [`EVENT_FINISHED`] it finds the table index whose slot
/// equals `entry_slot` and calls `0x33B040(index)` (complete the arcade entry). Other events do nothing.
pub fn map_maker_hook(table: &ObjectiveTable, entry_slot: u32, event: u32) -> Option<usize> {
    if event != EVENT_FINISHED {
        return None;
    }
    table
        .entries
        .iter()
        .position(|entry| entry.slot == entry_slot)
}

/// Kind of an entry (`entry + 4`, written by `0x2338D0` from the descriptor word `+0x14`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    /// Kind 0: bit i set when actor i of the slot is missing or its health `+0x1CC` is not positive. Pass 2 notifies rising bits.
    ActorsDown,
    /// Kind 1: no bits (state word 0). `0x2338D0` stores `descriptor +0x1C` at `entry + 0x7C` and counts `entry + 0x78`.
    Counter,
    /// Kind 2: bit i set when actor i exists and its extension flag word `+0x18` is non-zero. Pass 2 keeps the flag in step.
    ActorFlags,
    /// Kind 3: one bit group per member of the group list `[0x7789F8][slot]`, width depending on the member type.
    Members,
    /// Any other value: treated like kind 1 (no bits).
    Other(u32),
}

impl EntryKind {
    pub fn from_word(word: u32) -> Self {
        match word {
            0 => EntryKind::ActorsDown,
            1 => EntryKind::Counter,
            2 => EntryKind::ActorFlags,
            3 => EntryKind::Members,
            other => EntryKind::Other(other),
        }
    }
}

/// One watched actor of a kind 0 or kind 2 entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorState {
    pub present: bool,
    /// Health `+0x1CC`.
    pub health: f32,
    /// Extension flag word `+0x18` non-zero.
    pub flag: bool,
}

/// One member of a kind 3 entry. `kind` is the member type `[member+4 -> +8]`, 0 to 9.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Member {
    pub kind: u32,
}

/// Bits a member contributes in pass 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct MemberBits {
    pub bits: u32,
    pub width: u32,
}

/// One entry of the table (`0xA8` bytes in the image; only the fields the runtime uses).
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub slot: u32,
    pub kind: EntryKind,
    /// `+0xC` word of the image entry: state word of this frame (bits 0 to 9, finished bit 15, random bits 10 to 14).
    pub current: u16,
    /// Previous frame's word.
    pub previous: u16,
    pub members: Vec<Member>,
}

/// `0x2338D0` input: one descriptor (stride 0x60 at `level +0xE4`, count `+0xE8`), only the fields used here.
#[derive(Clone, Debug, PartialEq)]
pub struct EntryDescriptor {
    pub slot: u32,
    pub kind: u32,
    pub members: Vec<Member>,
}

/// World side of the runtime (everything `0x232270` asks the game).
pub trait ArcadeWorld {
    /// `0x2273C0`: is the slot in status 5 (finished)?
    fn slot_finished(&self, slot: u32) -> bool;
    /// `0x153B40`: next raw 32 bit random number.
    fn random_u32(&mut self) -> u32;
    /// Actor `index` (0 to 9) of the slot record, `None` when the pointer is null.
    fn actor(&self, slot: u32, index: usize) -> Option<ActorState>;
    /// `0x33B000`: has the member completed? (INFERRED name, calls the member's method `+0x10` and tests status 1.)
    fn member_complete(&mut self, slot: u32, member: usize) -> bool;
    /// Pass 1 bits of members of types 1, 2, 7, 8, 9 (types 0, 3, 4 use [`ArcadeWorld::member_complete`], 5 and 6 give none).
    fn member_bits(&mut self, slot: u32, member: usize, kind: u32) -> MemberBits;
    /// Types 1 and 2: number of sub entries of the member's group node (`0x3397F0(node +8)` count `+4`).
    fn sub_entry_count(&self, _slot: u32, _member: usize) -> usize {
        0
    }
    /// Types 1 and 2: trigger flag `i` of the sub entries (dword `+0x30 + 4*i`).
    fn sub_entry_triggered(&self, _slot: u32, _member: usize, _index: usize) -> bool {
        false
    }
    fn set_sub_entry_triggered(&mut self, _slot: u32, _member: usize, _index: usize) {}
    /// Type 7: the id wanted by the member (`[member record +0x14]`) and the candidates of the `0x3384F0` list in order.
    fn type7_candidates(&mut self, _slot: u32, _member: usize) -> (u32, Vec<TargetObject>) {
        (0, Vec::new())
    }
    /// Types 8 and 9: the objects of the `0x3384F0` list (the "any flag" variant).
    fn type89_targets(&mut self, _slot: u32, _member: usize) -> Vec<TargetObject> {
        Vec::new()
    }
}

/// One object of a member's target list. `flagged` is `[object +0x28] & 3 != 0`, `id` the resolved id of the object type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetObject {
    pub handle: u32,
    pub id: u32,
    pub flagged: bool,
}

/// What pass 2 asks the game to do. `hook` is the level's table hook `[table + 0x544]` call.
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectiveEvent {
    /// `hook(entry_slot, index, object)`: bit `index` of the entry changed (kind 0, 2: actor index, kind 3: member index).
    Hook {
        slot: u32,
        index: usize,
        kind: u32,
        rising: bool,
    },
    /// Kind 2: the actor's flag no longer matches its bit, `0xB80A0` toggles it.
    ToggleActorFlag { slot: u32, actor: usize },
    /// Kind 3 types 3 and 4: the member's flag follows the bit via `0xB80A0`.
    ToggleMemberFlag { slot: u32, member: usize },
    /// Kind 3 type 0: a score pop effect (`0xB91F0` / `0xBB6C0`, value = twice the object's health).
    ScoreEffect { slot: u32, member: usize },
    /// Bit 15: `hook(entry, 0xF, 0)` followed by `0x2274C0(slot, 0)` (finish the slot).
    SlotFinished { slot: u32 },
    /// Hook call with an object argument (types 7, 8, 9: the target, `None` = null; types 1, 2: always `None`).
    HookObject {
        slot: u32,
        index: usize,
        object: Option<u32>,
    },
    /// Types 1, 2: sub entry `sub` of the member got its trigger flag set; `triggered` is the running count.
    SubEntryTriggered {
        slot: u32,
        member: usize,
        sub: usize,
        triggered: usize,
    },
    /// Types 1, 2: the group node is marked complete (`+0x20 = 1`, `+0x18 = 1`).
    GroupNodeComplete { slot: u32, member: usize },
    /// Types 7, 8, 9: the group node is marked dirty (`+0xC |= 0x10`, `+0x20 = 1`).
    GroupNodeDirty { slot: u32, member: usize },
    /// Types 7, 8, 9: the target's flag bits `0x3` are cleared (`[object +0x28] &= ~3`).
    TargetFlagsCleared { object: u32 },
}

fn bit_set(word: u16, position: u32) -> bool {
    position < 16 && word >> position & 1 == 1
}

/// The table at `[0x74C3B0]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObjectiveTable {
    pub entries: Vec<Entry>,
    /// The hook chosen by `0x232EA0`.
    pub hook: Option<ObjectiveHook>,
}

impl ObjectiveTable {
    /// `0x2338D0` (only when the mode byte is 3 and the level struct exists): one entry per descriptor, words cleared.
    /// The reward short lists copied from the descriptor and the slot set-up calls (`0x228130`, `0x227A30`, `0x227880`,
    /// `0x227A50(0x2332F0)`) belong to the slot manager and are not modelled here.
    pub fn build(level_id: u32, descriptors: &[EntryDescriptor]) -> Self {
        let entries = descriptors
            .iter()
            .map(|descriptor| Entry {
                slot: descriptor.slot,
                kind: EntryKind::from_word(descriptor.kind),
                current: 0,
                previous: 0,
                members: descriptor.members.clone(),
            })
            .collect();
        ObjectiveTable {
            entries,
            hook: objective_hook_for_level(level_id),
        }
    }

    /// `0x355EC0`: the runtime only runs when this holds. `level_id` is `[0x79094C]`; `network_game` stands for `0x3533F0`
    /// (non-zero) and `peer_state_one` for `[0x76B140] == 1`. Level 0x65 depends only on the peer state.
    pub fn runtime_enabled(level_id: u32, network_game: bool, peer_state_one: bool) -> bool {
        if level_id == 0x65 {
            return peer_state_one;
        }
        !network_game || peer_state_one
    }

    /// Pass 1 of `0x232270` for one entry: the new state word.
    fn compute_word(&self, entry: &Entry, world: &mut dyn ArcadeWorld) -> u16 {
        let mut word: u16 = 0;
        match entry.kind {
            EntryKind::ActorsDown => {
                for index in 0..ACTOR_SLOTS {
                    let down = match world.actor(entry.slot, index) {
                        None => true,
                        Some(actor) => !(actor.health > 0.0),
                    };
                    if down {
                        word |= 1 << index;
                    }
                }
            }
            EntryKind::ActorFlags => {
                for index in 0..ACTOR_SLOTS {
                    if matches!(world.actor(entry.slot, index), Some(actor) if actor.flag) {
                        word |= 1 << index;
                    }
                }
            }
            EntryKind::Members => {
                // The width register is not reset between members (MEASURED quirk): types 5 and 6 add the previous width.
                let mut offset: u32 = 0;
                let mut width: u32 = 0;
                for (index, member) in entry.members.iter().enumerate() {
                    let bits = match member.kind {
                        0 | 3 | 4 => {
                            width = 1;
                            u32::from(world.member_complete(entry.slot, index))
                        }
                        1 | 2 | 7 | 8 | 9 => {
                            let got = world.member_bits(entry.slot, index, member.kind);
                            width = if matches!(member.kind, 1 | 2) {
                                got.width
                            } else {
                                1
                            };
                            got.bits
                        }
                        _ => 0,
                    };
                    if offset < 16 {
                        word |= (bits << offset) as u16;
                    }
                    offset += width;
                }
            }
            EntryKind::Counter | EntryKind::Other(_) => {}
        }
        word
    }

    /// `0x232270`: both passes. Returns the notifications in the order the image issues them.
    pub fn frame(&mut self, world: &mut dyn ArcadeWorld) -> Vec<ObjectiveEvent> {
        let mut events = Vec::new();
        // Pass 1.
        for position in 0..self.entries.len() {
            let entry = self.entries[position].clone();
            let old = entry.current & STATE_MASK;
            let mut word = self.compute_word(&entry, world);
            if world.slot_finished(entry.slot) {
                word |= FINISHED_BIT;
            }
            if word != old {
                if world.slot_finished(entry.slot) {
                    let raw = world.random_u32() as f32;
                    let value = ((raw * 2.328_306_4e-10 * RANDOM_SCALE) + 0.5) as i32 as u8;
                    word |= u16::from(value) << 10;
                }
                self.entries[position].current = word;
            }
        }
        // Pass 2.
        for position in 0..self.entries.len() {
            let (current, previous, slot, kind) = {
                let entry = &self.entries[position];
                (entry.current, entry.previous, entry.slot, entry.kind)
            };
            if current == previous {
                continue;
            }
            let now = current & STATE_MASK;
            let before = previous & STATE_MASK;
            match kind {
                EntryKind::ActorsDown => {
                    for index in 0..ACTOR_SLOTS {
                        if before >> index & 1 == 0 && now >> index & 1 == 1 {
                            events.push(ObjectiveEvent::Hook {
                                slot,
                                index,
                                kind: 0,
                                rising: true,
                            });
                        }
                    }
                }
                EntryKind::ActorFlags => {
                    for index in 0..ACTOR_SLOTS {
                        let Some(actor) = world.actor(slot, index) else {
                            continue;
                        };
                        let bit = now >> index & 1 == 1;
                        if (before >> index & 1 == 1) == bit {
                            continue;
                        }
                        events.push(ObjectiveEvent::Hook {
                            slot,
                            index,
                            kind: 2,
                            rising: bit,
                        });
                        // Bit set: toggle only when the flag is clear. Bit clear: toggle only when the flag is set.
                        if actor.flag != bit {
                            events.push(ObjectiveEvent::ToggleActorFlag { slot, actor: index });
                        }
                    }
                }
                EntryKind::Members => {
                    let members = self.entries[position].members.clone();
                    let mut counter: u32 = 0;
                    for (index, member) in members.iter().enumerate() {
                        match member.kind {
                            0 | 3 | 4 => {
                                let cur = counter < 16 && now >> counter & 1 == 1;
                                let prev = counter < 16 && before >> counter & 1 == 1;
                                counter += 1;
                                if cur == prev {
                                    continue;
                                }
                                let complete = world.member_complete(slot, index);
                                events.push(ObjectiveEvent::Hook {
                                    slot,
                                    index,
                                    kind: member.kind,
                                    rising: cur,
                                });
                                // Falling with a complete member, or rising with an incomplete one, runs the effect.
                                if cur != complete {
                                    if member.kind == 0 {
                                        events.push(ObjectiveEvent::ScoreEffect {
                                            slot,
                                            member: index,
                                        });
                                    } else {
                                        events.push(ObjectiveEvent::ToggleMemberFlag {
                                            slot,
                                            member: index,
                                        });
                                    }
                                }
                            }
                            1 | 2 => {
                                let count = world.sub_entry_count(slot, index);
                                // The running count starts at zero every frame (`[esp+0x14] = 0`) and counts new triggers only.
                                let mut triggered = 0;
                                for sub in 0..count {
                                    let rising = bit_set(now, counter + sub as u32);
                                    let was = bit_set(before, counter + sub as u32);
                                    if !rising || was {
                                        continue;
                                    }
                                    events.push(ObjectiveEvent::HookObject {
                                        slot,
                                        index,
                                        object: None,
                                    });
                                    if !world.sub_entry_triggered(slot, index, sub) {
                                        world.set_sub_entry_triggered(slot, index, sub);
                                        triggered += 1;
                                        events.push(ObjectiveEvent::SubEntryTriggered {
                                            slot,
                                            member: index,
                                            sub,
                                            triggered,
                                        });
                                    }
                                    if member.kind == 1 || triggered == count {
                                        events.push(ObjectiveEvent::GroupNodeComplete {
                                            slot,
                                            member: index,
                                        });
                                    }
                                }
                                counter += count as u32;
                            }
                            7 => {
                                let cur = counter < 16 && now >> counter & 1 == 1;
                                let prev = counter < 16 && before >> counter & 1 == 1;
                                let complete = world.member_complete(slot, index);
                                if complete && !cur {
                                    // eax != 0 and the new bit clear: skips the counter increment (MEASURED quirk).
                                    continue;
                                }
                                if cur && !prev && !complete {
                                    let (wanted, candidates) = world.type7_candidates(slot, index);
                                    let target = candidates
                                        .iter()
                                        .find(|c| c.id == wanted)
                                        .filter(|c| c.flagged)
                                        .map(|c| c.handle);
                                    events.push(ObjectiveEvent::HookObject {
                                        slot,
                                        index,
                                        object: target,
                                    });
                                    events.push(ObjectiveEvent::GroupNodeDirty {
                                        slot,
                                        member: index,
                                    });
                                    if let Some(object) = target {
                                        events.push(ObjectiveEvent::TargetFlagsCleared { object });
                                    }
                                }
                                counter += 1;
                            }
                            8 | 9 => {
                                // The image does not advance the bit counter for these types (MEASURED), see the doc.
                                let targets = world.type89_targets(slot, index);
                                for (i, target) in targets.iter().enumerate() {
                                    let rising = bit_set(now, counter + i as u32);
                                    let was = bit_set(before, counter + i as u32);
                                    if !rising || was {
                                        continue;
                                    }
                                    events.push(ObjectiveEvent::HookObject {
                                        slot,
                                        index,
                                        object: Some(target.handle),
                                    });
                                    if target.flagged {
                                        events.push(ObjectiveEvent::GroupNodeDirty {
                                            slot,
                                            member: index,
                                        });
                                        events.push(ObjectiveEvent::TargetFlagsCleared {
                                            object: target.handle,
                                        });
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                EntryKind::Counter | EntryKind::Other(_) => {}
            }
            if now & FINISHED_BIT != 0 {
                events.push(ObjectiveEvent::SlotFinished { slot });
            }
            self.entries[position].previous = self.entries[position].current;
        }
        events
    }
}

/// How an object is found by a table hook (all MEASURED call shapes, meanings INFERRED).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectRef {
    /// `0x23FF20(tag)`: object by four character tag (`"DOR1"` = `0x444F5231`).
    Tag(u32),
    /// `0x227980(slot, id, sub)`: object found in the data table of a slot record (`+0x54` list).
    SlotLookup { slot: u32, id: u32, sub: u32 },
    /// The actor `index` of the slot record in the 4C array (`[0x78CECC] + 0x4C*slot + 0xC`, `+0x54 + 4*index`).
    SlotActor { slot: u32, index: u32 },
    /// The object argument the hook was called with.
    Argument,
}

/// A world step requested by a table hook, in call order.
#[derive(Clone, Debug, PartialEq)]
pub enum TableHookOp {
    /// `0xA9070(object)`: callback dispatch kind 1 then destroy (only when the lookup found it).
    Destroy(ObjectRef),
    /// `0x21780(id)`: HUD sound request.
    Sound { id: u32 },
    /// `0x221EC0`, `0x221F20` and the tail of `0x207D10` (T1845 round 4): for every `(key a, key b)` pair, `0x9CC40` maps each key to its
    /// record of the table `[0x715F2C]` (stride 0x20), `0x9D780` finds the connection between the two records and, when it exists,
    /// its first dword gets bit `0x10000` ([`CONNECTION_FLAG`]).
    MarkConnections { pairs: &'static [(u32, u32)] },
    /// `0xEFBD0`: spawn a table `0x737BD4` effect (see [`EffectSpawn`]).
    SpawnEffect(EffectSpawn),
    /// `0x46A00(id, a, b, c)`: submit the sound twice and set both volumes (table `0x4CA9B4` byte times `a`); `b`, `c` are the two
    /// rate floats of the pair (INFERRED: pitch jitter).
    SoundPair {
        id: u32,
        scale: f32,
        rate_a: f32,
        rate_b: f32,
    },
    /// `0xA5CE0(object, state, 0)`: remove the tick function of `state`.
    ClearTick { object: ObjectRef, state: u32 },
    /// `0x9E720(object, value)`: store `value` in the dword at `+0x19C`.
    SetField19c { object: ObjectRef, value: u32 },
    /// `[object + 4] + 0x30` flag word: clear `clear`, then set `set` (level 0x1D wall: clear 2, set 4).
    ModelFlags {
        object: ObjectRef,
        clear: u32,
        set: u32,
    },
    /// `0x879E0(index)`: word `+0x16` of entry `index` in `[0x70878C]` is set to 0 (`0x879C0` sets 0xFFFF).
    ClearEntryWord { index: u32 },
    /// `0x12D350` / `0x148A10(player, 3, text)` for every local player with a valid slot: HUD message of kind 3.
    HudMessageAllPlayers { text_id: u32 },
    /// `0xB0DB0(tagged, mask)`: clear object flag bits in `[[obj+0x7C]+0xC]`.
    ClearFlags { tag: u32, mask: u32 },
    /// `0xB0D90(tagged, mask)`: set them.
    SetFlags { tag: u32, mask: u32 },
    /// `0xB91F0` + `0xBB6C0`: a hit record of `health + 1.0` against the object credited to the first local player: it dies
    /// and is scored (the same effect as [`ObjectiveEvent::ScoreEffect`]). Skipped when the object is not found.
    KillCredited(ObjectRef),
    /// `0x1129B0(object)` then `0xEFBD0(kind, ..)` at the object position `+0x64`: a table 737BD4 effect of `kind` (level 0x1D wall).
    Effect {
        object: ObjectRef,
        kind: u32,
        with_sound: Option<u32>,
    },
    /// Level 0x1D slot 3: move the two tagged props `"PRWL"` and the lookup `(4, 0x85, 0)` to the spawn record chosen by the
    /// random bits of the state word (`(state >> 10 & 0x1F) % 5` into [`SLOT3_SPAWN_IDS`]), heading wrapped to `[0, 360)`.
    PlacePropsAtSpawn { record_id: u32 },
    /// Level 0x1D slot 4: `0x966E0(object, 0x5B, 1.0)`: object animation state `0x5B`.
    SetAnimationState {
        object: ObjectRef,
        state: u32,
        rate: f32,
    },
}

/// Inputs of a table hook call `hook(entry, event, object)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableHookCall {
    /// `*entry`, the slot number of the entry (0 to 4 for all hooks here).
    pub slot: u32,
    /// Second argument: bit/member index or [`EVENT_FINISHED`].
    pub event: u32,
    /// The entry's state word (`entry + 8`).
    pub state: u16,
}

/// Spawn record ids picked by level 0x1D slot 3 (`0x504C30`, five dwords).
pub const SLOT3_SPAWN_IDS: [u32; 5] = [0x234E, 0x236A, 0x2349, 0x2397, 0x2369];

const TAG_DOR1: u32 = 0x444F_5231;

fn tag(n: u8) -> u32 {
    0x444F_5230 + u32::from(n)
}

/// Bit set on a connection by the three mark helpers.
pub const CONNECTION_FLAG: u32 = 0x10000;
/// `0x50ADC8..0x50ADD8` (`0x221EC0`): two `(key, key)` pairs (MEASURED).
pub const CONNECTIONS_221EC0: [(u32, u32); 2] = [(0x4AF, 0x4AE), (0x3F7, 0x3F6)];
/// `0x50ADD8..0x50ADF0` (`0x221F20`): three pairs (MEASURED).
pub const CONNECTIONS_221F20: [(u32, u32); 3] = [(0x42A, 0x432), (0x429, 0x4A0), (0x42E, 0x42D)];
/// `0x504C00..0x504C30` (`0x207D10`): six pairs (MEASURED).
pub const CONNECTIONS_207D10: [(u32, u32); 6] = [
    (0x57E, 0x57A),
    (0x57F, 0x57A),
    (0x580, 0x57A),
    (0x581, 0x57A),
    (0x563, 0x57D),
    (0x564, 0x57D),
];

/// `0x222050` (level 0x1B slot 4 finished): a kind 0x21 effect (extra 0x17) at the object position `+0x64` with velocity (0, -1, 0)
/// and the sound flag, owner = the object, then the ticks of states 3, 2 and 10 are removed, `+0x19C` = 0, the pair sound `0x4F0`
/// (0.1, 0.95... see the op) and the object is destroyed.
pub fn helper_222050(object: ObjectRef) -> Vec<TableHookOp> {
    vec![
        TableHookOp::SpawnEffect(EffectSpawn {
            kind: 0x21,
            extra: 0x17,
            position: EffectPosition::ObjectPosition(object),
            velocity: Some([0.0, -1.0, 0.0]),
            source: None,
            owner: Some(object),
            seconds: 0.0,
            sound: true,
        }),
        TableHookOp::ClearTick { object, state: 3 },
        TableHookOp::ClearTick { object, state: 2 },
        TableHookOp::ClearTick { object, state: 10 },
        TableHookOp::SetField19c { object, value: 0 },
        TableHookOp::SoundPair {
            id: 0x4F0,
            scale: 0.1,
            rate_a: 1.0,
            rate_b: 0.95,
        },
        TableHookOp::Destroy(object),
    ]
}

/// `0x207D10` (level 0x1D slot 0 finished, after the wall effect): a kind 0x26 effect (extra 0) at a fixed point with a fixed
/// velocity, mirrored in x when the first local player's `+0x27C` is below 1.25 (`mirrored`), the pair sound `0x653`, the "WALL"
/// model flags (clear 2, set 4), the six connection marks and `0x879E0(0x2A)`.
pub fn helper_207d10(mirrored: bool) -> Vec<TableHookOp> {
    let (vx, px) = if mirrored {
        (-0.75, -2.17)
    } else {
        (0.75, 5.83)
    };
    let wall = ObjectRef::Tag(0x5741_4C4C);
    vec![
        TableHookOp::SpawnEffect(EffectSpawn {
            kind: 0x26,
            extra: 0,
            position: EffectPosition::Fixed([px, 2.66, 29.54]),
            velocity: Some([vx, 0.67, 0.06]),
            source: None,
            owner: None,
            seconds: 0.0,
            sound: true,
        }),
        TableHookOp::SoundPair {
            id: 0x653,
            scale: 0.3,
            rate_a: 0.95,
            rate_b: 1.05,
        },
        TableHookOp::ModelFlags {
            object: wall,
            clear: 2,
            set: 4,
        },
        TableHookOp::MarkConnections {
            pairs: &CONNECTIONS_207D10,
        },
        TableHookOp::ClearEntryWord { index: 0x2A },
    ]
}

/// Where `0xEFBD0` places an effect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EffectPosition {
    Fixed([f32; 3]),
    /// The object position (`+0x64..+0x6C`).
    ObjectPosition(ObjectRef),
}

/// Arguments of `0xEFBD0(kind, extra, position, velocity, source, owner, seconds, _, sound)` (MEASURED call shape; the field roles are INFERRED):
/// the entry (`0xDC4A0(0x17)`) gets `kind` at `+4`, `extra` at `+0x1C0`, position at `+0x1C` (plus `velocity * 0.1` when a velocity is given),
/// velocity at `+0x180` (default (0, 1, 0)), `owner` at `+0xA0`, `source` at `+0x108`/`+0x10C`, timer `+0x98 = -seconds`; a timer at or above
/// zero starts the entry (`0xEDD40`), below zero it only gets flag `0x800000`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectSpawn {
    pub kind: u32,
    pub extra: u32,
    pub position: EffectPosition,
    pub velocity: Option<[f32; 3]>,
    pub source: Option<ObjectRef>,
    pub owner: Option<ObjectRef>,
    pub seconds: f32,
    pub sound: bool,
}

/// Position and velocity stored by `0xEFBD0`: `position + velocity * 0.1` (or the position alone) and the velocity (default (0, 1, 0)).
pub fn effect_entry_vectors(
    position: [f32; 3],
    velocity: Option<[f32; 3]>,
) -> ([f32; 3], [f32; 3]) {
    match velocity {
        Some(v) => (
            [
                position[0] + v[0] * 0.1,
                position[1] + v[1] * 0.1,
                position[2] + v[2] * 0.1,
            ],
            v,
        ),
        None => (position, [0.0, 1.0, 0.0]),
    }
}

/// Sound request chosen by `0xEFBD0` for a kind when the sound flag is set.
#[derive(Clone, Debug, PartialEq)]
pub enum EffectSound {
    None,
    /// `0x21A40(id, ticks, range, position)`: positional request with a range float.
    Ranged {
        id: u32,
        ticks: i32,
        range: f32,
    },
    /// `0x219E0(id, ticks, position)`.
    Plain {
        id: u32,
        ticks: i32,
    },
    /// Kinds `0x28` and `0x2F` call `0xE0080()` (no arguments, body not read here).
    Call0xE0080,
    /// The sound `0xE0080` fires once its rate limit allows (`0x46A00(0xCFB, 0.1, 0.95, 1.05)`).
    Throttled(super::callees_t1846::ThrottledSound),
}

/// The sound plan of `0xEFBD0` (jump table `0xEFF3C`, byte map `0xEFF5C` for kinds `0xB..=0x30`; anything else takes the default).
/// `seconds` is the float argument, `random` the raw output of `0x153B40` for the kinds that draw one. Frame rate 60.
pub fn effect_sounds(kind: u32, seconds: f32, random: u32) -> Vec<EffectSound> {
    let ticks = (60.0 * seconds) as i32;
    match kind {
        0xB | 0xC => vec![
            EffectSound::Ranged {
                id: 0x1A6,
                ticks,
                range: 75.0,
            },
            EffectSound::Ranged {
                id: 0x1A7,
                ticks: ((seconds + 1.0) * 60.0) as i32,
                range: 75.0,
            },
        ],
        0xD | 0xE => vec![EffectSound::Ranged {
            id: 0x1E6,
            ticks,
            range: 75.0,
        }],
        0xF => vec![],
        0x10 => vec![EffectSound::Plain { id: 0xC33, ticks }.into_ranged(150.0)],
        0x28 | 0x2F => vec![EffectSound::Call0xE0080],
        0x2C..=0x2E => vec![EffectSound::Plain { id: 0x631, ticks }],
        0x30 => {
            let fraction = random as f64 * f64::from(2.328_306_4e-10_f32);
            let pick = (fraction * 2.0 + 0.5) as u32;
            vec![EffectSound::Plain {
                id: 0x5F3 + pick,
                ticks,
            }]
        }
        _ => vec![EffectSound::Plain {
            id: 0xC33 + random % 6,
            ticks,
        }],
    }
}

/// Replace [`EffectSound::Call0xE0080`] with the sound `0xE0080` plays (T1846): [`EffectSound::Throttled`] when the rate
/// limit `[0x737C5C]` allows it (the stamp is updated), nothing otherwise. Other sounds pass through.
pub fn resolve_effect_sounds(
    sounds: Vec<EffectSound>,
    clock: i32,
    stamp: &mut i32,
) -> Vec<EffectSound> {
    sounds
        .into_iter()
        .filter_map(|sound| match sound {
            EffectSound::Call0xE0080 => {
                super::callees_t1846::throttled_sound(clock, stamp).map(EffectSound::Throttled)
            }
            other => Some(other),
        })
        .collect()
}

/// Group count of `0x33FCD0`: `max(0x33AD50(), 0x33F770())`, the argument of [`group_setup`] (T1846 wiring of the two callees).
pub fn group_setup_for_table(
    points: &[PointEntry],
    entries: &[super::callees_t1846::GroupEntry],
    max_region_key: u32,
) -> Vec<GroupSetup> {
    group_setup(
        points,
        super::callees_t1846::group_array_size(entries, max_region_key),
    )
}

impl EffectSound {
    fn into_ranged(self, range: f32) -> EffectSound {
        match self {
            EffectSound::Plain { id, ticks } => EffectSound::Ranged { id, ticks, range },
            other => other,
        }
    }
}

/// `0x2284D0` (mode 5, called from `0x228870` between the slot poll and the level hook): a proximity beeper. `others` are the
/// positions of the net slot players whose bit is in the team mask `[0x7A2BC8]`, `locals` those of the other players that have a live
/// controller (`+0x150` non-null and its word `+4` not -1); both lists hold at most 32 entries. `accumulator` is `[0x75C154]` (only this
/// function reads or writes it, MEASURED, so it starts at 0 and is never reset), `step` the float `[0x4E794C]`.
/// Returns the new accumulator and whether sound `0x1566` is submitted (then the accumulator gets `+0.1` afterwards).
pub fn proximity_beep(
    others: &[[f32; 3]],
    locals: &[[f32; 3]],
    accumulator: f32,
    step: f32,
) -> (f32, bool) {
    if others.is_empty() || locals.is_empty() {
        return (accumulator, false);
    }
    let mut nearest = 2500.0_f32;
    for a in locals {
        for b in others {
            let (dx, dy, dz) = (a[0] - b[0], a[2] - b[2], a[1] - b[1]);
            let distance = dz * dz + dy * dy + dx * dx;
            if distance < nearest {
                nearest = distance;
            }
            if nearest <= 4.0 {
                nearest = 4.0;
            }
        }
    }
    let rate = 1.0 - (f64::from(nearest).sqrt() - 2.0) * f64::from(0.02_f32);
    let next = (f64::from(accumulator) - rate * (1.0 / 60.0) * f64::from(step)) as f32;
    if next > 0.0 {
        (next, false)
    } else {
        (next + 0.1, true)
    }
}

/// Pass 1 of member types 8 and 9 (`0x23237D`): one bit per matching net slot entry (table `[0x732E60]`, stride 0xD8, count `[0x732E64]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetSlotEntry {
    /// Byte `[entry + 0x18 -> +1] & 1`.
    pub active: bool,
    /// `[entry + 0x14]`, compared with the class key of the member.
    pub class_key: u32,
    /// `[entry + 0xC]`, compared with the record value `0x2F7990` returns.
    pub value: u32,
    /// `[object + 0x28] & 2`.
    pub flag2: bool,
}

/// Bits of the pass 1 word for a type 8/9 member before it is shifted by the running counter: entries with `active`, the class key
/// and value equal; the match index increments for every matching entry (also those with flag 2, which contribute no bit). The member
/// width is 1 whatever the number of matches (MEASURED quirk, see the doc).
pub fn type89_pass1_bits(entries: &[NetSlotEntry], class_key: u32, value: u32) -> u32 {
    let mut bits = 0;
    let mut index = 0u32;
    for entry in entries {
        if entry.active && entry.class_key == class_key && entry.value == value {
            if !entry.flag2 && index < 32 {
                bits |= 1 << index;
            }
            index += 1;
        }
    }
    bits
}

/// Node of the script context (stride 0xB4 at `[struct + 0x130]`) as `0x33F870` sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptNode {
    /// Dword `+8`: node type, selects the predicate row of `0x52EDA8`.
    pub kind: u32,
    /// Dword `+0xAC`: the group the node belongs to.
    pub group: u32,
}

/// Group record (stride 0x220 at `[struct + 0x138]`): `node` is the first dword, `members` the node indices (`+0x54`, count `+0x50`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupRecord {
    pub node: usize,
    pub members: Vec<usize>,
}

/// Predicate row of `0x52EDA8` (stride 12, 8 rows): node kind -> `(function a, predicate)`. Kinds 0 and 7 use `0x33FB50`/`0x33B5B0`,
/// 1 and 2 `0x33B100`/`0x35DE60`, 3 and 4 `0x33B210`/`0x33B5B0`, 8 and 9 `0x33B290`/`0x33B5B0`. A kind outside the table makes the image call a
/// null slot (`[8]` of address 0): not modelled, [`collect_group_members`] returns `None` for it.
pub const NODE_KIND_PREDICATES: [(u32, u32, u32); 8] = [
    (0, 0x33FB50, 0x33B5B0),
    (1, 0x33B100, 0x35DE60),
    (2, 0x33B100, 0x35DE60),
    (3, 0x33B210, 0x33B5B0),
    (4, 0x33B210, 0x33B5B0),
    (7, 0x33FB50, 0x33B5B0),
    (8, 0x33B290, 0x33B5B0),
    (9, 0x33B290, 0x33B5B0),
];

/// `0x33F870(group, out)`: for every group record whose node's group (`+0xAC`) equals `group`, run the predicate of each member node's kind
/// and collect the node indices that pass. Returns the nodes (the count is the return value; `out == NULL` only counts).
/// `predicate(kind_predicate_address, node_index)` is the world's answer.
pub fn collect_group_members(
    nodes: &[ScriptNode],
    groups: &[GroupRecord],
    group: u32,
    predicate: &mut dyn FnMut(u32, usize) -> bool,
) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    for record in groups {
        if nodes.get(record.node)?.group != group {
            continue;
        }
        for &member in &record.members {
            let kind = nodes.get(member)?.kind;
            let row = NODE_KIND_PREDICATES.iter().find(|row| row.0 == kind)?;
            if predicate(row.2, member) {
                out.push(member);
            }
        }
    }
    Some(out)
}

/// Entry of the point list array (`[0x78CC24]`, stride 0x20) as `0x33C300` / `0x33C370` read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointEntry {
    /// Dword `+4`: the point id.
    pub id: u32,
    /// Dword `+8`: flags (`0x80` = reward point A, `0x100` = reward point B).
    pub flags: u32,
    /// Byte `+0x1C`: the group the point belongs to (0 = none).
    pub group: u8,
}

/// `0x33C300(group)`: true when the group has at least one point with flag `0x80` and one with flag `0x100`.
pub fn group_has_both_point_kinds(points: &[PointEntry], group: u32) -> bool {
    let mut low = false;
    let mut high = false;
    for point in points {
        if u32::from(point.group) == group {
            low |= point.flags & 0x80 != 0;
            high |= point.flags & 0x100 != 0;
        }
    }
    low && high
}

/// The two short lists `0x33C370(group)` writes into the group record (`+0x20` and `+0x40`, 16 words each): the ids of up to 16 points of
/// the group with flag `0x80` (first list) and with flag `0x100` (second list), in table order, 16 bit each. Groups are 1 based here, as in
/// the image. The loop stops scanning at the 17th point of a group (`ecx >= 16` check happens before the group test).
pub fn group_point_short_lists(points: &[PointEntry], group: u32) -> (Vec<u16>, Vec<u16>) {
    let pick = |mask: u32| {
        let mut list = Vec::new();
        for point in points {
            if point.group < 1 {
                continue;
            }
            if list.len() >= 16 {
                break;
            }
            if u32::from(point.group) == group && point.flags & mask != 0 {
                list.push(point.id as u16);
            }
        }
        list
    };
    (pick(0x80), pick(0x100))
}

/// One 0x60-byte group record of `[struct + 0xE4]` after `0x33FD90`: `+4` = group index (0 based), `+8..+0x14` = 0, 1, 2, 3 (the four
/// reward/progress phase ids, `+0x14` = 3 marks the final group for `0x33AF40`), `+0x20` / `+0x40` the short lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupSetup {
    pub index: u32,
    pub phases: [u32; 4],
    pub low_points: Vec<u16>,
    pub high_points: Vec<u16>,
}

/// `0x33FD90` for the groups `1..=group_count` (mode 3 only; `0x33FCD0` first sizes the arrays as `max(0x33AD50(), 0x33F770())` groups):
/// for every group the member list is built by `0x33F870` when [`group_has_both_point_kinds`] holds (via `0x33AE50`, which resolves each member's
/// tag through `0x23FF20` and the first dword `[0x78CD30 + 0xB4*i]` match, not modelled), the record fields are set and the point short lists
/// are filled (`0x33C370`; `0x33BFA0` additionally when `[0x7DE402] > 0`, body ported in T1846 batch 8 as `group_paths::update_area_paths`). The closing `0x33AF40` walks the records of
/// `[0x74C36C]` whose phase word `+0x14` is not 3, sets the 4C slot `[record +4]` to status 5 and overwrites the record with the next one whose
/// `+0x14` is 3; since this function has just written 3 into every record, that walk finds nothing to do unless other code changes the word
/// (MEASURED flow, so slots do not depend on it).
pub fn group_setup(points: &[PointEntry], group_count: u32) -> Vec<GroupSetup> {
    (1..=group_count)
        .map(|group| {
            let (low_points, high_points) = if group_has_both_point_kinds(points, group) {
                group_point_short_lists(points, group)
            } else {
                (Vec::new(), Vec::new())
            };
            GroupSetup {
                index: group - 1,
                phases: [0, 1, 2, 3],
                low_points,
                high_points,
            }
        })
        .collect()
}

pub fn level1b_hook(call: TableHookCall) -> Vec<TableHookOp> {
    let finished = call.event == EVENT_FINISHED;
    let finished_bit = call.state & FINISHED_BIT != 0;
    match call.slot {
        0 if finished => vec![
            TableHookOp::Destroy(ObjectRef::Tag(TAG_DOR1)),
            TableHookOp::MarkConnections {
                pairs: &CONNECTIONS_221EC0,
            },
            TableHookOp::Sound { id: 0x687 },
        ],
        1 if finished => vec![TableHookOp::Destroy(ObjectRef::Tag(tag(2)))],
        1 if call.event <= 1 && !finished_bit => {
            vec![TableHookOp::HudMessageAllPlayers { text_id: 0x11D8 }]
        }
        2 if finished => vec![TableHookOp::Destroy(ObjectRef::Tag(tag(3)))],
        3 if finished => vec![
            TableHookOp::Destroy(ObjectRef::Tag(tag(5))),
            TableHookOp::Destroy(ObjectRef::Tag(tag(4))),
            TableHookOp::KillCredited(ObjectRef::SlotLookup {
                slot: 3,
                id: 0xC3,
                sub: 0,
            }),
            TableHookOp::Sound { id: 0x688 },
            TableHookOp::MarkConnections {
                pairs: &CONNECTIONS_221F20,
            },
        ],
        4 if finished => helper_222050(ObjectRef::SlotLookup {
            slot: 4,
            id: 0xC4,
            sub: 0,
        }),
        _ => Vec::new(),
    }
}

/// `0x225A50`, level 0x3B: slot 1 with index 0 to 5 and slot 2 with index 0 to 3 score the watched actor (kill credited).
pub fn level3b_hook(call: TableHookCall) -> Vec<TableHookOp> {
    match call.slot {
        1 if call.event < 6 => vec![TableHookOp::KillCredited(ObjectRef::SlotLookup {
            slot: 1,
            id: 0xCF,
            sub: call.event,
        })],
        2 if call.event < 4 => vec![TableHookOp::KillCredited(ObjectRef::SlotActor {
            slot: 2,
            index: call.event,
        })],
        _ => Vec::new(),
    }
}

/// `0x208030`, level 0x1D (props "WALL" `0x57414C4C`, "DOR1" and "DOR6", "PRWL" `0x5052574C`).
pub fn level1d_hook(call: TableHookCall) -> Vec<TableHookOp> {
    let finished = call.event == EVENT_FINISHED;
    let finished_bit = call.state & FINISHED_BIT != 0;
    match call.slot {
        0 if finished => vec![TableHookOp::Effect {
            object: ObjectRef::Tag(0x5741_4C4C),
            kind: 0x1C,
            with_sound: Some(0x653),
        }]
        .into_iter()
        .chain(helper_207d10(false))
        .collect(),
        0 if call.event == 0 => vec![
            TableHookOp::Effect {
                object: ObjectRef::Argument,
                kind: 1,
                with_sound: None,
            },
            TableHookOp::Destroy(ObjectRef::Argument),
        ],
        1 if finished => vec![
            TableHookOp::ClearFlags {
                tag: TAG_DOR1,
                mask: 2,
            },
            TableHookOp::SetFlags {
                tag: TAG_DOR1,
                mask: 0x400,
            },
        ],
        1 if call.event <= 1 && !finished_bit => {
            vec![TableHookOp::HudMessageAllPlayers { text_id: 0x11E7 }]
        }
        3 if finished => {
            let pick = (u32::from(call.state) >> 10 & 0x1F) % 5;
            vec![
                TableHookOp::ClearFlags {
                    tag: tag(6),
                    mask: 2,
                },
                TableHookOp::SetFlags {
                    tag: tag(6),
                    mask: 0x400,
                },
                TableHookOp::PlacePropsAtSpawn {
                    record_id: SLOT3_SPAWN_IDS[pick as usize],
                },
            ]
        }
        4 if finished => vec![TableHookOp::SetAnimationState {
            object: ObjectRef::SlotLookup {
                slot: 4,
                id: 0x85,
                sub: 0,
            },
            state: 0x5B,
            rate: 1.0,
        }],
        _ => Vec::new(),
    }
}

/// Dispatch of the table hook chosen for a level (`0x232EA0`); `None` for the no-op and the Map Maker hook (see [`map_maker_hook`]).
pub fn run_table_hook(hook: ObjectiveHook, call: TableHookCall) -> Vec<TableHookOp> {
    match hook {
        ObjectiveHook::Level1B => level1b_hook(call),
        ObjectiveHook::Level1D => level1d_hook(call),
        ObjectiveHook::Level3B => level3b_hook(call),
        ObjectiveHook::Level30NoOp | ObjectiveHook::MapMaker => Vec::new(),
    }
}

/// What `0x33F980` reads of a group (list `[0x7789F8][group]`).
pub trait GroupWorld {
    /// Number of members of the group.
    fn member_count(&self, group: usize) -> usize;
    /// Group node of the member: `(status +0x18 == 1, dirty +0x20 != 0)`. Calls the member method `+0x10` when `refresh`.
    fn node_status(&mut self, group: usize, member: usize) -> (bool, bool);
    fn clear_dirty(&mut self, group: usize, member: usize);
    /// Run the member's method `[node+0x10]` and return whether the status is now 1.
    fn refresh_member(&mut self, group: usize, member: usize) -> bool;
    /// The group target (`[group +8]`): non-zero means "any member completes the group".
    fn any_member_suffices(&self, group: usize) -> bool;
}

/// What `0x33F980(group)` asks of the script layer after counting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupProgress {
    pub completed: usize,
    pub total: usize,
    pub finished: bool,
    /// `0x33BD50(g+1)` / `0x33C2B0(g+1)` were requested while draining dirty nodes (only when `[0x7DE402] > 0`).
    pub rebuild_point_lists: usize,
}

/// `0x33F980` for group `group` (the first table entry whose slot is not finished, `0x232100`): count the completed members
/// (draining dirty flags and refreshing members that are not yet complete) and decide whether the group has finished. The
/// caller then runs `0x33B040(group)` (script rule actions) and, when `group + 1 != [0x78CCE8]` and `[0x7DE402] > 0`,
/// `0x33C2B0(0x33F7D0(group))`. MEASURED control flow; the image dereferences a null node when a member id has no group node.
pub fn group_progress(
    world: &mut dyn GroupWorld,
    group: usize,
    point_list_flag: bool,
) -> GroupProgress {
    let total = world.member_count(group);
    let mut completed = 0;
    let mut rebuild = 0;
    for member in 0..total {
        let (_, dirty) = world.node_status(group, member);
        if dirty {
            world.clear_dirty(group, member);
            if point_list_flag {
                rebuild += 1;
            }
        }
        let done = if world.node_status(group, member).0 {
            true
        } else {
            world.refresh_member(group, member)
        };
        if done {
            completed += 1;
        }
    }
    let finished = if world.any_member_suffices(group) {
        completed != 0
    } else {
        completed == total
    };
    GroupProgress {
        completed,
        total,
        finished,
        rebuild_point_lists: rebuild,
    }
}

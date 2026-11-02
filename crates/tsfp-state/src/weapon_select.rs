// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable selection policy recovered from 0x1C4400, 0x1BF140 and 0x1BA410.
//! Tier 1 on synthetic inputs against translated roots; see docs/t2193-weapon-select.md.
//! This module owns decisions, not weapon-instance construction or view sway.
// The translated roots remain the production reference while host admission is pending.
// portreg: mirrors=tsfp-ports/c2r/player_wired
// portreg: mirrors=tsfp-ports/c2r/t1925_weapons
// portreg: mirrors=tsfp-ports/inventory_select
// portreg: mirrors=tsfp-ports/weapons_lane2
use crate::inventory::{AMMO_TYPES, ENTRY_COUNT, FLAG_HIDDEN_BY_X2, Inventory, WeaponEntries};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Next,
    Previous,
}

/// Slot means an entry index in the original 70-entry inventory table, not a key code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    SelectSlot(usize),
    Cycle(Direction),
    /// Original within-group variant selection (not the weapon's firing trigger).
    AltMode(Direction),
    CycleThrowable,
}

/// Explicit A/B choice, defaulting to the retained reference until host traces validate this slice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionPreset {
    #[default]
    Reference,
    ReadableOriginal,
}

/// Fields consumed by the selection branch of 0x1C4400. Other controller duties stay outside.
#[derive(Clone, Copy, Debug, Default)]
pub struct ControllerState {
    pub player_index: i32,
    pub match_state: u32,
    pub blocked_game_state: bool,
    pub global_input_blocked: bool,
    pub player_flags: u32,
    pub actor_flags: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerAction {
    None,
    ClearCurrent,
    ReadButtons,
}

/// Verification Tier 1: selection projection against both translated controllers, synthetic inputs.
// portreg: va=0x001c4400 tier=1
pub fn controller_action(s: ControllerState) -> ControllerAction {
    if s.player_index < 0 || s.match_state == 8 {
        return ControllerAction::None;
    }
    if s.blocked_game_state {
        return ControllerAction::ClearCurrent;
    }
    if s.global_input_blocked || s.player_flags & 0x4000 != 0 || s.actor_flags & 0x480e == 0 {
        return ControllerAction::None;
    }
    ControllerAction::ReadButtons
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ButtonBindings {
    pub next: u32,
    pub previous: u32,
    pub alt_next: u32,
    pub alt_previous: u32,
    pub throwable: u32,
}

/// Neighbouring systems provide these effects. `equip` must perform the complete 0x1BA130
/// contract (order representative and hand instances), then refresh the owned inventory.
/// Polling is deliberately repeated in original order; a callback may change the next poll.
pub trait SelectionEffects {
    fn buttons(&mut self) -> u32;
    /// Re-evaluate after group switches, as the original reads the newly equipped record.
    fn variant_locked(&mut self, initial: bool) -> bool {
        initial
    }
    fn candidate(&mut self, inventory: &Inventory, request: Request) -> u32;
    fn equip(&mut self, inventory: &mut Inventory, entry: u32);
    fn count_change(&mut self);
    fn group_changed(&mut self);
    fn variant_changed(&mut self, direction: Direction);
    fn variant_unchanged(&mut self);
    fn throwable_changed(&mut self, inventory: &mut Inventory, entry: u32);
}

/// Extension point shared by original bindings and direct slot requests. Direct selection
/// validates through the same gate; it does not poke current/previous or build partial instances.
/// Verification Tier 0 for direct-slot extension; cycle effects covered by the Tier 1 harness.
pub fn apply_request<E: SelectionEffects>(
    inventory: &mut Inventory,
    request: Request,
    effects: &mut E,
    mut selectable: impl FnMut(&Inventory, usize) -> bool,
) -> bool {
    let entry = match request {
        Request::SelectSlot(slot) => {
            if slot >= ENTRY_COUNT || !selectable(inventory, slot) {
                return false;
            }
            slot as u32
        }
        _ => effects.candidate(inventory, request),
    };
    if request == Request::CycleThrowable {
        effects.throwable_changed(inventory, entry);
        return true;
    }
    let old = inventory.current;
    effects.equip(inventory, entry);
    match request {
        Request::Cycle(_) | Request::SelectSlot(_) => effects.group_changed(),
        Request::AltMode(direction) => {
            if inventory.current == old {
                effects.variant_unchanged();
            } else {
                effects.variant_changed(direction);
            }
        }
        Request::CycleThrowable => unreachable!(),
    }
    true
}

/// 0x1BF140: next, previous, alt-next, alt-previous, throwable; simultaneous buttons
/// execute sequentially. A locked variant still permits group and throwable cycling.
/// Verification Tier 1: translated 0x1BF140, synthetic inputs and scripted neighbour effects.
// portreg: va=0x001bf140 tier=1
pub fn cycle_buttons<E: SelectionEffects>(
    inventory: &mut Inventory,
    bindings: ButtonBindings,
    variant_locked: bool,
    effects: &mut E,
) {
    let mut variant_locked = variant_locked;
    for (index, (mask, request)) in [
        (bindings.next, Request::Cycle(Direction::Next)),
        (bindings.previous, Request::Cycle(Direction::Previous)),
        (bindings.alt_next, Request::AltMode(Direction::Next)),
        (bindings.alt_previous, Request::AltMode(Direction::Previous)),
        (bindings.throwable, Request::CycleThrowable),
    ]
    .into_iter()
    .enumerate()
    {
        if index == 2 {
            variant_locked = effects.variant_locked(variant_locked);
        }
        if variant_locked && matches!(request, Request::AltMode(_)) {
            continue;
        }
        if effects.buttons() & mask == 0 {
            continue;
        }
        if request != Request::CycleThrowable {
            effects.count_change();
        }
        apply_request(inventory, request, effects, |_, _| false);
    }
}

/// Weapon-record ammo metadata (+0x14 primary, +0x1C secondary). Record -1 means no
/// weapon and contributes the original sentinel total of one.
#[derive(Clone, Copy, Debug, Default)]
pub struct AmmoRecord {
    pub primary: usize,
    pub secondary: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct LoadedHand {
    pub record: Option<AmmoRecord>,
    pub rounds: [u16; AMMO_TYPES],
}
impl Default for LoadedHand {
    fn default() -> Self {
        Self {
            record: None,
            rounds: [0; AMMO_TYPES],
        }
    }
}

/// Non-inventory state formerly obtained from global/actor tables and helper calls.
/// `records` follows WeaponEntry.words[3]/[4] (first/second hand respectively).
pub trait SelectabilityContext {
    fn local_player(&self) -> bool;
    fn hands_disabled(&self) -> bool;
    fn record(&self, index: i32) -> Option<AmmoRecord>;
    fn loaded_hands(&self) -> [LoadedHand; 2];
    fn owned_x2(&mut self, inventory: &Inventory, entry: usize) -> bool;
    fn ammo_exception(&mut self, inventory: &Inventory, entry: usize) -> bool;
    fn detonator_available(&mut self, inventory: &Inventory) -> bool;
    fn hand_transition_active(&self) -> bool;
    fn left_secondary_required(&self) -> bool;
}

fn ammo_total(inventory: &Inventory, record: Option<AmmoRecord>, hands: &[LoadedHand; 2]) -> i32 {
    let Some(record) = record else {
        return 1;
    };
    let secondary = record.secondary != 0 && record.secondary != record.primary;
    let mut total = inventory.ammo[record.primary] as i16 as i32;
    if secondary {
        total += inventory.ammo[record.secondary] as i16 as i32;
    }
    for hand in hands {
        let metadata = hand.record.unwrap_or_default();
        // The original initialises absent-hand primary metadata to zero, rounds to zero.
        if record.primary == metadata.primary && hand.record.is_some() {
            total += hand.rounds[metadata.primary] as i16 as i32;
        }
        if secondary
            && record.secondary == metadata.secondary
            && metadata.secondary != 0
            && metadata.secondary != metadata.primary
        {
            total += hand.rounds[metadata.secondary] as i16 as i32;
        }
    }
    total
}

/// 0x1BA410. Signed reserve/loaded words and nonzero (not positive) totals are intentional.
/// `ignore_ammo` changes exception policy; it does not bypass ownership or the hand-transition gate.
/// Verification Tier 1: translated 0x1BA410, synthetic inputs and scripted helper results.
// portreg: va=0x001ba410 tier=1
pub fn selectable<C: SelectabilityContext>(
    inventory: &Inventory,
    table: &WeaponEntries,
    entry: usize,
    ignore_ammo: bool,
    context: &mut C,
) -> bool {
    if entry >= ENTRY_COUNT {
        return false;
    }
    let local = context.local_player();
    if entry == 0 {
        return !local;
    }
    if entry == 1 {
        return local && !context.hands_disabled() && inventory.owned[0x38] == 0;
    }
    let weapon = &table.entries[entry];
    if weapon.flags() & 0x10 != 0 || (local && weapon.flags() & 2 != 0) {
        return false;
    }
    if local && weapon.flags() & FLAG_HIDDEN_BY_X2 != 0 && context.owned_x2(inventory, entry) {
        return false;
    }
    if !inventory.owns(entry) {
        return false;
    }
    let hands = context.loaded_hands();
    let left = ammo_total(inventory, context.record(weapon.left_record()), &hands);
    let right = ammo_total(inventory, context.record(weapon.record()), &hands);
    let exception = if ignore_ammo {
        match entry {
            0x2b | 0x2c | 0x2e | 0x2f => false,
            0x2d => context.detonator_available(inventory),
            _ => true,
        }
    } else {
        context.ammo_exception(inventory, entry)
    };
    if (left == 0 || right == 0) && !exception {
        return false;
    }
    if context.hand_transition_active() {
        let Some(current) = table.entries.get(inventory.current as usize) else {
            return false;
        };
        if current.record() == weapon.record() && current.left_record() != weapon.left_record() {
            return false;
        }
    }
    if local && entry == 0x31 && context.left_secondary_required() {
        let Some(record) = context.record(weapon.left_record()) else {
            return false;
        };
        if inventory.ammo[record.secondary] == 0 {
            return false;
        }
    }
    true
}

/// A/B dispatch is explicit at the system boundary. The reference callback remains the
/// default; integrations can switch per player without changing tables or guest bytes.
/// Verification Tier 0: compile and unit checks; no host admission or trace evidence.
pub fn dispatch_request<E: SelectionEffects>(
    preset: SelectionPreset,
    inventory: &mut Inventory,
    request: Request,
    effects: &mut E,
    selectable: impl FnMut(&Inventory, usize) -> bool,
    reference: impl FnOnce(&mut Inventory, Request, &mut E) -> bool,
) -> bool {
    match preset {
        SelectionPreset::Reference => reference(inventory, request, effects),
        SelectionPreset::ReadableOriginal => apply_request(inventory, request, effects, selectable),
    }
}

/// Selection portion of the frame controller. Clearing current uses the owned inventory's
/// write-through setter; the rest of 0x1C4400 (sway, slot ticks) belongs to the caller.
/// Verification Tier 0 for write-through integration; controller_action carries Tier 1 evidence.
pub fn controller_selection<M: crate::GuestMemory, E: SelectionEffects>(
    inventory: &mut Inventory,
    memory: &mut M,
    state: ControllerState,
    bindings: ButtonBindings,
    variant_locked: bool,
    effects: &mut E,
) {
    match controller_action(state) {
        ControllerAction::None => {}
        ControllerAction::ClearCurrent => inventory.set_current(memory, 0),
        ControllerAction::ReadButtons => {
            cycle_buttons(inventory, bindings, variant_locked, effects)
        }
    }
}

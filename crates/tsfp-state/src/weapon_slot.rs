// SPDX-License-Identifier: GPL-3.0-or-later
//! T2182: opt-in direct weapon requests. Contract: docs/t2182-weapon-slot.md.
use crate::inventory::{ENTRY_COUNT, FLAG_LEFT, Inventory, WeaponEntries};
use crate::weapon_select::{
    ControllerAction, ControllerState, Request, SelectabilityContext, SelectionEffects,
    apply_request, controller_action, selectable,
};

/// New keyboard policy has no original direct-key equivalent. Keep it off until admitted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DirectSlotPreset {
    #[default]
    Disabled,
    SelectableOrder,
}

/// A user-facing number, deliberately distinct from an inventory entry index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponSlot(u8);
impl WeaponSlot {
    pub fn new(number: u8) -> Option<Self> {
        (1..=9).contains(&number).then_some(Self(number))
    }
    pub fn number(self) -> u8 {
        self.0
    }
}

/// Resolve the Nth selectable non-hands entry in runtime cycle order. Duplicate and
/// invalid table indices cannot consume slots. Left companions and grenade entries
/// are excluded even if a neighbour's selectability gate would admit them.
/// Tier 0 design; gate projection has synthetic translated-reference Tier 1 evidence.
pub fn resolve_slot(
    inventory: &Inventory,
    table: &WeaponEntries,
    order: &[u32],
    slot: WeaponSlot,
    mut gate: impl FnMut(&Inventory, usize) -> bool,
) -> Option<usize> {
    let mut seen = [false; ENTRY_COUNT];
    let mut remaining = slot.number();
    for &raw in order {
        let entry = raw as usize;
        if entry >= ENTRY_COUNT || seen[entry] {
            continue;
        }
        seen[entry] = true;
        if entry <= 1
            || (0x28..=0x2a).contains(&entry)
            || table.entries[entry].flags() & FLAG_LEFT != 0
            || !gate(inventory, entry)
        {
            continue;
        }
        remaining -= 1;
        if remaining == 0 {
            return Some(entry);
        }
    }
    None
}

/// Consume one owning player's mailbox request before ordinary cycle handling.
/// Disabled, controller-blocked, malformed and unavailable requests have no effects.
/// The caller consumes the mailbox even on rejection; this API does not queue/retry.
/// Runtime tables/context belong to the inventory adapter, never the input router.
/// `equip` must implement the complete SelectionEffects contract, then refresh inventory.
/// No direct writes to current/previous, ownership or ammo occur here.
pub fn request_slot<E: SelectionEffects, C: SelectabilityContext>(
    preset: DirectSlotPreset,
    state: ControllerState,
    inventory: &mut Inventory,
    table: &WeaponEntries,
    order: &[u32],
    number: u8,
    context: &mut C,
    effects: &mut E,
) -> bool {
    if preset == DirectSlotPreset::Disabled
        || controller_action(state) != ControllerAction::ReadButtons
    {
        return false;
    }
    let Some(slot) = WeaponSlot::new(number) else {
        return false;
    };
    let Some(entry) = resolve_slot(inventory, table, order, slot, |inventory, entry| {
        selectable(inventory, table, entry, true, context)
    }) else {
        return false;
    };
    // No state-changing callback intervenes between validation and the extension point.
    // Reusing the result avoids polling context helpers twice for the chosen entry.
    apply_request(
        inventory,
        Request::SelectSlot(entry),
        effects,
        |_, candidate| candidate == entry,
    )
}

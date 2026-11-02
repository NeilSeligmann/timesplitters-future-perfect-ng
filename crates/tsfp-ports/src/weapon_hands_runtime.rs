// SPDX-License-Identifier: GPL-3.0-or-later
//! T2376 opt-in adapters from the live guest ABI to the Rust-owned hand state.
//!
//! The default-off nested reload/unload path seeds a Rust-owned player store once, then retains
//! that state across calls. Other hand edges still use the legacy guest snapshot adapter. The
//! current guest write-through is a hybrid compatibility seam; invalid ABI inputs decline so the
//! existing reference route remains usable.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
    object_pool::ObjectId,
    scene_membership::SceneMembership,
    weapon_hands::{
        AmmoType, HandAmmoState, HandPresentation, HandRealiseError, HandRealiseOutcome,
        HandRealisePairOutcome, HandRealiseRequest, HandSide, ReloadSession as HandReloadSession,
        WeaponAmmoRecord, WeaponRecordId,
    },
    weapon_hands_inventory::{AMMO_TYPES, Inventory},
    weapon_inventory::{
        AMMO_TYPE_COUNT, AmmoAutoSelectionContext, AmmoReservePolicy, ENTRY_COUNT,
        FireModeSelection, GrantSavedRounds, InventoryEntryDefinition, PickupAcquisitionBatch,
        PickupAmmoPair, ProfileWeaponInventory, ReloadSession, ReloadSessionAdvance,
        WeaponInventory, WeaponInventoryStore, WeaponObjectRetireError, WeaponPayloadSpawnError,
        WeaponPickupAmmo, WeaponPickupOutcome, WeaponPlayerHandle, WeaponPlayerId,
        WeaponSubsystemState,
    },
    weapon_payload::{
        C1950PayloadInputs, ShotAdmission, ShotAmmoSource, WeaponPayloadLifecycleError,
        WeaponPayloadPose,
    },
};

const RECORD_BASE_CELL: u32 = 0x4f9bac;
const RECORD_STRIDE: u32 = 0x29c;
const ENTRY_FLAGS_BASE: u32 = 0x4fa128;
const ENTRY_DATA_TABLE: u32 = 0x4fb190;
const ENTRY_DATA_STRIDE: u32 = 0x54;
const ENTRY_FLAGS_STRIDE: u32 = 0x3c;
const ENTRY_RECORD_OFFSET: u32 = 0x10;
const ENTRY_GROUP_OFFSET: u32 = 0x38;
// REA maps hand A at +0xa4 to left and hand B at +0x1d4 to right.
const HAND_LEFT_OFFSET: u32 = 0xa4;
const HAND_RIGHT_OFFSET: u32 = 0x1d4;
const HAND_RECORD: u32 = 0x30;
const HAND_LOADED: u32 = 0x38;
const HAND_SAVED: u32 = 0x90;
const HAND_COUNTERS: u32 = 0xf0;
const HAND_CHILD: u32 = 0xe8;
const HAND_RELOAD_TIMER: u32 = 0xf8;
const AMMO_TABLE: u32 = 0x4ea3f8;
const AMMO_ROW_STRIDE: u32 = 0xe0;
// Player-extension +0x28 collect latch (T2411 trace projection), separate from table flags.
const EXT_COLLECT_LATCH: u32 = 0x8;
// Fixed host-owned capacity; unsupported slots decline instead of retaining a guest address.
pub(crate) const WEAPON_PLAYER_SLOTS: usize = 16;
// T2411 core rust_trace projection: inventory flags/selections, ownership/equipped-once, reserve,
// two typed hand records, and the resolved fire-mode pair.
const WEAPON_TRACE_WORDS: usize = 378;
const GRANT_DIAGNOSTIC_WORDS: usize = 24;
const TRANSFER_DIAGNOSTIC_WORDS: usize = 32;
// Only roots with a readable typed-owner handler are selected by this switch. The other hand
// transitions stay on the lifted reference until their native state and presentation contracts
// are implemented; selecting their translated bodies was not sufficient evidence of a port.
const HAND_ROOTS: [u32; 4] = [0x1b6ec0, 0x1b7bc0, 0x1b9760, 0x1bf9e0];
// Player extensions and the measured weapon/entry tables are XBE data/heap objects (the player
// extension array begins at 0x007356d8). A low nonzero word is not a valid pointer for these
// objects. This is an inferred safety floor at the hybrid ABI edge, not an original-code range
// contract; decline such inputs so the caller can keep the retained route.
const MIN_GUEST_DATA_ADDRESS: u32 = 0x1_0000;

fn valid_guest_data_address(address: u32) -> bool {
    address >= MIN_GUEST_DATA_ADDRESS
}

fn valid_player_extension_address(address: u32) -> bool {
    valid_guest_data_address(address)
}

/// Decode only the ammo-row fields whose reserve-cap behavior is established by T2367. The
/// neighboring definition/resource payload remains owned by its own typed table view.
fn ammo_reserve_maximum<M: GuestMemory + ?Sized>(m: &M, ammo_index: usize) -> i32 {
    let row = AMMO_TABLE + ammo_index as u32 * AMMO_ROW_STRIDE;
    let policy = AmmoReservePolicy::from_row_fields(m.read32(row) as i32, m.read8(row + 8));
    if policy.is_frame_scaled() {
        let frames = m.read32(0x4e8cec) as i32;
        let scale = f32::from_bits(m.read32(0x478394));
        policy.maximum(frames, scale)
    } else {
        policy.maximum(0, 0.0)
    }
}

#[cfg(not(test))]
static ENABLED: AtomicBool = AtomicBool::new(false);

// Unit tests run on parallel threads: a test that turned the runtime on must not make unrelated
// unlocked tests (lane, slice and c2r wrapper tests that gate on `enabled()`) dispatch into the
// shared owned store and mutate it under a locked test (T3490 flake). The switch is per test thread;
// tests that touch the store itself still serialise on `RUNTIME_STORE_TEST_LOCK`.
#[cfg(test)]
std::thread_local! {
    static ENABLED_FOR_TEST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
static GRANT_DIAGNOSTICS: [AtomicU32; GRANT_DIAGNOSTIC_WORDS] =
    [const { AtomicU32::new(0) }; GRANT_DIAGNOSTIC_WORDS];
static TRANSFER_DIAGNOSTICS: [AtomicU32; TRANSFER_DIAGNOSTIC_WORDS] =
    [const { AtomicU32::new(0) }; TRANSFER_DIAGNOSTIC_WORDS];
static TRANSFER_DIAGNOSTIC_LOCK: AtomicBool = AtomicBool::new(false);

type LockedInventoryStore =
    tsfp_weapon_realization::owned::runtime::LockedInventoryStore<WEAPON_PLAYER_SLOTS>;

static OWNED_INVENTORIES: LockedInventoryStore = LockedInventoryStore::new();

/// Serialises unit tests that call `configure` or touch the process-wide `OWNED_INVENTORIES` store.
#[cfg(test)]
pub(crate) static RUNTIME_STORE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Take `RUNTIME_STORE_TEST_LOCK` ignoring poison: a test that panics while holding it must fail alone,
/// not turn every later test in the process into a `PoisonError` (the 2026-10-20 red main showed 1 real
/// failure as 5).
#[cfg(test)]
pub(crate) fn runtime_store_test_guard() -> std::sync::MutexGuard<'static, ()> {
    RUNTIME_STORE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// T3610: `tsfp-port-wire::leaf_wire` (a layer below this crate) reaches the runtime through this hook table.
static WIRE_HOOK: tsfp_port_wire::weapon_hands_hook::WeaponHandsHook =
    tsfp_port_wire::weapon_hands_hook::WeaponHandsHook {
        enabled,
        mark_equipped_once: mark_equipped_once_erased,
    };

fn mark_equipped_once_erased(m: &mut dyn GuestMemory, ext: u32, entry: usize) -> bool {
    mark_equipped_once(
        &mut tsfp_port_wire::weapon_hands_hook::ErasedMemory(m),
        ext,
        entry,
    )
}

pub fn configure(enabled: bool) {
    // Registered by the first `configure` (before that `enabled()` is false anyway, so the wiring layer sees no difference).
    tsfp_port_wire::weapon_hands_hook::register(&WIRE_HOOK);
    // Each explicit adapter session begins with fresh player lifetimes. Persistent host-owned
    // state is populated once at first attachment; it is never keyed by guest address.
    OWNED_INVENTORIES.with(|store| *store = WeaponSubsystemState::default());
    for counter in &GRANT_DIAGNOSTICS {
        counter.store(0, Ordering::Relaxed);
    }
    for word in &TRANSFER_DIAGNOSTICS {
        word.store(0, Ordering::Relaxed);
    }
    #[cfg(not(test))]
    ENABLED.store(enabled, Ordering::Release);
    #[cfg(test)]
    ENABLED_FOR_TEST.with(|flag| flag.set(enabled));
}

/// Attach typed callback and mission-marker state to a live native scene object.
pub fn attach_interaction_target(
    scene: &SceneMembership,
    target: ObjectId,
    state: crate::weapon_interaction::NativeInteractionTarget,
) -> Result<(), crate::weapon_interaction::InteractionTargetStoreError> {
    OWNED_INVENTORIES.with(|store| store.interaction_targets.attach(scene, target, state))
}

/// Retire one interaction target and its scene generation as one Rust-owned operation.
pub fn retire_interaction_target(
    scene: &mut SceneMembership,
    target: ObjectId,
) -> Result<
    crate::weapon_interaction::NativeInteractionTarget,
    crate::weapon_interaction::InteractionTargetStoreError,
> {
    OWNED_INVENTORIES.with(|store| store.interaction_targets.retire(scene, target))
}

/// Submit one native interaction against Rust-owned player, target, and scene generations.
///
/// The caller captures player and actor identity before dispatch and supplies native sender and
/// callback behavior. This boundary accepts no guest address or synthetic input.
pub fn submit_native_interaction<H: crate::weapon_interaction::NativeInteractionHandler>(
    scene: &mut SceneMembership,
    captured: crate::weapon_interaction::CapturedInteractionPlayer,
    target: ObjectId,
    handler: &mut H,
) -> Result<
    crate::weapon_interaction::InteractionSubmissionOutcome,
    crate::weapon_interaction::NativeInteractionError<H::Error>,
> {
    OWNED_INVENTORIES
        .with(|store| store.submit_native_interaction(scene, captured, target, handler))
}

pub fn enabled() -> bool {
    #[cfg(not(test))]
    {
        ENABLED.load(Ordering::Acquire)
    }
    #[cfg(test)]
    {
        ENABLED_FOR_TEST.with(|flag| flag.get())
    }
}

/// Read-only selection flag for the host trace adapter.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_weapon_hands_enabled() -> bool {
    enabled()
}

/// Serialize the current typed player inventory and hand state for T2411's read-only host trace
/// window. The fixed projection has no guest addresses and does not modify subsystem state.
/// Returns false until that player has attached an owner, allowing the host trace adapter to use
/// the documented one-time guest snapshot compatibility path for the initial sample.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_weapon_inventory_trace_snapshot(
    player_slot: u32,
    output: *mut u32,
    output_words: u32,
) -> bool {
    if output.is_null() || output_words < WEAPON_TRACE_WORDS as u32 {
        return false;
    }
    let output = unsafe { core::slice::from_raw_parts_mut(output, WEAPON_TRACE_WORDS) };
    OWNED_INVENTORIES.with(|store| {
        let Some(owner) = store.get(WeaponPlayerId::new(player_slot as u16)) else {
            return false;
        };
        encode_weapon_inventory_trace(owner, output)
    })
}

/// Snapshot adapter diagnostics in a separate host trace window. These words describe whether
/// the hybrid grant reducer accepted its ABI inputs; they are excluded from subsystem equality.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_weapon_inventory_adapter_diagnostics(
    output: *mut u32,
    output_words: u32,
) -> bool {
    if output.is_null() || output_words < GRANT_DIAGNOSTIC_WORDS as u32 {
        return false;
    }
    let output = unsafe { core::slice::from_raw_parts_mut(output, GRANT_DIAGNOSTIC_WORDS) };
    for (destination, source) in output.iter_mut().zip(&GRANT_DIAGNOSTICS) {
        *destination = source.load(Ordering::Relaxed);
    }
    output[0] = 1; // schema version
    true
}

/// Store one bounded 0x1B6EC0 transfer observation. The lifted C bridge and the typed Rust path
/// use this same host-only buffer, so the captured fields have one schema on both sides. Word 0
/// is the schema, word 1 is the event ordinal, and words 2..32 are the event payload.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_weapon_transfer_diagnostic_record(
    input: *const u32,
    input_words: u32,
) {
    if input.is_null() || input_words < TRANSFER_DIAGNOSTIC_WORDS as u32 {
        return;
    }
    let input = unsafe { core::slice::from_raw_parts(input, TRANSFER_DIAGNOSTIC_WORDS) };
    while TRANSFER_DIAGNOSTIC_LOCK
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        core::hint::spin_loop();
    }
    let ordinal = TRANSFER_DIAGNOSTICS[1]
        .load(Ordering::Relaxed)
        .wrapping_add(1);
    TRANSFER_DIAGNOSTICS[0].store(1, Ordering::Relaxed);
    TRANSFER_DIAGNOSTICS[1].store(ordinal, Ordering::Relaxed);
    for (destination, source) in TRANSFER_DIAGNOSTICS[2..].iter().zip(&input[2..]) {
        destination.store(*source, Ordering::Relaxed);
    }
    TRANSFER_DIAGNOSTIC_LOCK.store(false, Ordering::Release);
}

/// Read the last completed reload-transfer observation in the separate host trace window.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_weapon_transfer_diagnostics_snapshot(
    output: *mut u32,
    output_words: u32,
) -> bool {
    if output.is_null() || output_words < TRANSFER_DIAGNOSTIC_WORDS as u32 {
        return false;
    }
    while TRANSFER_DIAGNOSTIC_LOCK
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        core::hint::spin_loop();
    }
    let output = unsafe { core::slice::from_raw_parts_mut(output, TRANSFER_DIAGNOSTIC_WORDS) };
    for (destination, source) in output.iter_mut().zip(&TRANSFER_DIAGNOSTICS) {
        *destination = source.load(Ordering::Relaxed);
    }
    TRANSFER_DIAGNOSTIC_LOCK.store(false, Ordering::Release);
    true
}

fn record_transfer_observation(
    player_slot: u32,
    ext: u32,
    hand_address: u32,
    side: HandSide,
    record: WeaponAmmoRecord,
    controller_state: u32,
    saved_round_limit: bool,
    restrict_primary_to_one: bool,
    before_hand: &HandAmmoState,
    after_hand: &HandAmmoState,
    before_reserve: &[i16; AMMO_TYPE_COUNT],
    after_reserve: &[i16; AMMO_TYPE_COUNT],
    moved: i32,
) {
    let primary = record.primary.map_or(u32::MAX, |ammo| ammo.index() as u32);
    let secondary = record
        .secondary
        .map_or(u32::MAX, |ammo| ammo.index() as u32);
    let value = |cells: &[i16; AMMO_TYPE_COUNT], ammo: u32| {
        if ammo == u32::MAX {
            0
        } else {
            cells[ammo as usize] as i32 as u32
        }
    };
    let loaded = |hand: &HandAmmoState, ammo: u32| {
        if ammo == u32::MAX {
            0
        } else {
            hand.loaded[ammo as usize] as i32 as u32
        }
    };
    let side_code = match side {
        HandSide::Left => 0,
        HandSide::Right => 1,
    };
    let mut event = [0u32; TRANSFER_DIAGNOSTIC_WORDS];
    event[2] = 0x1b6ec0; // logical root; run switches identify the producing implementation.
    event[3] = 0;
    event[4] = ext;
    event[5] = player_slot;
    event[6] = hand_address;
    event[7] = side_code;
    event[8] = record.id.0 as u32;
    event[9] = primary;
    event[10] = secondary;
    event[11] = record.primary_capacity as u32;
    event[12] = record.secondary_capacity as u32;
    event[13] = u32::from(saved_round_limit);
    event[14] = u32::from(restrict_primary_to_one);
    event[15] = controller_state;
    event[16] = loaded(before_hand, primary);
    event[17] = loaded(after_hand, primary);
    event[18] = loaded(before_hand, secondary);
    event[19] = loaded(after_hand, secondary);
    event[20] = value(before_reserve, primary);
    event[21] = value(after_reserve, primary);
    event[22] = value(before_reserve, secondary);
    event[23] = value(after_reserve, secondary);
    event[24] = moved as u32;
    event[25] = moved as u32; // 0x1B6EC0 returns its transfer total in EAX.
    event[26] = 1; // committed transfer
    event[27] = before_hand.child.unwrap_or(u32::MAX);
    event[28] = after_hand.child.unwrap_or(u32::MAX);
    event[29] = before_hand.reload_timer as u32;
    event[30] = after_hand.reload_timer as u32;
    unsafe {
        tsfp_weapon_transfer_diagnostic_record(event.as_ptr(), TRANSFER_DIAGNOSTIC_WORDS as u32);
    }
}

/// Record the normalized desired-entry written by the legacy 0x1BA130 selector in the typed
/// player owner. This is a TEMPORARY-SHIM at the hybrid ABI edge: the translated normalizer still
/// computes and writes the selection, then this adapter carries that event into Rust-owned state.
/// T2411 replaces it when the native selector and its bounded table inputs are connected.
pub fn commit_normalized_selection_from_guest<M: GuestMemory>(m: &M, ext: u32) -> bool {
    if !enabled() || !valid_player_extension_address(ext) {
        return false;
    }
    let slot = m.read32(ext.wrapping_add(8)) as i32;
    if !(0..WEAPON_PLAYER_SLOTS as i32).contains(&slot) {
        return false;
    }
    let player = WeaponPlayerId::new(slot as u16);
    let desired = m.read32(ext.wrapping_add(0x94));
    let desired = (desired < ENTRY_COUNT as u32).then_some(desired as u8);
    OWNED_INVENTORIES
        .try_with(|store| {
            if let Some(owner) = store.get_mut(player) {
                owner.desired_entry = desired;
                return true;
            }
            let (mut initial, _) = read_weapon_inventory(m, ext);
            initial.desired_entry = desired;
            let Some(owner) = store.get_or_default(player) else {
                return false;
            };
            *owner = initial;
            true
        })
        .unwrap_or(false)
}

/// Commit the native 0x1B6BF0 cycle selector result to the Rust-owned desired entry.
/// Its order/eligibility snapshot is still read at the hybrid boundary; realised hand state
/// remains owned by the hand-transition path and is deliberately left unchanged here.
pub fn commit_cycle_selection_from_guest<M: GuestMemory>(m: &M, ext: u32, selected: u32) -> bool {
    if !enabled() || !valid_player_extension_address(ext) {
        return false;
    }
    let Some(desired) = (selected < ENTRY_COUNT as u32).then_some(selected as u8) else {
        return false;
    };
    let slot = m.read32(ext.wrapping_add(8)) as i32;
    if !(0..WEAPON_PLAYER_SLOTS as i32).contains(&slot) {
        return false;
    }
    let player = WeaponPlayerId::new(slot as u16);
    OWNED_INVENTORIES
        .try_with(|store| {
            if let Some(owner) = store.get_mut(player) {
                owner.desired_entry = Some(desired);
                return true;
            }
            let (mut initial, _) = read_weapon_inventory(m, ext);
            initial.desired_entry = Some(desired);
            let Some(owner) = store.get_or_default(player) else {
                return false;
            };
            *owner = initial;
            true
        })
        .unwrap_or(false)
}

/// Select and commit through the Rust-owned inventory lifetime when it exists.
/// Order and eligibility remain neighboring table inputs; owned bytes come only from
/// typed state, and selection plus the desired-entry write are one operation.
pub fn next_cycle_entry_for_player(
    player: WeaponPlayerId,
    entries: &[crate::rea_weapon_cycle::WeaponCycleEntry;
         crate::rea_weapon_cycle::WEAPON_CYCLE_SLOTS],
    previous: u32,
) -> Option<u32> {
    if !enabled() {
        return None;
    }
    OWNED_INVENTORIES.with(|store| {
        let handle = store.handle_for(player)?;
        store
            .select_cycle_entry_for_lifetime(handle, entries, previous)
            .map(|outcome| outcome.selected_id)
    })
}

/// Commit a resolved fire-mode pair to the Rust-owned player, then dispatch the Rust fire action.
/// The generation-checked handle prevents delayed input from mutating a reused player slot. This
/// API does not read or mirror guest memory; the current lifted selector root is not wired to it.
pub fn commit_fire_mode_and_dispatch_for_lifetime(
    player: WeaponPlayerHandle,
    selection: FireModeSelection,
    fire: impl FnOnce(&WeaponInventory),
) -> bool {
    if !enabled() {
        return false;
    }
    OWNED_INVENTORIES.with(|store| store.select_fire_mode_and_fire(player, selection, fire))
}

fn encode_weapon_inventory_trace(owner: &WeaponInventory, words: &mut [u32]) -> bool {
    if words.len() < WEAPON_TRACE_WORDS {
        return false;
    }
    let mut cursor = 0;
    words[cursor] = 1; // schema version
    cursor += 1;
    words[cursor] = 1; // owner present
    cursor += 1;
    words[cursor] = owner.desired_entry.map_or(u32::MAX, u32::from);
    cursor += 1;
    words[cursor] = owner
        .hands
        .realised_entry
        .map_or(u32::MAX, |entry| entry as u32);
    cursor += 1;
    words[cursor] = owner.default_throwable.map_or(u32::MAX, u32::from);
    cursor += 1;
    words[cursor] = u32::from(owner.collected_latch);
    cursor += 1;
    for value in owner.owned {
        words[cursor] = u32::from(value);
        cursor += 1;
    }
    for value in owner.hands.equipped_once {
        words[cursor] = u32::from(value);
        cursor += 1;
    }
    for value in owner.reserve {
        words[cursor] = value as i32 as u32;
        cursor += 1;
    }
    for hand in owner.hands.hands {
        words[cursor] = hand.record.map_or(u32::MAX, |record| record.0 as u32);
        cursor += 1;
        for value in hand.loaded {
            words[cursor] = value as i32 as u32;
            cursor += 1;
        }
        for value in hand.saved_rounds {
            words[cursor] = value as i32 as u32;
            cursor += 1;
        }
        words[cursor] = hand.reload_timer as u32;
        cursor += 1;
        for value in hand.hand_counters {
            words[cursor] = value as u32;
            cursor += 1;
        }
        words[cursor] = hand.child.unwrap_or(u32::MAX);
        cursor += 1;
    }
    if let Some(selection) = owner.fire_mode_selection {
        words[cursor] = selection.mode_index;
        words[cursor + 1] = selection.selector;
    } else {
        words[cursor] = u32::MAX;
        words[cursor + 1] = u32::MAX;
    }
    cursor += 2;
    debug_assert_eq!(cursor, WEAPON_TRACE_WORDS);
    true
}

/// Start a player inventory lifetime from Rust-owned subsystem state.
///
/// A native player/spawn owner calls this before inventory dispatch. Duplicate starts are
/// rejected so a live player cannot be silently overwritten; retire the previous slot before
/// reusing its stable identity. When no native owner has started the slot, the current hybrid
/// adapter still imports one guest snapshot as a TEMPORARY-SHIM.
pub fn begin_player_lifetime(player: WeaponPlayerId, inventory: WeaponInventory) -> bool {
    begin_player_lifetime_handle(player, inventory).is_some()
}

/// Begin a player lifetime and return the generation-checked identity native work should retain.
/// A stale handle cannot mutate a later player that reuses the same slot.
pub fn begin_player_lifetime_handle(
    player: WeaponPlayerId,
    inventory: WeaponInventory,
) -> Option<WeaponPlayerHandle> {
    OWNED_INVENTORIES.with(|store| store.begin_player_lifetime(player, inventory))
}

/// Begin a Rust-owned player generation at spawn with its resolved hands entry selected.
pub fn begin_spawn_player_lifetime_handle(
    player: WeaponPlayerId,
    hands_entry: usize,
) -> Option<WeaponPlayerHandle> {
    OWNED_INVENTORIES.with(|store| store.begin_player_lifetime_for_spawn(player, hands_entry))
}

/// Begin a Rust-owned player generation from typed profile values. The upstream profile owner
/// resolves selected-entry fallback and definition-backed metadata before calling this boundary.
pub fn begin_profile_player_lifetime_handle(
    player: WeaponPlayerId,
    profile: ProfileWeaponInventory,
) -> Option<WeaponPlayerHandle> {
    OWNED_INVENTORIES.with(|store| store.begin_player_lifetime_from_profile(player, profile))
}

/// Import the values written by the retained `0x25EC70` profile restore into typed Rust state.
///
/// TEMPORARY-SHIM (T2411): this adapter reads the post-restore player extension because the
/// native profile/save resolver is not ported yet. Remove it when that resolver supplies
/// `ProfileWeaponInventory` directly to `replace_player_lifetime_from_profile`.
pub fn import_profile_restore_from_guest<M: GuestMemory>(
    m: &M,
    player_record: u32,
) -> Option<WeaponPlayerHandle> {
    if !enabled() || !valid_guest_data_address(player_record) {
        return None;
    }
    let extension_table = m.read32(player_record.wrapping_add(0x14));
    if !valid_guest_data_address(extension_table) {
        return None;
    }
    let ext = m.read32(extension_table.wrapping_add(0x7c));
    if !valid_player_extension_address(ext) {
        return None;
    }
    let slot = m.read32(ext.wrapping_add(8));
    if slot >= WEAPON_PLAYER_SLOTS as u32 {
        return None;
    }

    let mut owned = [false; ENTRY_COUNT];
    for (index, value) in owned.iter_mut().enumerate() {
        *value = m.read8(ext.wrapping_add(0x54c + index as u32)) != 0;
    }
    let mut reserve = [0i16; AMMO_TYPE_COUNT];
    let mut loaded_rounds = [[0i16; AMMO_TYPE_COUNT]; 2];
    for index in 0..AMMO_TYPE_COUNT {
        let offset = index as u32 * 2;
        reserve[index] = m.read16(ext.wrapping_add(0x61e + offset)) as i16;
        loaded_rounds[0][index] = m.read16(ext.wrapping_add(0xdc + offset)) as i16;
        loaded_rounds[1][index] = m.read16(ext.wrapping_add(0x20c + offset)) as i16;
    }
    let desired = m.read32(ext.wrapping_add(0x94));
    let profile = ProfileWeaponInventory {
        desired_entry: u8::try_from(desired).ok(),
        owned,
        reserve,
        loaded_rounds,
    };
    let player = WeaponPlayerId::new(slot as u16);
    OWNED_INVENTORIES.with(|store| store.replace_player_lifetime_from_profile(player, profile))
}

/// End a player inventory lifetime before its stable player slot is reused.
pub fn retire_player_lifetime(player: WeaponPlayerId) -> bool {
    OWNED_INVENTORIES.with(|store| store.retire(player).is_some())
}

/// Retire a player only when the supplied generation still names its current lifetime.
pub fn retire_player_lifetime_handle(player: WeaponPlayerHandle) -> bool {
    OWNED_INVENTORIES.with(|store| store.retire_handle(player).is_some())
}

/// Realise a hand record against an existing Rust-owned player lifetime. Definition resolution
/// and child/model construction stay in the adjacent native owners; stale player generations do
/// not invoke presentation effects or mutate inventory state.
pub fn realise_hand_for_lifetime<P: HandPresentation>(
    player: WeaponPlayerHandle,
    side: HandSide,
    old_record: Option<WeaponAmmoRecord>,
    next_record: WeaponAmmoRecord,
    presentation: &mut P,
    restrict_primary_to_one: bool,
) -> Option<Result<HandRealiseOutcome, HandRealiseError>> {
    OWNED_INVENTORIES.with(|store| {
        Some(store.get_handle_mut(player)?.realise_hand(
            side,
            old_record,
            next_record,
            presentation,
            restrict_primary_to_one,
        ))
    })
}

/// Realise the right then left hand records for one Rust-owned player lifetime.
/// A stale generation declines before inventory mutation or presentation effects.
pub fn realise_actor_update_for_lifetime<P: HandPresentation>(
    player: WeaponPlayerHandle,
    right: HandRealiseRequest,
    left: HandRealiseRequest,
    presentation: &mut P,
) -> Option<HandRealisePairOutcome> {
    OWNED_INVENTORIES
        .with(|store| store.realise_actor_update_for_lifetime(player, right, left, presentation))
}

/// Reset one hand for a live Rust-owned player generation and return its prior resource receipt.
///
/// The receipt lets the neighboring object owner dispose of a former child explicitly. This
/// adapter does not infer that the original `0x1B7A30` reset releases the child itself.
pub fn reset_hand_for_lifetime(
    player: WeaponPlayerHandle,
    side: HandSide,
) -> Option<crate::weapon_hands::HandResetReceipt> {
    OWNED_INVENTORIES.with(|store| store.reset_hand_for_lifetime(player, side))
}

/// Attach a creation-time weapon pose to the object lifetime used by the shared subsystem owner.
/// The native scene/object producer supplies its generation-checked identity and retires it before
/// pool reuse. This is a typed Rust attachment point; the live object-creation route is still open.
pub fn attach_payload_for_object(
    object: ObjectId,
    pose: WeaponPayloadPose,
) -> Result<(), WeaponPayloadLifecycleError> {
    OWNED_INVENTORIES.with(|store| store.attach_payload(object, pose))
}

/// Build and attach the measured C1950 creation pose, then project it into the native transform.
/// Definition lookup and object allocation remain adjacent owners; a rejected object generation
/// leaves both the transform and payload registry unchanged.
pub fn create_c1950_payload_for_object(
    object: ObjectId,
    inputs: C1950PayloadInputs,
    transform: &mut crate::object_transform::TransformState,
) -> Result<WeaponPayloadPose, WeaponPayloadLifecycleError> {
    OWNED_INVENTORIES.with(|store| {
        let pose = store.create_c1950_payload(object, inputs)?;
        project_payload_pose(pose, transform);
        Ok(pose)
    })
}

/// Allocate a native scene identity and attach its C1950 pose before exposing it to the caller.
///
/// Transform projection happens only after both the scene identity and payload registry accept
/// the same generation. On failure, the spawn transaction retires the new scene identity and the
/// caller's transform remains unchanged.
pub fn spawn_c1950_payload_in_scene(
    scene: &mut SceneMembership,
    object_slot: u32,
    inputs: C1950PayloadInputs,
    transform: &mut crate::object_transform::TransformState,
) -> Result<ObjectId, WeaponPayloadSpawnError> {
    OWNED_INVENTORIES.with(|store| {
        let (object, pose) = store.spawn_c1950_payload(scene, object_slot, inputs)?;
        project_payload_pose(pose, transform);
        Ok(object)
    })
}

pub fn payload_pose_for_object(
    object: ObjectId,
) -> Result<WeaponPayloadPose, WeaponPayloadLifecycleError> {
    OWNED_INVENTORIES.with(|store| store.payload_pose(object))
}

/// Apply an owned payload's consumed pose to the native transform state for that exact object
/// generation. The staged animation yaw stays separate from the consumed O+0x70 yaw.
pub fn apply_payload_pose_to_transform(
    object: ObjectId,
    transform: &mut crate::object_transform::TransformState,
) -> Result<(), WeaponPayloadLifecycleError> {
    let pose = payload_pose_for_object(object)?;
    project_payload_pose(pose, transform);
    Ok(())
}

fn project_payload_pose(
    pose: WeaponPayloadPose,
    transform: &mut crate::object_transform::TransformState,
) {
    transform.angles_degrees = pose.pitch_yaw_roll_degrees;
    transform.rotation_mode = match pose.rotation_mode {
        crate::weapon_payload::PayloadRotationMode::Yaw => {
            crate::object_transform::RotationMode::Yaw
        }
        crate::weapon_payload::PayloadRotationMode::Euler => {
            crate::object_transform::RotationMode::Euler
        }
    };
    transform.rotation_dirty = true;
}

/// Remove payload state before the object owner releases this pool generation for reuse.
pub fn retire_payload_for_object(
    object: ObjectId,
) -> Result<WeaponPayloadPose, WeaponPayloadLifecycleError> {
    OWNED_INVENTORIES.with(|store| store.retire_payload(object))
}

/// Retire a payload-bearing native scene object through the shared weapon owner.
///
/// Both object-generation owners are checked before either is changed. The returned pose is a
/// receipt for adjacent resource cleanup; no lifted-C release callback is implied.
pub fn retire_weapon_scene_object(
    scene: &mut SceneMembership,
    object: ObjectId,
) -> Result<WeaponPayloadPose, WeaponObjectRetireError> {
    OWNED_INVENTORIES.with(|store| store.retire_payload_object(scene, object))
}

pub fn selects_root(va: u32) -> bool {
    enabled() && HAND_ROOTS.contains(&va)
}

/// Apply the native 0x1BC650 reserve-admission decision to an already-owned player inventory.
/// Resolved weapon/resource rows and actor gate facts come from adjacent typed owners. This does
/// not construct or emit a projectile; callers preserve that separate payload/render boundary.
/// Missing player state declines instead of importing from guest memory or creating a new life.
pub fn admit_shot_with_owned_store<const PLAYER_SLOTS: usize>(
    store: &mut WeaponInventoryStore<PLAYER_SLOTS>,
    player: WeaponPlayerId,
    sources: &[ShotAmmoSource; ENTRY_COUNT],
    game_type: u8,
    actor_flags: u32,
    actor_low16: u16,
) -> Option<ShotAdmission> {
    let owner = store.get_mut(player)?;
    Some(owner.admit_shot(sources, game_type, actor_flags, actor_low16))
}

/// Apply shot admission to the retained native player owner. This typed entrypoint deliberately
/// declines when the player lifetime has not been installed; it never snapshots guest memory.
pub fn admit_shot_for_player(
    player: WeaponPlayerId,
    sources: &[ShotAmmoSource; ENTRY_COUNT],
    game_type: u8,
    actor_flags: u32,
    actor_low16: u16,
) -> Option<ShotAdmission> {
    OWNED_INVENTORIES.with(|store| {
        admit_shot_with_owned_store(store, player, sources, game_type, actor_flags, actor_low16)
    })
}

/// Apply shot admission through a generation-checked native player lifetime.
pub fn admit_shot_for_lifetime(
    player: WeaponPlayerHandle,
    sources: &[ShotAmmoSource; ENTRY_COUNT],
    game_type: u8,
    actor_flags: u32,
    actor_low16: u16,
) -> Option<ShotAdmission> {
    OWNED_INVENTORIES.with(|store| {
        let owner = store.get_handle_mut(player)?;
        Some(owner.admit_shot(sources, game_type, actor_flags, actor_low16))
    })
}

/// Apply the owned portion of a pickup and its grant/selection scan to an existing Rust player
/// inventory. The native pickup coordinator supplies bounded table facts and realised hand
/// records; this adapter neither imports guest state nor starts a player lifetime.
pub fn collect_ammo_pickup_with_owned_store<const PLAYER_SLOTS: usize>(
    store: &mut WeaponInventoryStore<PLAYER_SLOTS>,
    player: WeaponPlayerId,
    pairs: [PickupAmmoPair; 2],
    realised_hand_records: [Option<WeaponAmmoRecord>; 2],
    maximums: &[i16; AMMO_TYPE_COUNT],
    pickup_flag_allows_unchanged_acquisition: bool,
    definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
    auto_selection: [Option<AmmoAutoSelectionContext>; 2],
) -> Option<PickupAcquisitionBatch> {
    let owner = store.get_mut(player)?;
    Some(owner.collect_ammo_pickup_acquire_and_select(
        pairs,
        realised_hand_records,
        maximums,
        pickup_flag_allows_unchanged_acquisition,
        definitions,
        auto_selection,
    ))
}

/// Apply pickup, acquisition and resolved selection to the retained native player owner. The
/// coordinator supplies owned table snapshots; absent player lifetimes decline without importing
/// or creating guest-backed state.
pub fn collect_ammo_pickup_for_player(
    player: WeaponPlayerId,
    pairs: [PickupAmmoPair; 2],
    realised_hand_records: [Option<WeaponAmmoRecord>; 2],
    maximums: &[i16; AMMO_TYPE_COUNT],
    pickup_flag_allows_unchanged_acquisition: bool,
    definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
    auto_selection: [Option<AmmoAutoSelectionContext>; 2],
) -> Option<PickupAcquisitionBatch> {
    OWNED_INVENTORIES.with(|store| {
        collect_ammo_pickup_with_owned_store(
            store,
            player,
            pairs,
            realised_hand_records,
            maximums,
            pickup_flag_allows_unchanged_acquisition,
            definitions,
            auto_selection,
        )
    })
}

/// Apply pickup, acquisition and resolved selection through a generation-checked native lifetime.
/// Delayed pickup work for a retired player fails closed after the slot is reused.
pub fn collect_ammo_pickup_for_lifetime(
    player: WeaponPlayerHandle,
    pairs: [PickupAmmoPair; 2],
    realised_hand_records: [Option<WeaponAmmoRecord>; 2],
    maximums: &[i16; AMMO_TYPE_COUNT],
    pickup_flag_allows_unchanged_acquisition: bool,
    definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
    auto_selection: [Option<AmmoAutoSelectionContext>; 2],
) -> Option<PickupAcquisitionBatch> {
    OWNED_INVENTORIES.with(|store| {
        let owner = store.get_handle_mut(player)?;
        Some(owner.collect_ammo_pickup_acquire_and_select(
            pairs,
            realised_hand_records,
            maximums,
            pickup_flag_allows_unchanged_acquisition,
            definitions,
            auto_selection,
        ))
    })
}

/// Apply a weapon pickup's measured partial inventory transaction to an existing generation-checked
/// player owner. This does not retire or retain the neighboring world pickup object.
pub fn collect_weapon_pickup_with_owned_store<const PLAYER_SLOTS: usize>(
    store: &mut WeaponInventoryStore<PLAYER_SLOTS>,
    player: WeaponPlayerHandle,
    entry: usize,
    ammo_pairs: [Option<WeaponPickupAmmo>; 2],
    realised_hand_records: [Option<WeaponAmmoRecord>; 2],
    definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
    zero_delta_duplicate_predicate_passed: bool,
    select: bool,
    normalized_entry: Option<usize>,
) -> Option<WeaponPickupOutcome> {
    let owner = store.get_handle_mut(player)?;
    Some(owner.collect_weapon_pickup_with_duplicate_predicate(
        entry,
        ammo_pairs,
        realised_hand_records,
        definitions,
        zero_delta_duplicate_predicate_passed,
        select,
        normalized_entry,
    ))
}

/// Apply a weapon pickup through the shared store, declining when the originating player lifetime
/// has been retired or its slot has been reused.
pub fn collect_weapon_pickup_for_lifetime(
    player: WeaponPlayerHandle,
    entry: usize,
    ammo_pairs: [Option<WeaponPickupAmmo>; 2],
    realised_hand_records: [Option<WeaponAmmoRecord>; 2],
    definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
    zero_delta_duplicate_predicate_passed: bool,
    select: bool,
    normalized_entry: Option<usize>,
) -> Option<WeaponPickupOutcome> {
    OWNED_INVENTORIES.with(|store| {
        collect_weapon_pickup_with_owned_store(
            store,
            player,
            entry,
            ammo_pairs,
            realised_hand_records,
            definitions,
            zero_delta_duplicate_predicate_passed,
            select,
            normalized_entry,
        )
    })
}

/// Dispatch a callee frame with a return slot at ESP; GuestCall callers use the wrapper below.
pub fn dispatch_nested<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !enabled() {
        return false;
    }
    if va == 0x25ec70 {
        let player_record = regs.eax;
        let handled = crate::inventory_pickup::run(va, m, guest, regs);
        if handled {
            // TEMPORARY-SHIM (T2411): attach the measured restore output to typed state until
            // the native profile/save resolver supplies this value directly.
            let _ = import_profile_restore_from_guest(m, player_record);
        }
        return handled;
    }
    if matches!(va, 0x1bfaa0 | 0x1bfca0) {
        // TEMPORARY-SHIM: update the persistent Rust owner from the typed acquisition event, then let the retained lane
        // finish its still-unported order/throwable/notification effects. T2411 removes this path once those are native.
        let _ = apply_grant_event_to_runtime_owner(va, m, regs);
        return false;
    }
    match va {
        0x1b6ec0 | 0x1b9760 | 0x1bf9e0 | 0x1ba220 => {
            dispatch_with_runtime_owned_store(va, m, guest, regs)
        }
        0x1b7bc0 => crate::weapons_slice::run(va, true, m, guest, regs),
        _ => false,
    }
}

/// Resolve immutable entry/record facts at the guest ABI edge for the existing Rust inventory
/// reducer. Entry ordering and selection are a separate owner; only the original grant's group,
/// eligibility, ammo-pair and saved-round inputs cross this shim.
fn read_inventory_entry_definitions<M: GuestMemory>(
    m: &M,
) -> [Option<InventoryEntryDefinition>; ENTRY_COUNT] {
    core::array::from_fn(|index| {
        let entry = index as u32;
        let row = ENTRY_DATA_TABLE.wrapping_add(entry.wrapping_mul(ENTRY_DATA_STRIDE));
        let record_id = m.read32(row.wrapping_add(ENTRY_RECORD_OFFSET));
        let record_row = record_address(m, record_id)?;
        let ammo_type = |offset| record_ammo_type(m.read32(record_row + offset)).flatten();
        let flag_row = ENTRY_FLAGS_BASE.wrapping_add(entry.wrapping_mul(ENTRY_FLAGS_STRIDE));
        Some(InventoryEntryDefinition {
            group: m.read32(flag_row.wrapping_add(ENTRY_GROUP_OFFSET)),
            flags: m.read32(flag_row),
            record_present: true,
            // The original ammo getters return these dwords directly. A malformed or
            // unsupported ammo index must not erase the independently valid entry/group row.
            primary_ammo: ammo_type(0x14),
            secondary_ammo: ammo_type(0x1c),
            grant_saved_rounds: Some(GrantSavedRounds {
                primary: m.read32(record_row + 0x18) as i32 as i16,
                secondary: m.read32(record_row + 0x20) as i32 as i16,
            }),
        })
    })
}

/// Feed a measured grant/collect event into the retained Rust owner before the legacy-compatible
/// callee runs. This does not import guest values after the lifetime attaches; only the immutable
/// table rows and the event's extension/entry arguments cross the TEMPORARY-SHIM boundary.
fn apply_grant_event_to_runtime_owner<M: GuestMemory>(
    va: u32,
    m: &mut M,
    regs: &ExtRegisters,
) -> bool {
    let event_index = match va {
        0x1bfaa0 => 0usize,
        0x1bfca0 => 1usize,
        _ => return false,
    };
    GRANT_DIAGNOSTICS[1 + event_index].fetch_add(1, Ordering::Relaxed);
    let ext = m.read32(regs.esp.wrapping_add(4));
    let entry = m.read32(regs.esp.wrapping_add(8));
    GRANT_DIAGNOSTICS[9].store(va, Ordering::Relaxed);
    GRANT_DIAGNOSTICS[10].store(ext, Ordering::Relaxed);
    GRANT_DIAGNOSTICS[11].store(entry, Ordering::Relaxed);
    GRANT_DIAGNOSTICS[12].store(u32::MAX, Ordering::Relaxed);
    GRANT_DIAGNOSTICS[13].store(0, Ordering::Relaxed);
    GRANT_DIAGNOSTICS[14].store(0, Ordering::Relaxed);
    GRANT_DIAGNOSTICS[15..24]
        .iter()
        .for_each(|word| word.store(0, Ordering::Relaxed));
    if !valid_player_extension_address(ext) || entry >= ENTRY_COUNT as u32 {
        GRANT_DIAGNOSTICS[5].fetch_add(1, Ordering::Relaxed);
        GRANT_DIAGNOSTICS[13].store(1, Ordering::Relaxed);
        return false;
    }
    let slot = m.read32(ext.wrapping_add(8)) as i32;
    GRANT_DIAGNOSTICS[12].store(slot as u32, Ordering::Relaxed);
    if !(0..WEAPON_PLAYER_SLOTS as i32).contains(&slot) {
        GRANT_DIAGNOSTICS[6].fetch_add(1, Ordering::Relaxed);
        GRANT_DIAGNOSTICS[13].store(2, Ordering::Relaxed);
        return false;
    }
    let player = WeaponPlayerId::new(slot as u16);
    let definitions = read_inventory_entry_definitions(m);
    let definition = definitions[entry as usize];
    if let Some(definition) = definition {
        GRANT_DIAGNOSTICS[14].store(1, Ordering::Relaxed);
        GRANT_DIAGNOSTICS[15].store(definition.group, Ordering::Relaxed);
        GRANT_DIAGNOSTICS[16].store(definition.flags, Ordering::Relaxed);
        GRANT_DIAGNOSTICS[17].store(
            definition
                .primary_ammo
                .map_or(u32::MAX, |ammo| ammo.index() as u32),
            Ordering::Relaxed,
        );
        GRANT_DIAGNOSTICS[18].store(
            definition
                .secondary_ammo
                .map_or(u32::MAX, |ammo| ammo.index() as u32),
            Ordering::Relaxed,
        );
        if let Some(saved) = definition.grant_saved_rounds {
            GRANT_DIAGNOSTICS[19].store(saved.primary as i32 as u32, Ordering::Relaxed);
            GRANT_DIAGNOSTICS[20].store(saved.secondary as i32 as u32, Ordering::Relaxed);
        }
    } else {
        GRANT_DIAGNOSTICS[7].fetch_add(1, Ordering::Relaxed);
        GRANT_DIAGNOSTICS[13].store(3, Ordering::Relaxed);
    }
    let outcome = OWNED_INVENTORIES
        .try_with(|store| {
            if store.get(player).is_none() {
                let (initial, _) = read_weapon_inventory(m, ext);
                let Some(owner) = store.get_or_default(player) else {
                    return None;
                };
                *owner = initial;
            }
            let Some(owner) = store.get_mut(player) else {
                return None;
            };
            let result = if event_index == 0 {
                owner.grant_entry_with_saved_rounds(&definitions, entry as usize)
            } else {
                owner.collect_entry(&definitions, entry as usize)
            };
            Some(result)
        })
        .unwrap_or(None);
    let Some(outcome) = outcome else {
        GRANT_DIAGNOSTICS[8].fetch_add(1, Ordering::Relaxed);
        GRANT_DIAGNOSTICS[13].store(4, Ordering::Relaxed);
        return false;
    };
    GRANT_DIAGNOSTICS[3 + event_index].fetch_add(1, Ordering::Relaxed);
    GRANT_DIAGNOSTICS[21].store(
        outcome.newly_owned.iter().filter(|owned| **owned).count() as u32,
        Ordering::Relaxed,
    );
    GRANT_DIAGNOSTICS[22].store(u32::from(outcome.acquired), Ordering::Relaxed);
    GRANT_DIAGNOSTICS[23].store(
        u32::from(outcome.saved_rounds_unresolved),
        Ordering::Relaxed,
    );
    true
}

/// Dispatch a call made by translated Rust with the guest return-slot layout (`GuestCall::call` runs before the trampoline
/// pushes its return address): owner adapters, incl. the `0x1B7BC0` wrapper, see a synthetic slot at `[esp]`, arguments at
/// `[esp + 4]`. The slot word is put back: after a tail jump it is the caller's live return address, which x86 keeps.
pub fn dispatch_nested_guest_call<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    let args_esp = regs.esp;
    regs.esp = args_esp.wrapping_sub(4);
    let saved = m.read32(regs.esp);
    m.write32(regs.esp, 0);
    let handled = dispatch_nested(va, m, guest, regs);
    m.write32(args_esp.wrapping_sub(4), saved);
    regs.esp = args_esp;
    handled
}

/// Route the `0x1B7BC0` transfer and final hand-counter reset through its retained owner. Pass
/// these resolved arguments directly because the Rust adapter runs inside the translated frame,
/// before a nested guest call would add the callee return slot.
pub fn transfer_from_runtime_owned_store_args<M: GuestMemory>(
    m: &mut M,
    ext: u32,
    controller: u32,
    hand_address: u32,
    saved_round_limit: bool,
    regs: &mut ExtRegisters,
) -> bool {
    if !enabled() || !valid_player_extension_address(ext) {
        return false;
    }
    let slot = m.read32(ext.wrapping_add(8)) as i32;
    if !(0..WEAPON_PLAYER_SLOTS as i32).contains(&slot) {
        return false;
    }
    let player = WeaponPlayerId::new(slot as u16);
    OWNED_INVENTORIES.with(|store| {
        if store.get(player).is_none() {
            let (initial, _) = read_weapon_inventory(m, ext);
            let Some(owner) = store.get_or_default(player) else {
                return false;
            };
            *owner = initial;
        }
        let Some((side, _, record)) = resolve_hand(m, ext, hand_address) else {
            return false;
        };
        let Some(owner) = store.get_mut(player) else {
            return false;
        };
        let before_reserve = owner.reserve;
        let before_hand = owner.hands.hand(side).clone();
        let controller_state = if controller != 0 {
            m.read32(controller.wrapping_add(0x44))
        } else {
            u32::MAX
        };
        let restrict_primary_to_one = controller_state == 7 && record.primary_single_round;
        let mut session = HandReloadSession::new(side, saved_round_limit, restrict_primary_to_one);
        let moved = match owner.apply_reload_session(&mut session, record) {
            crate::weapon_hands::ReloadSessionOutcome::Committed(moved) => moved,
            _ => return false,
        };
        write_owned_reserve_changes(m, ext, &before_reserve, owner);
        write_hand(m, hand_address, &before_hand, owner.hands.hand(side));
        record_transfer_observation(
            slot as u32,
            ext,
            hand_address,
            side,
            record,
            controller_state,
            saved_round_limit,
            restrict_primary_to_one,
            &before_hand,
            owner.hands.hand(side),
            &before_reserve,
            &owner.reserve,
            moved,
        );
        owner.hands.hand_mut(side).hand_counters = [1000; 2];
        regs.eax = moved as u32;
        true
    })
}

/// Apply `0x1BDFD0`'s measured pre-transfer clear to an existing owner when the following
/// `0x1B7BC0` call requests saved-round restoration. The caller already clears the same guest
/// words; this keeps the retained Rust hand in sync before the wrapper performs its typed transfer.
pub fn clear_owned_hand_for_saved_restore<M: GuestMemory>(
    m: &mut M,
    ext: u32,
    hand_address: u32,
) -> bool {
    if !enabled() || !valid_player_extension_address(ext) {
        return false;
    }
    let slot = m.read32(ext.wrapping_add(8)) as i32;
    if !(0..WEAPON_PLAYER_SLOTS as i32).contains(&slot) {
        return false;
    }
    let player = WeaponPlayerId::new(slot as u16);
    OWNED_INVENTORIES.with(|store| {
        let Some((side, _, _)) = resolve_hand(m, ext, hand_address) else {
            return false;
        };
        let Some(owner) = store.get_mut(player) else {
            // If this is the first attachment, the caller has already cleared guest loaded words;
            // the subsequent one-time import will therefore start from the correct state.
            return false;
        };
        let before = owner.hands.hand(side).clone();
        owner.hands.hand_mut(side).loaded.fill(0);
        write_hand(m, hand_address, &before, owner.hands.hand(side));
        true
    })
}

/// Route reload/unload through the session-owned inventory. TEMPORARY-SHIM: T2411 removes the
/// import/write-through when the native spawn/save producer and the remaining inventory edges are
/// connected. The guest extension pointer is used only to resolve the measured signed player slot.
fn dispatch_with_runtime_owned_store<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    let ext = m.read32(regs.esp.wrapping_add(4));
    if !valid_player_extension_address(ext) {
        return false;
    }
    let slot = m.read32(ext.wrapping_add(8)) as i32;
    if !(0..WEAPON_PLAYER_SLOTS as i32).contains(&slot) {
        return false;
    }
    let player = WeaponPlayerId::new(slot as u16);
    OWNED_INVENTORIES.with(|store| {
        if store.get(player).is_none() {
            // One initial state import lets this hand slice retain ownership across calls. Later
            // guest drift does not replace the Rust state.
            let (initial, _) = read_weapon_inventory(m, ext);
            let Some(owner) = store.get_or_default(player) else {
                return false;
            };
            *owner = initial;
        }
        dispatch_nested_with_owned_store(va, m, guest, regs, store, player)
    })
}

/// Dispatch the hand operations that have a durable Rust owner. The embedding lane supplies the
/// stable player slot and store; it must select this path only when the T2376 switch is enabled.
/// Unsupported calls decline so the embedding lane can continue its normal routing.
pub fn dispatch_nested_with_owned_store<M: GuestMemory, C: GuestCall, const PLAYER_SLOTS: usize>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    store: &mut WeaponInventoryStore<PLAYER_SLOTS>,
    player: WeaponPlayerId,
) -> bool {
    match va {
        0x1b6ec0 => transfer_from_owned_store(m, regs, store, player),
        0x1b9760 => unload_from_owned_store(m, regs, store, player),
        0x1bf9e0 => handoff_from_owned_store(m, guest, regs, store, player),
        0x1ba220 => add_ammo_reserve_from_stack(m, regs, store, player),
        _ => false,
    }
}

/// Route the measured `0x1BA220(ext, ammo_type, short_amount)` operation through an existing
/// player owner. The ammo row and realized hand record facts are resolved at this ABI boundary;
/// only the signed reserve word remains mutable subsystem state.
fn add_ammo_reserve_from_stack<M: GuestMemory, const PLAYER_SLOTS: usize>(
    m: &mut M,
    regs: &mut ExtRegisters,
    store: &mut WeaponInventoryStore<PLAYER_SLOTS>,
    player: WeaponPlayerId,
) -> bool {
    let ext = m.read32(regs.esp.wrapping_add(4));
    let ammo_raw = m.read32(regs.esp.wrapping_add(8));
    let Some(ammo) = AmmoType::new(ammo_raw as u8).filter(|ammo| ammo.index() as u32 == ammo_raw)
    else {
        return false;
    };
    let amount = m.read32(regs.esp.wrapping_add(12)) as u16 as i16;
    let Some(owner) = store.get(player) else {
        return false;
    };
    let loaded = owner
        .hands
        .hands
        .iter()
        .filter_map(|hand| {
            let id = hand.record?;
            let record = record(m, u32::from(id.0))?;
            (record.primary == Some(ammo) || record.secondary == Some(ammo))
                .then_some(hand.loaded[ammo.index()] as i32)
        })
        .fold(0i32, i32::wrapping_add);
    let maximum = ammo_reserve_maximum(m, ammo.index());
    let Some(owner) = store.get_mut(player) else {
        return false;
    };
    let delta = owner.add_ammo_reserve(ammo, amount, loaded, maximum);
    let reserve = owner.reserve[ammo.index()];
    crate::c2r::rt::write16(m, ext + 0x61e + ammo.index() as u32 * 2, reserve as u16);
    regs.eax = delta as u32;
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rust_weapon_hands_configure(enabled: bool) {
    configure(enabled);
}

fn read_hand<M: GuestMemory + ?Sized>(m: &M, address: u32) -> HandAmmoState {
    let mut hand = HandAmmoState::empty();
    let record = m.read32(address + HAND_RECORD);
    hand.record = (record != u32::MAX).then_some(WeaponRecordId(record as u16));
    hand.child = match m.read32(address + HAND_CHILD) {
        u32::MAX => None,
        child => Some(child),
    };
    hand.reload_timer = m.read32(address + HAND_RELOAD_TIMER) as i32;
    for (index, counter) in hand.hand_counters.iter_mut().enumerate() {
        *counter = m.read32(address + HAND_COUNTERS + index as u32 * 4) as i32;
    }
    for ammo in 0..AMMO_TYPES {
        hand.loaded[ammo] = m.read16(address + HAND_LOADED + (ammo as u32) * 2) as i16;
        hand.saved_rounds[ammo] = m.read16(address + HAND_SAVED + (ammo as u32) * 2) as i16;
    }
    hand
}

fn write_hand<M: GuestMemory + ?Sized>(
    m: &mut M,
    address: u32,
    before: &HandAmmoState,
    after: &HandAmmoState,
) {
    for ammo in 0..AMMO_TYPES {
        if before.loaded[ammo] != after.loaded[ammo] {
            crate::c2r::rt::write16(
                m,
                address + HAND_LOADED + (ammo as u32) * 2,
                after.loaded[ammo] as u16,
            );
        }
        if before.saved_rounds[ammo] != after.saved_rounds[ammo] {
            crate::c2r::rt::write16(
                m,
                address + HAND_SAVED + (ammo as u32) * 2,
                after.saved_rounds[ammo] as u16,
            );
        }
    }
    if before.reload_timer != after.reload_timer {
        m.write32(address + HAND_RELOAD_TIMER, after.reload_timer as u32);
    }
    for (index, counter) in after.hand_counters.iter().enumerate() {
        if before.hand_counters[index] != *counter {
            m.write32(address + HAND_COUNTERS + index as u32 * 4, *counter as u32);
        }
    }
}

fn record_address<M: GuestMemory + ?Sized>(m: &M, id: u32) -> Option<u32> {
    if id >= 0x42 {
        return None;
    }
    let table = m.read32(RECORD_BASE_CELL);
    if !valid_guest_data_address(table) {
        return None;
    }
    table.checked_add(id.checked_mul(RECORD_STRIDE)?)
}

fn record_ammo_type(value: u32) -> Option<Option<AmmoType>> {
    if value == 0 {
        return Some(None);
    }
    let value = u8::try_from(value).ok()?;
    Some(AmmoType::new(value))
}

fn record<M: GuestMemory + ?Sized>(m: &M, id: u32) -> Option<WeaponAmmoRecord> {
    let base = record_address(m, id)?;
    Some(WeaponAmmoRecord {
        id: WeaponRecordId(id as u16),
        primary: record_ammo_type(m.read32(base + 0x14))?,
        secondary: record_ammo_type(m.read32(base + 0x1c))?,
        primary_capacity: m.read32(base + 0x18) as i32,
        secondary_capacity: m.read32(base + 0x20) as i32,
        primary_single_round: m.read32(base + 0x0c) & 0x2000 != 0,
    })
}

/// Read the current guest view once at the ABI edge, including both hand records, then run T2411
/// hand operations through the same typed inventory owner used by pickup, grant, and shot state.
/// This is an adapter snapshot; it is not yet the durable Rust state owner required for subsystem
/// adoption.
fn read_weapon_inventory<M: GuestMemory + ?Sized>(m: &M, ext: u32) -> (WeaponInventory, Inventory) {
    let legacy = Inventory::load(m, ext);
    let mut owner = WeaponInventory::default();
    owner.desired_entry = (legacy.current < ENTRY_COUNT as u32).then_some(legacy.current as u8);
    owner.hands.realised_entry =
        (legacy.previous < ENTRY_COUNT as u32).then_some(legacy.previous as usize);
    owner.default_throwable =
        (legacy.throwable < ENTRY_COUNT as u32).then_some(legacy.throwable as u8);
    owner.fire_mode_selection = Some(FireModeSelection::new(
        m.read32(ext + 0x5c),
        m.read32(ext + 0x60),
    ));
    owner.collected_latch = legacy.flags & EXT_COLLECT_LATCH != 0;
    for entry in 0..ENTRY_COUNT {
        owner.owned[entry] = legacy.owned[entry] != 0;
        owner.hands.equipped_once[entry] = legacy.equipped[entry] != 0;
    }
    for ammo in 0..AMMO_TYPE_COUNT {
        owner.reserve[ammo] = legacy.ammo[ammo] as i16;
    }
    owner.hands.hands[HandSide::Left.index()] = read_hand(m, ext + HAND_LEFT_OFFSET);
    owner.hands.hands[HandSide::Right.index()] = read_hand(m, ext + HAND_RIGHT_OFFSET);
    (owner, legacy)
}

fn write_reserve_changes<M: GuestMemory>(
    m: &mut M,
    ext: u32,
    before: &Inventory,
    after: &WeaponInventory,
) {
    for ammo in 0..AMMO_TYPES {
        let reserve = after.reserve[ammo] as u16;
        if reserve != before.ammo[ammo] {
            crate::c2r::rt::write16(m, ext + 0x61e + ammo as u32 * 2, reserve);
        }
    }
}

/// Write-through for an existing Rust-owned inventory. Unlike `read_weapon_inventory`, this
/// adapter never hydrates state from guest bytes; a native host must create and retire the player
/// inventory through `WeaponInventoryStore` and pass its stable identity here. Guest writes are
/// retained only for the disabled lifted-call compatibility path.
fn write_owned_reserve_changes<M: GuestMemory + ?Sized>(
    m: &mut M,
    ext: u32,
    before: &[i16; AMMO_TYPE_COUNT],
    after: &WeaponInventory,
) {
    for (ammo, (&old, &new)) in before.iter().zip(after.reserve.iter()).enumerate() {
        if old != new {
            crate::c2r::rt::write16(m, ext + 0x61e + ammo as u32 * 2, new as u16);
        }
    }
}

fn resolve_hand<M: GuestMemory>(
    m: &M,
    ext: u32,
    hand: u32,
) -> Option<(HandSide, WeaponRecordId, WeaponAmmoRecord)> {
    let side = match hand {
        address if address == ext.checked_add(HAND_LEFT_OFFSET)? => HandSide::Left,
        address if address == ext.checked_add(HAND_RIGHT_OFFSET)? => HandSide::Right,
        _ => return None,
    };
    let id = m.read32(hand + HAND_RECORD);
    if id == u32::MAX {
        return None;
    }
    let record = record(m, id)?;
    Some((side, record.id, record))
}

/// Implements the 0x1B6EC0 state transfer from its measured live register/stack contract.
/// Caller supplies the current guest state and persists only cells changed by the owner.
pub fn transfer_from_registers<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) -> bool {
    let hand_address = regs.eax;
    let ext = m.read32(regs.esp.wrapping_add(4));
    let controller = m.read32(regs.esp.wrapping_add(8));
    let saved_round_limit = m.read32(regs.esp.wrapping_add(12)) != 0;
    if !valid_player_extension_address(ext) {
        return false;
    }
    let Some((side, _id, record)) = resolve_hand(m, ext, hand_address) else {
        return false;
    };
    let before = read_hand(m, hand_address);
    let (mut system, legacy) = read_weapon_inventory(m, ext);
    let restrict_primary_to_one = controller != 0
        && m.read32(controller.wrapping_add(0x44)) == 7
        && record.primary_single_round;
    let mut session = HandReloadSession::new(side, saved_round_limit, restrict_primary_to_one);
    let moved = match system.apply_reload_session(&mut session, record) {
        crate::weapon_hands::ReloadSessionOutcome::Committed(moved) => moved,
        _ => return false,
    };
    write_reserve_changes(m, ext, &legacy, &system);
    write_hand(m, hand_address, &before, system.hands.hand(side));
    regs.eax = moved as u32;
    true
}

/// Execute a reload transfer against persistent caller-owned state. The caller supplies a stable
/// Rust player identity; a missing state declines the adapter rather than inferring a new owner
/// from an extension pointer. This ABI bridge only resolves immutable record/controller inputs
/// and mirrors changed cells for lifted readers.
pub fn transfer_from_owned_store<M: GuestMemory, const PLAYER_SLOTS: usize>(
    m: &mut M,
    regs: &mut ExtRegisters,
    store: &mut WeaponInventoryStore<PLAYER_SLOTS>,
    player: WeaponPlayerId,
) -> bool {
    let hand_address = regs.eax;
    let ext = m.read32(regs.esp.wrapping_add(4));
    let controller = m.read32(regs.esp.wrapping_add(8));
    let saved_round_limit = m.read32(regs.esp.wrapping_add(12)) != 0;
    if !valid_player_extension_address(ext) {
        return false;
    }
    let Some((side, _id, record)) = resolve_hand(m, ext, hand_address) else {
        return false;
    };
    let Some(owner) = store.get_mut(player) else {
        return false;
    };
    let before_reserve = owner.reserve;
    let before_hand = owner.hands.hand(side).clone();
    let controller_state = if controller != 0 {
        m.read32(controller.wrapping_add(0x44))
    } else {
        u32::MAX
    };
    let restrict_primary_to_one = controller_state == 7 && record.primary_single_round;
    let moved = owner.transfer_reload(side, record, saved_round_limit, restrict_primary_to_one);
    write_owned_reserve_changes(m, ext, &before_reserve, owner);
    write_hand(m, hand_address, &before_hand, owner.hands.hand(side));
    record_transfer_observation(
        m.read32(ext.wrapping_add(8)),
        ext,
        hand_address,
        side,
        record,
        controller_state,
        saved_round_limit,
        restrict_primary_to_one,
        &before_hand,
        owner.hands.hand(side),
        &before_reserve,
        &owner.reserve,
        moved,
    );
    regs.eax = moved as u32;
    true
}

/// Start one native typed reload sequence for an existing Rust-owned player lifetime.
pub fn begin_reload_for_lifetime(
    player: WeaponPlayerHandle,
    side: HandSide,
    controller: usize,
    saved_round_limit: bool,
    restrict_primary_to_one: bool,
) -> Option<ReloadSession> {
    OWNED_INVENTORIES.with(|store| {
        store.begin_reload_session(
            player,
            side,
            controller,
            saved_round_limit,
            restrict_primary_to_one,
        )
    })
}

/// Run the measured synchronous `0x1B6EC0` transfer for one existing Rust-owned player lifetime.
/// Both distinct eligible ammo channels commit in primary-then-secondary order before return;
/// controller transition resets remain the adjacent `0x1B7BC0` owner’s responsibility.
pub fn transfer_reload_for_lifetime(
    player: WeaponPlayerHandle,
    side: HandSide,
    record: WeaponAmmoRecord,
    saved_round_limit: bool,
    restrict_primary_to_one: bool,
) -> Option<i32> {
    OWNED_INVENTORIES.with(|store| {
        Some(store.get_handle_mut(player)?.transfer_reload(
            side,
            record,
            saved_round_limit,
            restrict_primary_to_one,
        ))
    })
}

/// Apply the Rust-owned ammo/selection portion of the state-3 holster edge for an existing player
/// lifetime. Controller completion remains with its separate callback owner.
///
/// The owner supplies its desired entry; neighboring typed table owners supply the resolved
/// ammo record and reserve limits. This path saves rounds, returns loaded ammo to capped reserve,
/// clears loaded cells, and publishes the realised entry without importing or mirroring guest
/// memory. The controller must be in state 3 and attached to this hand; its state and timer are
/// preserved because the inspected REA does not establish their completion values here. Child
/// release and external callbacks remain separate lifecycle effects.
pub fn complete_holster_for_lifetime(
    player: WeaponPlayerHandle,
    side: HandSide,
    record: WeaponAmmoRecord,
    reserve_maximum: &[i16; AMMO_TYPE_COUNT],
    controller_index: usize,
) -> Option<u8> {
    OWNED_INVENTORIES.with(|store| {
        let owner = store.get_handle_mut(player)?;
        let desired = owner.desired_entry?;
        if !owner.complete_state3_holster(side, controller_index, record, reserve_maximum) {
            return None;
        }
        Some(desired)
    })
}

/// Commit both ammo channels synchronously for the reload's checked owner. The neighboring typed
/// definition-table owner supplies the record active at this transfer point; no guest snapshot or
/// write-through is performed by this native boundary.
pub fn advance_reload_for_lifetime(
    player: WeaponPlayerHandle,
    session: &mut ReloadSession,
    active_record: WeaponAmmoRecord,
) -> Option<ReloadSessionAdvance> {
    OWNED_INVENTORIES.with(|store| store.advance_reload_session(player, session, active_record))
}

/// Cancel a pending transfer. A completed two-channel transfer remains committed.
pub fn cancel_reload_for_lifetime(
    player: WeaponPlayerHandle,
    session: &mut ReloadSession,
) -> Option<ReloadSessionAdvance> {
    OWNED_INVENTORIES.with(|store| store.cancel_reload_session(player, session))
}

/// Implements the 0x1B9760 sequential, unclamped unload contract.
pub fn unload_from_stack<M: GuestMemory>(m: &mut M, regs: &ExtRegisters) -> bool {
    let ext = m.read32(regs.esp.wrapping_add(4));
    let hand_address = m.read32(regs.esp.wrapping_add(8));
    if !valid_player_extension_address(ext) {
        return false;
    }
    let Some((side, _id, record)) = resolve_hand(m, ext, hand_address) else {
        return false;
    };
    let before = read_hand(m, hand_address);
    let (mut system, legacy) = read_weapon_inventory(m, ext);
    system.unload_without_clamp(side, record);
    write_reserve_changes(m, ext, &legacy, &system);
    write_hand(m, hand_address, &before, system.hands.hand(side));
    true
}

/// Execute the measured sequential unload against an already-owned player inventory.
pub fn unload_from_owned_store<M: GuestMemory, const PLAYER_SLOTS: usize>(
    m: &mut M,
    regs: &ExtRegisters,
    store: &mut WeaponInventoryStore<PLAYER_SLOTS>,
    player: WeaponPlayerId,
) -> bool {
    let ext = m.read32(regs.esp.wrapping_add(4));
    let hand_address = m.read32(regs.esp.wrapping_add(8));
    if !valid_player_extension_address(ext) {
        return false;
    }
    let Some((side, _id, record)) = resolve_hand(m, ext, hand_address) else {
        return false;
    };
    let Some(owner) = store.get_mut(player) else {
        return false;
    };
    let before_reserve = owner.reserve;
    let before_hand = owner.hands.hand(side).clone();
    owner.unload_without_clamp(side, record);
    write_owned_reserve_changes(m, ext, &before_reserve, owner);
    write_hand(m, hand_address, &before_hand, owner.hands.hand(side));
    true
}

/// Runs the typed state portion of the 0x1C3100 state-3 holster edge against the retained player
/// owner. The first hybrid attachment imports once; later holsters use the stable slot state.
/// The C2R caller keeps adjacent controller, child-release, callback and presentation work in its
/// original order.
pub fn holster_transition<M: GuestMemory + ?Sized>(
    m: &mut M,
    ext: u32,
    hand_address: u32,
    active_record_id: u32,
) -> bool {
    if !valid_player_extension_address(ext) {
        return false;
    }
    let slot = m.read32(ext.wrapping_add(8)) as i32;
    if !(0..WEAPON_PLAYER_SLOTS as i32).contains(&slot) {
        return false;
    }
    let player = WeaponPlayerId::new(slot as u16);
    OWNED_INVENTORIES.with(|store| {
        if store.get(player).is_none() {
            let (initial, _) = read_weapon_inventory(m, ext);
            let Some(owner) = store.get_or_default(player) else {
                return false;
            };
            *owner = initial;
        }
        holster_transition_with_owned_store(m, ext, hand_address, active_record_id, store, player)
    })
}

/// Apply the measured typed holster state change to an existing owner. Guest memory supplies
/// immutable weapon/ammo definitions and receives compatibility writes for lifted callers.
pub fn holster_transition_with_owned_store<M: GuestMemory + ?Sized, const PLAYER_SLOTS: usize>(
    m: &mut M,
    ext: u32,
    hand_address: u32,
    active_record_id: u32,
    store: &mut WeaponInventoryStore<PLAYER_SLOTS>,
    player: WeaponPlayerId,
) -> bool {
    if !valid_player_extension_address(ext) {
        return false;
    }
    let side = if hand_address == ext.wrapping_add(HAND_LEFT_OFFSET) {
        HandSide::Left
    } else if hand_address == ext.wrapping_add(HAND_RIGHT_OFFSET) {
        HandSide::Right
    } else {
        return false;
    };
    let record = if active_record_id == u32::MAX {
        WeaponAmmoRecord {
            id: WeaponRecordId(u16::MAX),
            ..WeaponAmmoRecord::default()
        }
    } else {
        let Some(record) = record(m, active_record_id) else {
            return false;
        };
        record
    };
    let mut reserve_maximum = [0i16; AMMO_TYPES];
    for (ammo, maximum) in reserve_maximum.iter_mut().enumerate().skip(1) {
        *maximum = ammo_reserve_maximum(m, ammo) as i16;
    }
    let Some(owner) = store.get_mut(player) else {
        return false;
    };
    let Some(desired) = owner.desired_entry.map(usize::from) else {
        return false;
    };
    let before_reserve = owner.reserve;
    let before_hand = owner.hands.hand(side).clone();
    owner.holster(side, record, &reserve_maximum, desired);
    write_owned_reserve_changes(m, ext, &before_reserve, owner);
    write_hand(m, hand_address, &before_hand, owner.hands.hand(side));
    if owner.hands.realised_entry.is_some() {
        m.write32(ext + 0x98, desired as u32);
    }
    true
}

/// Commit the equipped-once cell after 0x1BD820's existing callbacks have completed.
/// The player-slot store is authoritative once seeded; the legacy byte is mirrored only for
/// lifted readers at this hybrid boundary.
pub fn mark_equipped_once<M: GuestMemory>(m: &mut M, ext: u32, entry: usize) -> bool {
    if !valid_player_extension_address(ext) || entry >= ENTRY_COUNT {
        return false;
    }
    let slot = m.read32(ext.wrapping_add(8)) as i32;
    if !(0..WEAPON_PLAYER_SLOTS as i32).contains(&slot) {
        return false;
    }
    let player = WeaponPlayerId::new(slot as u16);
    OWNED_INVENTORIES.with(|store| {
        if store.get(player).is_none() {
            // The hybrid adapter imports once at first attachment. Later equipped writes do not
            // re-read the equipped bitmap, so changes in the guest view cannot replace Rust state.
            let (initial, _) = read_weapon_inventory(m, ext);
            let Some(owner) = store.get_or_default(player) else {
                return false;
            };
            *owner = initial;
        }
        let Some(handle) = store.handle_for(player) else {
            return false;
        };
        if !store.mark_equipped_once_for_lifetime(handle, entry) {
            return false;
        }
        m.write8(
            ext + crate::leaf_wire::inventory::EXT_EQUIPPED + entry as u32,
            1,
        );
        true
    })
}

/// Capture the typed owner target for the measured post-callback write in `0x1BD820`.
/// The lane calls `mark_equipped_once` with this target only after the original callback returns.
/// T1678 establishes `ext` at entry stack `+4`, and the current realised entry at `ext+0x98`.
pub fn equipped_once_effect_target<M: GuestMemory>(
    va: u32,
    m: &M,
    regs: &ExtRegisters,
) -> Option<(u32, usize)> {
    if va != 0x1bd820 {
        return None;
    }
    let ext = m.read32(regs.esp.wrapping_add(4));
    if !valid_player_extension_address(ext) {
        return None;
    }
    let entry = m.read32(ext.wrapping_add(0x98));
    (entry < ENTRY_COUNT as u32).then_some((ext, entry as usize))
}

/// Rust-owned request/normalization/transition-start boundary for 0x1BF9E0.
pub fn handoff_from_stack<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    let entry_esp = regs.esp;
    let ext = m.read32(entry_esp.wrapping_add(4));
    let raw_entry = m.read32(entry_esp.wrapping_add(8));
    if !valid_player_extension_address(ext) {
        return false;
    }
    // Mirror the lifted prologue and its persistent stack writes: save ESI before the
    // unchanged-edge check, then push the normalizer's cdecl arguments only when needed.
    m.write32(entry_esp.wrapping_sub(4), regs.esi);
    regs.esp = entry_esp.wrapping_sub(4);
    regs.esi = ext;
    let (mut system, mut legacy) = read_weapon_inventory(m, ext);
    // Preserve the original root's register results even when it takes the no-change edge.
    regs.ecx = raw_entry;
    regs.eax = legacy.current;
    if legacy.current == raw_entry {
        regs.esi = m.read32(regs.esp);
        regs.esp = regs.esp.wrapping_add(4);
        return true;
    }
    let old_desired = legacy.current as usize;
    crate::c2r::rt::RegsExt::push(regs, m, raw_entry);
    crate::c2r::rt::RegsExt::push(regs, m, ext);
    crate::c2r::rt::RegsExt::call(regs, guest, 0x1ba130);
    let normalized = m.read32(ext + 0x94) as usize;
    if !system.request_handoff(raw_entry as usize, normalized) {
        return false;
    }
    // The lifted caller pops the two arguments after the normalizer's RET, then restores ESI.
    regs.esp = regs.esp.wrapping_add(8);
    m.write32(
        ext + 0x98,
        system.hands.realised_entry.unwrap_or(old_desired) as u32,
    );
    legacy.set_current(m, system.desired_entry.map(u32::from).unwrap_or(u32::MAX));
    let player_index = m.read32(ext + 8);
    let player_count = crate::state_wire::get_players(m);
    // The lifted root leaves these values when no local-player controller is addressable.
    regs.eax = player_index;
    regs.ecx = player_count;
    if (player_index as i32) < (player_count as i32) {
        let player = m
            .read32(0x7b0c48)
            .wrapping_add(player_index.wrapping_mul(0x1584));
        for (index, offset) in [0x1c8, 0x5a0, 0x978].into_iter().enumerate() {
            let controller = &system.hands.controllers[index];
            m.write32(player.wrapping_add(offset), controller.state);
            m.write32(player.wrapping_add(offset + 4), controller.timer);
            regs.ecx = controller.state;
        }
        regs.eax = player;
    }
    regs.esi = m.read32(regs.esp);
    regs.esp = regs.esp.wrapping_add(4);
    true
}

/// Run the equip request against retained Rust-owned selection, realised-hand, and controller
/// state. The guest normalizer remains an ABI-edge dependency; its published result is committed
/// into the owner and mirrored only for lifted readers.
pub fn handoff_from_owned_store<M: GuestMemory, C: GuestCall, const PLAYER_SLOTS: usize>(
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    store: &mut WeaponInventoryStore<PLAYER_SLOTS>,
    player_id: WeaponPlayerId,
) -> bool {
    let entry_esp = regs.esp;
    let ext = m.read32(entry_esp.wrapping_add(4));
    let raw_entry = m.read32(entry_esp.wrapping_add(8));
    if !valid_player_extension_address(ext) {
        return false;
    }
    let Some(owner) = store.get_mut(player_id) else {
        return false;
    };
    let Some(current) = owner.desired_entry.map(u32::from) else {
        return false;
    };

    // Keep the root's persistent ESI save and no-change result contract.
    m.write32(entry_esp.wrapping_sub(4), regs.esi);
    regs.esp = entry_esp.wrapping_sub(4);
    regs.esi = ext;
    regs.ecx = raw_entry;
    regs.eax = current;
    if current == raw_entry {
        regs.esi = m.read32(regs.esp);
        regs.esp = regs.esp.wrapping_add(4);
        return true;
    }

    crate::c2r::rt::RegsExt::push(regs, m, raw_entry);
    crate::c2r::rt::RegsExt::push(regs, m, ext);
    crate::c2r::rt::RegsExt::call(regs, guest, 0x1ba130);
    let normalized = m.read32(ext + 0x94) as usize;
    if !owner.request_handoff(raw_entry as usize, normalized) {
        regs.esp = regs.esp.wrapping_add(8);
        regs.esi = m.read32(regs.esp);
        regs.esp = regs.esp.wrapping_add(4);
        return false;
    }

    // The normalizer's output and the handoff result are now committed from the owner.
    regs.esp = regs.esp.wrapping_add(8);
    let realised = owner.hands.realised_entry.unwrap_or(current as usize);
    m.write32(
        ext + 0x94,
        owner.desired_entry.map(u32::from).unwrap_or(u32::MAX),
    );
    m.write32(ext + 0x98, realised as u32);
    let player_index = m.read32(ext + 8);
    let player_count = crate::state_wire::get_players(m);
    regs.eax = player_index;
    regs.ecx = player_count;
    if (player_index as i32) < (player_count as i32) {
        let player = m
            .read32(0x7b0c48)
            .wrapping_add(player_index.wrapping_mul(0x1584));
        for (index, offset) in [0x1c8, 0x5a0, 0x978].into_iter().enumerate() {
            let controller = &owner.hands.controllers[index];
            m.write32(player.wrapping_add(offset), controller.state);
            m.write32(player.wrapping_add(offset + 4), controller.timer);
            regs.ecx = controller.state;
        }
        regs.eax = player;
    }
    regs.esi = m.read32(regs.esp);
    regs.esp = regs.esp.wrapping_add(4);
    true
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    /// T3610 detector: `tsfp-port-wire::leaf_wire` only sees this runtime through the registered hook. If `configure` ever
    /// stopped registering it, the equipped-byte path would silently go back to the raw store.
    #[test]
    fn the_wire_hook_follows_the_runtime_switch() {
        let _guard = runtime_store_test_guard();
        configure(true);
        assert!(tsfp_port_wire::weapon_hands_hook::enabled());
        let mut memory = Ram::default();
        assert_eq!(
            tsfp_port_wire::weapon_hands_hook::mark_equipped_once(&mut memory, 0, 0),
            mark_equipped_once(&mut memory, 0, 0)
        );
        configure(false);
        assert!(!tsfp_port_wire::weapon_hands_hook::enabled());
    }

    #[derive(Clone, Default, PartialEq, Eq)]
    struct Ram(BTreeMap<u32, u8>);

    impl GuestMemory for Ram {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    struct RejectLowRam(Ram);

    impl GuestMemory for RejectLowRam {
        fn read8(&self, address: u32) -> u8 {
            assert!(
                address >= MIN_GUEST_DATA_ADDRESS,
                "low guest dereference at {address:#x}"
            );
            self.0.read8(address)
        }

        fn write8(&mut self, address: u32, value: u8) {
            self.0.write8(address, value);
        }
    }

    impl Ram {
        fn write16(&mut self, address: u32, value: u16) {
            for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
                self.write8(address + offset as u32, byte);
            }
        }
        fn write32(&mut self, address: u32, value: u32) {
            for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
                self.write8(address + offset as u32, byte);
            }
        }
    }

    #[test]
    fn low_weapon_record_table_pointer_is_declined_before_dereference() {
        let mut ram = Ram::default();
        ram.write32(RECORD_BASE_CELL, 0x1812);
        let memory = RejectLowRam(ram.clone());
        assert!(record(&memory, 0).is_none());
    }

    #[test]
    fn entry_definition_keeps_group_when_ammo_index_is_unmapped() {
        const RECORDS: u32 = 0x0030_0000;
        const ENTRY: u32 = 2;
        let mut memory = Ram::default();
        memory.write32(RECORD_BASE_CELL, RECORDS);
        memory.write32(
            ENTRY_DATA_TABLE + ENTRY * ENTRY_DATA_STRIDE + ENTRY_RECORD_OFFSET,
            1,
        );
        let flags = ENTRY_FLAGS_BASE + ENTRY * ENTRY_FLAGS_STRIDE;
        memory.write32(flags, 0x08);
        memory.write32(flags + ENTRY_GROUP_OFFSET, 7);
        let record = RECORDS + RECORD_STRIDE;
        memory.write32(record + 0x14, 44);
        memory.write32(record + 0x18, 5);
        memory.write32(record + 0x1c, u32::MAX);
        memory.write32(record + 0x20, 6);

        let definitions = read_inventory_entry_definitions(&memory);
        let definition = definitions[ENTRY as usize].expect("valid entry record");
        assert_eq!(definition.group, 7);
        assert_eq!(definition.flags, 0x08);
        assert!(definition.record_present);
        assert_eq!(definition.primary_ammo, None);
        assert_eq!(definition.secondary_ammo, None);
        assert_eq!(
            definition.grant_saved_rounds,
            Some(GrantSavedRounds {
                primary: 5,
                secondary: 6,
            })
        );
    }

    #[test]
    fn weapon_inventory_trace_projection_is_typed_and_stable() {
        let mut owner = WeaponInventory::default();
        owner.desired_entry = Some(7);
        owner.hands.realised_entry = Some(9);
        owner.default_throwable = Some(10);
        owner.collected_latch = true;
        owner.owned[3] = true;
        owner.hands.equipped_once[4] = true;
        owner.reserve[5] = -6;
        owner.hands.hands[0].record = Some(WeaponRecordId(12));
        owner.hands.hands[0].loaded[6] = -7;
        owner.hands.hands[0].saved_rounds[8] = 9;
        owner.hands.hands[0].reload_timer = -10;
        owner.hands.hands[0].hand_counters = [11, 12];
        owner.hands.hands[0].child = Some(13);
        owner.fire_mode_selection = Some(FireModeSelection::new(14, 15));

        let mut words = [0; WEAPON_TRACE_WORDS];
        assert!(encode_weapon_inventory_trace(&owner, &mut words));
        assert_eq!(&words[..6], &[1, 1, 7, 9, 10, 1]);
        assert_eq!(words[6 + 3], 1);
        assert_eq!(words[76 + 4], 1);
        assert_eq!(words[146 + 5], (-6i32) as u32);
        assert_eq!(words[190], 12);
        assert_eq!(words[191 + 6], (-7i32) as u32);
        assert_eq!(words[235 + 8], 9);
        assert_eq!(words[279], (-10i32) as u32);
        assert_eq!(&words[280..282], &[11, 12]);
        assert_eq!(words[282], 13);
        // The source-backed mode pair extends the projection after both complete hand records.
        assert_eq!(&words[376..378], &[14, 15]);
        assert_eq!(words.len(), 378);
    }

    #[test]
    fn guest_snapshot_import_preserves_throwable_and_collect_latch_bit() {
        const EXT: u32 = 0x0010_0000;
        let mut memory = Ram::default();
        memory.write32(EXT + 0x28, EXT_COLLECT_LATCH);
        memory.write32(EXT + 0xa0, 12);
        memory.write32(EXT + 0x5c, 13);
        memory.write32(EXT + 0x60, 14);

        let (owner, _) = read_weapon_inventory(&memory, EXT);

        assert_eq!(owner.default_throwable, Some(12));
        assert_eq!(
            owner.fire_mode_selection,
            Some(FireModeSelection::new(13, 14))
        );
        assert!(owner.collected_latch);

        memory.write32(EXT + 0x28, 0x100);
        let (other_flags, _) = read_weapon_inventory(&memory, EXT);
        assert!(!other_flags.collected_latch);
    }

    #[test]
    fn grant_event_resolves_live_rows_into_owned_group_and_saved_rounds() {
        const RECORDS: u32 = 0x0030_0000;
        const EXT: u32 = 0x0010_0000;
        const STACK: u32 = 0x0012_0000;
        const PLAYER: u16 = 0;
        let _serial = runtime_store_test_guard();
        configure(true);
        let player = WeaponPlayerId::new(PLAYER);
        assert!(begin_player_lifetime(player, WeaponInventory::default()));

        let mut memory = Ram::default();
        memory.write32(EXT + 8, u32::from(PLAYER));
        memory.write32(RECORD_BASE_CELL, RECORDS);
        for (entry, record_id) in [(2u32, 1u32), (3, 2)] {
            let data_row = ENTRY_DATA_TABLE + entry * ENTRY_DATA_STRIDE;
            memory.write32(data_row + ENTRY_RECORD_OFFSET, record_id);
            let flag_row = ENTRY_FLAGS_BASE + entry * ENTRY_FLAGS_STRIDE;
            memory.write32(flag_row, 0x08);
            memory.write32(flag_row + ENTRY_GROUP_OFFSET, 7);
            let record_row = RECORDS + record_id * RECORD_STRIDE;
            memory.write32(record_row + 0x14, 3);
            memory.write32(record_row + 0x18, 5);
            memory.write32(record_row + 0x1c, 4);
            memory.write32(record_row + 0x20, 6);
        }

        let mut regs = ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: STACK,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: crate::abi::Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [crate::abi::Xmm { lanes: [0; 4] }; 5],
            xmm_hi: [crate::abi::Xmm { lanes: [0; 4] }; 3],
        };
        memory.write32(STACK + 4, EXT);
        memory.write32(STACK + 8, 2);
        assert!(!dispatch_nested(
            0x1bfaa0,
            &mut memory,
            &mut NoopGuest,
            &mut regs
        ));
        OWNED_INVENTORIES.with(|store| {
            let owner = store.get(player).expect("retained player owner");
            assert!(owner.owned[2] && owner.owned[3]);
            assert!(!owner.collected_latch);
            for side in [HandSide::Left, HandSide::Right] {
                assert_eq!(owner.hands.hand(side).saved_rounds[3], 5);
                assert_eq!(owner.hands.hand(side).saved_rounds[4], 6);
            }
        });

        assert!(!dispatch_nested(
            0x1bfca0,
            &mut memory,
            &mut NoopGuest,
            &mut regs
        ));
        OWNED_INVENTORIES.with(|store| {
            let owner = store.get(player).expect("retained player owner");
            assert!(owner.owned[2] && owner.owned[3]);
            assert!(owner.collected_latch);
            for side in [HandSide::Left, HandSide::Right] {
                assert_eq!(owner.hands.hand(side).saved_rounds[3], 5);
                assert_eq!(owner.hands.hand(side).saved_rounds[4], 6);
            }
        });
        let mut diagnostics = [0; GRANT_DIAGNOSTIC_WORDS];
        assert!(unsafe {
            tsfp_weapon_inventory_adapter_diagnostics(
                diagnostics.as_mut_ptr(),
                diagnostics.len() as u32,
            )
        });
        assert_eq!(&diagnostics[..9], &[1, 1, 1, 1, 1, 0, 0, 0, 0]);
        assert_eq!(
            &diagnostics[9..],
            &[0x1bfca0, EXT, 2, 0, 0, 1, 7, 0x08, 3, 4, 5, 6, 0, 0, 0,]
        );
        assert!(retire_player_lifetime(player));
        configure(false);
    }

    #[test]
    fn normalized_selector_updates_only_the_owned_desired_entry() {
        let _serial = runtime_store_test_guard();
        configure(true);
        let player = WeaponPlayerId::new(0);
        let mut owner = WeaponInventory::default();
        owner.desired_entry = Some(1);
        owner.hands.realised_entry = Some(0);
        owner.reserve[4] = 12;
        assert!(begin_player_lifetime(player, owner));

        let mut memory = Ram::default();
        let ext = MIN_GUEST_DATA_ADDRESS;
        memory.write32(ext + 8, 0);
        memory.write32(ext + 0x94, 2);
        assert!(commit_normalized_selection_from_guest(&memory, ext));
        OWNED_INVENTORIES.with(|store| {
            let owner = store.get(player).expect("player owner remains installed");
            assert_eq!(owner.desired_entry, Some(2));
            assert_eq!(owner.hands.realised_entry, Some(0));
            assert_eq!(owner.reserve[4], 12);
        });
        configure(false);
    }

    #[test]
    fn native_cycle_selector_updates_only_the_owned_desired_entry() {
        let _serial = runtime_store_test_guard();
        configure(true);
        let player = WeaponPlayerId::new(0);
        let mut owner = WeaponInventory::default();
        owner.desired_entry = Some(1);
        owner.hands.realised_entry = Some(0);
        owner.reserve[4] = 12;
        assert!(begin_player_lifetime(player, owner));

        let mut memory = Ram::default();
        let ext = MIN_GUEST_DATA_ADDRESS;
        memory.write32(ext + 8, 0);
        assert!(commit_cycle_selection_from_guest(&memory, ext, 2));
        OWNED_INVENTORIES.with(|store| {
            let owner = store.get(player).expect("player owner remains installed");
            assert_eq!(owner.desired_entry, Some(2));
            assert_eq!(owner.hands.realised_entry, Some(0));
            assert_eq!(owner.reserve[4], 12);
        });
        assert!(!commit_cycle_selection_from_guest(
            &memory,
            ext,
            ENTRY_COUNT as u32
        ));
        configure(false);
    }

    #[test]
    fn cycle_selector_uses_owned_entry_flags_for_installed_player() {
        let _serial = runtime_store_test_guard();
        configure(true);
        let player = WeaponPlayerId::new(0);
        let mut owner = WeaponInventory::default();
        owner.desired_entry = Some(1);
        owner.hands.realised_entry = Some(0);
        owner.owned[3] = true;
        owner.reserve[4] = 12;
        assert!(begin_player_lifetime(player, owner));

        let entries = core::array::from_fn(|slot| crate::rea_weapon_cycle::WeaponCycleEntry {
            id: slot as u32,
            selection_flag_0x10: true,
            // Deliberately unlike owner.owned[2]: the native path must ignore this guest-shaped
            // compatibility field once a Rust player lifetime is installed.
            owned: slot == 2,
        });
        assert_eq!(next_cycle_entry_for_player(player, &entries, 1), Some(3));
        OWNED_INVENTORIES.with(|store| {
            let owner = store.get(player).expect("player owner remains installed");
            assert_eq!(owner.desired_entry, Some(3));
            assert_eq!(owner.hands.realised_entry, Some(0));
            assert_eq!(owner.reserve[4], 12);
        });
        configure(false);
    }

    #[derive(Clone, Default)]
    struct SharedRam(Rc<RefCell<BTreeMap<u32, u8>>>);

    impl GuestMemory for SharedRam {
        fn read8(&self, address: u32) -> u8 {
            self.0.borrow().get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.borrow_mut().insert(address, value);
        }
    }

    impl SharedRam {
        fn write32(&mut self, address: u32, value: u32) {
            for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
                self.write8(address + offset as u32, byte);
            }
        }
        fn snapshot(&self) -> BTreeMap<u32, u8> {
            self.0.borrow().clone()
        }
    }

    struct HandoffGuest {
        memory: SharedRam,
        normalized_entry: u32,
        calls: Vec<(u32, u32)>,
    }

    impl GuestCall for HandoffGuest {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            assert_eq!(va, 0x1ba130);
            let ext = self.memory.read32(regs.esp);
            let raw_entry = self.memory.read32(regs.esp.wrapping_add(4));
            self.calls.push((ext, raw_entry));
            self.memory.write32(ext + 0x94, self.normalized_entry);
            regs.eax = self.normalized_entry;
            regs.esp = regs.esp.wrapping_add(4);
        }
    }

    const EXT: u32 = 0x100000;
    const HAND: u32 = EXT + HAND_RIGHT_OFFSET;
    const CONTROLLER: u32 = 0x110000;
    const STACK: u32 = 0x120000;
    const TABLE: u32 = 0x130000;
    const RECORD_ID: u32 = 4;
    use super::runtime_store_test_guard;

    #[derive(Default)]
    struct RecordingHandPresentation {
        events: Vec<String>,
        hand_at_create: Option<crate::weapon_hands::HandAmmoState>,
    }

    impl HandPresentation for RecordingHandPresentation {
        fn release_child(&mut self, child: u32) {
            self.events.push(format!("release:{child}"));
        }

        fn create_child(
            &mut self,
            record: WeaponRecordId,
            hand: crate::weapon_hands::HandAmmoState,
        ) -> Option<u32> {
            self.hand_at_create = Some(hand);
            self.events.push(format!("create:{}", record.0));
            Some(77)
        }

        fn attach_child(&mut self, side: HandSide, child: Option<u32>) {
            let side = match side {
                HandSide::Right => "right",
                HandSide::Left => "left",
            };
            self.events
                .push(format!("attach:{side}:{}", child.unwrap_or(u32::MAX)));
        }
    }

    struct NoopGuest;

    impl GuestCall for NoopGuest {
        fn call(&mut self, va: u32, _: &mut ExtRegisters) {
            panic!("unexpected nested call to {va:#x}");
        }
    }

    fn fixture(seed: u32) -> Ram {
        let mut memory = Ram::default();
        memory.write32(RECORD_BASE_CELL, TABLE);
        memory.write32(
            TABLE + RECORD_ID * RECORD_STRIDE + 0x0c,
            if seed & 1 != 0 { 0x2000 } else { 0 },
        );
        memory.write32(TABLE + RECORD_ID * RECORD_STRIDE + 0x14, 3);
        memory.write32(TABLE + RECORD_ID * RECORD_STRIDE + 0x18, 9 + seed % 20);
        memory.write32(TABLE + RECORD_ID * RECORD_STRIDE + 0x1c, 5);
        memory.write32(TABLE + RECORD_ID * RECORD_STRIDE + 0x20, 15 + seed % 20);
        memory.write32(HAND + HAND_RECORD, RECORD_ID);
        memory.write32(HAND + HAND_CHILD, u32::MAX);
        memory.write32(HAND + HAND_RELOAD_TIMER, 1000);
        memory.write32(CONTROLLER + 0x44, if seed & 2 != 0 { 7 } else { 0 });
        for ammo in 0..AMMO_TYPES {
            let reserve = if ammo == 3 || ammo == 5 {
                (seed.rotate_left(ammo as u32) % 35) as u16
            } else {
                0
            };
            let loaded = if ammo == 3 || ammo == 5 {
                (seed.rotate_right(ammo as u32) % 16) as u16
            } else {
                0
            };
            let saved = if ammo == 3 || ammo == 5 {
                (seed.wrapping_add(ammo as u32 * 3) % 12) as u16
            } else {
                0
            };
            memory.write16(EXT + 0x61e + ammo as u32 * 2, reserve);
            memory.write16(HAND + HAND_LOADED + ammo as u32 * 2, loaded);
            memory.write16(HAND + HAND_SAVED + ammo as u32 * 2, saved);
        }
        memory
    }

    #[test]
    fn inventory_snapshot_keeps_both_hands_and_counters() {
        assert_eq!(HAND_LEFT_OFFSET, 0xa4);
        assert_eq!(HAND_RIGHT_OFFSET, 0x1d4);
        let mut memory = fixture(0);
        let right = EXT + HAND_RIGHT_OFFSET;
        let left = EXT + HAND_LEFT_OFFSET;
        memory.write16(right + HAND_LOADED + 3 * 2, 7);
        memory.write16(left + HAND_SAVED + 5 * 2, 11);
        memory.write32(right + HAND_COUNTERS, 23);
        memory.write32(right + HAND_COUNTERS + 4, 29);
        memory.write32(left + HAND_COUNTERS, 31);
        memory.write32(left + HAND_COUNTERS + 4, 37);

        let (owner, _) = read_weapon_inventory(&memory, EXT);
        let right_state = owner.hands.hand(HandSide::Right);
        let left_state = owner.hands.hand(HandSide::Left);
        assert_eq!(right_state.loaded[3], 7);
        assert_eq!(right_state.hand_counters, [23, 29]);
        assert_eq!(left_state.saved_rounds[5], 11);
        assert_eq!(left_state.hand_counters, [31, 37]);
    }

    #[test]
    fn owned_holster_uses_retained_ammo_and_desired_entry() {
        let mut memory = fixture(0);
        memory.write32(EXT + 8, 0);
        memory.write32(EXT + 0x94, 5); // stale desired-entry view
        memory.write32(EXT + 0x98, 9); // stale realised-entry view
        memory.write32(AMMO_TABLE + 3 * AMMO_ROW_STRIDE, 100);
        memory.write16(EXT + 0x61e + 3 * 2, 77); // stale reserve view
        memory.write16(HAND + HAND_LOADED + 3 * 2, 88); // stale loaded view

        let player = WeaponPlayerId::new(0);
        let mut store = WeaponInventoryStore::<1>::default();
        let owner = store.get_or_default(player).unwrap();
        owner.desired_entry = Some(2);
        owner.reserve[3] = 10;
        owner.hands.realised_entry = Some(1);
        let right = owner.hands.hand_mut(HandSide::Right);
        right.record = Some(WeaponRecordId(RECORD_ID as u16));
        right.loaded[3] = 5;

        assert!(holster_transition_with_owned_store(
            &mut memory,
            EXT,
            HAND,
            RECORD_ID,
            &mut store,
            player,
        ));

        let owner = store.get(player).unwrap();
        assert_eq!(owner.reserve[3], 15);
        assert_eq!(owner.hands.hand(HandSide::Right).loaded[3], 0);
        assert_eq!(owner.hands.hand(HandSide::Right).saved_rounds[3], 5);
        assert_eq!(owner.hands.realised_entry, Some(2));
        assert_eq!(memory.read16(EXT + 0x61e + 3 * 2), 15);
        assert_eq!(memory.read16(HAND + HAND_LOADED + 3 * 2), 0);
        assert_eq!(memory.read16(HAND + HAND_SAVED + 3 * 2), 5);
        assert_eq!(memory.read32(EXT + 0x98), 2);
    }

    #[test]
    fn owned_shot_admission_mutates_only_the_existing_player_owner() {
        let player = WeaponPlayerId::new(0);
        let ammo_type = crate::weapon_payload::ShotAmmoType::new(7).unwrap();
        let mut sources = [ShotAmmoSource::default(); ENTRY_COUNT];
        sources[3] = ShotAmmoSource {
            weapon: Some(WeaponRecordId(77)),
            resource_flags: 0x400,
            ammo_type: Some(ammo_type),
        };
        let mut store = WeaponInventoryStore::<1>::default();

        // Native ownership must exist before firing; no guest snapshot is an implicit fallback.
        assert!(admit_shot_with_owned_store(&mut store, player, &sources, 0, 0, 0).is_none());
        assert!(store.get(player).is_none());

        let owner = store.get_or_default(player).unwrap();
        owner.owned[3] = true;
        owner.reserve[7] = 1;
        assert_eq!(
            admit_shot_with_owned_store(&mut store, player, &sources, 0, 0x100, 0),
            Some(ShotAdmission::Selected(
                crate::weapon_payload::ShotSelection {
                    weapon: WeaponRecordId(77),
                    ammo_type: Some(ammo_type),
                    reserve_consumed: true,
                    emission_allowed: false,
                }
            ))
        );
        assert_eq!(store.get(player).unwrap().reserve[7], 0);

        // Mode 10 bypasses reserve consumption and chooses the measured fixed record.
        store.get_mut(player).unwrap().reserve[7] = 5;
        assert_eq!(
            admit_shot_with_owned_store(&mut store, player, &sources, 10, 0, 0),
            Some(ShotAdmission::Selected(
                crate::weapon_payload::ShotSelection {
                    weapon: WeaponRecordId(22),
                    ammo_type: None,
                    reserve_consumed: false,
                    emission_allowed: true,
                }
            ))
        );
        assert_eq!(store.get(player).unwrap().reserve[7], 5);
    }

    #[test]
    fn owned_ammo_add_uses_retained_hands_and_mirrors_signed_delta() {
        let mut memory = Ram::default();
        memory.write32(RECORD_BASE_CELL, TABLE);
        memory.write32(TABLE + RECORD_ID * RECORD_STRIDE + 0x14, 3);
        memory.write32(TABLE + RECORD_ID * RECORD_STRIDE + 0x1c, 3);
        memory.write32(AMMO_TABLE + 3 * AMMO_ROW_STRIDE, 10);

        let player = WeaponPlayerId::new(0);
        let mut store = WeaponInventoryStore::<16>::default();
        let owner = store.get_or_default(player).unwrap();
        owner.reserve[3] = 4;
        owner.hands.hand_mut(HandSide::Right).record = Some(WeaponRecordId(RECORD_ID as u16));
        owner.hands.hand_mut(HandSide::Right).loaded[3] = 3;
        owner.hands.hand_mut(HandSide::Left).record = Some(WeaponRecordId(RECORD_ID as u16));
        owner.hands.hand_mut(HandSide::Left).loaded[3] = 1;

        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = STACK;
        memory.write32(STACK + 4, EXT);
        memory.write32(STACK + 8, 3);
        memory.write32(STACK + 12, 9);

        assert!(add_ammo_reserve_from_stack(
            &mut memory,
            &mut regs,
            &mut store,
            player,
        ));

        assert_eq!(regs.eax as i32, 2);
        assert_eq!(store.get(player).unwrap().reserve[3], 6);
        assert_eq!(memory.read16(EXT + 0x61e + 3 * 2), 6);
    }

    /// A translated tail jump (`RegsExt::tail_call`) calls with ESP one word above the root's own return slot, so the
    /// synthetic slot is the caller's live return address: x86 keeps it (capture Tier 2 0x1CE650, 0xC84A0 write@entry ESP).
    #[test]
    fn nested_guest_call_restores_the_word_under_its_synthetic_slot() {
        let _serial = runtime_store_test_guard();
        configure(false);
        let mut memory = Ram::default();
        memory.write32(STACK - 4, 0x0003_c56f);
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = STACK;
        let handled = dispatch_nested_guest_call(0x89b90, &mut memory, &mut NoopGuest, &mut regs);
        assert!(!handled);
        assert_eq!(memory.read32(STACK - 4), 0x0003_c56f);
        assert_eq!(regs.esp, STACK);
    }

    #[test]
    fn nested_guest_call_dispatch_uses_guest_entry_stack_layout() {
        let _serial = runtime_store_test_guard();
        configure(true);
        let mut memory = Ram::default();
        memory.write32(EXT + 8, 0);
        memory.write32(AMMO_TABLE + 3 * AMMO_ROW_STRIDE, 10);
        memory.write32(STACK, EXT);
        memory.write32(STACK + 4, 3);
        memory.write32(STACK + 8, 9);
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = STACK;
        memory.write32(STACK - 4, 0x0003_c56f);
        let mut guest = NoopGuest;

        assert!(dispatch_nested_guest_call(
            0x1ba220,
            &mut memory,
            &mut guest,
            &mut regs,
        ));

        assert_eq!(regs.esp, STACK);
        assert_eq!(
            memory.read32(STACK - 4),
            0x0003_c56f,
            "handled call keeps the slot word too"
        );
        assert_eq!(regs.eax as i32, 9);
        assert_eq!(memory.read16(EXT + 0x61e + 3 * 2), 9);
        OWNED_INVENTORIES.with(|store| {
            assert_eq!(store.get(WeaponPlayerId::new(0)).unwrap().reserve[3], 9);
        });
        configure(false);
    }

    #[test]
    fn owned_store_reload_and_unload_keep_state_across_guest_view_drift() {
        let mut memory = fixture(0x35);
        let (initial, _) = read_weapon_inventory(&memory, EXT);
        let player = WeaponPlayerId::new(0);
        let mut store = WeaponInventoryStore::<1>::default();
        *store.get_or_default(player).unwrap() = initial;

        memory.write32(STACK + 4, EXT);
        memory.write32(STACK + 8, CONTROLLER);
        memory.write32(STACK + 12, 1);
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = STACK;
        regs.eax = HAND;
        let mut guest = NoopGuest;
        let mut empty_store = WeaponInventoryStore::<1>::default();
        assert!(!transfer_from_owned_store(
            &mut memory,
            &mut regs,
            &mut empty_store,
            player
        ));
        assert!(empty_store.get(player).is_none());
        assert!(dispatch_nested_with_owned_store(
            0x1b6ec0,
            &mut memory,
            &mut guest,
            &mut regs,
            &mut store,
            player
        ));
        let after_reload = store.get(player).unwrap().clone();

        // Simulate stale legacy bytes: the next typed operation must use the retained owner.
        memory.write16(EXT + 0x61e + 3 * 2, 0x7777);
        memory.write16(HAND + HAND_LOADED + 3 * 2, 0);
        memory.write32(STACK + 8, HAND);
        let (side, _, record) = resolve_hand(&memory, EXT, HAND).unwrap();
        let mut expected = after_reload;
        expected.unload_without_clamp(side, record);
        assert!(dispatch_nested_with_owned_store(
            0x1b9760,
            &mut memory,
            &mut guest,
            &mut regs,
            &mut store,
            player
        ));
        assert_eq!(store.get(player), Some(&expected));
    }

    #[test]
    fn enabled_dispatch_seeds_once_then_retains_rust_owned_hand_state() {
        let _serial = runtime_store_test_guard();
        configure(false);
        let mut memory = fixture(0x35);
        memory.write32(EXT + 8, 0);
        memory.write32(STACK + 4, EXT);
        memory.write32(STACK + 8, CONTROLLER);
        memory.write32(STACK + 12, 1);
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = STACK;
        regs.eax = HAND;
        let mut guest = NoopGuest;
        let player = WeaponPlayerId::new(0);

        configure(true);
        assert!(dispatch_nested(
            0x1b6ec0,
            &mut memory,
            &mut guest,
            &mut regs
        ));
        let after_reload = OWNED_INVENTORIES
            .with(|store| store.get(player).cloned())
            .expect("first hand call seeds the player-owned inventory");

        // Drift the legacy view. The next dispatch must unload the retained Rust hand and reserve.
        memory.write16(EXT + 0x61e + 3 * 2, 0x7777);
        memory.write16(HAND + HAND_LOADED + 3 * 2, 0);
        memory.write32(STACK + 8, HAND);
        let (side, _, record) = resolve_hand(&memory, EXT, HAND).unwrap();
        let mut expected = after_reload;
        expected.unload_without_clamp(side, record);
        assert!(dispatch_nested(
            0x1b9760,
            &mut memory,
            &mut guest,
            &mut regs
        ));
        assert_eq!(
            OWNED_INVENTORIES.with(|store| store.get(player).cloned()),
            Some(expected)
        );
        configure(false);
    }

    #[test]
    fn selected_dispatch_declines_low_extension_before_dereference() {
        let _serial = runtime_store_test_guard();
        configure(true);
        let mut memory = RejectLowRam(Ram::default());
        memory.0.write32(STACK + 4, 1);
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = STACK;
        let mut guest = NoopGuest;

        assert!(!dispatch_nested(
            0x1b6ec0,
            &mut memory,
            &mut guest,
            &mut regs
        ));
        configure(false);
    }

    #[test]
    fn reload_wrapper_host_frame_preserves_extension_and_callee_saved_registers() {
        check_reload_wrapper_frame(false);
    }

    #[test]
    fn reload_wrapper_guest_call_adds_exactly_one_return_slot() {
        check_reload_wrapper_frame(true);
    }

    fn check_reload_wrapper_frame(nested: bool) {
        let _serial = runtime_store_test_guard();
        configure(true);
        let mut memory = fixture(0);
        memory.write32(EXT + 8, 0);
        memory.write32(EXT + 0xa8, 0x12345678);
        memory.write32(CONTROLLER + 0xa8, 0x76543210);
        memory.write32(HAND + HAND_COUNTERS, 123);
        memory.write32(HAND + HAND_COUNTERS + 4, 456);
        memory.write32(0x7b0c7c, 0x140000);
        memory.write32(STACK, 0xabcdef00);
        for (index, value) in [EXT, CONTROLLER, HAND, 0].into_iter().enumerate() {
            memory.write32(STACK + 4 + index as u32 * 4, value);
        }
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = if nested { STACK + 4 } else { STACK };
        regs.esi = 0x11223344;
        regs.edi = 0x55667788;
        let entry_esp = regs.esp;
        let mut guest = NoopGuest;
        let handled = if nested {
            dispatch_nested_guest_call(0x1b7bc0, &mut memory, &mut guest, &mut regs)
        } else {
            dispatch_nested(0x1b7bc0, &mut memory, &mut guest, &mut regs)
        };
        assert!(handled);
        assert_eq!(memory.read32(EXT + 0xa8), 0x12345678);
        assert_eq!(memory.read32(CONTROLLER + 0xa8), u32::MAX);
        assert_eq!(memory.read32(HAND + HAND_COUNTERS), 1000);
        assert_eq!(memory.read32(HAND + HAND_COUNTERS + 4), 1000);
        let owner_counters = OWNED_INVENTORIES.with(|store| {
            store
                .get(WeaponPlayerId::new(0))
                .unwrap()
                .hands
                .hand(HandSide::Right)
                .hand_counters
        });
        assert_eq!(owner_counters, [1000; 2]);
        assert_eq!(regs.esp, entry_esp);
        assert_eq!(regs.esi, 0x11223344);
        assert_eq!(regs.edi, 0x55667788);
        assert_eq!(regs.eax, 0);
        if !nested {
            assert_eq!(memory.read32(STACK), 0xabcdef00);
        }
        configure(false);
    }

    #[test]
    fn saved_round_wrapper_uses_retained_hand_after_restore_clear() {
        let _serial = runtime_store_test_guard();
        configure(true);
        let mut memory = fixture(0);
        memory.write32(EXT + 8, 0);

        let player = WeaponPlayerId::new(0);
        let mut inventory = WeaponInventory::default();
        inventory.reserve[3] = 5;
        let hand = inventory.hands.hand_mut(HandSide::Right);
        hand.record = Some(WeaponRecordId(RECORD_ID as u16));
        hand.loaded[3] = 4;
        hand.saved_rounds[3] = 2;
        assert!(begin_player_lifetime(player, inventory));

        // 0x1B7BC0(ext, controller, hand, saved_round_limit=1), reached after the caller's
        // measured loaded-word clear at 0x1BDFD0.
        memory.write32(STACK + 4, EXT);
        memory.write32(STACK + 8, CONTROLLER);
        memory.write32(STACK + 12, HAND);
        memory.write32(STACK + 16, 1);
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = STACK;
        let mut guest = NoopGuest;

        assert!(crate::weapons_slice::run(
            0x1b7bc0,
            true,
            &mut memory,
            &mut guest,
            &mut regs,
        ));
        let retained = OWNED_INVENTORIES
            .with(|store| store.get(player).cloned())
            .expect("native player lifetime remains installed");
        assert_eq!(retained.reserve[3], 3);
        assert_eq!(retained.hands.hand(HandSide::Right).loaded[3], 2);
        assert_eq!(memory.read16(EXT + 0x61e + 3 * 2), 3);
        assert_eq!(memory.read16(HAND + HAND_LOADED + 3 * 2), 2);
        configure(false);
    }

    #[test]
    fn native_player_lifetime_installs_and_retires_rust_owned_state() {
        let _serial = runtime_store_test_guard();
        configure(false);
        let player = WeaponPlayerId::new(0);
        let mut inventory = WeaponInventory::default();
        inventory.reserve[3] = 17;
        inventory.owned[4] = true;

        assert!(begin_player_lifetime(player, inventory.clone()));
        assert!(!begin_player_lifetime(player, WeaponInventory::default()));
        assert_eq!(
            OWNED_INVENTORIES.with(|store| store.get(player).cloned()),
            Some(inventory)
        );
        assert!(retire_player_lifetime(player));
        assert!(!retire_player_lifetime(player));
        assert!(OWNED_INVENTORIES.with(|store| store.get(player).is_none()));

        let mut next_life = WeaponInventory::default();
        next_life.reserve[3] = 2;
        assert!(begin_player_lifetime(player, next_life.clone()));
        assert_eq!(
            OWNED_INVENTORIES.with(|store| store.get(player).cloned()),
            Some(next_life)
        );
        configure(false);
    }

    #[test]
    fn profile_restore_import_replaces_lifetime_with_typed_extension_values() {
        let _serial = RUNTIME_STORE_TEST_LOCK.lock().unwrap();
        configure(true);
        let player = WeaponPlayerId::new(0);
        let previous = begin_player_lifetime_handle(player, WeaponInventory::default()).unwrap();
        let player_record = 0x10000;
        let extension_table = 0x11000;
        let ext = 0x12000;
        let mut memory = Ram::default();
        memory.write32(player_record + 0x14, extension_table);
        memory.write32(extension_table + 0x7c, ext);
        memory.write32(ext + 8, player.slot() as u32);
        memory.write32(ext + 0x94, 7);
        memory.write8(ext + 0x54c + 3, 1);
        memory.write16(ext + 0x61e + 4 * 2, 33);
        memory.write16(ext + 0xdc + 5 * 2, 7);
        memory.write16(ext + 0x20c + 6 * 2, 9);

        let current = import_profile_restore_from_guest(&memory, player_record).unwrap();
        let snapshot = OWNED_INVENTORIES.with(|store| {
            (
                store.players.get_handle(previous).is_none(),
                store.players.get_handle(current).cloned(),
            )
        });

        assert_ne!(previous, current);
        assert!(snapshot.0);
        let owner = snapshot.1.unwrap();
        assert_eq!(owner.desired_entry, Some(7));
        assert!(owner.owned[3]);
        assert_eq!(owner.reserve[4], 33);
        assert_eq!(owner.hands.hand(HandSide::Right).loaded[5], 7);
        assert_eq!(owner.hands.hand(HandSide::Left).loaded[6], 9);
        configure(false);
    }

    #[test]
    fn payload_lifecycle_applies_only_the_live_generation_pose_to_transform() {
        let _serial = runtime_store_test_guard();
        configure(true);
        let object = ObjectId {
            slot: 2,
            generation: 9,
        };
        let pose = WeaponPayloadPose {
            pitch_yaw_roll_degrees: [8.0, -19.0, 27.0],
            staged_yaw_degrees: 73.0,
            rotation_mode: crate::weapon_payload::PayloadRotationMode::Euler,
        };
        assert_eq!(attach_payload_for_object(object, pose), Ok(()));
        let mut transform = crate::object_transform::TransformState {
            rotation_mode: crate::object_transform::RotationMode::Identity,
            rotation_dirty: false,
            reflected_rotation: false,
            angles_degrees: [0.0; 3],
            parent_uses_euler: false,
            position: [1.0, 2.0, 3.0],
            cached_rotation: crate::object_transform::WorldTransform::IDENTITY,
        };

        assert_eq!(
            apply_payload_pose_to_transform(object, &mut transform),
            Ok(())
        );
        assert_eq!(
            transform.rotation_mode,
            crate::object_transform::RotationMode::Euler
        );
        assert_eq!(transform.angles_degrees, [8.0, -19.0, 27.0]);
        assert!(transform.rotation_dirty);
        assert_eq!(transform.position, [1.0, 2.0, 3.0]);

        let before_stale = transform;
        assert_eq!(
            apply_payload_pose_to_transform(
                ObjectId {
                    generation: 10,
                    ..object
                },
                &mut transform,
            ),
            Err(WeaponPayloadLifecycleError::StaleObjectId)
        );
        assert_eq!(transform.rotation_mode, before_stale.rotation_mode);
        assert_eq!(transform.angles_degrees, before_stale.angles_degrees);
        assert_eq!(transform.rotation_dirty, before_stale.rotation_dirty);
        configure(false);
    }

    #[test]
    fn native_payload_create_attaches_c1950_pose_before_transform_projection() {
        let _serial = runtime_store_test_guard();
        configure(true);
        let object = ObjectId {
            slot: 3,
            generation: 12,
        };
        let inputs = C1950PayloadInputs {
            definition_angles_degrees: [0.0, 17.0, 0.0],
            staged_yaw_degrees: 73.0,
            template: Some(crate::weapon_payload::C1950PayloadTemplate {
                flags: 0x2,
                angles_degrees: [8.0, -19.0, 27.0],
            }),
        };
        let expected_pose = WeaponPayloadPose {
            pitch_yaw_roll_degrees: [8.0, -19.0, 27.0],
            staged_yaw_degrees: 73.0,
            rotation_mode: crate::weapon_payload::PayloadRotationMode::Euler,
        };
        let mut transform = crate::object_transform::TransformState {
            rotation_mode: crate::object_transform::RotationMode::Identity,
            rotation_dirty: false,
            reflected_rotation: false,
            angles_degrees: [1.0, 2.0, 3.0],
            parent_uses_euler: false,
            position: [4.0, 5.0, 6.0],
            cached_rotation: crate::object_transform::WorldTransform::IDENTITY,
        };

        assert_eq!(
            create_c1950_payload_for_object(object, inputs, &mut transform),
            Ok(expected_pose)
        );
        assert_eq!(payload_pose_for_object(object), Ok(expected_pose));
        assert_eq!(transform.angles_degrees, [8.0, -19.0, 27.0]);
        assert_eq!(
            transform.rotation_mode,
            crate::object_transform::RotationMode::Euler
        );
        assert!(transform.rotation_dirty);
        assert_eq!(transform.position, [4.0, 5.0, 6.0]);

        let before_rejected_create = transform;
        let rejected_inputs = C1950PayloadInputs {
            definition_angles_degrees: [90.0, 91.0, 92.0],
            staged_yaw_degrees: -18.0,
            template: None,
        };
        assert_eq!(
            create_c1950_payload_for_object(object, rejected_inputs, &mut transform),
            Err(WeaponPayloadLifecycleError::AlreadyAttached)
        );
        assert_eq!(
            transform.angles_degrees,
            before_rejected_create.angles_degrees
        );
        assert_eq!(
            transform.rotation_mode,
            before_rejected_create.rotation_mode
        );
        assert_eq!(
            transform.rotation_dirty,
            before_rejected_create.rotation_dirty
        );
        assert_eq!(payload_pose_for_object(object), Ok(expected_pose));
        configure(false);
    }

    #[test]
    fn native_fire_mode_commits_before_its_rust_fire_action() {
        let _serial = runtime_store_test_guard();
        configure(true);
        let player = WeaponPlayerId::new(0);
        let handle = begin_player_lifetime_handle(player, WeaponInventory::default()).unwrap();
        let selection = FireModeSelection::new(3, 5);
        let mut fired = false;

        assert!(commit_fire_mode_and_dispatch_for_lifetime(
            handle,
            selection,
            |owner| {
                assert_eq!(owner.fire_mode_selection, Some(selection));
                fired = true;
            },
        ));
        assert!(fired);
        assert!(retire_player_lifetime_handle(handle));

        fired = false;
        assert!(!commit_fire_mode_and_dispatch_for_lifetime(
            handle,
            selection,
            |_| fired = true,
        ));
        assert!(!fired);
        configure(false);
    }

    #[test]
    fn native_hand_realisation_mutates_owned_lifetime_in_rea_order() {
        let _serial = runtime_store_test_guard();
        configure(true);
        let player = WeaponPlayerId::new(0);
        let ammo = AmmoType::new(2).unwrap();
        let old_record = WeaponAmmoRecord {
            id: WeaponRecordId(4),
            primary: Some(ammo),
            primary_capacity: 9,
            ..WeaponAmmoRecord::default()
        };
        let next_record = WeaponAmmoRecord {
            id: WeaponRecordId(8),
            primary: Some(ammo),
            primary_capacity: 4,
            ..WeaponAmmoRecord::default()
        };
        let mut initial = WeaponInventory::default();
        initial.reserve[ammo.index()] = 2;
        let hand = initial.hands.hand_mut(HandSide::Right);
        hand.record = Some(old_record.id);
        hand.loaded[ammo.index()] = 3;
        hand.child = Some(42);
        let handle = begin_player_lifetime_handle(player, initial).unwrap();

        let mut presentation = RecordingHandPresentation::default();
        let outcome = realise_hand_for_lifetime(
            handle,
            HandSide::Right,
            Some(old_record),
            next_record,
            &mut presentation,
            false,
        )
        .expect("live player generation")
        .expect("old definition resolves");
        assert_eq!(outcome.transferred_rounds, 4);
        assert_eq!(
            outcome.publication,
            crate::weapon_hands::HandRealisePublication::Replaced {
                record: next_record.id,
                retired_child: Some(42),
                created_child: Some(77),
            }
        );
        assert_eq!(
            presentation.events,
            ["release:42", "create:8", "attach:right:77"]
        );
        let hand_at_create = presentation.hand_at_create.expect("typed request snapshot");
        assert_eq!(hand_at_create.record, Some(next_record.id));
        assert_eq!(hand_at_create.child, None);
        assert_eq!(hand_at_create.loaded[ammo.index()], 0);
        OWNED_INVENTORIES.with(|store| {
            let owner = store.get_handle(handle).unwrap();
            let hand = owner.hands.hand(HandSide::Right);
            assert_eq!(hand.record, Some(next_record.id));
            assert_eq!(hand.child, Some(77));
            assert_eq!(hand.loaded[ammo.index()], 4);
            assert_eq!(owner.reserve[ammo.index()], 1);
        });

        assert!(retire_player_lifetime_handle(handle));
        let reused = begin_player_lifetime_handle(player, WeaponInventory::default()).unwrap();
        presentation.events.clear();
        assert_eq!(
            realise_hand_for_lifetime(
                handle,
                HandSide::Right,
                Some(next_record),
                old_record,
                &mut presentation,
                false,
            ),
            None
        );
        assert!(presentation.events.is_empty());
        assert!(retire_player_lifetime_handle(reused));
        configure(false);
    }

    #[test]
    fn native_reload_transfer_commits_both_channels_for_owned_lifetime() {
        let _serial = runtime_store_test_guard();
        configure(true);
        let player = WeaponPlayerId::new(0);
        let primary = AmmoType::new(2).unwrap();
        let secondary = AmmoType::new(3).unwrap();
        let record = WeaponAmmoRecord {
            id: WeaponRecordId(9),
            primary: Some(primary),
            secondary: Some(secondary),
            primary_capacity: 5,
            secondary_capacity: 4,
            ..WeaponAmmoRecord::default()
        };
        let mut initial = WeaponInventory::default();
        initial.reserve[primary.index()] = 3;
        initial.reserve[secondary.index()] = 2;
        let hand = initial.hands.hand_mut(HandSide::Right);
        hand.record = Some(record.id);
        hand.loaded[primary.index()] = 1;
        hand.loaded[secondary.index()] = 1;
        hand.reload_timer = 24;
        let handle = begin_player_lifetime_handle(player, initial).unwrap();

        assert_eq!(
            transfer_reload_for_lifetime(handle, HandSide::Right, record, false, false),
            Some(5)
        );
        OWNED_INVENTORIES.with(|store| {
            let owner = store.get_handle(handle).unwrap();
            let hand = owner.hands.hand(HandSide::Right);
            assert_eq!(hand.loaded[primary.index()], 4);
            assert_eq!(hand.loaded[secondary.index()], 3);
            assert_eq!(hand.reload_timer, 0);
            assert_eq!(owner.reserve[primary.index()], 0);
            assert_eq!(owner.reserve[secondary.index()], 0);
        });

        assert!(retire_player_lifetime_handle(handle));
        assert_eq!(
            transfer_reload_for_lifetime(handle, HandSide::Right, record, false, false),
            None
        );
        configure(false);
    }

    #[test]
    fn native_holster_transition_uses_owned_state_and_rejects_retired_lifetime() {
        let _serial = runtime_store_test_guard();
        configure(true);
        let player = WeaponPlayerId::new(0);
        let primary = AmmoType::new(3).unwrap();
        let secondary = AmmoType::new(5).unwrap();
        let record = WeaponAmmoRecord {
            id: WeaponRecordId(11),
            primary: Some(primary),
            secondary: Some(secondary),
            ..WeaponAmmoRecord::default()
        };
        let mut initial = WeaponInventory::default();
        initial.desired_entry = Some(6);
        initial.reserve[primary.index()] = 9;
        initial.reserve[secondary.index()] = 8;
        initial.reserve[7] = 1;
        initial.hands.controllers[0].hand = Some(HandSide::Right.index());
        initial.hands.controllers[0].state = 3;
        initial.hands.controllers[0].timer = 77;
        let right = initial.hands.hand_mut(HandSide::Right);
        right.record = Some(record.id);
        right.loaded[primary.index()] = 5;
        right.loaded[secondary.index()] = 4;
        right.loaded[7] = 2;
        initial.hands.hand_mut(HandSide::Left).loaded[primary.index()] = 12;
        let handle = begin_player_lifetime_handle(player, initial).unwrap();
        let mut reserve_maximum = [0; AMMO_TYPE_COUNT];
        reserve_maximum[primary.index()] = 12;
        reserve_maximum[secondary.index()] = 10;
        reserve_maximum[7] = 5;

        assert_eq!(
            complete_holster_for_lifetime(handle, HandSide::Right, record, &reserve_maximum, 1,),
            None
        );
        OWNED_INVENTORIES.with(|store| {
            let owner = store.get_handle(handle).unwrap();
            assert_eq!(owner.reserve[primary.index()], 9);
            assert_eq!(owner.hands.hand(HandSide::Right).loaded[primary.index()], 5);
            assert_eq!(owner.hands.realised_entry, None);
        });

        assert_eq!(
            complete_holster_for_lifetime(handle, HandSide::Right, record, &reserve_maximum, 0,),
            Some(6)
        );
        OWNED_INVENTORIES.with(|store| {
            let owner = store.get_handle(handle).unwrap();
            let hand = owner.hands.hand(HandSide::Right);
            assert_eq!(owner.reserve[primary.index()], 12);
            assert_eq!(owner.reserve[secondary.index()], 10);
            assert_eq!(owner.reserve[7], 3);
            assert_eq!(hand.loaded[primary.index()], 0);
            assert_eq!(hand.loaded[secondary.index()], 0);
            assert_eq!(hand.loaded[7], 0);
            assert_eq!(hand.saved_rounds[primary.index()], 5);
            assert_eq!(hand.saved_rounds[secondary.index()], 4);
            assert_eq!(owner.hands.realised_entry, Some(6));
            assert_eq!(owner.hands.controllers[0].state, 3);
            assert_eq!(owner.hands.controllers[0].timer, 77);
            assert_eq!(owner.hands.hand(HandSide::Left).loaded[primary.index()], 12);
        });

        assert!(retire_player_lifetime_handle(handle));
        assert_eq!(
            complete_holster_for_lifetime(handle, HandSide::Right, record, &reserve_maximum, 0,),
            None
        );
        configure(false);
    }

    #[test]
    fn owned_pickup_updates_only_an_existing_player_inventory() {
        let player = WeaponPlayerId::new(0);
        let mut store = WeaponInventoryStore::<1>::default();
        let mut definitions = [None; ENTRY_COUNT];
        definitions[2] = Some(InventoryEntryDefinition {
            group: 1,
            flags: 0x10,
            record_present: true,
            primary_ammo: AmmoType::new(3),
            ..InventoryEntryDefinition::default()
        });
        let pairs = [
            PickupAmmoPair {
                ammo_type: 3,
                amount: 4,
            },
            PickupAmmoPair::default(),
        ];
        let maximums = [10; AMMO_TYPE_COUNT];

        assert!(
            collect_ammo_pickup_with_owned_store(
                &mut store,
                player,
                pairs,
                [None; 2],
                &maximums,
                false,
                &definitions,
                [None; 2],
            )
            .is_none()
        );
        store.get_or_default(player).unwrap();
        let result = collect_ammo_pickup_with_owned_store(
            &mut store,
            player,
            pairs,
            [None; 2],
            &maximums,
            false,
            &definitions,
            [None; 2],
        )
        .unwrap();

        assert_eq!(result.pickups[0].unwrap().reserve_after, 4);
        assert!(result.acquisitions[0].acquired);
        assert!(store.get(player).unwrap().owned[2]);
        assert_eq!(store.get(player).unwrap().reserve[3], 4);
    }

    #[test]
    fn native_weapon_pickup_uses_only_the_current_player_lifetime() {
        let player = WeaponPlayerId::new(0);
        let mut store = WeaponInventoryStore::<1>::default();
        let mut definitions = [None; ENTRY_COUNT];
        definitions[2] = Some(InventoryEntryDefinition {
            group: 1,
            flags: 0x200,
            record_present: true,
            primary_ammo: AmmoType::new(3),
            ..InventoryEntryDefinition::default()
        });
        let first = store
            .begin_lifetime(player, WeaponInventory::default())
            .unwrap();
        let pairs = [
            Some(WeaponPickupAmmo {
                ammo_type: AmmoType::new(3).unwrap(),
                amount: 4,
                reserve_maximum: 10,
            }),
            None,
        ];

        let accepted = collect_weapon_pickup_with_owned_store(
            &mut store,
            first,
            2,
            pairs,
            [None; 2],
            &definitions,
            false,
            false,
            None,
        )
        .unwrap();
        assert!(accepted.inventory_accepted);
        assert!(store.get_handle(first).unwrap().owned[2]);
        assert_eq!(store.get_handle(first).unwrap().reserve[3], 4);

        store.retire_handle(first).unwrap();
        let replacement = store
            .begin_lifetime(player, WeaponInventory::default())
            .unwrap();
        assert!(
            collect_weapon_pickup_with_owned_store(
                &mut store,
                first,
                2,
                pairs,
                [None; 2],
                &definitions,
                false,
                false,
                None,
            )
            .is_none()
        );
        assert!(!store.get_handle(replacement).unwrap().owned[2]);
        assert_eq!(store.get_handle(replacement).unwrap().reserve[3], 0);
    }

    #[test]
    fn native_pickup_and_shot_entrypoints_use_the_retained_player_owner() {
        let _serial = runtime_store_test_guard();
        configure(false);
        let player = WeaponPlayerId::new(0);
        let ammo_type = crate::weapon_payload::ShotAmmoType::new(7).unwrap();
        let mut sources = [ShotAmmoSource::default(); ENTRY_COUNT];
        sources[3] = ShotAmmoSource {
            weapon: Some(WeaponRecordId(77)),
            resource_flags: 0x400,
            ammo_type: Some(ammo_type),
        };

        assert!(admit_shot_for_player(player, &sources, 0, 0, 0).is_none());
        let mut initial = WeaponInventory::default();
        initial.owned[3] = true;
        initial.reserve[7] = 1;
        assert!(begin_player_lifetime(player, initial));
        assert!(matches!(
            admit_shot_for_player(player, &sources, 0, 0, 0),
            Some(ShotAdmission::Selected(selection)) if selection.reserve_consumed
        ));
        assert_eq!(
            OWNED_INVENTORIES.with(|store| store.get(player).unwrap().reserve[7]),
            0
        );

        let mut definitions = [None; ENTRY_COUNT];
        definitions[2] = Some(InventoryEntryDefinition {
            group: 1,
            flags: 0x10,
            record_present: true,
            primary_ammo: AmmoType::new(3),
            ..InventoryEntryDefinition::default()
        });
        let result = collect_ammo_pickup_for_player(
            player,
            [
                PickupAmmoPair {
                    ammo_type: 3,
                    amount: 4,
                },
                PickupAmmoPair::default(),
            ],
            [None; 2],
            &[10; AMMO_TYPE_COUNT],
            false,
            &definitions,
            [None; 2],
        )
        .unwrap();
        assert_eq!(result.pickups[0].unwrap().reserve_after, 4);
        assert!(result.acquisitions[0].acquired);
        assert_eq!(
            OWNED_INVENTORIES.with(|store| store.get(player).unwrap().reserve[3]),
            4
        );

        assert!(retire_player_lifetime(player));
        assert!(
            collect_ammo_pickup_for_player(
                player,
                [PickupAmmoPair::default(); 2],
                [None; 2],
                &[10; AMMO_TYPE_COUNT],
                false,
                &[None; ENTRY_COUNT],
                [None; 2],
            )
            .is_none()
        );
        configure(false);
    }

    #[test]
    fn equipped_once_updates_the_retained_player_inventory() {
        let _serial = runtime_store_test_guard();
        configure(false);
        let mut memory = fixture(0);
        memory.write32(EXT + 8, 0);
        for entry in 0..ENTRY_COUNT {
            memory.write8(
                EXT + crate::leaf_wire::inventory::EXT_EQUIPPED + entry as u32,
                0,
            );
        }
        let player = WeaponPlayerId::new(0);

        configure(true);
        assert!(mark_equipped_once(&mut memory, EXT, 3));
        assert_eq!(
            memory.read8(EXT + crate::leaf_wire::inventory::EXT_EQUIPPED + 3),
            1
        );

        // A later call must preserve the first typed mark when the compatibility view drifts.
        memory.write8(EXT + crate::leaf_wire::inventory::EXT_EQUIPPED + 3, 0);
        memory.write8(EXT + crate::leaf_wire::inventory::EXT_EQUIPPED + 5, 1);
        assert!(mark_equipped_once(&mut memory, EXT, 4));
        let owner = OWNED_INVENTORIES
            .with(|store| store.get(player).cloned())
            .expect("equipped-once attachment seeds the stable player owner");
        assert!(owner.hands.equipped_once[3]);
        assert!(owner.hands.equipped_once[4]);
        assert!(!owner.hands.equipped_once[5]);
        assert_eq!(
            memory.read8(EXT + crate::leaf_wire::inventory::EXT_EQUIPPED + 4),
            1
        );
        configure(false);
    }

    #[test]
    fn equipped_once_callback_target_uses_measured_extension_and_realised_entry() {
        let mut memory = fixture(0);
        memory.write32(EXT + 8, 0);
        memory.write32(EXT + 0x98, 4);
        memory.write32(STACK + 4, EXT);
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = STACK;

        assert_eq!(
            equipped_once_effect_target(0x1bd820, &memory, &regs),
            Some((EXT, 4))
        );
        assert_eq!(equipped_once_effect_target(0x1bd821, &memory, &regs), None);

        memory.write32(EXT + 0x98, ENTRY_COUNT as u32);
        assert_eq!(equipped_once_effect_target(0x1bd820, &memory, &regs), None);
        memory.write32(EXT + 0x98, 4);
        memory.write32(STACK + 4, 0);
        assert_eq!(equipped_once_effect_target(0x1bd820, &memory, &regs), None);
    }

    #[test]
    fn native_reload_and_unload_match_translated_contracts_for_seeded_guest_states() {
        for seed in 0..2048u32 {
            let mut expected = fixture(seed);
            let saved_limit = u32::from(seed & 4 != 0);
            expected.write32(STACK + 4, EXT);
            expected.write32(STACK + 8, CONTROLLER);
            expected.write32(STACK + 12, saved_limit);
            let mut actual = expected.clone();
            let mut expected_regs: ExtRegisters = unsafe { core::mem::zeroed() };
            expected_regs.esp = STACK;
            expected_regs.eax = HAND;
            let mut actual_regs = expected_regs;
            crate::c2r::objchar_wired3::game_ammo_reload_transfer_reserve_to_clip_for_both_ammo_types(
                &mut expected,
                &mut expected_regs,
            );
            assert!(transfer_from_registers(&mut actual, &mut actual_regs));
            for ammo in 0..AMMO_TYPES {
                assert_eq!(
                    actual.read16(EXT + 0x61e + ammo as u32 * 2),
                    expected.read16(EXT + 0x61e + ammo as u32 * 2),
                    "reload reserve seed {seed} ammo {ammo}"
                );
                assert_eq!(
                    actual.read16(HAND + HAND_LOADED + ammo as u32 * 2),
                    expected.read16(HAND + HAND_LOADED + ammo as u32 * 2),
                    "reload loaded seed {seed} ammo {ammo}"
                );
            }
            assert_eq!(
                actual.read32(HAND + HAND_RELOAD_TIMER),
                expected.read32(HAND + HAND_RELOAD_TIMER)
            );
            assert_eq!(
                actual_regs.eax, expected_regs.eax,
                "reload result seed {seed}"
            );
            assert_eq!(
                actual_regs.esp, expected_regs.esp,
                "reload stack seed {seed}"
            );

            let mut expected = fixture(seed);
            expected.write32(STACK + 4, EXT);
            expected.write32(STACK + 8, HAND);
            let mut actual = expected.clone();
            let mut expected_regs: ExtRegisters = unsafe { core::mem::zeroed() };
            expected_regs.esp = STACK;
            let actual_regs = expected_regs;
            crate::c2r::objchar_wired3::game_ammo_unload_clip_back_into_reserve_both_types(
                &mut expected,
                &mut expected_regs,
            );
            assert!(unload_from_stack(&mut actual, &actual_regs));
            for ammo in 0..AMMO_TYPES {
                assert_eq!(
                    actual.read16(EXT + 0x61e + ammo as u32 * 2),
                    expected.read16(EXT + 0x61e + ammo as u32 * 2),
                    "unload reserve seed {seed} ammo {ammo}"
                );
                assert_eq!(
                    actual.read16(HAND + HAND_LOADED + ammo as u32 * 2),
                    expected.read16(HAND + HAND_LOADED + ammo as u32 * 2),
                    "unload loaded seed {seed} ammo {ammo}"
                );
            }
        }
    }

    #[test]
    fn owned_store_reload_matches_lifted_transfer_for_seeded_initial_states() {
        for seed in 0..2048u32 {
            let mut expected = fixture(seed);
            let saved_limit = u32::from(seed & 4 != 0);
            expected.write32(STACK + 4, EXT);
            expected.write32(STACK + 8, CONTROLLER);
            expected.write32(STACK + 12, saved_limit);
            let mut expected_regs: ExtRegisters = unsafe { core::mem::zeroed() };
            expected_regs.esp = STACK;
            expected_regs.eax = HAND;
            crate::c2r::objchar_wired3::game_ammo_reload_transfer_reserve_to_clip_for_both_ammo_types(
                &mut expected,
                &mut expected_regs,
            );

            let mut actual = fixture(seed);
            actual.write32(STACK + 4, EXT);
            actual.write32(STACK + 8, CONTROLLER);
            actual.write32(STACK + 12, saved_limit);
            let (initial, _) = read_weapon_inventory(&actual, EXT);
            let player = WeaponPlayerId::new(0);
            let mut store = WeaponInventoryStore::<1>::default();
            *store.get_or_default(player).unwrap() = initial;
            let mut actual_regs: ExtRegisters = unsafe { core::mem::zeroed() };
            actual_regs.esp = STACK;
            actual_regs.eax = HAND;
            assert!(transfer_from_owned_store(
                &mut actual,
                &mut actual_regs,
                &mut store,
                player,
            ));

            for ammo in 0..AMMO_TYPES {
                assert_eq!(
                    actual.read16(EXT + 0x61e + ammo as u32 * 2),
                    expected.read16(EXT + 0x61e + ammo as u32 * 2),
                    "owned reload reserve seed {seed} ammo {ammo}"
                );
                assert_eq!(
                    actual.read16(HAND + HAND_LOADED + ammo as u32 * 2),
                    expected.read16(HAND + HAND_LOADED + ammo as u32 * 2),
                    "owned reload loaded seed {seed} ammo {ammo}"
                );
            }
            assert_eq!(
                actual.read32(HAND + HAND_RELOAD_TIMER),
                expected.read32(HAND + HAND_RELOAD_TIMER),
                "owned reload timer seed {seed}"
            );
            assert_eq!(
                actual_regs.eax, expected_regs.eax,
                "owned reload result seed {seed}"
            );
            assert_eq!(actual_regs.esp, expected_regs.esp);
        }
    }

    #[test]
    fn handoff_matches_lifted_root_registers_and_memory_on_changed_and_unchanged_edges() {
        const PLAYER: u32 = 0x140000;
        for (current, raw, normalized, player_index, player_count) in [
            (3, 3, 3, 0, 1), // no-change edge does not call the normalizer
            (1, 4, 2, 0, 1), // local player: starts all three transitions
            (1, 4, 2, 2, 1), // non-local/out-of-range player: leaves controller records alone
        ] {
            let expected_backing = Rc::new(RefCell::new(BTreeMap::new()));
            let mut expected = SharedRam(expected_backing.clone());
            expected.write32(EXT + 0x94, current);
            expected.write32(EXT + 0x98, 0x55);
            expected.write32(EXT + 8, player_index);
            expected.write32(0x790950, player_count);
            expected.write32(0x7b0c48, PLAYER);
            expected.write32(PLAYER + 0x1c8, 8);
            expected.write32(PLAYER + 0x1cc, 12);
            expected.write32(PLAYER + 0x5a0, 9);
            expected.write32(PLAYER + 0x5a4, 13);
            expected.write32(PLAYER + 0x978, 10);
            expected.write32(PLAYER + 0x97c, 14);
            expected.write32(STACK + 4, EXT);
            expected.write32(STACK + 8, raw);
            let mut actual = SharedRam(Rc::new(RefCell::new(expected.snapshot())));

            let mut expected_guest = HandoffGuest {
                memory: expected.clone(),
                normalized_entry: normalized,
                calls: Vec::new(),
            };
            let mut actual_guest = HandoffGuest {
                memory: actual.clone(),
                normalized_entry: normalized,
                calls: Vec::new(),
            };
            let mut expected_regs: ExtRegisters = unsafe { core::mem::zeroed() };
            expected_regs.esp = STACK;
            expected_regs.eax = 0xaaaa;
            expected_regs.ecx = 0xbbbb;
            expected_regs.esi = 0xcccc;
            let mut actual_regs = expected_regs;

            crate::c2r::gameplay_inventory::run(
                0x1bf9e0,
                &mut expected,
                &mut expected_guest,
                &mut expected_regs,
            );
            assert!(handoff_from_stack(
                &mut actual,
                &mut actual_guest,
                &mut actual_regs
            ));

            assert_eq!(actual.snapshot(), expected.snapshot());
            assert_eq!(actual_regs, expected_regs);
            assert_eq!(actual_guest.calls, expected_guest.calls);
            assert_eq!(actual_guest.calls.len(), usize::from(current != raw));
        }
    }

    #[test]
    fn owned_handoff_uses_retained_selection_and_hand_state() {
        const PLAYER: u32 = 0x140000;
        let backing = Rc::new(RefCell::new(BTreeMap::new()));
        let mut memory = SharedRam(backing.clone());
        memory.write32(EXT + 8, 0);
        memory.write32(EXT + 0x94, 3); // stale guest desired entry; owner below has entry 1
        memory.write32(EXT + 0x98, 9); // stale realised entry
        memory.write32(0x790950, 1);
        memory.write32(0x7b0c48, PLAYER);
        memory.write32(STACK + 4, EXT);
        memory.write32(STACK + 8, 4);

        let player = WeaponPlayerId::new(0);
        let mut store = WeaponInventoryStore::<1>::default();
        let owner = store.get_or_default(player).unwrap();
        owner.desired_entry = Some(1);
        owner.hands.realised_entry = Some(0);
        let mut guest = HandoffGuest {
            memory: SharedRam(backing),
            normalized_entry: 2,
            calls: Vec::new(),
        };
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = STACK;
        regs.esi = 0xcccc;

        // ABI residue belongs to the replaceable boundary adapter. The REA contract does not
        // promise preserved ESI/ESP, so compare those outputs with the existing guest-snapshot
        // adapter instead of asserting a guessed native register convention.
        let mut reference_memory = SharedRam(Rc::new(RefCell::new(memory.snapshot())));
        reference_memory.write32(EXT + 0x94, 1);
        let mut reference_guest = HandoffGuest {
            memory: reference_memory.clone(),
            normalized_entry: 2,
            calls: Vec::new(),
        };
        let mut reference_regs = regs;
        assert!(handoff_from_stack(
            &mut reference_memory,
            &mut reference_guest,
            &mut reference_regs,
        ));

        assert!(dispatch_nested_with_owned_store(
            0x1bf9e0,
            &mut memory,
            &mut guest,
            &mut regs,
            &mut store,
            player,
        ));

        let owner = store.get(player).unwrap();
        assert_eq!(owner.desired_entry, Some(2));
        assert_eq!(owner.hands.realised_entry, Some(1));
        assert!(
            owner
                .hands
                .controllers
                .iter()
                .all(|controller| controller.state == 3 && controller.timer == 0)
        );
        assert_eq!(memory.read32(EXT + 0x94), 2);
        assert_eq!(memory.read32(EXT + 0x98), 1);
        assert_eq!(memory.read32(PLAYER + 0x1c8), 3);
        assert_eq!(regs.eax, PLAYER);
        assert_eq!(regs.ecx, 3);
        assert_eq!(regs, reference_regs);
        assert_eq!(guest.calls, [(EXT, 4)]);
        assert_eq!(guest.calls, reference_guest.calls);
    }

    #[test]
    fn handoff_root_dispatch_keeps_the_persistent_inventory_owner() {
        const PLAYER: u32 = 0x140000;
        let _serial = runtime_store_test_guard();
        configure(true);
        let fixture = fixture(0);
        let mut memory = SharedRam(Rc::new(RefCell::new(fixture.0.clone())));
        memory.write32(EXT + 8, 0);
        memory.write32(EXT + 0x94, 3); // stale guest desired entry
        memory.write32(EXT + 0x98, 9); // stale realised entry
        memory.write32(0x790950, 1);
        memory.write32(0x7b0c48, PLAYER);
        memory.write32(STACK + 4, EXT);
        memory.write32(STACK + 8, 4);

        let player = WeaponPlayerId::new(0);
        let mut owner = WeaponInventory::default();
        owner.desired_entry = Some(1);
        owner.hands.realised_entry = Some(0);
        owner.owned[1] = true;
        owner.reserve[3] = 17;
        assert!(begin_player_lifetime(player, owner));

        let mut guest = HandoffGuest {
            memory: memory.clone(),
            normalized_entry: 2,
            calls: Vec::new(),
        };
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = STACK;

        assert!(dispatch_nested(
            0x1bf9e0,
            &mut memory,
            &mut guest,
            &mut regs
        ));

        OWNED_INVENTORIES.with(|store| {
            let owner = store.get(player).expect("handoff retains attached owner");
            assert_eq!(owner.desired_entry, Some(2));
            assert_eq!(owner.hands.realised_entry, Some(1));
            assert!(owner.owned[1]);
            assert_eq!(owner.reserve[3], 17);
        });
        assert_eq!(memory.read32(EXT + 0x94), 2);
        assert_eq!(memory.read32(EXT + 0x98), 1);
        assert_eq!(guest.calls, [(EXT, 4)]);
        configure(false);
    }

    #[test]
    fn runtime_switch_is_default_off_and_explicitly_configurable() {
        let _serial = runtime_store_test_guard();
        configure(false);
        assert!(!enabled());
        assert!(!selects_root(0x1b6ec0));
        assert!(crate::runtime::ROOTS.contains(&0x1bf9e0));
        assert!(!selects_root(0x1bf9e0));
        configure(true);
        assert!(enabled());
        assert!(selects_root(0x1b6ec0));
        assert!(selects_root(0x1b7bc0));
        assert!(selects_root(0x1b9760));
        assert!(selects_root(0x1bf9e0));
        assert!(!selects_root(0x1bc990));
        assert!(!selects_root(0x1bd820));
        assert!(!selects_root(0x1bdfd0));
        assert!(!selects_root(0x1be870));
        assert!(!selects_root(0x1bf000));
        assert!(!selects_root(0x1c3100));
        assert!(!selects_root(0xdeadbeef));
        configure(false);
        assert!(!enabled());
        assert!(!selects_root(0x1b6ec0));
    }

    #[derive(Default)]
    struct NativeInteractionProbe {
        calls: Vec<&'static str>,
        invoked: Option<(
            crate::weapon_interaction::InteractionCallbackId,
            ObjectId,
            Option<ObjectId>,
        )>,
    }

    impl crate::weapon_interaction::NativeInteractionHandler for NativeInteractionProbe {
        type Error = ();

        fn send_interaction(
            &mut self,
            _: WeaponPlayerId,
            _: ObjectId,
            _: &mut SceneMembership,
            _: &mut crate::weapon_interaction::InteractionTargetStore,
        ) {
            self.calls.push("send");
        }

        fn invoke_target_callback(
            &mut self,
            callback: crate::weapon_interaction::InteractionCallbackId,
            target: ObjectId,
            actor: Option<ObjectId>,
            _: &mut SceneMembership,
            _: &mut crate::weapon_interaction::InteractionTargetStore,
        ) -> Result<(), Self::Error> {
            self.calls.push("callback");
            self.invoked = Some((callback, target, actor));
            Ok(())
        }
    }

    #[test]
    fn native_interaction_runtime_uses_owned_player_target_and_scene_state() {
        let _serial = RUNTIME_STORE_TEST_LOCK.lock().unwrap();
        configure(true);
        let player = WeaponPlayerId::new(0);
        let handle = begin_player_lifetime_handle(player, WeaponInventory::default())
            .expect("start player lifetime");
        let mut scene = SceneMembership::default();
        let target = scene.spawn(4).expect("live target generation");
        let actor = scene.spawn(5).expect("live actor generation");
        let callback = crate::weapon_interaction::InteractionCallbackId(17);
        attach_interaction_target(
            &scene,
            target,
            crate::weapon_interaction::NativeInteractionTarget::new(Some(callback), 23, Vec::new()),
        )
        .expect("attach target owner");
        let mut handler = NativeInteractionProbe::default();
        let result = submit_native_interaction(
            &mut scene,
            crate::weapon_interaction::CapturedInteractionPlayer {
                player: handle,
                actor: Some(actor),
            },
            target,
            &mut handler,
        );

        assert_eq!(
            result,
            Ok(
                crate::weapon_interaction::InteractionSubmissionOutcome::Completed {
                    callback_invoked: true
                }
            )
        );
        assert_eq!(handler.calls, ["send", "callback"]);
        assert_eq!(handler.invoked, Some((callback, target, Some(actor))));
        assert!(OWNED_INVENTORIES.with(|store| {
            store
                .players
                .get_handle(handle)
                .is_some_and(|owner| owner.interaction_flag)
        }));
        let retired = retire_interaction_target(&mut scene, target).expect("retire target owner");
        assert_eq!(
            retired,
            crate::weapon_interaction::NativeInteractionTarget::new(Some(callback), 23, Vec::new())
        );
        assert!(!scene.is_active(target));
        configure(false);
    }

    #[test]
    fn native_interaction_runtime_rejects_retired_player_before_native_effects() {
        let _serial = RUNTIME_STORE_TEST_LOCK.lock().unwrap();
        configure(true);
        let player = WeaponPlayerId::new(0);
        let handle = begin_player_lifetime_handle(player, WeaponInventory::default())
            .expect("start player lifetime");
        assert!(retire_player_lifetime_handle(handle));

        let mut scene = SceneMembership::default();
        let mut handler = NativeInteractionProbe::default();
        let result = submit_native_interaction(
            &mut scene,
            crate::weapon_interaction::CapturedInteractionPlayer {
                player: handle,
                actor: None,
            },
            ObjectId {
                slot: 4,
                generation: 1,
            },
            &mut handler,
        );

        assert!(matches!(
            result,
            Ok(crate::weapon_interaction::InteractionSubmissionOutcome::MissingPlayer)
        ));
        assert!(handler.calls.is_empty());
        configure(false);
    }
}

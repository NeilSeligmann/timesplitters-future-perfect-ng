// SPDX-License-Identifier: GPL-3.0-or-later
//! T3398's default-off seam from the lifted D01C0 caller to Rust-owned lock-on output state.
//!
//! The opt-in path owns the CFF20 profile/session rejection and otherwise delegates the scan to
//! the retained body while the readable scanner is incomplete. Successful scan outputs are copied
//! into the generation-checked player owner. Attachment carries the current record's measured
//! index in the inline player array plus the PlayerSet rebase generation. The selected candidate
//! also carries a bounded view of its record table; this does not infer object or inventory
//! identity.

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
    player_interaction::{
        LockOnCandidateTableView, LockOnScanOutput, LockOnVector, PlayerInteractionStore,
        PlayerLockOnCandidate, PlayerLockOnSnapshot, PlayerRecordIndex, PlayerSetGeneration,
    },
};
const ROOT_D01C0: u32 = 0x000d_01c0;
const ROOT_D00F0: u32 = 0x000d_00f0;
const ROOT_CFF20: u32 = 0x000c_ff20;
const MIN_GUEST_DATA_ADDRESS: u32 = 0x1_0000;
const PLAYER_RECORD_ARRAY_BASE: u32 = 0x007b_0c48;
const PLAYER_RECORD_COUNT: u32 = 0x0079_0950;
const TARGET_CANDIDATE_COUNT: u32 = 0x0074_c37c;
const TARGET_CANDIDATE_RECORDS: u32 = 0x0073_56d8;
const CURRENT_PLAYER_RECORD: u32 = 0x007b_0c7c;
const PLAYER_TARGETING_STATE: u32 = 0x114;
const PLAYER_TARGETING_PROFILE: u32 = 0x1500;
const PLAYER_LOCK_ON_STATE: u32 = 0xf7c;
const SESSION_FLAGS: u32 = 0x007d_e458;
const SESSION_MODE: u32 = 0x007d_e470;
const PROFILE_CONTROL_FLAGS: u32 = 2;
const PROFILE_AUTO_AIM: u8 = 1 << 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TargetingProfileRejection {
    AutoAimDisabled,
    SessionModeDisabled(u32),
}

/// Typed result of CFF20's profile/session prefix; mirrors T2194's `profile_allows` rule.
fn targeting_profile_rejection(
    auto_aim: bool,
    session_flags: u8,
    session_mode: u32,
) -> Option<TargetingProfileRejection> {
    if !auto_aim {
        Some(TargetingProfileRejection::AutoAimDisabled)
    } else if session_flags & 0x40 != 0 && matches!(session_mode, 10 | 11 | 16) {
        Some(TargetingProfileRejection::SessionModeDisabled(session_mode))
    } else {
        None
    }
}

static ENABLED: AtomicBool = AtomicBool::new(false);
const TRACE_WORDS: usize = 12;
static REFERENCE_VALID: AtomicBool = AtomicBool::new(false);
static REFERENCE_WORDS: [AtomicU32; TRACE_WORDS] = [const { AtomicU32::new(0) }; TRACE_WORDS];
const DIAGNOSTICS_WORDS: usize = 6;
static DIAGNOSTICS: [AtomicU32; DIAGNOSTICS_WORDS - 1] =
    [const { AtomicU32::new(0) }; DIAGNOSTICS_WORDS - 1];

struct LockedPlayerInteractionStore {
    held: AtomicBool,
    store: UnsafeCell<Option<PlayerInteractionStore>>,
}

// The atomic claim grants exclusive access to the optional store for the duration of `with`.
unsafe impl Sync for LockedPlayerInteractionStore {}

impl LockedPlayerInteractionStore {
    const fn new() -> Self {
        Self {
            held: AtomicBool::new(false),
            store: UnsafeCell::new(None),
        }
    }

    fn with<R>(&self, f: impl FnOnce(&mut PlayerInteractionStore) -> R) -> R {
        while self
            .held
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        struct Unlock<'a>(&'a AtomicBool);
        impl Drop for Unlock<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _unlock = Unlock(&self.held);
        // SAFETY: `held` grants this call exclusive access until `_unlock` drops.
        let store = unsafe { &mut *self.store.get() }.get_or_insert_with(Default::default);
        f(store)
    }
}

static PLAYER_INTERACTION_STATE: LockedPlayerInteractionStore = LockedPlayerInteractionStore::new();

pub(crate) fn observe_player_set_generation(generation: PlayerSetGeneration) -> bool {
    PLAYER_INTERACTION_STATE.with(|store| store.observe_player_set_generation(generation))
}

pub(crate) fn clear_current_player_set_lock_on_output() -> bool {
    PLAYER_INTERACTION_STATE.with(PlayerInteractionStore::clear_current_player_set_lock_on_output)
}

pub(crate) fn attach_player_lock_on_output(
    generation: PlayerSetGeneration,
    output: LockOnScanOutput,
) -> bool {
    PLAYER_INTERACTION_STATE
        .with(|store| store.attach_lock_on_output_for_generation(generation, output))
}

pub(crate) fn attach_player_record_lock_on_output_with_candidate_count(
    generation: PlayerSetGeneration,
    player_record: PlayerRecordIndex,
    output: LockOnScanOutput,
    candidate_count: u32,
) -> bool {
    PLAYER_INTERACTION_STATE.with(|store| {
        store.attach_lock_on_output_for_player_with_candidate_count(
            generation,
            player_record,
            output,
            candidate_count,
        )
    })
}

pub(crate) fn attach_player_record_lock_on_output_with_candidate_table(
    generation: PlayerSetGeneration,
    player_record: PlayerRecordIndex,
    output: LockOnScanOutput,
    candidate_table: LockOnCandidateTableView,
) -> bool {
    PLAYER_INTERACTION_STATE.with(|store| {
        store.attach_lock_on_output_for_player_with_candidate_table(
            generation,
            player_record,
            output,
            candidate_table,
        )
    })
}

pub(crate) fn player_lock_on_output(generation: PlayerSetGeneration) -> Option<LockOnScanOutput> {
    PLAYER_INTERACTION_STATE.with(|store| Some(store.get_for_generation(generation)?.lock_on()?))
}

/// Read the current Rust-owned lock-on scan with its PlayerSet lifecycle token.
/// Native consumers must keep that token distinct from the slot index and inventory identity.
pub fn current_player_lock_on_snapshot() -> Option<PlayerLockOnSnapshot> {
    PLAYER_INTERACTION_STATE.with(|store| store.current_lock_on_snapshot())
}

/// Read a checked candidate observation for native player-interaction consumers.
/// This returns no value when the scan has no selected candidate or its bound is unavailable.
pub fn current_player_lock_on_candidate() -> Option<PlayerLockOnCandidate> {
    PLAYER_INTERACTION_STATE.with(|store| store.current_lock_on_candidate())
}

/// Enable only when the explicit `--rust-ports player-interaction` token is selected.
pub fn configure(enabled: bool) {
    ENABLED.store(enabled, Ordering::Release);
    REFERENCE_VALID.store(false, Ordering::Release);
    PLAYER_INTERACTION_STATE.with(|store| {
        *store = PlayerInteractionStore::default();
        store.configure(enabled);
    });
    for counter in &DIAGNOSTICS {
        counter.store(0, Ordering::Release);
    }
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

/// Native port of the D01C0 candidate predicate, with its two original stack words kept opaque.
///
/// The field tests and helper-call order follow the retained translated body. REA has not
/// established semantic types for either argument, so this adapter does not give them object or
/// player identities. It remains behind the player-interaction switch until a route comparison
/// can exercise it.
pub fn run_candidate_filter<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if va != ROOT_D00F0 || !enabled() {
        return false;
    }

    let entry_esp = regs.esp;
    let candidate_word = memory.read32(entry_esp.wrapping_add(8));
    let saved_esi = regs.esi;
    let saved_esi_address = entry_esp.wrapping_sub(4);
    memory.write32(saved_esi_address, saved_esi);
    regs.esp = saved_esi_address;

    let threshold = f32::from_bits(memory.read32(0x0047_5cac));
    let eligible = if memory.read32(candidate_word.wrapping_add(0x20)) != 8 {
        let distance_bits = memory.read32(candidate_word.wrapping_add(0x1cc));
        regs.eax = candidate_word;
        regs.xmm[0] = crate::abi::Xmm::scalar(distance_bits);
        let distance = f32::from_bits(distance_bits);
        if distance > threshold && memory.read8(candidate_word.wrapping_add(0x28)) & 1 != 0 {
            call_candidate_helper(memory, guest, regs, 0x0009_e6f0, &[candidate_word]);
            regs.eax != 0
        } else {
            false
        }
    } else {
        let candidate_owner = memory.read32(candidate_word.wrapping_add(0x7c));
        regs.esi = candidate_owner;
        let measured_object = memory.read32(candidate_owner);
        let distance_bits = memory.read32(measured_object.wrapping_add(0x1cc));
        regs.eax = measured_object;
        regs.xmm[0] = crate::abi::Xmm::scalar(distance_bits);
        let distance = f32::from_bits(distance_bits);
        if distance > threshold {
            // At this point the original stack-relative read addresses the first opaque input.
            let first_word = memory.read32(entry_esp.wrapping_add(4));
            regs.ecx = first_word;
            call_candidate_helper(
                memory,
                guest,
                regs,
                0x000d_6f00,
                &[first_word, candidate_owner],
            );
            if regs.eax == 0 {
                call_candidate_helper(memory, guest, regs, 0x000d_a220, &[candidate_owner]);
                regs.eax != 0
            } else {
                false
            }
        } else {
            false
        }
    };

    regs.esi = memory.read32(saved_esi_address);
    regs.eax = u32::from(eligible);
    regs.esp = entry_esp;
    true
}

/// Intercept D00F0 from a translated caller; GuestCall supplies arguments before its return slot.
pub fn run_nested_candidate_filter<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if va != ROOT_D00F0 || !enabled() {
        return false;
    }
    let args_esp = regs.esp;
    regs.esp = args_esp.wrapping_sub(4);
    memory.write32(regs.esp, 0);
    let handled = run_candidate_filter(va, memory, guest, regs);
    regs.esp = args_esp;
    handled
}

fn call_candidate_helper<M: GuestMemory, C: GuestCall>(
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    va: u32,
    args: &[u32],
) {
    let entry_esp = regs.esp;
    for argument in args.iter().rev() {
        regs.esp = regs.esp.wrapping_sub(4);
        memory.write32(regs.esp, *argument);
    }
    tsfp_port_gameplay_player::interaction::dispatch_candidate_helper(va, memory, guest, regs);
    regs.esp = entry_esp;
}

fn read_vector<M: GuestMemory>(m: &M, address: u32) -> Option<LockOnVector> {
    if address < MIN_GUEST_DATA_ADDRESS || address & 3 != 0 {
        return None;
    }
    let y = address.checked_add(4)?;
    let z = address.checked_add(8)?;
    Some(LockOnVector([
        f32::from_bits(m.read32(address)),
        f32::from_bits(m.read32(y)),
        f32::from_bits(m.read32(z)),
    ]))
}

fn resolve_player_record<M: GuestMemory>(m: &M, current_record: u32) -> Option<PlayerRecordIndex> {
    PlayerRecordIndex::from_array(
        m.read32(PLAYER_RECORD_ARRAY_BASE),
        m.read32(PLAYER_RECORD_COUNT),
        current_record,
    )
}

const OUTPUT_VECTOR_BYTES: u32 = 12;
const OUTPUT_VECTOR_MASK: u16 = (1 << OUTPUT_VECTOR_BYTES) - 1;

/// Observe writes made by the translated scanner to its two caller-owned vec3 outputs.
///
/// Comparing values cannot distinguish an untouched buffer from a valid scan that writes the
/// same bits. This proxy records byte coverage while forwarding every operation unchanged.
struct OutputWriteTracker<'a, M> {
    inner: &'a mut M,
    output_addresses: [u32; 2],
    written_bytes: [u16; 2],
}

impl<'a, M> OutputWriteTracker<'a, M> {
    fn new(inner: &'a mut M, output_addresses: (u32, u32)) -> Self {
        Self {
            inner,
            output_addresses: [output_addresses.0, output_addresses.1],
            written_bytes: [0; 2],
        }
    }

    fn both_vectors_written(&self) -> bool {
        self.written_bytes
            .iter()
            .all(|written| written & OUTPUT_VECTOR_MASK == OUTPUT_VECTOR_MASK)
    }
}

impl<M: GuestMemory> GuestMemory for OutputWriteTracker<'_, M> {
    fn is_readable(&self, address: u32, len: u32) -> bool {
        self.inner.is_readable(address, len)
    }

    fn read8(&self, address: u32) -> u8 {
        self.inner.read8(address)
    }

    fn write8(&mut self, address: u32, value: u8) {
        for (index, output) in self.output_addresses.iter().copied().enumerate() {
            if let Some(offset) = address.checked_sub(output)
                && offset < OUTPUT_VECTOR_BYTES
            {
                self.written_bytes[index] |= 1 << offset;
            }
        }
        self.inner.write8(address, value);
    }

    fn as_slice(&self, address: u32, len: u32) -> Option<&[u8]> {
        self.inner.as_slice(address, len)
    }
}

fn has_fresh_scan_output(
    after: (Option<LockOnVector>, Option<LockOnVector>),
    both_vectors_written: bool,
) -> bool {
    both_vectors_written && after.0.is_some() && after.1.is_some()
}

/// Reproduce the CFF20 profile/session-rejected early return from D01C0.
///
/// CFF20's shared typed contract rejects when profile Auto Aim is off or when session flag
/// `0x40` coincides with mode 10, 11 or 16. On those paths it does not call the neighboring
/// object predicate. D01C0 clears `record+0xf7c`, returns -1 and leaves both output vectors
/// untouched. Other CFF20 cases and the candidate scan still use the retained body. This
/// fast path is derived from the lifted body and T2194 gate contract (INFERRED); no owner route
/// has yet exercised its D01C0 output.
fn reject_scan_when_targeting_profile_is_disabled<M: GuestMemory>(
    va: u32,
    memory: &mut M,
    regs: &mut ExtRegisters,
) -> bool {
    if va != ROOT_D01C0 {
        return false;
    }

    let record = memory.read32(CURRENT_PLAYER_RECORD);
    let Some(profile_field) = record.checked_add(PLAYER_TARGETING_PROFILE) else {
        return false;
    };
    let Some(state_field) = record.checked_add(PLAYER_TARGETING_STATE) else {
        return false;
    };
    let Some(lock_on_state) = record.checked_add(PLAYER_LOCK_ON_STATE) else {
        return false;
    };
    if record < MIN_GUEST_DATA_ADDRESS
        || record & 3 != 0
        || !memory.is_readable(profile_field, 4)
        || !memory.is_readable(state_field, 4)
        || !memory.is_readable(lock_on_state, 4)
    {
        return false;
    }

    let profile = memory.read32(profile_field);
    let Some(flags_address) = profile.checked_add(PROFILE_CONTROL_FLAGS as u32) else {
        return false;
    };
    if profile < MIN_GUEST_DATA_ADDRESS || !memory.is_readable(flags_address, 1) {
        return false;
    }
    let auto_aim = memory.read8(flags_address) & PROFILE_AUTO_AIM != 0;
    let (session_flags, session_mode) = if auto_aim {
        if !memory.is_readable(SESSION_FLAGS, 4) || !memory.is_readable(SESSION_MODE, 4) {
            return false;
        }
        (
            memory.read32(SESSION_FLAGS) as u8,
            memory.read32(SESSION_MODE),
        )
    } else {
        (0, 0)
    };
    let player_state = memory.read32(state_field);
    let Some(rejection) = targeting_profile_rejection(auto_aim, session_flags, session_mode) else {
        return false;
    };

    // Preserve the translated D01C0 prologue's guest stack writes. Although the saved words are
    // transient frame data, the lifted body leaves them in guest memory after returning.
    let frame_esp = regs.esp.wrapping_sub(0xac);
    memory.write32(frame_esp.wrapping_sub(4), regs.ebx);
    memory.write32(frame_esp.wrapping_sub(8), regs.ebp);
    memory.write32(frame_esp.wrapping_sub(12), regs.esi);
    memory.write32(frame_esp.wrapping_sub(16), record);

    // The translated false branch reaches its shared epilogue with ESP at frame_esp-8. Mirror
    // those loads literally: this unusual path restores ESI from saved EBP, EBP from saved EBX,
    // and EBX from the frame word, then leaves ESP advanced by four bytes.
    regs.esi = memory.read32(frame_esp.wrapping_sub(8));
    regs.ebp = memory.read32(frame_esp.wrapping_sub(4));
    regs.ebx = memory.read32(frame_esp);
    memory.write32(lock_on_state, 0);
    regs.eax = u32::MAX;
    regs.ecx = player_state;
    regs.edx = match rejection {
        TargetingProfileRejection::AutoAimDisabled => profile,
        TargetingProfileRejection::SessionModeDisabled(mode) => mode,
    };
    regs.esp = regs.esp.wrapping_add(4);
    true
}

/// Run D01C0, then attach its outputs to the stable PlayerSet record generation.
/// The guest ABI result and vectors are untouched; a missing or changing record owner declines
/// the sidecar write. The two output pointers are the first two cdecl arguments at entry.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if va != ROOT_D01C0 || !enabled() {
        return false;
    }
    DIAGNOSTICS[0].fetch_add(1, Ordering::Relaxed);

    let stack = regs.esp;
    let output_pointers = (
        m.read32(stack.wrapping_add(4)),
        m.read32(stack.wrapping_add(8)),
    );
    let current_record = m.read32(crate::raw_look::PLAYER_POINTER);
    let array_base_before = m.read32(PLAYER_RECORD_ARRAY_BASE);
    let record_count_before = m.read32(PLAYER_RECORD_COUNT);
    let candidate_count_before = m.read32(TARGET_CANDIDATE_COUNT);
    let candidate_records_before = m.read32(TARGET_CANDIDATE_RECORDS);
    let player_record_before = resolve_player_record(m, current_record);
    crate::player_wire::PLAYER.sync_ptrs(m, |_, _, _| {});
    let generation = crate::player_wire::PLAYER.rebase_count();
    let player_generation = PlayerSetGeneration::new(generation);
    let record_owner_before = player_record_before.is_some()
        && current_record == crate::player_wire::PLAYER.ptr(crate::leaf_wire::player::PTR_RECORD);
    if record_owner_before
        && let (Some(player_generation), Some(player_record)) =
            (player_generation, player_record_before)
    {
        PLAYER_INTERACTION_STATE
            .with(|store| store.observe_player_record(player_generation, player_record));
    }
    clear_current_player_set_lock_on_output();
    if player_generation.is_some() {
        DIAGNOSTICS[2].fetch_add(1, Ordering::Relaxed);
    }
    if reject_scan_when_targeting_profile_is_disabled(va, m, regs) {
        return true;
    }
    let mut output_tracker = OutputWriteTracker::new(m, output_pointers);
    if !crate::c2r::player_wired::run(va, &mut output_tracker, guest, regs) {
        return false;
    }
    let vectors = (
        read_vector(&output_tracker, output_pointers.0),
        read_vector(&output_tracker, output_pointers.1),
    );
    let both_vectors_written = output_tracker.both_vectors_written();
    drop(output_tracker);

    let current_record_after = m.read32(crate::raw_look::PLAYER_POINTER);
    let array_base_after = m.read32(PLAYER_RECORD_ARRAY_BASE);
    let record_count_after = m.read32(PLAYER_RECORD_COUNT);
    let candidate_count_after = m.read32(TARGET_CANDIDATE_COUNT);
    let candidate_records_after = m.read32(TARGET_CANDIDATE_RECORDS);
    let player_record_after = resolve_player_record(m, current_record_after);
    crate::player_wire::PLAYER.sync_ptrs(m, |_, _, _| {});
    let generation_after = crate::player_wire::PLAYER.rebase_count();
    let record_owner_after = player_record_after.is_some()
        && current_record_after
            == crate::player_wire::PLAYER.ptr(crate::leaf_wire::player::PTR_RECORD);
    if record_owner_after
        && let (Some(player_generation_after), Some(player_record_after)) = (
            PlayerSetGeneration::new(generation_after),
            player_record_after,
        )
    {
        PLAYER_INTERACTION_STATE.with(|store| {
            store.observe_player_record(player_generation_after, player_record_after)
        });
    }
    let record_owner_matches = current_record >= MIN_GUEST_DATA_ADDRESS
        && current_record & 3 == 0
        && current_record == current_record_after
        && array_base_before == array_base_after
        && record_count_before == record_count_after
        && candidate_count_before == candidate_count_after
        && candidate_records_before == candidate_records_after
        && player_record_before.is_some()
        && player_record_before == player_record_after
        && current_record == crate::player_wire::PLAYER.ptr(crate::leaf_wire::player::PTR_RECORD)
        && generation != 0
        && generation == generation_after;
    if record_owner_matches {
        DIAGNOSTICS[1].fetch_add(1, Ordering::Relaxed);
    }
    let fresh_output = has_fresh_scan_output(vectors, both_vectors_written);
    if vectors.0.is_some() && vectors.1.is_some() {
        DIAGNOSTICS[3].fetch_add(1, Ordering::Relaxed);
    }
    if record_owner_matches
        && let (Some(player_generation), Some(first), Some(second)) =
            (player_generation, vectors.0, vectors.1)
        && fresh_output
    {
        let output = LockOnScanOutput::new(regs.eax as i32, first, second);
        if let Some(player_record) = player_record_before {
            let attached = match LockOnCandidateTableView::new(
                candidate_records_before,
                candidate_count_before,
            ) {
                Some(candidate_table) => attach_player_record_lock_on_output_with_candidate_table(
                    player_generation,
                    player_record,
                    output,
                    candidate_table,
                ),
                None => attach_player_record_lock_on_output_with_candidate_count(
                    player_generation,
                    player_record,
                    output,
                    candidate_count_before,
                ),
            };
            if attached {
                DIAGNOSTICS[4].fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    true
}

/// Intercept D01C0 when translated Rust calls it through `GuestCall`.
///
/// `GuestCall::call` supplies the arguments at `[esp]`, before the lifted trampoline adds its
/// return slot. Match the root ABI for the adapter by reserving a synthetic return slot, then
/// restore the caller's argument-stack pointer after the nested root returns.
pub fn run_nested<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if va != ROOT_D01C0 || !enabled() {
        return false;
    }
    let args_esp = regs.esp;
    regs.esp = args_esp.wrapping_sub(4);
    m.write32(regs.esp, 0);
    let handled = run(va, m, guest, regs);
    regs.esp = args_esp;
    handled
}

/// Route D01C0's nested CFF20 call through T2194's readable targeting gate when both
/// the interaction path and native look gate are selected. GuestCall enters before the
/// x86 return slot is pushed, so reserve the slot expected by the root adapter.
pub(crate) fn run_nested_targeting_gate<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    run_nested_targeting_gate_selected(
        va,
        enabled(),
        crate::player_look_gate::selected(),
        memory,
        guest,
        regs,
    )
}

fn run_nested_targeting_gate_selected<M: GuestMemory, C: GuestCall>(
    va: u32,
    interaction_enabled: bool,
    look_gate_selected: bool,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if va != ROOT_CFF20 || !interaction_enabled || !look_gate_selected {
        return false;
    }
    let args_esp = regs.esp;
    let saved_ebx = regs.ebx;
    let saved_esi = regs.esi;
    let player = memory.read32(args_esp);
    let profile = memory.read32(player.wrapping_add(PLAYER_TARGETING_PROFILE));
    let profile_auto_aim =
        memory.read8(profile.wrapping_add(PROFILE_CONTROL_FLAGS)) & PROFILE_AUTO_AIM != 0;
    let session_flags = memory.read32(SESSION_FLAGS) as u8;
    let session_mode = memory.read32(SESSION_MODE);
    // Preserve the translated CFF20's caller-visible register residue at the nested call
    // boundary. Its prefix leaves the record mode in ECX and the profile (or blocked session
    // mode) in EDX even when the typed T2194 gate returns early.
    regs.ecx = memory.read32(player.wrapping_add(PLAYER_TARGETING_STATE));
    regs.edx = if profile_auto_aim && session_flags & 0x40 != 0 {
        session_mode
    } else {
        profile
    };
    regs.esp = args_esp.wrapping_sub(4);
    memory.write32(regs.esp, 0);
    // Match CFF20's translated prologue/epilogue stack spills. The register values are
    // restored by the translated body, but the written words remain guest-visible.
    memory.write32(regs.esp.wrapping_sub(4), saved_ebx);
    memory.write32(regs.esp.wrapping_sub(8), saved_esi);
    let handled = crate::player_look_gate::run(va, true, memory, guest, regs);
    regs.esp = args_esp;
    handled
}

/// Capture the lifted nested call's ABI outputs for the off/reference projection.
/// The caller's argument block begins at `args_esp`; it contains the two output pointers.
pub fn record_lifted_reference<M: GuestMemory>(m: &M, args_esp: u32, result: u32) {
    DIAGNOSTICS[0].fetch_add(1, Ordering::Relaxed);
    let first = m.read32(args_esp);
    let second = m.read32(args_esp.wrapping_add(4));
    let (Some(first), Some(second)) = (read_vector(m, first), read_vector(m, second)) else {
        return;
    };
    DIAGNOSTICS[3].fetch_add(1, Ordering::Relaxed);
    crate::player_wire::PLAYER.sync_ptrs(m, |_, _, _| {});
    let player_record = resolve_player_record(m, m.read32(crate::raw_look::PLAYER_POINTER))
        .map_or(u32::MAX, PlayerRecordIndex::get);
    let mut words = [0; 10];
    words[0] = crate::player_wire::PLAYER.rebase_count();
    words[1] = player_record;
    words[2] = m.read32(TARGET_CANDIDATE_COUNT);
    words[3] = result;
    for (index, vector) in [first, second].into_iter().enumerate() {
        for (axis, value) in vector.0.into_iter().enumerate() {
            words[4 + index * 3 + axis] = value.to_bits();
        }
    }
    store_reference_words(&words);
}

/// Store the last lifted-C D01C0 output for the off/reference trace projection.
/// `words` is the signed result followed by the two exact three-float ABI outputs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_player_interaction_trace_reference(
    words: *const u32,
    word_count: u32,
) {
    if words.is_null() || word_count < 10 {
        return;
    }
    let words = unsafe { core::slice::from_raw_parts(words, 10) };
    store_reference_words(words);
}

fn store_reference_words(words: &[u32]) {
    REFERENCE_VALID.store(false, Ordering::Release);
    REFERENCE_WORDS[0].store(2, Ordering::Relaxed);
    REFERENCE_WORDS[1].store(1, Ordering::Relaxed);
    for (index, value) in words.iter().enumerate() {
        REFERENCE_WORDS[index + 2].store(*value, Ordering::Relaxed);
    }
    REFERENCE_VALID.store(true, Ordering::Release);
}

/// Serialize the selected typed owner output or lifted reference into T3398's trace region.
/// Schema v2: version, present, generation, player-record index, candidate count, signed result,
/// first vec3, second vec3. `u32::MAX` marks a missing candidate bound or player-record index.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_player_interaction_trace_snapshot(
    output: *mut u32,
    output_words: u32,
) -> bool {
    if output.is_null() || output_words < TRACE_WORDS as u32 {
        return false;
    }
    let output = unsafe { core::slice::from_raw_parts_mut(output, TRACE_WORDS) };
    output.fill(0);
    output[0] = 2;
    if enabled() {
        let Some(snapshot) = current_player_lock_on_snapshot() else {
            return true;
        };
        encode_snapshot_trace(Some(snapshot), output);
        return true;
    }
    if REFERENCE_VALID.load(Ordering::Acquire) {
        for (index, word) in output.iter_mut().enumerate() {
            *word = REFERENCE_WORDS[index].load(Ordering::Relaxed);
        }
    }
    true
}

/// Serialize host-only call/attachment counters used to diagnose route coverage. The six words
/// are version, observed calls, stable record owner, valid PlayerSet generation, readable vectors,
/// and attached output. These diagnostics are not subsystem state or adoption input.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_player_interaction_trace_diagnostics_snapshot(
    output: *mut u32,
    output_words: u32,
) -> bool {
    if output.is_null() || output_words < DIAGNOSTICS_WORDS as u32 {
        return false;
    }
    let output = unsafe { core::slice::from_raw_parts_mut(output, DIAGNOSTICS_WORDS) };
    output[0] = 1;
    for (index, counter) in DIAGNOSTICS.iter().enumerate() {
        output[index + 1] = counter.load(Ordering::Acquire);
    }
    true
}

fn encode_snapshot_trace(snapshot: Option<PlayerLockOnSnapshot>, words: &mut [u32]) {
    words.fill(0);
    if words.len() < TRACE_WORDS {
        return;
    }
    words[0] = 2;
    let Some(snapshot) = snapshot else {
        return;
    };
    words[1] = 1;
    words[2] = snapshot.generation().get();
    words[3] = snapshot.player_record().get();
    words[4] = snapshot.candidate_count().unwrap_or(u32::MAX);
    words[5] = snapshot.scan().returned_index() as u32;
    for (index, vector) in snapshot.scan().vectors().iter().enumerate() {
        for (axis, value) in vector.0.iter().enumerate() {
            words[6 + index * 3 + axis] = value.to_bits();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct FixtureMemory(BTreeMap<u32, u8>);

    impl GuestMemory for FixtureMemory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }

        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct FixtureGuest {
        calls: Vec<u32>,
        result: u32,
    }

    impl GuestCall for FixtureGuest {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            self.calls.push(va);
            regs.eax = self.result;
            regs.esp = regs.esp.wrapping_add(4);
        }
    }

    struct AutoAimOffGuest {
        state: u32,
        profile: u32,
    }

    impl GuestCall for AutoAimOffGuest {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            assert_eq!(va, 0x000c_ff20);
            regs.eax = 0;
            regs.ecx = self.state;
            regs.edx = self.profile;
            regs.esp = regs.esp.wrapping_add(4);
        }
    }

    struct SessionBlockedGuest {
        state: u32,
        mode: u32,
    }

    impl GuestCall for SessionBlockedGuest {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            assert_eq!(va, 0x000c_ff20);
            regs.eax = 0;
            regs.ecx = self.state;
            regs.edx = self.mode;
            regs.esp = regs.esp.wrapping_add(4);
        }
    }

    struct CandidateLimitGuest {
        values: [f32; 3],
        calls: usize,
    }

    impl GuestCall for CandidateLimitGuest {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            assert_eq!(va, 0x000c_ffa0);
            let value = self.values[self.calls];
            self.calls += 1;
            regs.fpu.push(value as f64);
            regs.esp = regs.esp.wrapping_add(4);
        }
    }

    fn fixture(seed: u32) -> (FixtureMemory, ExtRegisters) {
        let mut memory = FixtureMemory::default();
        let stack = 0x8000;
        let first = 0x19000;
        let second = 0x19020;
        let player_array = 0x10000;
        let current_record = player_array + PlayerRecordIndex::RECORD_STRIDE;
        memory.write32(PLAYER_RECORD_ARRAY_BASE, player_array);
        memory.write32(PLAYER_RECORD_COUNT, 4);
        memory.write32(TARGET_CANDIDATE_COUNT, 4);
        memory.write32(crate::raw_look::PLAYER_POINTER, current_record);
        memory.write32(stack + 4, first);
        memory.write32(stack + 8, second);
        for (index, value) in [first, second].into_iter().enumerate() {
            for axis in 0..3 {
                let bits =
                    ((seed.wrapping_mul(17 + axis) ^ (index as u32 * 0x5a5a)) as f32).to_bits();
                memory.write32(value + axis * 4, bits);
            }
        }
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.eax = seed ^ 0xD01C0;
        regs.ecx = seed.rotate_left(5);
        regs.edx = seed.rotate_right(7);
        regs.ebx = seed.wrapping_mul(3);
        regs.ebp = seed.wrapping_mul(5);
        regs.esi = seed.wrapping_mul(7);
        regs.edi = seed.wrapping_mul(11);
        regs.esp = stack;
        (memory, regs)
    }

    #[test]
    fn native_auto_aim_off_exit_matches_translated_false_gate_effects() {
        let (mut source_memory, source_regs) = fixture(0x3398);
        let record = source_memory.read32(CURRENT_PLAYER_RECORD);
        let profile = 0x22000;
        let state = 0x80;
        source_memory.write32(record + PLAYER_TARGETING_PROFILE, profile);
        source_memory.write32(record + PLAYER_TARGETING_STATE, state);
        source_memory.write32(record + PLAYER_LOCK_ON_STATE, 0xfeed_1234);
        source_memory.write8(profile + PROFILE_CONTROL_FLAGS as u32, 0x05);

        let mut translated_memory = source_memory.clone();
        let mut native_memory = source_memory;
        let mut translated_regs = source_regs;
        let mut native_regs = translated_regs;
        let mut guest = AutoAimOffGuest { state, profile };

        assert!(crate::c2r::player_wired::run(
            ROOT_D01C0,
            &mut translated_memory,
            &mut guest,
            &mut translated_regs,
        ));
        assert!(reject_scan_when_targeting_profile_is_disabled(
            ROOT_D01C0,
            &mut native_memory,
            &mut native_regs,
        ));

        assert_eq!(native_memory, translated_memory);
        assert_eq!(native_regs, translated_regs);
    }

    #[test]
    fn native_auto_aim_on_exit_defers_to_the_scanner() {
        let (mut memory, mut regs) = fixture(0x3399);
        let record = memory.read32(CURRENT_PLAYER_RECORD);
        let profile = 0x22000;
        memory.write32(record + PLAYER_TARGETING_PROFILE, profile);
        memory.write32(record + PLAYER_TARGETING_STATE, 0x80);
        memory.write8(profile + PROFILE_CONTROL_FLAGS as u32, PROFILE_AUTO_AIM);
        let before = (memory.clone(), regs);

        assert!(!reject_scan_when_targeting_profile_is_disabled(
            ROOT_D01C0,
            &mut memory,
            &mut regs,
        ));
        assert_eq!((memory, regs), before);
    }

    #[test]
    fn native_session_profile_rejections_match_translated_false_gate_effects() {
        for (seed, mode) in [10, 11, 16].into_iter().enumerate() {
            let (mut source_memory, source_regs) = fixture(0x33a0 + seed as u32);
            let record = source_memory.read32(CURRENT_PLAYER_RECORD);
            let profile = 0x22000;
            let state = 0x1234;
            source_memory.write32(record + PLAYER_TARGETING_PROFILE, profile);
            source_memory.write32(record + PLAYER_TARGETING_STATE, state);
            source_memory.write32(record + PLAYER_LOCK_ON_STATE, 0xfeed_1234);
            source_memory.write8(profile + PROFILE_CONTROL_FLAGS as u32, PROFILE_AUTO_AIM);
            source_memory.write32(SESSION_FLAGS, 0x40);
            source_memory.write32(SESSION_MODE, mode);

            let mut translated_memory = source_memory.clone();
            let mut native_memory = source_memory;
            let mut translated_regs = source_regs;
            let mut native_regs = translated_regs;
            let mut guest = SessionBlockedGuest { state, mode };

            assert!(crate::c2r::player_wired::run(
                ROOT_D01C0,
                &mut translated_memory,
                &mut guest,
                &mut translated_regs,
            ));
            assert!(reject_scan_when_targeting_profile_is_disabled(
                ROOT_D01C0,
                &mut native_memory,
                &mut native_regs,
            ));

            assert_eq!(native_memory, translated_memory);
            assert_eq!(native_regs, translated_regs);
        }
    }

    #[test]
    fn nested_targeting_gate_uses_typed_t2194_gate_and_restores_guestcall_stack() {
        let _serial = crate::weapon_hands_runtime::runtime_store_test_guard();
        configure(true);
        for (seed, auto_aim, session_flags, session_mode) in [
            (0x33b0, false, 0, 0),
            (0x33b1, true, 0x40, 10),
            (0x33b2, true, 0x40, 11),
            (0x33b3, true, 0x40, 16),
        ] {
            let (mut source_memory, source_regs) = fixture(seed);
            let record = source_memory.read32(CURRENT_PLAYER_RECORD);
            let profile = 0x22000;
            let owner = 0x23000;
            source_memory.write32(source_regs.esp, record);
            source_memory.write32(record + PLAYER_TARGETING_PROFILE, profile);
            source_memory.write32(record + 0x14, owner - 0x7c);
            source_memory.write32(owner, 0x24000);
            source_memory.write32(record + PLAYER_TARGETING_STATE, 0);
            source_memory.write32(record + PLAYER_LOCK_ON_STATE, 0xfeed_1234);
            source_memory.write8(
                profile + PROFILE_CONTROL_FLAGS as u32,
                if auto_aim { PROFILE_AUTO_AIM } else { 0 },
            );
            source_memory.write32(SESSION_FLAGS, session_flags);
            source_memory.write32(SESSION_MODE, session_mode);

            // GuestCall enters with ESP on the first argument. It pushes the return
            // address before the lifted CFF20 body reads [ESP + 4].
            let args_esp = source_regs.esp;
            let mut translated_memory = source_memory.clone();
            let mut native_memory = source_memory;
            let mut translated_regs = source_regs;
            let mut native_regs = translated_regs;
            translated_regs.esp = args_esp.wrapping_sub(4);
            translated_memory.write32(translated_regs.esp, 0);
            let mut translated_guest = FixtureGuest::default();
            let mut native_guest = FixtureGuest::default();

            assert!(crate::c2r::gameplay_player::run(
                ROOT_CFF20,
                &mut translated_memory,
                &mut translated_guest,
                &mut translated_regs,
            ));
            // The lifted root retains its synthetic return slot; GuestCall consumes it
            // before returning to its caller, as the native adapter does above.
            translated_regs.esp = args_esp;
            assert!(run_nested_targeting_gate_selected(
                ROOT_CFF20,
                true,
                true,
                &mut native_memory,
                &mut native_guest,
                &mut native_regs,
            ));
            assert_eq!(native_memory, translated_memory, "seed {seed:#x}");
            assert_eq!(native_regs, translated_regs, "seed {seed:#x}");
            assert_eq!(native_regs.esp, args_esp, "seed {seed:#x}");
            assert!(native_guest.calls.is_empty(), "seed {seed:#x}");
        }
    }

    #[test]
    fn candidate_limit_reduction_matches_translated_d0090_for_fabricated_values() {
        let _serial = crate::weapon_hands_runtime::runtime_store_test_guard();
        configure(true);
        let (mut source_memory, source_regs) = fixture(0xd0090);
        let record = 0x23400;
        let stack = source_regs.esp;
        source_memory.write32(stack, 0);
        source_memory.write32(stack + 4, record);

        let mut translated_memory = source_memory.clone();
        let mut native_memory = source_memory;
        let mut translated_regs = source_regs;
        let mut native_regs = source_regs;
        let values = [0.5, 2.25, 1.75];
        let mut translated_guest = CandidateLimitGuest { values, calls: 0 };
        let mut native_guest = CandidateLimitGuest { values, calls: 0 };

        crate::c2r::gameplay_player::game_player_get_max_float_over_kinds_0_to_2_via_record_29c(
            &mut translated_memory,
            &mut translated_guest,
            &mut translated_regs,
        );
        tsfp_port_gameplay_player::interaction::run_nested_candidate_limit(
            &mut native_memory,
            &mut native_guest,
            &mut native_regs,
        );

        assert_eq!(native_memory, translated_memory);
        assert_eq!(native_regs, translated_regs);
        assert_eq!(native_guest.calls, 3);
        assert_eq!(translated_guest.calls, 3);
        assert_eq!(native_regs.fpu.st(0) as f32, 1.75);
        configure(false);
    }

    #[test]
    fn adapter_is_disabled_until_explicitly_configured() {
        let _serial = crate::weapon_hands_runtime::runtime_store_test_guard();
        configure(false);
        assert!(!enabled());
        configure(true);
        assert!(enabled());
        configure(false);
    }

    /// Tier 0 wrapper comparison: the adapter preserves retained C2R D01C0 guest effects.
    /// Inputs and callee results are FABRICATED. The C2R body has separate Tier 1 lifted-C
    /// evidence; this test itself does not invoke compiled lifted C.
    #[test]
    fn adapter_preserves_c2r_guest_effects_for_2048_fabricated_inputs() {
        let _serial = crate::weapon_hands_runtime::runtime_store_test_guard();
        for seed in 0..2048u32 {
            let (initial_memory, initial_regs) = fixture(seed);
            let mut lifted_memory = initial_memory.clone();
            let mut adapted_memory = initial_memory;
            let mut lifted_regs = initial_regs;
            let mut adapted_regs = initial_regs;
            let mut lifted_guest = FixtureGuest::default();
            let mut adapted_guest = FixtureGuest::default();

            assert!(crate::c2r::player_wired::run(
                ROOT_D01C0,
                &mut lifted_memory,
                &mut lifted_guest,
                &mut lifted_regs,
            ));
            configure(true);
            assert!(crate::player_system::run(
                ROOT_D01C0,
                true,
                &mut adapted_memory,
                &mut adapted_guest,
                &mut adapted_regs,
            ));
            configure(false);

            assert_eq!(adapted_memory, lifted_memory, "guest memory seed={seed}");
            assert_eq!(adapted_regs, lifted_regs, "registers seed={seed}");
            assert_eq!(adapted_guest, lifted_guest, "callee sequence seed={seed}");
        }
    }

    #[test]
    fn unchanged_output_buffers_do_not_publish_a_no_slot_scan() {
        let _runtime_store = crate::weapon_hands_runtime::runtime_store_test_guard();
        configure(true);

        let (mut memory, mut regs) = fixture(0x3398);
        let before = (read_vector(&memory, 0x19000), read_vector(&memory, 0x19020));
        assert!(enabled());
        let mut guest = FixtureGuest::default();
        assert!(crate::player_system::run(
            ROOT_D01C0,
            true,
            &mut memory,
            &mut guest,
            &mut regs,
        ));
        assert_eq!(regs.eax as i32, -1);
        assert_eq!(
            (read_vector(&memory, 0x19000), read_vector(&memory, 0x19020)),
            before,
            "the false eligibility gate leaves the caller's output buffers unchanged"
        );
        let mut diagnostics = [0; DIAGNOSTICS_WORDS];
        assert!(unsafe {
            tsfp_player_interaction_trace_diagnostics_snapshot(
                diagnostics.as_mut_ptr(),
                diagnostics.len() as u32,
            )
        });
        assert_eq!(diagnostics, [1, 1, 1, 1, 1, 0]);
        let generation = crate::player_wire::PLAYER.rebase_count();
        assert_eq!(
            player_lock_on_output(PlayerSetGeneration::new(generation).unwrap()),
            None,
            "stale caller buffers must not become a Rust-owned scan"
        );

        configure(false);
        assert_eq!(
            player_lock_on_output(PlayerSetGeneration::new(generation).unwrap()),
            None
        );
    }

    #[test]
    fn identical_complete_vec3_writes_are_fresh_no_slot_output() {
        let (mut memory, _) = fixture(0x3398);
        let first = read_vector(&memory, 0x19000).unwrap();
        let second = read_vector(&memory, 0x19020).unwrap();
        let mut writes = OutputWriteTracker::new(&mut memory, (0x19000, 0x19020));

        for (base, vector) in [(0x19000, first), (0x19020, second)] {
            for (axis, value) in vector.0.into_iter().enumerate() {
                writes.write32(base + axis as u32 * 4, value.to_bits());
            }
        }

        assert!(writes.both_vectors_written());
        let after = (read_vector(&writes, 0x19000), read_vector(&writes, 0x19020));
        assert!(has_fresh_scan_output(after, writes.both_vectors_written()));
    }

    #[test]
    fn partial_vec3_writes_do_not_publish_a_no_slot_scan() {
        let (mut memory, _) = fixture(0x3398);
        let mut writes = OutputWriteTracker::new(&mut memory, (0x19000, 0x19020));
        for address in [0x19000, 0x19004, 0x19008, 0x19020, 0x19024] {
            writes.write32(address, 0x3f80_0000);
        }

        assert!(!writes.both_vectors_written());
        let after = (read_vector(&writes, 0x19000), read_vector(&writes, 0x19020));
        assert!(!has_fresh_scan_output(after, writes.both_vectors_written()));
    }

    #[test]
    fn selected_slot_with_unwritten_vec3_buffers_is_not_fresh_output() {
        let (memory, _) = fixture(0x3398);
        let after = (read_vector(&memory, 0x19000), read_vector(&memory, 0x19020));

        assert!(after.0.is_some() && after.1.is_some());
        assert!(!has_fresh_scan_output(after, false));
    }

    /// FABRICATED Tier 0: the runtime seam must retire the previous attachment when the
    /// synchronized current PlayerSet record changes, and publish only under the new generation.
    #[test]
    fn replacing_current_player_record_moves_attachment_to_new_generation() {
        let _runtime_store = crate::weapon_hands_runtime::runtime_store_test_guard();
        configure(true);

        let (mut first_memory, mut first_regs) = fixture(0x3398);
        let mut first_guest = FixtureGuest::default();
        assert!(crate::player_system::run(
            ROOT_D01C0,
            true,
            &mut first_memory,
            &mut first_guest,
            &mut first_regs,
        ));
        let first_generation =
            PlayerSetGeneration::new(crate::player_wire::PLAYER.rebase_count()).unwrap();
        let first_output = LockOnScanOutput::new(
            3,
            LockOnVector([1.0, 2.0, 3.0]),
            LockOnVector([4.0, 5.0, 6.0]),
        );
        assert!(attach_player_lock_on_output(first_generation, first_output));

        let (mut second_memory, mut second_regs) = fixture(0x3399);
        second_memory.write32(crate::raw_look::PLAYER_POINTER, 0x12000);
        let mut second_guest = FixtureGuest::default();
        assert!(crate::player_system::run(
            ROOT_D01C0,
            true,
            &mut second_memory,
            &mut second_guest,
            &mut second_regs,
        ));
        let second_generation =
            PlayerSetGeneration::new(crate::player_wire::PLAYER.rebase_count()).unwrap();

        assert!(second_generation > first_generation);
        assert_eq!(player_lock_on_output(first_generation), None);
        assert_eq!(player_lock_on_output(second_generation), None);
        assert_eq!(first_output.returned_index(), 3);

        configure(false);
    }

    #[test]
    fn native_snapshot_pairs_output_with_generation_and_expires_on_replacement() {
        let _runtime_store = crate::weapon_hands_runtime::runtime_store_test_guard();
        configure(true);

        let generation = PlayerSetGeneration::new(81).unwrap();
        let output = LockOnScanOutput::new(
            4,
            LockOnVector([1.5, -2.0, 3.25]),
            LockOnVector([-4.5, 5.0, -6.25]),
        );
        let candidate_table = LockOnCandidateTableView::new(0x80_000, 5).unwrap();
        assert!(observe_player_set_generation(generation));
        let player_record = PlayerRecordIndex::new(1, 4).unwrap();
        assert!(attach_player_record_lock_on_output_with_candidate_table(
            generation,
            player_record,
            output,
            candidate_table,
        ));
        let snapshot = current_player_lock_on_snapshot().unwrap();
        assert_eq!(snapshot.generation(), generation);
        assert_eq!(snapshot.player_record(), player_record);
        assert_eq!(snapshot.candidate_count(), Some(5));
        assert_eq!(snapshot.candidate_table(), Some(candidate_table));
        assert_eq!(
            snapshot.selection(),
            crate::player_interaction::LockOnCandidateSelection::Candidate(
                crate::player_interaction::LockOnCandidateIndex::new(4, 5).unwrap()
            )
        );
        assert_eq!(snapshot.scan(), output);
        let candidate = current_player_lock_on_candidate().unwrap();
        assert_eq!(candidate.generation(), generation);
        assert_eq!(candidate.player_record(), player_record);
        assert_eq!(candidate.index().get(), 4);
        assert_eq!(
            candidate.record_address().unwrap().get(),
            0x80_000 + 4 * LockOnCandidateTableView::RECORD_STRIDE
        );
        assert_eq!(candidate.scan(), output);

        let replacement = PlayerSetGeneration::new(82).unwrap();
        assert!(observe_player_set_generation(replacement));
        assert_eq!(current_player_lock_on_snapshot(), None);
        assert_eq!(current_player_lock_on_candidate(), None);

        configure(false);
    }

    #[test]
    fn weapon_configuration_does_not_reset_player_interaction_state() {
        let _weapon_store = crate::weapon_hands_runtime::RUNTIME_STORE_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        configure(true);

        let generation = PlayerSetGeneration::new(77).unwrap();
        let output = LockOnScanOutput::new(
            2,
            LockOnVector([1.0, 2.0, 3.0]),
            LockOnVector([4.0, 5.0, 6.0]),
        );
        assert!(observe_player_set_generation(generation));
        assert!(attach_player_lock_on_output(generation, output));

        crate::weapon_hands_runtime::configure(false);

        assert_eq!(player_lock_on_output(generation), Some(output));
        configure(false);
    }

    #[test]
    fn negative_result_with_unchanged_vectors_does_not_publish_stale_output() {
        let _runtime_store = crate::weapon_hands_runtime::runtime_store_test_guard();
        configure(true);

        let (mut memory, mut regs) = fixture(0xD01C0);
        let mut guest = FixtureGuest {
            result: u32::MAX,
            ..FixtureGuest::default()
        };
        let before = (read_vector(&memory, 0x19000), read_vector(&memory, 0x19020));

        assert!(crate::player_system::run(
            ROOT_D01C0,
            true,
            &mut memory,
            &mut guest,
            &mut regs,
        ));
        let generation = crate::player_wire::PLAYER.rebase_count();
        assert_eq!(
            player_lock_on_output(PlayerSetGeneration::new(generation).unwrap()),
            None,
            "unchanged caller buffers are not evidence of outputs from this scan"
        );
        assert_eq!(
            (read_vector(&memory, 0x19000), read_vector(&memory, 0x19020)),
            before
        );

        configure(false);
    }

    #[test]
    fn unreadable_next_scan_clears_the_previous_current_player_output() {
        let _runtime_store = crate::weapon_hands_runtime::runtime_store_test_guard();
        configure(true);

        let (mut first_memory, mut first_regs) = fixture(0x3398);
        let mut first_guest = FixtureGuest::default();
        assert!(crate::player_system::run(
            ROOT_D01C0,
            true,
            &mut first_memory,
            &mut first_guest,
            &mut first_regs,
        ));
        let generation = crate::player_wire::PLAYER.rebase_count();
        let typed_generation = PlayerSetGeneration::new(generation).unwrap();
        let prior_output = LockOnScanOutput::new(
            2,
            LockOnVector([1.0, 2.0, 3.0]),
            LockOnVector([4.0, 5.0, 6.0]),
        );
        assert!(attach_player_lock_on_output(typed_generation, prior_output));
        assert_eq!(player_lock_on_output(typed_generation), Some(prior_output));

        let (mut second_memory, mut second_regs) = fixture(0x3399);
        second_memory.write32(second_regs.esp + 4, 0x100);
        let mut second_guest = FixtureGuest::default();
        assert!(crate::player_system::run(
            ROOT_D01C0,
            true,
            &mut second_memory,
            &mut second_guest,
            &mut second_regs,
        ));
        assert_eq!(crate::player_wire::PLAYER.rebase_count(), generation);
        assert_eq!(player_lock_on_output(typed_generation), None,);

        configure(false);
    }

    #[test]
    fn nested_adapter_rejects_unchanged_outputs_and_restores_the_callers_argument_stack() {
        let _runtime_store = crate::weapon_hands_runtime::runtime_store_test_guard();
        configure(true);

        let (mut memory, mut regs) = fixture(0xD01C0);
        let args_esp = regs.esp;
        memory.write32(args_esp, 0x19000);
        memory.write32(args_esp + 4, 0x19020);
        let mut guest = FixtureGuest::default();

        assert!(run_nested(ROOT_D01C0, &mut memory, &mut guest, &mut regs));
        assert_eq!(regs.esp, args_esp);
        let generation = crate::player_wire::PLAYER.rebase_count();
        assert_eq!(
            player_lock_on_output(PlayerSetGeneration::new(generation).unwrap()),
            None,
            "the nested no-slot scan must not publish unchanged buffers"
        );

        configure(false);
    }

    #[test]
    fn nested_lifted_reference_projection_records_result_and_both_vectors() {
        let _serial = crate::weapon_hands_runtime::runtime_store_test_guard();
        configure(false);
        let (mut memory, regs) = fixture(0xD01C1);
        memory.write32(regs.esp, 0x19000);
        memory.write32(regs.esp + 4, 0x19020);
        record_lifted_reference(&memory, regs.esp, u32::MAX);

        let mut diagnostics = [0; DIAGNOSTICS_WORDS];
        assert!(unsafe {
            tsfp_player_interaction_trace_diagnostics_snapshot(
                diagnostics.as_mut_ptr(),
                diagnostics.len() as u32,
            )
        });
        assert_eq!(diagnostics, [1, 1, 0, 0, 1, 0]);

        let mut output = [0; TRACE_WORDS];
        assert!(unsafe {
            tsfp_player_interaction_trace_snapshot(output.as_mut_ptr(), TRACE_WORDS as u32)
        });
        assert_eq!(&output[..2], &[2, 1]);
        assert_ne!(output[2], 0);
        assert_eq!(output[3..6], [1, 4, u32::MAX]);
        for (index, address) in [0x19000, 0x19020].into_iter().enumerate() {
            for axis in 0..3 {
                assert_eq!(
                    output[6 + index * 3 + axis],
                    memory.read32(address + axis as u32 * 4)
                );
            }
        }
        configure(false);
    }

    #[test]
    fn interaction_trace_projection_keeps_signed_result_and_both_vectors() {
        let scan = LockOnScanOutput::new(
            -1,
            LockOnVector([1.25, -2.5, 3.75]),
            LockOnVector([-4.5, 5.25, -6.75]),
        );
        let snapshot = PlayerLockOnSnapshot::new(
            PlayerSetGeneration::new(7).unwrap(),
            PlayerRecordIndex::new(1, 4).unwrap(),
            Some(5),
            scan,
        );
        let mut words = [0; TRACE_WORDS];
        encode_snapshot_trace(Some(snapshot), &mut words);
        assert_eq!(words[0..6], [2, 1, 7, 1, 5, u32::MAX]);
        assert_eq!(
            &words[6..],
            &[1.25f32, -2.5, 3.75, -4.5, 5.25, -6.75].map(f32::to_bits)
        );
    }

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct CandidateFilterGuest {
        calls: Vec<u32>,
        query: u32,
    }

    impl GuestCall for CandidateFilterGuest {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            self.calls.push(va);
            regs.eax = match va {
                0x000d_6f00 => 0,
                0x000d_a220 => 1,
                0x0009_e6f0 => {
                    regs.ecx = 1;
                    regs.edx = self.query;
                    1
                }
                _ => panic!("unexpected D00F0 helper {va:#x}"),
            };
            // GuestCall receives ESP before the trampoline pushes the return slot;
            // the callee's RET restores that slot, so this boundary preserves ESP.
        }
    }

    fn assert_candidate_filter_matches_translated(candidate_kind: u32, nested: bool) -> Vec<u32> {
        let _serial = crate::weapon_hands_runtime::runtime_store_test_guard();
        configure(true);
        let mut native_memory = FixtureMemory::default();
        let mut translated_memory = FixtureMemory::default();
        let stack = 0x8000;
        let player = 0x19000;
        let candidate = 0x19100;
        let candidate_owner = 0x19200;
        let measured_object = 0x19300;
        for memory in [&mut native_memory, &mut translated_memory] {
            let native_args = if nested { stack } else { stack + 4 };
            let return_slot = if nested { stack - 4 } else { stack };
            memory.write32(return_slot, if nested { 0 } else { 0xdead_beef });
            memory.write32(native_args, player);
            memory.write32(native_args + 4, candidate);
            memory.write32(0x007b_625c, 1);
            memory.write32(0x0071_6148, candidate);
            memory.write32(candidate + 0x20, candidate_kind);
            memory.write32(candidate + 0x1cc, 2.0f32.to_bits());
            memory.write8(candidate + 0x28, 1);
            memory.write32(0x0047_5cac, 1.0f32.to_bits());
            memory.write32(candidate + 0x7c, candidate_owner);
            memory.write32(candidate_owner, measured_object);
            memory.write32(measured_object + 0x1cc, 2.0f32.to_bits());
        }

        let mut native_regs: ExtRegisters = unsafe { core::mem::zeroed() };
        native_regs.esp = stack;
        native_regs.esi = 0x1122_3344;
        native_regs.ecx = 0x5566_7788;
        let mut translated_regs = native_regs;
        translated_regs.esp = if nested { stack - 4 } else { stack };
        let mut native_guest = CandidateFilterGuest {
            query: candidate,
            ..CandidateFilterGuest::default()
        };
        let mut translated_guest = native_guest.clone();

        let handled = if nested {
            run_nested_candidate_filter(
                ROOT_D00F0,
                &mut native_memory,
                &mut native_guest,
                &mut native_regs,
            )
        } else {
            run_candidate_filter(
                ROOT_D00F0,
                &mut native_memory,
                &mut native_guest,
                &mut native_regs,
            )
        };
        assert!(handled);
        assert!(crate::c2r::gameplay_player::run(
            ROOT_D00F0,
            &mut translated_memory,
            &mut translated_guest,
            &mut translated_regs,
        ));
        if nested {
            translated_regs.esp = stack;
        }
        assert_eq!(native_memory, translated_memory);
        assert_eq!(native_regs, translated_regs);
        assert_eq!(
            native_guest.calls,
            if candidate_kind == 8 {
                vec![0x000d_6f00, 0x000d_a220]
            } else {
                Vec::new()
            }
        );
        assert_eq!(
            translated_guest.calls,
            if candidate_kind == 8 {
                vec![0x000d_6f00, 0x000d_a220]
            } else {
                vec![0x0009_e6f0]
            }
        );
        configure(false);
        translated_guest.calls
    }

    #[test]
    fn native_candidate_filter_matches_translated_non_type_eight_branch() {
        assert_eq!(
            assert_candidate_filter_matches_translated(0, false),
            [0x0009_e6f0]
        );
    }

    #[test]
    fn native_candidate_filter_matches_translated_type_eight_helper_chain() {
        assert_eq!(
            assert_candidate_filter_matches_translated(8, false),
            [0x000d_6f00, 0x000d_a220]
        );
    }

    #[test]
    fn nested_native_candidate_filter_matches_translated_stack_contract() {
        assert_eq!(
            assert_candidate_filter_matches_translated(8, true),
            [0x000d_6f00, 0x000d_a220]
        );
    }
}

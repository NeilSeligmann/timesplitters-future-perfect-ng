// SPDX-License-Identifier: GPL-3.0-or-later
//! T3396 host adapter for the measured deferred contact queue.
//!
//! The typed queue is owned by `ContactDeliverySystem` per guest thread. Until
//! the contact owner callbacks are ported, the four guest row words and owner
//! pointer array are a TEMPORARY-SHIM required by the lifted callback ABI. The
//! original `0x170690` contact pass still decides which rows to append.

use crate::ui_live::HostUiGuestApi;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashSet};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::OnceLock;
use tsfp_engine_physics::rea_events::{
    ContactArena, ContactDeliveryHandlers, ContactDeliverySystem, ContactGuestId, ContactIndex,
    DeferredEvents, ImmediateContactEvent, NativeContactCallbacks, OwnerCallbacks, OwnerHandle,
    PhysicsContact, PhysicsEvent,
};

const ENQUEUE: u32 = 0x16_c070;
const DRAIN: u32 = 0x16_c0d0;
const CONTACT_PASS: u32 = 0x17_0690;
const GUEST_FREE: u32 = 0x03_e1e0;
const OWNERS_GLOBAL: u32 = 0x74_c484;
const ROWS_GLOBAL: u32 = 0x74_c474;
const CAPACITY_GLOBAL: u32 = 0x74_c47c;
const COUNT_GLOBAL: u32 = 0x74_c490;
const CONTACT_NEXT_OFFSET: u32 = 0x34;
// These are the two return sites in the lifted 0x170690 pass that issue the
// measured +0x1EC body callbacks. The direct-call interceptor is global while
// enabled, so the return-site guard keeps unrelated callbacks on the guest path.
const CONTACT_CALLBACK_RETURNS: [u32; 2] = [0x0017_0aa2, 0x0017_0dd8];

pub type DirectCallCallback = unsafe extern "C" fn(
    callee: u32,
    eax: u32,
    ecx: u32,
    esp: u32,
    eax_out: *mut u32,
    api: *const HostUiGuestApi,
) -> bool;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ContactPassArguments {
    header: u32,
    output: u32,
    capacity: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct QueueGlobals {
    owners: u32,
    rows: u32,
    capacity: u32,
    count: u32,
}

struct LiveQueue {
    globals: QueueGlobals,
    frame: tsfp_engine_physics::rea_events::ContactDeliveryFrame,
}

thread_local! {
    static QUEUE: RefCell<Option<LiveQueue>> = const { RefCell::new(None) };
    static NATIVE_CALLBACKS: RefCell<NativeContactHandlerRegistry> =
        RefCell::new(NativeContactHandlerRegistry::default());
    static NATIVE_OWNER_REGISTRATIONS: RefCell<HashSet<OwnerHandle>> =
        RefCell::new(HashSet::new());
    static PENDING_NATIVE_OWNER_RETIREMENTS: RefCell<HashSet<OwnerHandle>> =
        RefCell::new(HashSet::new());
    static LOGGED_CALLBACK_TARGETS: RefCell<BTreeSet<(u32, u32)>> =
        RefCell::new(BTreeSet::new());
    static LOGGED_CONTACT_CALLS: RefCell<BTreeSet<(u32, u32)>> =
        RefCell::new(BTreeSet::new());
    static LOGGED_CONTACT_PASS_ARGS: RefCell<BTreeSet<ContactPassArguments>> =
        RefCell::new(BTreeSet::new());
}

type NativeContactHandlerRegistry = NativeContactCallbacks<()>;

static TARGET_LOG_ENABLED: OnceLock<bool> = OnceLock::new();
static CONTACT_CALL_LOG_ENABLED: OnceLock<bool> = OnceLock::new();
static CONTACT_PASS_ARGS_LOG_ENABLED: OnceLock<bool> = OnceLock::new();

/// Thread-affine lifetime token for a native physics owner. Handlers capture
/// the owner's Rust state in the thread-local callback table; dropping the
/// token unregisters both callback kinds before that state can be retired.
///
/// This is the production registration seam for Rust-owned owners. Guest
/// owner pointers remain compatibility identities only while the lifted
/// contact pass calls through the default-off adapter.
pub struct NativeContactOwnerRegistration {
    owner: OwnerHandle,
    _thread_affine: PhantomData<Rc<()>>,
}

impl NativeContactOwnerRegistration {
    /// Reserve a guest-compatible owner identity for one live native owner.
    /// Null identities and duplicate live registrations are rejected.
    pub fn new(owner: OwnerHandle) -> Option<Self> {
        if owner.0 == 0 {
            return None;
        }
        let reserved = NATIVE_OWNER_REGISTRATIONS.with(|owners| owners.borrow_mut().insert(owner));
        if !reserved {
            return None;
        }
        Some(Self {
            owner,
            _thread_affine: PhantomData,
        })
    }

    pub fn owner(&self) -> OwnerHandle {
        self.owner
    }

    /// Install or replace the immediate contact handler for this owner.
    pub fn on_immediate<H>(&mut self, handler: H)
    where
        H: FnMut(ImmediateContactEvent, &mut DeferredEvents) + 'static,
    {
        register_native_immediate_callback(self.owner, handler);
    }

    /// Install or replace the deferred contact handler for this owner.
    pub fn on_deferred<H>(&mut self, handler: H)
    where
        H: FnMut(OwnerHandle, usize, &mut DeferredEvents) + 'static,
    {
        register_native_deferred_callback(self.owner, handler);
    }
}

impl Drop for NativeContactOwnerRegistration {
    fn drop(&mut self) {
        PENDING_NATIVE_OWNER_RETIREMENTS.with(|pending| pending.borrow_mut().insert(self.owner));
        process_pending_native_owner_retirements();
    }
}

/// Retire tokens dropped from inside a native callback after its registry borrow
/// ends. Callback closures are held by `NATIVE_CALLBACKS`, so unregistering
/// reentrantly would otherwise panic in `RefCell` and could drop captured owner
/// state while that callback is still running.
fn process_pending_native_owner_retirements() {
    let pending = PENDING_NATIVE_OWNER_RETIREMENTS
        .with(|owners| owners.borrow().iter().copied().collect::<Vec<_>>());
    if pending.is_empty() {
        return;
    }

    let Some(()) = NATIVE_CALLBACKS.with(|callbacks| {
        let Ok(mut callbacks) = callbacks.try_borrow_mut() else {
            return None;
        };
        for owner in &pending {
            callbacks.unregister(*owner);
        }
        Some(())
    }) else {
        return;
    };

    NATIVE_OWNER_REGISTRATIONS.with(|owners| {
        let mut owners = owners.borrow_mut();
        for owner in &pending {
            owners.remove(owner);
        }
    });
    PENDING_NATIVE_OWNER_RETIREMENTS.with(|owners| {
        let mut owners = owners.borrow_mut();
        for owner in &pending {
            owners.remove(owner);
        }
    });
}

fn remember_contact_call(seen: &mut BTreeSet<(u32, u32)>, root: u32, caller: u32) -> bool {
    seen.insert((root, caller))
}

fn contact_pass_arguments(
    api: &HostUiGuestApi,
    header: u32,
    esp: u32,
) -> Option<ContactPassArguments> {
    Some(ContactPassArguments {
        header,
        output: api.read32_raw(esp.checked_add(4)?)?,
        capacity: api.read32_raw(esp.checked_add(8)?)? as i32,
    })
}

/// Throw-away bounded input measurement for the outer contact pass. The REA
/// contract places the list header in EAX and output pointer/capacity on the
/// cdecl stack; this log preserves those inputs for port design without
/// changing the lifted call or traversing guest contact records.
fn log_contact_pass_arguments(api: &HostUiGuestApi, header: u32, esp: u32) {
    if !*CONTACT_PASS_ARGS_LOG_ENABLED
        .get_or_init(|| std::env::var_os("TSFP_CONTACT_PASS_ARGS_LOG").is_some())
    {
        return;
    }
    let Some(args) = contact_pass_arguments(api, header, esp) else {
        return;
    };
    let first = LOGGED_CONTACT_PASS_ARGS.with(|seen| {
        let mut seen = seen.borrow_mut();
        seen.len() < 16 && seen.insert(args)
    });
    if first {
        eprintln!(
            "T3396 contact pass args header=0x{:08X} output=0x{:08X} capacity={}",
            args.header, args.output, args.capacity
        );
    }
}

/// Throw-away runtime measurement for locating contact-pass/helper coverage.
/// The opt-in logs each distinct root/caller pair once without changing the
/// lifted call or Rust adapter result.
fn log_contact_call(api: &HostUiGuestApi, callee: u32, esp: u32) {
    if !matches!(callee, CONTACT_PASS | ENQUEUE | DRAIN)
        || !*CONTACT_CALL_LOG_ENABLED
            .get_or_init(|| std::env::var_os("TSFP_CONTACT_CALLS_LOG").is_some())
    {
        return;
    }
    let Some(caller) = api.read32_raw(esp) else {
        return;
    };
    let first = LOGGED_CONTACT_CALLS
        .with(|seen| remember_contact_call(&mut seen.borrow_mut(), callee, caller));
    if first {
        eprintln!("T3396 contact direct-call root=0x{callee:08X} caller=0x{caller:08X}");
    }
}

fn remember_callback_target(
    seen: &mut BTreeSet<(u32, u32)>,
    return_address: u32,
    callee: u32,
) -> bool {
    seen.insert((return_address, callee))
}

fn is_contact_callback_log_site(return_address: u32) -> bool {
    // The target census must observe any direct call returning inside the
    // physics contact pass, including unexpected callsites that would otherwise
    // make an exact-site-only diagnostic falsely report no targets. Dispatch
    // below still accepts only CONTACT_CALLBACK_RETURNS.
    (0x0017_0000..0x0017_1000).contains(&return_address)
}

/// Throw-away runtime measurement for finding production contact owners. The
/// opt-in writes each distinct target once; it does not consume or alter the
/// lifted callback path. Keep this disabled in normal runs.
fn log_callback_target(return_address: u32, callee: u32) {
    if !is_contact_callback_log_site(return_address) {
        return;
    }
    if !*TARGET_LOG_ENABLED.get_or_init(|| std::env::var_os("TSFP_CONTACT_TARGETS_LOG").is_some()) {
        return;
    }
    let first = LOGGED_CALLBACK_TARGETS
        .with(|seen| remember_callback_target(&mut seen.borrow_mut(), return_address, callee));
    if first {
        eprintln!(
            "T3396 contact callback target return=0x{return_address:08X} callee=0x{callee:08X}"
        );
    }
}

/// Register an owned Rust deferred callback for one owner handle. The closure
/// may capture native typed owner state; the lifted `owner+0x170` callback
/// remains the fallback for unregistered owners. Registration is thread-local
/// to the guest thread that runs the callback.
/// Verification tier: Tier 0 host attachment; production owners are not yet
/// registered by the game.
fn register_native_deferred_callback<H>(owner: OwnerHandle, mut handler: H)
where
    H: FnMut(OwnerHandle, usize, &mut DeferredEvents) + 'static,
{
    NATIVE_CALLBACKS.with(|callbacks| {
        callbacks
            .borrow_mut()
            .register_deferred(owner, move |_, owner, slot, queue| {
                handler(owner, slot, queue)
            });
    });
}

/// Register an owned Rust immediate callback for one owner. The closure may
/// capture native typed owner state. With the opt-in bridge enabled, direct
/// calls from the two measured `0x170690` `+0x1EC` return sites dispatch here;
/// all other callsites remain lifted. Registration is thread-local to the
/// guest thread that runs the callback.
/// Verification tier: Tier 0 host registry attachment; no production owner is
/// registered yet.
fn register_native_immediate_callback<H>(owner: OwnerHandle, mut handler: H)
where
    H: FnMut(ImmediateContactEvent, &mut DeferredEvents) + 'static,
{
    NATIVE_CALLBACKS.with(|callbacks| {
        callbacks
            .borrow_mut()
            .register_immediate(owner, move |_, event, queue| handler(event, queue));
    });
}

/// Dispatch an already-resolved immediate event through a Rust-owned owner
/// callback. The caller owns the deferred queue, so callback-time enqueues stay
/// in the same typed frame state. The host direct-call adapter connects the two
/// measured callback return sites; the surrounding contact pass remains lifted.
/// Verification tier: Tier 0 host attachment; event construction is FABRICATED.
pub fn dispatch_native_immediate_callback(
    event: ImmediateContactEvent,
    deferred: &mut DeferredEvents,
) -> bool {
    let dispatched = NATIVE_CALLBACKS.with(|callbacks| {
        let mut callbacks = callbacks.borrow_mut();
        if !event.recipient_is_contact_endpoint()
            || !callbacks.has_immediate_callback(event.recipient)
        {
            return false;
        }
        callbacks.deliver_immediate(event, deferred);
        true
    });
    process_pending_native_owner_retirements();
    dispatched
}

/// Remove immediate and deferred native callback registrations for an owner.
fn unregister_native_callbacks(owner: OwnerHandle) -> bool {
    let removed = NATIVE_CALLBACKS.with(|callbacks| callbacks.borrow_mut().unregister(owner));
    process_pending_native_owner_retirements();
    removed
}

fn read_globals(api: &HostUiGuestApi) -> Option<QueueGlobals> {
    Some(QueueGlobals {
        owners: api.read32_raw(OWNERS_GLOBAL)?,
        rows: api.read32_raw(ROWS_GLOBAL)?,
        capacity: api.read32_raw(CAPACITY_GLOBAL)?,
        count: api.read32_raw(COUNT_GLOBAL)?,
    })
}

fn word_address(base: u32, index: u32) -> Option<u32> {
    base.checked_add(index.checked_mul(4)?)
}

fn row_address(base: u32, index: u32) -> Option<u32> {
    base.checked_add(index.checked_mul(16)?)
}

fn event_from_words(
    api: &HostUiGuestApi,
    recipient: u32,
    words: [u32; 4],
    frame: &mut tsfp_engine_physics::rea_events::ContactDeliveryFrame,
) -> Option<PhysicsEvent> {
    let guest_contact = words[3];
    if guest_contact == 0 {
        return None;
    }
    let contact = contact_lease_at(
        api,
        guest_contact,
        frame,
        Some([f32::from_bits(words[1]), f32::from_bits(words[2])]),
        None,
    )?;
    Some(PhysicsEvent {
        recipient: OwnerHandle(recipient),
        other: (words[0] != 0).then_some(OwnerHandle(words[0])),
        horizontal_normal_bits: [words[1], words[2]],
        contact,
    })
}

fn contact_lease_at(
    api: &HostUiGuestApi,
    guest_contact: u32,
    frame: &mut tsfp_engine_physics::rea_events::ContactDeliveryFrame,
    horizontal_normal: Option<[f32; 2]>,
    relative_speed_bits: Option<u32>,
) -> Option<tsfp_engine_physics::rea_events::ContactLease> {
    let kind_word = api.read32_raw(guest_contact.checked_add(0x2c)?)?;
    let owner_a = api.read32_raw(guest_contact.checked_add(0x24)?)?;
    let owner_b = api.read32_raw(guest_contact.checked_add(0x28)?)?;
    let guest_next = api.read32_raw(guest_contact.checked_add(CONTACT_NEXT_OFFSET)?)?;
    frame
        .retain_contact(PhysicsContact {
            kind: (kind_word >> 16) as u16,
            owner_a: (owner_a != 0).then_some(OwnerHandle(owner_a)),
            owner_b: (owner_b != 0).then_some(OwnerHandle(owner_b)),
            horizontal_normal,
            relative_speed_bits,
            next: None,
            guest_next: (guest_next != 0).then_some(ContactGuestId(guest_next)),
            guest_identity: Some(ContactGuestId(guest_contact)),
        })
        .ok()
}

fn immediate_event_from_stack(
    api: &HostUiGuestApi,
    esp: u32,
    frame: &mut tsfp_engine_physics::rea_events::ContactDeliveryFrame,
) -> Option<ImmediateContactEvent> {
    let recipient = api.read32_raw(esp.checked_add(4)?)?;
    let guest_contact = api.read32_raw(esp.checked_add(8)?)?;
    let relative_speed_bits = api.read32_raw(esp.checked_add(12)?)?;
    if recipient == 0 || guest_contact == 0 {
        return None;
    }
    Some(ImmediateContactEvent {
        recipient: OwnerHandle(recipient),
        contact: contact_lease_at(api, guest_contact, frame, None, Some(relative_speed_bits))?,
        relative_speed_bits,
    })
}

fn read_event_at(
    api: &HostUiGuestApi,
    globals: QueueGlobals,
    slot: u32,
    frame: &mut tsfp_engine_physics::rea_events::ContactDeliveryFrame,
) -> Option<PhysicsEvent> {
    let owner_address = word_address(globals.owners, slot)?;
    let row = row_address(globals.rows, slot)?;
    let recipient = api.read32_raw(owner_address)?;
    let words = [
        api.read32_raw(row)?,
        api.read32_raw(row.checked_add(4)?)?,
        api.read32_raw(row.checked_add(8)?)?,
        api.read32_raw(row.checked_add(12)?)?,
    ];
    event_from_words(api, recipient, words, frame)
}

fn build_queue(api: &HostUiGuestApi, globals: QueueGlobals) -> Option<LiveQueue> {
    let count = globals.count as i32;
    let capacity = globals.capacity as i32;
    if count < 0 || count > capacity || (count > 0 && (globals.owners == 0 || globals.rows == 0)) {
        return None;
    }
    let mut frame = tsfp_engine_physics::rea_events::ContactDeliveryFrame::new(
        Vec::new(),
        globals.capacity as usize,
    );
    for slot in 0..globals.count {
        let event = read_event_at(api, globals, slot, &mut frame)?;
        if !frame.delivery_mut().enqueue_deferred(event) {
            return None;
        }
    }
    Some(LiveQueue { globals, frame })
}

fn ensure_queue(api: &HostUiGuestApi, globals: QueueGlobals) -> bool {
    QUEUE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.as_ref().is_some_and(|live| {
            live.globals.owners == globals.owners
                && live.globals.rows == globals.rows
                && live.globals.capacity == globals.capacity
                && live.frame.delivery().deferred_count() == globals.count as usize
        }) {
            return true;
        }
        let Some(live) = build_queue(api, globals) else {
            return false;
        };
        *slot = Some(live);
        true
    })
}

fn dispatch_registered_immediate(
    system: &mut ContactDeliverySystem,
    event: ImmediateContactEvent,
) -> bool {
    let dispatched = NATIVE_CALLBACKS
        .with(|callbacks| system.dispatch_immediate(event, &mut *callbacks.borrow_mut()));
    process_pending_native_owner_retirements();
    dispatched
}

/// Route one lifted body `+0x1EC` callback through the Rust owner registry.
/// The original pass still chooses the callback edge; only registered owners
/// are consumed here. The stack arguments and contact identity are borrowed
/// from guest state for this call, while callback-time deferred additions use
/// the same typed queue as the later `0x16C0D0` drain.
fn immediate_callback(api: &HostUiGuestApi, callee: u32, esp: u32) -> bool {
    let Some(return_address) = api.read32_raw(esp) else {
        return false;
    };
    if callee == 0 {
        return false;
    }
    log_callback_target(return_address, callee);
    if !CONTACT_CALLBACK_RETURNS.contains(&return_address) {
        return false;
    }
    let Some(recipient) = esp
        .checked_add(4)
        .and_then(|address| api.read32_raw(address))
        .map(OwnerHandle)
    else {
        return false;
    };
    let registered =
        NATIVE_CALLBACKS.with(|callbacks| callbacks.borrow().has_immediate_callback(recipient));
    if !registered {
        return false;
    }
    let Some(globals) = read_globals(api) else {
        return false;
    };
    if !ensure_queue(api, globals) {
        return false;
    }
    let result = QUEUE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let live = slot.as_mut()?;
        let event = immediate_event_from_stack(api, esp, &mut live.frame)?;
        let live_contact = event.contact.clone();
        let (Some(guest_identity), old_guest_next) =
            live_contact.with(|contact| (contact.guest_identity, contact.guest_next))?
        else {
            return None;
        };
        let dispatched = dispatch_registered_immediate(live.frame.delivery_mut(), event);
        Some((dispatched, live_contact, guest_identity, old_guest_next))
    });
    let Some((true, live_contact, guest_identity, old_guest_next)) = result else {
        return false;
    };
    if !sync_guest_contact_next(api, &live_contact, guest_identity, old_guest_next) {
        eprintln!("T3396 native contact callback could not mirror contact+0x34");
        std::process::abort();
    }
    let mirrored = QUEUE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(live) = slot.as_mut() else {
            return false;
        };
        let Some(globals) = mirror_queue(api, globals, live.frame.delivery()) else {
            return false;
        };
        live.globals = globals;
        true
    });
    // The callback has already run. Falling through would invoke it twice.
    if !mirrored {
        eprintln!("T3396 native contact callback queue could not be mirrored to guest state");
        std::process::abort();
    }
    true
}

/// Mirror a callback-edited typed list link to the temporary guest adapter.
/// T1675's measured-static pass reads `+0x34` after endpoint callbacks, so this
/// typed link writeback must complete before the lifted caller resumes. The
/// adapter remains default-off; mutation or retirement of later nodes remains
/// unresolved, and no production Rust owner is registered.
fn sync_guest_contact_next(
    api: &HostUiGuestApi,
    contact: &tsfp_engine_physics::rea_events::ContactLease,
    guest_identity: ContactGuestId,
    previous: Option<ContactGuestId>,
) -> bool {
    let Some(next) = contact.with(|record| record.guest_next) else {
        return false;
    };
    if next == previous {
        return true;
    }
    let Some(address) = guest_identity.0.checked_add(CONTACT_NEXT_OFFSET) else {
        return false;
    };
    if api.read32_raw(address).is_none() {
        return false;
    }
    api.write32_raw(address, next.map_or(0, |id| id.0))
}

fn reset_queue(api: &HostUiGuestApi, globals: QueueGlobals) -> bool {
    let Some(live) = build_queue(api, globals) else {
        return false;
    };
    QUEUE.with(|slot| *slot.borrow_mut() = Some(live));
    true
}

/// Publish the typed queue into its guest compatibility arrays, writing every
/// active owner/row before publishing the count. Native callbacks can only
/// enqueue contacts imported from guest memory, so each row retains a guest
/// contact identity for this TEMPORARY-SHIM mirror.
fn mirror_queue(
    api: &HostUiGuestApi,
    globals: QueueGlobals,
    system: &ContactDeliverySystem,
) -> Option<QueueGlobals> {
    let count = u32::try_from(system.deferred_count()).ok()?;
    if count > globals.capacity || (count != 0 && (globals.owners == 0 || globals.rows == 0)) {
        return None;
    }
    let mut writes = Vec::with_capacity(count as usize * 5);
    for slot in 0..count {
        let event = system.deferred_event(slot as usize)?;
        let contact = event.contact.guest_identity()?.0;
        let owner_slot = word_address(globals.owners, slot)?;
        let row_slot = row_address(globals.rows, slot)?;
        let words = [
            event.other.map_or(0, |owner| owner.0),
            event.horizontal_normal_bits[0],
            event.horizontal_normal_bits[1],
            contact,
        ];
        if api.read32_raw(owner_slot).is_none() {
            return None;
        }
        writes.push((owner_slot, event.recipient.0));
        for (index, word) in words.into_iter().enumerate() {
            let address = row_slot.checked_add(index as u32 * 4)?;
            if api.read32_raw(address).is_none() {
                return None;
            }
            writes.push((address, word));
        }
    }
    if api.read32_raw(COUNT_GLOBAL).is_none() {
        return None;
    }
    for (address, value) in writes {
        if !api.write32_raw(address, value) {
            return None;
        }
    }
    if !api.write32_raw(COUNT_GLOBAL, count) {
        return None;
    }
    Some(QueueGlobals { count, ..globals })
}

/// Simulate the measured write order first, including source/destination aliasing,
/// so an unsupported pointer declines before it mutates guest state.
fn append(api: &HostUiGuestApi, ecx: u32, esp: u32) -> bool {
    let Some(mut globals) = read_globals(api) else {
        return false;
    };
    if globals.owners == 0 || globals.rows == 0 || globals.capacity == 0 {
        return true;
    }
    if (globals.count as i32) < 0 || (globals.count as i32) > globals.capacity as i32 {
        return false;
    }
    if globals.count >= globals.capacity {
        return true;
    }
    if !ensure_queue(api, globals) {
        return false;
    }

    let Some(stack_owner) = esp.checked_add(4) else {
        return false;
    };
    let Some(recipient) = api.read32_raw(stack_owner) else {
        return false;
    };
    let Some(owner_slot) = word_address(globals.owners, globals.count) else {
        return false;
    };
    let Some(row_slot) = row_address(globals.rows, globals.count) else {
        return false;
    };
    if api.read32_raw(owner_slot).is_none() || api.read32_raw(COUNT_GLOBAL).is_none() {
        return false;
    }
    let mut writes = vec![(owner_slot, recipient)];
    let mut words = [0; 4];
    for (index, word) in words.iter_mut().enumerate() {
        let offset = (index as u32) * 4;
        let Some(source) = ecx.checked_add(offset) else {
            return false;
        };
        let Some(destination) = row_slot.checked_add(offset) else {
            return false;
        };
        if api.read32_raw(destination).is_none() {
            return false;
        }
        let value = writes
            .iter()
            .rev()
            .find_map(|(address, value)| (*address == source).then_some(*value))
            .or_else(|| api.read32_raw(source));
        let Some(value) = value else {
            return false;
        };
        *word = value;
        writes.push((destination, value));
    }
    let event = QUEUE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let live = slot.as_mut()?;
        event_from_words(api, recipient, words, &mut live.frame)
    });
    let Some(event) = event else {
        return false;
    };

    for (address, value) in writes {
        if !api.write32_raw(address, value) {
            return true;
        }
    }
    let accepted = QUEUE.with(|slot| {
        let mut slot = slot.borrow_mut();
        slot.as_mut()
            .is_some_and(|live| live.frame.delivery_mut().enqueue_deferred(event))
    });
    globals.count = globals.count.wrapping_add(1);
    if !accepted {
        // The guest writes are already committed. Rebuild the typed view from
        // the same row and publish the original count transition.
        let _ = reset_queue(
            api,
            QueueGlobals {
                count: globals.count,
                ..globals
            },
        );
    }
    if !api.write32_raw(COUNT_GLOBAL, globals.count) {
        return true;
    }
    QUEUE.with(|slot| {
        if let Some(live) = slot.borrow_mut().as_mut() {
            live.globals = globals;
        }
    });
    true
}

fn drain(api: &HostUiGuestApi) -> bool {
    let Some(mut globals) = read_globals(api) else {
        return false;
    };
    if globals.owners == 0 || globals.rows == 0 {
        return true;
    }
    if !ensure_queue(api, globals) {
        return false;
    }
    let mut captured_owners = globals.owners;
    let mut local_count = globals.count;
    while local_count != 0 {
        let popped = QUEUE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let live = slot.as_mut()?;
            let (index, _event) = live.frame.delivery_mut().pop_deferred_lifo()?;
            let count = live.frame.delivery().deferred_count() as u32;
            live.globals.count = count;
            Some((index as u32, count))
        });
        let Some((index, count)) = popped else {
            return false;
        };
        local_count = count;
        if !api.write32_raw(COUNT_GLOBAL, local_count) {
            return true;
        }

        let Some(owner_address) = word_address(captured_owners, index) else {
            return true;
        };
        let Some(owner) = api.read32_raw(owner_address) else {
            return true;
        };
        let Some(rows) = api.read32_raw(ROWS_GLOBAL) else {
            return true;
        };
        let Some(row) = row_address(rows, index) else {
            return true;
        };
        let Some(other) = api.read32_raw(row) else {
            return true;
        };
        if other == 0 || owner == 0 {
            continue;
        }
        let owner_handle = OwnerHandle(owner);
        let has_native =
            NATIVE_CALLBACKS.with(|callbacks| callbacks.borrow().has_callback(owner_handle));
        let guest_callback = if has_native {
            None
        } else {
            let Some(callback_address) = owner.checked_add(0x170) else {
                continue;
            };
            let Some(callback) = api.read32_raw(callback_address) else {
                continue;
            };
            if callback == 0 {
                continue;
            }
            Some(callback)
        };
        if has_native {
            let event = QUEUE.with(|slot| {
                let mut slot = slot.borrow_mut();
                let live = slot.as_mut()?;
                read_event_at(api, globals, index, &mut live.frame)
            });
            let Some(event) = event else {
                let Some(original_count) = local_count.checked_add(1) else {
                    return true;
                };
                let restored = api.write32_raw(COUNT_GLOBAL, original_count);
                let _ = reset_queue(
                    api,
                    QueueGlobals {
                        count: original_count,
                        ..globals
                    },
                );
                return !restored;
            };
            let delivered = NATIVE_CALLBACKS.with(|callbacks| {
                let mut callbacks = callbacks.borrow_mut();
                QUEUE.with(|slot| {
                    let mut slot = slot.borrow_mut();
                    let live = slot.as_mut()?;
                    if !live
                        .frame
                        .delivery_mut()
                        .replace_deferred_row(index as usize, event)
                    {
                        return Some(false);
                    }
                    Some(live.frame.delivery_mut().deliver_deferred_callback(
                        owner_handle,
                        index as usize,
                        &mut *callbacks,
                    ))
                })
            });
            process_pending_native_owner_retirements();
            if delivered != Some(true) {
                let Some(original_count) = local_count.checked_add(1) else {
                    return true;
                };
                let restored = api.write32_raw(COUNT_GLOBAL, original_count);
                let _ = reset_queue(
                    api,
                    QueueGlobals {
                        count: original_count,
                        ..globals
                    },
                );
                return !restored;
            }
            let mirrored = QUEUE.with(|slot| {
                let mut slot = slot.borrow_mut();
                let live = slot.as_mut()?;
                let updated = mirror_queue(api, live.globals, live.frame.delivery())?;
                live.globals = updated;
                Some(updated)
            });
            let Some(updated) = mirrored else {
                eprintln!(
                    "T3396 native contact callback queue could not be mirrored to guest state"
                );
                std::process::abort();
            };
            globals = updated;
            captured_owners = updated.owners;
            local_count = updated.count;
            continue;
        }
        let args = [owner, row];
        if api
            .call_raw(
                guest_callback.expect("unregistered owners require a lifted callback"),
                &args,
            )
            .is_none()
        {
            // A failed guest-call lookup is guaranteed to leave guest registers
            // untouched. Restore the published count and let the original drain
            // body take over the still-unmodified row and owner arrays.
            let Some(original_count) = local_count.checked_add(1) else {
                return true;
            };
            let restored = api.write32_raw(COUNT_GLOBAL, original_count);
            let _ = reset_queue(
                api,
                QueueGlobals {
                    count: original_count,
                    ..globals
                },
            );
            return !restored;
        }
        let Some(after_callback) = read_globals(api) else {
            return true;
        };
        captured_owners = after_callback.owners;
        local_count = after_callback.count;
        globals = after_callback;
        if !reset_queue(api, after_callback) {
            return true;
        }
    }

    // Match the original teardown order: free the last owner table, then reread
    // and free the current row table, and only then clear the four globals.
    let Some(rows) = api.read32_raw(ROWS_GLOBAL) else {
        return true;
    };
    let _ = api.call_raw(GUEST_FREE, &[captured_owners]);
    let _ = api.call_raw(GUEST_FREE, &[rows]);
    for address in [OWNERS_GLOBAL, ROWS_GLOBAL, CAPACITY_GLOBAL, COUNT_GLOBAL] {
        if !api.write32_raw(address, 0) {
            return true;
        }
    }
    let closed = QUEUE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(live) = slot.as_mut()
            && live.frame.finish().is_err()
        {
            return false;
        }
        *slot = None;
        true
    });
    if !closed {
        eprintln!("T3396 native contact frame closed with pending deferred events");
        std::process::abort();
    }
    let _ = globals;
    true
}

/// Called by the generated `RECOMP_ABI_CALL` wrapper only when
/// `--rust-contact-delivery` is selected. Unsupported/malformed entries decline
/// before writing, leaving the lifted helper as the fallback.
/// Verification tier: host adapter wiring; real contact state remains Tier 2 debt.
pub unsafe extern "C" fn direct_call(
    callee: u32,
    eax: u32,
    ecx: u32,
    esp: u32,
    eax_out: *mut u32,
    api: *const HostUiGuestApi,
) -> bool {
    let Some(api) = (unsafe { api.as_ref() }) else {
        return false;
    };
    if eax_out.is_null() {
        return false;
    }
    log_contact_call(api, callee, esp);
    if callee == CONTACT_PASS {
        log_contact_pass_arguments(api, eax, esp);
        // Endpoint ownership and callback effects are not resolved for the
        // production world-step path. Keep the entire pass in lifted code.
        return false;
    }
    match callee {
        ENQUEUE => append(api, ecx, esp),
        DRAIN => drain(api),
        _ => immediate_callback(api, callee, esp),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    thread_local! {
        static MEMORY: RefCell<BTreeMap<u32, u32>> = const { RefCell::new(BTreeMap::new()) };
        static WRITES: RefCell<Vec<(u32, u32)>> = const { RefCell::new(Vec::new()) };
        static REENQUEUE_ON_CALLBACK: RefCell<bool> = const { RefCell::new(false) };
        static CALLBACKS: RefCell<Vec<(u32, Vec<u32>)>> = const { RefCell::new(Vec::new()) };
    }

    extern "C" fn read32(address: u32, value: *mut u32) -> u32 {
        MEMORY.with(|memory| {
            let Some(found) = memory.borrow().get(&address).copied() else {
                return 0;
            };
            // SAFETY: the fake API caller passes a valid output word.
            unsafe { *value = found };
            1
        })
    }

    extern "C" fn write32(address: u32, value: u32) -> u32 {
        MEMORY.with(|memory| memory.borrow_mut().insert(address, value));
        WRITES.with(|writes| writes.borrow_mut().push((address, value)));
        1
    }

    fn native_immediate_noop(_event: ImmediateContactEvent, _queue: &mut DeferredEvents) {}

    fn native_immediate_record(event: ImmediateContactEvent, _queue: &mut DeferredEvents) {
        let kind = event.contact.with(|contact| contact.kind).unwrap();
        CALLBACKS.with(|calls| {
            calls.borrow_mut().push((
                event.recipient.0,
                vec![kind as u32, event.relative_speed_bits],
            ));
        });
    }

    fn native_immediate_stack_record(event: ImmediateContactEvent, _queue: &mut DeferredEvents) {
        let fields = event.contact.with(|contact| {
            assert_eq!(contact.horizontal_normal, None);
            assert_eq!(contact.relative_speed_bits, Some(event.relative_speed_bits));
            (contact.kind, contact.guest_identity)
        });
        CALLBACKS.with(|calls| {
            calls.borrow_mut().push((
                event.recipient.0,
                vec![fields.unwrap().0 as u32, event.relative_speed_bits],
            ));
        });
    }

    extern "C" fn call(callee: u32, args: *const u32, len: u32, result: *mut u32) -> u32 {
        // SAFETY: `HostUiGuestApi::call_raw` passes a live slice for this call.
        let args = unsafe { std::slice::from_raw_parts(args, len as usize) };
        if callee == 0x777
            && REENQUEUE_ON_CALLBACK.with(|flag| std::mem::take(&mut *flag.borrow_mut()))
        {
            let Some(&row) = args.get(1) else {
                return 0;
            };
            let api = fake_api();
            if !append(&api, row, 0x5000) {
                return 0;
            }
        }
        CALLBACKS.with(|calls| calls.borrow_mut().push((callee, args.to_vec())));
        // SAFETY: the host API caller supplies a valid result pointer.
        unsafe { *result = 0 };
        1
    }

    extern "C" fn call_st0(_: u32, _: *const u32, _: u32, _: *mut f64) -> u32 {
        0
    }

    fn fake_api() -> HostUiGuestApi {
        HostUiGuestApi::test_api(read32, write32, call, call_st0)
    }

    fn call_direct_for_test(
        callee: u32,
        eax: u32,
        ecx: u32,
        esp: u32,
        api: &HostUiGuestApi,
    ) -> (bool, u32) {
        let mut eax_out = eax;
        // SAFETY: this test provides valid pointers for the duration of the call.
        let handled = unsafe { direct_call(callee, eax, ecx, esp, &mut eax_out, api) };
        (handled, eax_out)
    }

    #[test]
    fn direct_call_keeps_contact_pass_in_lifted_fallback() {
        let eax = 0xdead_beef;
        let (handled, eax_out) = call_direct_for_test(CONTACT_PASS, eax, 0, 0x5000, &fake_api());
        assert!(!handled);
        assert_eq!(eax_out, eax);
    }

    #[test]
    fn direct_call_return_register_is_preserved_when_declined() {
        let eax = 0xdead_beef;
        let (handled, eax_out) = call_direct_for_test(0x1234, eax, 0, 0x5000, &fake_api());
        assert!(!handled);
        assert_eq!(eax_out, eax);
    }

    #[test]
    fn contact_pass_inputs_read_header_from_eax_and_output_capacity_from_stack() {
        MEMORY.with(|memory| {
            let mut memory = memory.borrow_mut();
            memory.clear();
            memory.insert(0x5004, 0x6000);
            memory.insert(0x5008, (-7_i32) as u32);
        });

        assert_eq!(
            contact_pass_arguments(&fake_api(), 0x1234, 0x5000),
            Some(ContactPassArguments {
                header: 0x1234,
                output: 0x6000,
                capacity: -7,
            })
        );
        assert_eq!(contact_pass_arguments(&fake_api(), 0x1234, u32::MAX), None);
    }

    #[test]
    fn imports_measured_deferred_row_as_a_typed_contact_lease() {
        MEMORY.with(|memory| {
            let mut memory = memory.borrow_mut();
            memory.clear();
            memory.insert(0x102c, 3 << 16);
            memory.insert(0x1034, 0);
            memory.insert(0x1024, 0x1110);
            memory.insert(0x1028, 0x2220);
        });
        let api = fake_api();
        let mut frame = tsfp_engine_physics::rea_events::ContactDeliveryFrame::new(Vec::new(), 1);
        let event = event_from_words(
            &api,
            0x2220,
            [0x1110, 0xbf80_0000, 0x3f00_0000, 0x1000],
            &mut frame,
        )
        .unwrap();
        assert_eq!(event.recipient, OwnerHandle(0x2220));
        assert_eq!(event.other, Some(OwnerHandle(0x1110)));
        assert_eq!(event.horizontal_normal_bits, [0xbf80_0000, 0x3f00_0000]);
        let contact = event.contact.with(|contact| {
            (
                contact.kind,
                contact.owner_a,
                contact.owner_b,
                contact.relative_speed_bits,
                contact.guest_identity,
            )
        });
        assert_eq!(
            contact,
            Some((
                3,
                Some(OwnerHandle(0x1110)),
                Some(OwnerHandle(0x2220)),
                None,
                Some(ContactGuestId(0x1000)),
            ))
        );
    }

    #[test]
    fn append_owns_a_typed_event_and_publishes_the_guest_mirror_last() {
        MEMORY.with(|memory| {
            let mut memory = memory.borrow_mut();
            memory.clear();
            memory.extend([
                (OWNERS_GLOBAL, 0x2000),
                (ROWS_GLOBAL, 0x3000),
                (CAPACITY_GLOBAL, 1),
                (COUNT_GLOBAL, 0),
                (0x2000, 0),
                (0x3000, 0),
                (0x3004, 0),
                (0x3008, 0),
                (0x300c, 0),
                (0x4000, 0x1110),
                (0x4004, 0xbf80_0000),
                (0x4008, 0x3f00_0000),
                (0x400c, 0x1000),
                (0x5004, 0x2220),
                (0x1024, 0x1110),
                (0x1028, 0x2220),
                (0x102c, 3 << 16),
                (0x1034, 0),
            ]);
        });
        WRITES.with(|writes| writes.borrow_mut().clear());
        QUEUE.with(|queue| *queue.borrow_mut() = None);

        let api = fake_api();
        assert!(append(&api, 0x4000, 0x5000));
        assert_eq!(MEMORY.with(|memory| memory.borrow()[&COUNT_GLOBAL]), 1);
        assert_eq!(MEMORY.with(|memory| memory.borrow()[&0x2000]), 0x2220);
        assert_eq!(
            [0, 4, 8, 12].map(|offset| MEMORY.with(|memory| memory.borrow()[&(0x3000 + offset)])),
            [0x1110, 0xbf80_0000, 0x3f00_0000, 0x1000]
        );
        assert_eq!(
            QUEUE.with(|queue| queue
                .borrow()
                .as_ref()
                .map(|live| live.frame.delivery().deferred_count())),
            Some(1)
        );
        assert_eq!(
            QUEUE.with(|queue| queue.borrow().as_ref().map(|live| live.frame.arena().len())),
            Some(1),
            "the deferred event contact must be retained by its live frame"
        );
        assert_eq!(
            WRITES.with(|writes| writes.borrow().clone()),
            vec![
                (0x2000, 0x2220),
                (0x3000, 0x1110),
                (0x3004, 0xbf80_0000),
                (0x3008, 0x3f00_0000),
                (0x300c, 0x1000),
                (COUNT_GLOBAL, 1),
            ]
        );

        WRITES.with(|writes| writes.borrow_mut().clear());
        assert!(append(&api, 0x4000, 0x5000));
        assert!(WRITES.with(|writes| writes.borrow().is_empty()));
    }

    #[test]
    fn drain_delivers_callback_appended_to_the_popped_slot_in_the_same_pass() {
        MEMORY.with(|memory| {
            let mut memory = memory.borrow_mut();
            memory.clear();
            memory.extend([
                (OWNERS_GLOBAL, 0x2000),
                (ROWS_GLOBAL, 0x3000),
                (CAPACITY_GLOBAL, 1),
                (COUNT_GLOBAL, 1),
                (0x2000, 0x2220),
                (0x3000, 0x1110),
                (0x3004, 0xbf80_0000),
                (0x3008, 0x3f00_0000),
                (0x300c, 0x1000),
                (0x1024, 0x1110),
                (0x1028, 0x2220),
                (0x102c, 3 << 16),
                (0x1034, 0),
                (0x2220 + 0x170, 0x777),
                (0x3330 + 0x170, 0x888),
                (0x5004, 0x3330),
            ]);
        });
        WRITES.with(|writes| writes.borrow_mut().clear());
        CALLBACKS.with(|calls| calls.borrow_mut().clear());
        REENQUEUE_ON_CALLBACK.with(|flag| *flag.borrow_mut() = true);
        QUEUE.with(|queue| *queue.borrow_mut() = None);

        assert!(drain(&fake_api()));

        assert_eq!(
            CALLBACKS.with(|calls| calls.borrow().clone()),
            vec![
                (0x777, vec![0x2220, 0x3000]),
                (0x888, vec![0x3330, 0x3000]),
                (GUEST_FREE, vec![0x2000]),
                (GUEST_FREE, vec![0x3000]),
            ]
        );
        assert_eq!(
            WRITES.with(|writes| writes
                .borrow()
                .iter()
                .filter_map(|(address, value)| (*address == COUNT_GLOBAL).then_some(*value))
                .collect::<Vec<_>>()),
            vec![0, 1, 0, 0]
        );
        assert_eq!(
            MEMORY.with(|memory| {
                [OWNERS_GLOBAL, ROWS_GLOBAL, CAPACITY_GLOBAL, COUNT_GLOBAL]
                    .map(|address| memory.borrow()[&address])
            }),
            [0; 4]
        );
        assert!(QUEUE.with(|queue| queue.borrow().is_none()));
    }

    fn native_deferred_append(
        owner: OwnerHandle,
        slot: usize,
        queue: &mut tsfp_engine_physics::rea_events::DeferredEvents,
    ) {
        let current = queue.row(slot).expect("popped native row remains leased");
        let appended = PhysicsEvent {
            recipient: OwnerHandle(0x3330),
            other: Some(OwnerHandle(0x4440)),
            horizontal_normal_bits: [0x3f80_0000, 0xbf00_0000],
            contact: current.contact,
        };
        assert!(queue.enqueue(appended));
        CALLBACKS.with(|calls| calls.borrow_mut().push((0x778, vec![owner.0, slot as u32])));
    }

    #[test]
    fn native_deferred_owner_runs_typed_and_unregistered_appended_owner_falls_back() {
        MEMORY.with(|memory| {
            let mut memory = memory.borrow_mut();
            memory.clear();
            memory.extend([
                (OWNERS_GLOBAL, 0x2000),
                (ROWS_GLOBAL, 0x3000),
                (CAPACITY_GLOBAL, 2),
                (COUNT_GLOBAL, 1),
                (0x2000, 0x2220),
                (0x3000, 0x1110),
                (0x3004, 0xbf80_0000),
                (0x3008, 0x3f00_0000),
                (0x300c, 0x1000),
                (0x1024, 0x1110),
                (0x1028, 0x2220),
                (0x102c, 3 << 16),
                (0x1034, 0),
                // A native owner does not need a lifted callback slot.
                (0x2220 + 0x170, 0),
                (0x3330 + 0x170, 0x888),
            ]);
        });
        WRITES.with(|writes| writes.borrow_mut().clear());
        CALLBACKS.with(|calls| calls.borrow_mut().clear());
        QUEUE.with(|queue| *queue.borrow_mut() = None);
        NATIVE_CALLBACKS.with(|callbacks| {
            *callbacks.borrow_mut() = NativeContactHandlerRegistry::default();
        });
        register_native_deferred_callback(OwnerHandle(0x2220), native_deferred_append);

        assert!(drain(&fake_api()));

        let calls = CALLBACKS.with(|calls| calls.borrow().clone());
        assert_eq!(calls[0], (0x778, vec![0x2220, 0]));
        assert_eq!(calls[1], (0x888, vec![0x3330, 0x3000]));
        assert_eq!(
            calls[2..],
            [(GUEST_FREE, vec![0x2000]), (GUEST_FREE, vec![0x3000]),]
        );
        assert_eq!(
            WRITES.with(|writes| writes
                .borrow()
                .iter()
                .filter_map(|(address, value)| (*address == COUNT_GLOBAL).then_some(*value))
                .collect::<Vec<_>>()),
            vec![0, 1, 0, 0]
        );
        assert_eq!(
            MEMORY.with(|memory| [0x2000, 0x3000, 0x3004, 0x3008, 0x300c]
                .map(|address| memory.borrow()[&address])),
            [0x3330, 0x4440, 0x3f80_0000, 0xbf00_0000, 0x1000]
        );
        assert!(unregister_native_callbacks(OwnerHandle(0x2220)));
        assert!(QUEUE.with(|queue| queue.borrow().is_none()));
    }

    #[test]
    fn native_immediate_callback_registration_is_visible_to_host() {
        let owner = OwnerHandle(0x6a20);
        NATIVE_CALLBACKS.with(|callbacks| {
            *callbacks.borrow_mut() = NativeContactHandlerRegistry::default();
        });

        register_native_immediate_callback(owner, native_immediate_noop);
        assert!(
            NATIVE_CALLBACKS.with(|callbacks| callbacks.borrow().has_immediate_callback(owner))
        );
        assert!(unregister_native_callbacks(owner));
        assert!(
            !NATIVE_CALLBACKS.with(|callbacks| callbacks.borrow().has_immediate_callback(owner))
        );
    }

    #[test]
    fn native_owner_registration_reserves_identity_and_retires_both_handlers() {
        let owner = OwnerHandle(0x6a21);
        NATIVE_CALLBACKS.with(|callbacks| {
            *callbacks.borrow_mut() = NativeContactHandlerRegistry::default();
        });
        NATIVE_OWNER_REGISTRATIONS.with(|owners| owners.borrow_mut().clear());
        PENDING_NATIVE_OWNER_RETIREMENTS.with(|owners| owners.borrow_mut().clear());

        assert!(NativeContactOwnerRegistration::new(OwnerHandle(0)).is_none());
        let mut registration = NativeContactOwnerRegistration::new(owner).unwrap();
        assert_eq!(registration.owner(), owner);
        registration.on_immediate(|_, _| {});
        registration.on_deferred(|_, _, _| {});
        assert!(NativeContactOwnerRegistration::new(owner).is_none());
        assert!(
            NATIVE_CALLBACKS.with(|callbacks| { callbacks.borrow().has_immediate_callback(owner) }),
            "owner registration did not install its immediate callback"
        );
        assert!(
            NATIVE_CALLBACKS.with(|callbacks| callbacks.borrow().has_callback(owner)),
            "owner registration did not install its deferred callback"
        );

        drop(registration);
        assert!(!NATIVE_CALLBACKS.with(|callbacks| {
            let callbacks = callbacks.borrow();
            callbacks.has_immediate_callback(owner) || callbacks.has_callback(owner)
        }));
        assert!(NativeContactOwnerRegistration::new(owner).is_some());
    }

    #[test]
    fn native_owner_can_retire_from_inside_its_immediate_callback() {
        let owner = OwnerHandle(0x6a22);
        let other = OwnerHandle(0x6a23);
        NATIVE_CALLBACKS.with(|callbacks| {
            *callbacks.borrow_mut() = NativeContactHandlerRegistry::default();
        });
        NATIVE_OWNER_REGISTRATIONS.with(|owners| owners.borrow_mut().clear());
        PENDING_NATIVE_OWNER_RETIREMENTS.with(|owners| owners.borrow_mut().clear());

        let registration_slot = Rc::new(RefCell::new(None));
        let callback_slot = Rc::clone(&registration_slot);
        let mut registration = NativeContactOwnerRegistration::new(owner).unwrap();
        registration.on_immediate(move |_, _| {
            callback_slot.borrow_mut().take();
        });
        *registration_slot.borrow_mut() = Some(registration);

        let arena = ContactArena::new(vec![PhysicsContact {
            kind: 1,
            owner_a: Some(owner),
            owner_b: Some(other),
            horizontal_normal: Some([0.0, 1.0]),
            relative_speed_bits: Some(0x3f80_0000),
            next: None,
            guest_next: None,
            guest_identity: None,
        }]);
        let event = ImmediateContactEvent {
            recipient: owner,
            contact: arena.lease(ContactIndex(0)).unwrap(),
            relative_speed_bits: 0x3f80_0000,
        };

        let mut system = ContactDeliverySystem::new(0);
        system.set_enabled(true);
        assert!(dispatch_registered_immediate(&mut system, event));
        assert!(registration_slot.borrow().is_none());
        assert!(!NATIVE_CALLBACKS.with(|callbacks| {
            callbacks.borrow().has_immediate_callback(owner)
                || callbacks.borrow().has_callback(owner)
        }));
        assert!(!NATIVE_OWNER_REGISTRATIONS.with(|owners| owners.borrow().contains(&owner)));
        assert!(NativeContactOwnerRegistration::new(owner).is_some());
    }

    #[test]
    fn native_owner_can_retire_from_inside_its_deferred_callback() {
        let owner = OwnerHandle(0x6a24);
        let other = OwnerHandle(0x6a25);
        NATIVE_CALLBACKS.with(|callbacks| {
            *callbacks.borrow_mut() = NativeContactHandlerRegistry::default();
        });
        NATIVE_OWNER_REGISTRATIONS.with(|owners| owners.borrow_mut().clear());
        PENDING_NATIVE_OWNER_RETIREMENTS.with(|owners| owners.borrow_mut().clear());
        QUEUE.with(|queue| *queue.borrow_mut() = None);
        CALLBACKS.with(|calls| calls.borrow_mut().clear());
        MEMORY.with(|memory| {
            let mut memory = memory.borrow_mut();
            memory.clear();
            memory.extend([
                (OWNERS_GLOBAL, 0x2000),
                (ROWS_GLOBAL, 0x3000),
                (CAPACITY_GLOBAL, 1),
                (COUNT_GLOBAL, 1),
                (0x2000, owner.0),
                (0x3000, other.0),
                (0x3004, 0x3f80_0000),
                (0x3008, 0xbf00_0000),
                (0x300c, 0x1000),
                (0x1024, other.0),
                (0x1028, owner.0),
                (0x102c, 1 << 16),
                (0x1034, 0),
                (owner.0 + 0x170, 0),
            ]);
        });

        let registration_slot = Rc::new(RefCell::new(None));
        let callback_slot = Rc::clone(&registration_slot);
        let mut registration = NativeContactOwnerRegistration::new(owner).unwrap();
        registration.on_deferred(move |callback_owner, slot, _| {
            assert_eq!(callback_owner, owner);
            assert_eq!(slot, 0);
            CALLBACKS.with(|calls| calls.borrow_mut().push((owner.0, vec![slot as u32])));
            callback_slot.borrow_mut().take();
        });
        *registration_slot.borrow_mut() = Some(registration);

        assert!(drain(&fake_api()));
        assert_eq!(
            CALLBACKS.with(|calls| calls.borrow().clone()),
            vec![
                (owner.0, vec![0]),
                (GUEST_FREE, vec![0x2000]),
                (GUEST_FREE, vec![0x3000]),
            ]
        );
        assert!(registration_slot.borrow().is_none());
        assert!(!NATIVE_CALLBACKS.with(|callbacks| {
            let callbacks = callbacks.borrow();
            callbacks.has_immediate_callback(owner) || callbacks.has_callback(owner)
        }));
        assert!(!NATIVE_OWNER_REGISTRATIONS.with(|owners| owners.borrow().contains(&owner)));
        assert!(QUEUE.with(|queue| queue.borrow().is_none()));
        assert!(NativeContactOwnerRegistration::new(owner).is_some());
    }

    #[test]
    fn typed_native_immediate_dispatch_invokes_registered_owner_without_guest_call() {
        let owner = OwnerHandle(0x6a20);
        let arena = ContactArena::new(vec![PhysicsContact {
            kind: 1,
            owner_a: Some(OwnerHandle(0x1110)),
            owner_b: Some(owner),
            horizontal_normal: Some([0.0, 1.0]),
            relative_speed_bits: Some(0x3f80_0000),
            next: None,
            guest_next: None,
            guest_identity: None,
        }]);
        let event = ImmediateContactEvent {
            recipient: owner,
            contact: arena.lease(ContactIndex(0)).unwrap(),
            relative_speed_bits: 0x3f80_0000,
        };
        NATIVE_CALLBACKS.with(|callbacks| {
            *callbacks.borrow_mut() = NativeContactHandlerRegistry::default();
        });
        CALLBACKS.with(|calls| calls.borrow_mut().clear());

        let mut deferred = DeferredEvents::new(1);
        assert!(!dispatch_native_immediate_callback(
            event.clone(),
            &mut deferred
        ));
        register_native_immediate_callback(owner, native_immediate_record);
        assert!(dispatch_native_immediate_callback(event, &mut deferred));
        assert_eq!(
            CALLBACKS.with(|calls| calls.borrow().clone()),
            vec![(owner.0, vec![1, 0x3f80_0000])]
        );
    }

    /// Tier 0 with FABRICATED contact data; verifies closure-owned Rust state.
    #[test]
    fn native_callback_closures_keep_rust_owned_state_across_immediate_and_deferred_delivery() {
        use std::rc::Rc;

        let immediate_owner = OwnerHandle(0x6a20);
        let deferred_owner = OwnerHandle(0x6a30);
        let arena = ContactArena::new(vec![PhysicsContact {
            kind: 1,
            owner_a: Some(OwnerHandle(0x1110)),
            owner_b: Some(immediate_owner),
            horizontal_normal: Some([1.0, 0.0]),
            relative_speed_bits: Some(0x3f80_0000),
            next: None,
            guest_next: None,
            guest_identity: None,
        }]);
        let event = ImmediateContactEvent {
            recipient: immediate_owner,
            contact: arena.lease(ContactIndex(0)).unwrap(),
            relative_speed_bits: 0x3f80_0000,
        };
        let received = Rc::new(RefCell::new(Vec::new()));
        let immediate_state = Rc::clone(&received);
        let deferred_state = Rc::clone(&received);
        NATIVE_CALLBACKS.with(|callbacks| {
            *callbacks.borrow_mut() = NativeContactHandlerRegistry::default();
        });
        register_native_immediate_callback(immediate_owner, move |event, queue| {
            let kind = event.contact.with(|contact| contact.kind).unwrap();
            immediate_state
                .borrow_mut()
                .push((0, event.recipient.0, kind as u32));
            assert!(queue.enqueue(PhysicsEvent {
                recipient: deferred_owner,
                other: Some(event.recipient),
                horizontal_normal_bits: [0x3f80_0000, 0],
                contact: event.contact,
            }));
        });
        register_native_deferred_callback(deferred_owner, move |owner, slot, queue| {
            let row = queue
                .row(slot)
                .expect("popped row remains available to callback");
            let kind = row.contact.with(|contact| contact.kind).unwrap();
            deferred_state.borrow_mut().push((1, owner.0, kind as u32));
        });

        let mut deferred = DeferredEvents::new(1);
        assert!(dispatch_native_immediate_callback(event, &mut deferred));
        let (slot, _) = deferred
            .pop_lifo()
            .expect("immediate handler enqueues a row");
        NATIVE_CALLBACKS.with(|callbacks| {
            callbacks
                .borrow_mut()
                .deliver(deferred_owner, slot, &mut deferred);
        });

        assert_eq!(
            *received.borrow(),
            [(0, immediate_owner.0, 1), (1, deferred_owner.0, 1)]
        );
        NATIVE_CALLBACKS.with(|callbacks| {
            assert!(callbacks.borrow_mut().unregister(immediate_owner));
            assert!(callbacks.borrow_mut().unregister(deferred_owner));
        });
    }

    #[test]
    fn lifted_immediate_callback_uses_typed_stack_event_for_registered_owner() {
        let owner = OwnerHandle(0x2220);
        NATIVE_CALLBACKS.with(|callbacks| {
            *callbacks.borrow_mut() = NativeContactHandlerRegistry::default();
        });
        QUEUE.with(|queue| *queue.borrow_mut() = None);
        CALLBACKS.with(|calls| calls.borrow_mut().clear());
        MEMORY.with(|memory| {
            let mut memory = memory.borrow_mut();
            memory.clear();
            memory.extend([
                (0x5000, CONTACT_CALLBACK_RETURNS[0]),
                (0x5004, owner.0),
                (0x5008, 0x1000),
                (0x500c, 0xbf80_0000),
                (0x1024, 0x1110),
                (0x1028, owner.0),
                (0x102c, 1 << 16),
                (0x1034, 0),
                (OWNERS_GLOBAL, 0x6000),
                (ROWS_GLOBAL, 0x7000),
                (CAPACITY_GLOBAL, 1),
                (COUNT_GLOBAL, 0),
            ]);
        });
        register_native_immediate_callback(owner, native_immediate_stack_record);

        let api = fake_api();
        // The same owner-shaped stack at any other callsite remains lifted.
        MEMORY.with(|memory| {
            memory.borrow_mut().insert(0x5000, 0x1234);
        });
        assert!(!call_direct_for_test(0x1234, 0, 0, 0x5000, &api).0);
        MEMORY.with(|memory| {
            memory
                .borrow_mut()
                .insert(0x5000, CONTACT_CALLBACK_RETURNS[0]);
        });
        let (handled, eax_out) = call_direct_for_test(0x1234, 0x1357_9bdf, 0, 0x5000, &api);
        assert!(handled);
        assert_eq!(eax_out, 0x1357_9bdf);
        assert_eq!(
            CALLBACKS.with(|calls| calls.borrow().clone()),
            vec![(owner.0, vec![1, 0xbf80_0000])]
        );
        assert_eq!(
            MEMORY.with(|memory| memory.borrow().get(&COUNT_GLOBAL).copied()),
            Some(0)
        );
        QUEUE.with(|queue| *queue.borrow_mut() = None);
    }

    #[test]
    fn lifted_immediate_callback_declines_a_non_endpoint_recipient() {
        let owner = OwnerHandle(0x2220);
        NATIVE_CALLBACKS.with(|callbacks| {
            *callbacks.borrow_mut() = NativeContactHandlerRegistry::default();
        });
        QUEUE.with(|queue| *queue.borrow_mut() = None);
        CALLBACKS.with(|calls| calls.borrow_mut().clear());
        WRITES.with(|writes| writes.borrow_mut().clear());
        MEMORY.with(|memory| {
            let mut memory = memory.borrow_mut();
            memory.clear();
            memory.extend([
                (0x5000, CONTACT_CALLBACK_RETURNS[0]),
                (0x5004, owner.0),
                (0x5008, 0x1000),
                (0x500c, 0xbf80_0000),
                (0x1024, 0x1110),
                (0x1028, 0x3330),
                (0x102c, 1 << 16),
                (0x1034, 0),
                (OWNERS_GLOBAL, 0x6000),
                (ROWS_GLOBAL, 0x7000),
                (CAPACITY_GLOBAL, 1),
                (COUNT_GLOBAL, 0),
            ]);
        });
        register_native_immediate_callback(owner, native_immediate_record);

        assert!(!call_direct_for_test(0x1234, 0, 0, 0x5000, &fake_api()).0);
        assert!(CALLBACKS.with(|calls| calls.borrow().is_empty()));
        assert!(WRITES.with(|writes| writes.borrow().is_empty()));
        assert!(unregister_native_callbacks(owner));
        QUEUE.with(|queue| *queue.borrow_mut() = None);
    }

    #[test]
    fn contact_link_mirror_rejects_an_unwritable_guest_link() {
        let arena = ContactArena::new(vec![PhysicsContact {
            kind: 1,
            owner_a: None,
            owner_b: None,
            horizontal_normal: None,
            relative_speed_bits: None,
            next: None,
            guest_next: Some(ContactGuestId(0x1050)),
            guest_identity: Some(ContactGuestId(0x1000)),
        }]);
        let contact = arena.lease(ContactIndex(0)).unwrap();

        assert!(!sync_guest_contact_next(
            &fake_api(),
            &contact,
            ContactGuestId(0x1000),
            None
        ));
        // An unchanged link requires no guest write and remains valid.
        assert!(sync_guest_contact_next(
            &fake_api(),
            &contact,
            ContactGuestId(0x1000),
            Some(ContactGuestId(0x1050))
        ));
    }

    /// Tier 0 with FABRICATED guest memory; exercises captured native owner state
    /// through the same direct-call hook used by the lifted host.
    #[test]
    fn lifted_immediate_callback_closure_mirrors_and_native_drain_delivers() {
        use std::rc::Rc;

        let immediate_owner = OwnerHandle(0x2220);
        let deferred_owner = OwnerHandle(0x3330);
        MEMORY.with(|memory| {
            let mut memory = memory.borrow_mut();
            memory.clear();
            memory.extend([
                (0x5000, CONTACT_CALLBACK_RETURNS[0]),
                (0x5004, immediate_owner.0),
                (0x5008, 0x1000),
                (0x500c, 0x3f80_0000),
                (0x1024, 0x1110),
                (0x1028, immediate_owner.0),
                (0x102c, 1 << 16),
                (0x1034, 0x1050),
                (OWNERS_GLOBAL, 0x6000),
                (ROWS_GLOBAL, 0x7000),
                (CAPACITY_GLOBAL, 1),
                (COUNT_GLOBAL, 0),
                (0x6000, 0),
                (0x7000, 0),
                (0x7004, 0),
                (0x7008, 0),
                (0x700c, 0),
            ]);
        });
        QUEUE.with(|queue| *queue.borrow_mut() = None);
        NATIVE_CALLBACKS.with(|callbacks| {
            *callbacks.borrow_mut() = NativeContactHandlerRegistry::default();
        });
        NATIVE_OWNER_REGISTRATIONS.with(|owners| owners.borrow_mut().clear());
        PENDING_NATIVE_OWNER_RETIREMENTS.with(|owners| owners.borrow_mut().clear());
        let received = Rc::new(RefCell::new(Vec::new()));
        let immediate_received = Rc::clone(&received);
        let deferred_received = Rc::clone(&received);
        let mut immediate_registration = NativeContactOwnerRegistration::new(immediate_owner)
            .expect("immediate owner should be registered");
        immediate_registration.on_immediate(move |event, queue| {
            let kind = event.contact.with(|contact| contact.kind).unwrap();
            immediate_received
                .borrow_mut()
                .push((0, event.recipient.0, kind as u32));
            assert_eq!(
                event.contact.with_mut(|contact| {
                    contact.guest_next = Some(ContactGuestId(0x1060));
                }),
                Some(())
            );
            assert!(queue.enqueue(PhysicsEvent {
                recipient: deferred_owner,
                other: Some(event.recipient),
                horizontal_normal_bits: [0x3f80_0000, 0],
                contact: event.contact,
            }));
        });
        let mut deferred_registration = NativeContactOwnerRegistration::new(deferred_owner)
            .expect("deferred owner should be registered");
        deferred_registration.on_deferred(move |owner, slot, queue| {
            let row = queue.row(slot).unwrap();
            let kind = row.contact.with(|contact| contact.kind).unwrap();
            deferred_received
                .borrow_mut()
                .push((1, owner.0, kind as u32));
        });

        let api = fake_api();
        assert!(call_direct_for_test(0x1234, 0, 0, 0x5000, &api).0);
        assert_eq!(MEMORY.with(|memory| memory.borrow()[&COUNT_GLOBAL]), 1);
        assert_eq!(
            MEMORY.with(|memory| memory.borrow()[&0x6000]),
            deferred_owner.0
        );
        assert_eq!(
            MEMORY.with(|memory| memory.borrow()[&0x7000]),
            immediate_owner.0
        );
        assert_eq!(MEMORY.with(|memory| memory.borrow()[&0x1034]), 0x1060);
        assert!(call_direct_for_test(DRAIN, 0, 0, 0x5000, &api).0);
        assert_eq!(
            *received.borrow(),
            [(0, immediate_owner.0, 1), (1, deferred_owner.0, 1)]
        );
        assert_eq!(MEMORY.with(|memory| memory.borrow()[&COUNT_GLOBAL]), 0);
        drop(immediate_registration);
        drop(deferred_registration);
        assert!(NativeContactOwnerRegistration::new(immediate_owner).is_some());
        assert!(NativeContactOwnerRegistration::new(deferred_owner).is_some());
        QUEUE.with(|queue| *queue.borrow_mut() = None);
    }

    #[test]
    fn callback_target_diagnostic_covers_contact_pass_window_and_deduplicates() {
        assert!(is_contact_callback_log_site(CONTACT_CALLBACK_RETURNS[0]));
        assert!(is_contact_callback_log_site(CONTACT_CALLBACK_RETURNS[1]));
        // The logger intentionally also sees other callees in the pass so a
        // return-site mismatch cannot hide an unexpected callback target.
        assert!(is_contact_callback_log_site(0x0017_06a9));
        assert!(is_contact_callback_log_site(0x0017_0aa1));
        assert!(is_contact_callback_log_site(0x0017_0dd9));
        assert!(!is_contact_callback_log_site(0x0016_ffff));
        assert!(!is_contact_callback_log_site(0x0017_1000));
        let mut seen = BTreeSet::new();
        assert!(remember_callback_target(
            &mut seen,
            CONTACT_CALLBACK_RETURNS[0],
            0x0012_3456
        ));
        assert!(!remember_callback_target(
            &mut seen,
            CONTACT_CALLBACK_RETURNS[0],
            0x0012_3456
        ));
        assert!(remember_callback_target(
            &mut seen,
            CONTACT_CALLBACK_RETURNS[1],
            0x0012_3456
        ));
        assert!(remember_callback_target(
            &mut seen,
            CONTACT_CALLBACK_RETURNS[0],
            0x0065_4321
        ));
        assert_eq!(seen.len(), 3);
    }

    #[test]
    fn contact_call_diagnostic_deduplicates_by_root_and_caller() {
        let mut seen = BTreeSet::new();
        assert!(remember_contact_call(&mut seen, CONTACT_PASS, 0x0012_3456));
        assert!(!remember_contact_call(&mut seen, CONTACT_PASS, 0x0012_3456));
        assert!(remember_contact_call(&mut seen, ENQUEUE, 0x0012_3456));
        assert!(remember_contact_call(&mut seen, CONTACT_PASS, 0x0065_4321));
        assert_eq!(seen.len(), 3);
    }
}

//! T3770: the delete flow of the profile selection page (builder `0x194980`), ported from its update
//! callback `0x192770` so the native page input drives it.
//!
//! The original update reads the player's pad directly (`0x427B0`): Y (`0x40`) in the list state 2
//! asks to delete the profile under the cursor (state 15, the "are you sure" panel), A (`0x80`) in
//! state 15 deletes when the cursor is on the Yes item (id -11, state 16, whose next update calls
//! `0x191110` and `XDeleteSaveGame`) and cancels on any other item. A Rust page owning input holds
//! every pad press back from the guest, so neither press arrived (`docs/t3770-profile-delete.md`).
//! These two functions are the Y and A branches (`0x1928E3..0x192992`, `0x192D65..0x192DFC`),
//! MEASURED from the retail code; the storage delete itself stays the original state-16 path.
//!
//! T3772 ports the remaining pad reads of `0x192770`: the START join of state 1 ([`join`], `0x1927A3..0x192849`:
//! any present port that is not playing yet takes the waiting player) and the A acknowledge of state 11 (in
//! [`accept`], `0x192CBA..0x192D65`, with the register-argument `0x190FD0` inline). Which page instance a port drives is
//! [`crate::profile_ports`] (a two player game shows one instance per player, `docs/t3772-profile-page-ports.md`).
use crate::native_page::{GuestUi, Outcome, PageRef};

/// Builder of the profile selection page.
pub const BUILDER: u32 = 0x19_4980;

/// Per-player state words of the save menu (`0x74F9EC + 4 * player`).
const PLAYER_STATE: u32 = 0x74_F9EC;
/// Owner (memory unit) of the profile each player slot names (`0x74E924 + 4 * player`).
const SLOT_OWNER: u32 = 0x74_E924;
/// 30-byte profile names, one per player slot (`0x74ED60 + 30 * player`).
const SLOT_NAME: u32 = 0x74_ED60;
const SLOT_NAME_BYTES: u32 = 0x1E;
/// Enumerated profiles: name at `+0`, owner at `+0x44`, stride `0x150`.
const PROFILES: u32 = 0x79_EF04;
const PROFILE_STRIDE: u32 = 0x150;
const PROFILE_OWNER: u32 = 0x44;
/// Set when the profile list must be rebuilt.
const LIST_DIRTY: u32 = 0x74_E938;
/// Fade threshold the update compares the page fade (`+0x1B8`) with before it reads the pad.
const FADE_READY: u32 = 0x47_5CD4;
/// Sound enable and table of the denied sound (`0x21780(table[3])`).
const SOUND_READY: u32 = 0x7B_A16C;
const SOUND_TABLE: u32 = 0x7B_A178;

/// Page record fields read by the update.
const PAGE_PLAYER: u32 = 0x38;
const PAGE_CURSOR_ID: u32 = 0x18C;
const PAGE_FADE: u32 = 0x1B8;
/// `+0x28` bit 0 of the cursor object: the row is disabled.
const NODE_FLAGS: u32 = 0x28;

/// State 1: the player waits for a controller ("press START"), state 2: the profile list.
const STATE_JOIN: u32 = 1;
const STATE_LIST: u32 = 2;
/// State 11: a message the player acknowledges with A, then the save menu goes on (`0x190FD0`).
const STATE_ACKNOWLEDGE: u32 = 0xB;
/// State 6 (memory unit profile, owner below 8) and state 14 (other players still choosing).
const STATE_UNIT_LOAD: u32 = 6;
const STATE_WAIT_OTHERS: u32 = 0xE;
/// Controller port of each player (`0x74E914 + 4 * player`, -1 while the player has none) and the player count.
const PLAYER_PORT: u32 = 0x74_E914;
const PLAYER_COUNT: u32 = 0x52_36C4;
/// `0x42710(port)`: the port holds a pad. `0x42C30(port)`: its raw buttons (START `0x200`). `0x42D00(player, port)`:
/// bind the player's pad reads to the port.
const PAD_PRESENT: u32 = 0x4_2710;
const PAD_RAW_BUTTONS: u32 = 0x4_2C30;
const PAD_MAP_SET: u32 = 0x4_2D00;
/// State 11 globals: the unit load request (`0x7A1564`, `0x74E93C`), the players still choosing (bit per player,
/// `0x74F9E4`) and the done descriptor `0x524598` pushed on the context `0x74F9E0` with the summary of `0x74E934`.
const UNIT_LOAD_REQUEST: u32 = 0x7A_1564;
const UNIT_LOAD_PENDING: u32 = 0x74_E93C;
const PLAYERS_CHOOSING: u32 = 0x74_F9E4;
const MENU_CONTEXT: u32 = 0x74_F9E0;
const SUMMARY_SLOTS: u32 = 0x74_E934;
const DONE_DESCRIPTOR: u32 = 0x52_4598;
/// Unit owners below this are memory units (state 6 loads from them).
const UNIT_OWNER_LIMIT: i32 = 8;
const APPLY_PROFILE: u32 = 0x6_9D90;
const PROFILE_EVENT: u32 = 0x6_97D0;
const AGGREGATE_STATS: u32 = 0x6_9A60;
const PUSH_CHILD: u32 = 0x7_9B70;
const STATE_MESSAGE: u32 = 8;
const STATE_ERROR: u32 = 0xA;
/// State a storage message returns to (`0x7A1560`), and the per-player panel timer (`0x74E94C`).
const MESSAGE_RETURN_STATE: u32 = 0x7A_1560;
const PANEL_TIMER: u32 = 0x74_E94C;
const STATE_CONFIRM_DELETE: u32 = 0xF;
const STATE_DELETE: u32 = 0x10;
/// Item id of Yes in the confirm panel; -6 is the create-profile row.
const ITEM_YES: i32 = -11;
const ITEM_CREATE: i32 = -6;

/// `0x71A60`: the page's current cursor object.
const CURSOR_OBJECT: u32 = 0x7_1A60;
/// `0x42870`: neutral pad state for the player (the original clears the press it consumed).
const PAD_NEUTRAL: u32 = 0x4_2870;
const PLAY_SOUND: u32 = 0x2_1780;

fn read_u8(g: &dyn GuestUi, va: u32) -> Option<u8> {
    let word = g.read32(va & !3)?;
    Some((word >> (8 * (va & 3))) as u8)
}

fn write_u8(g: &mut dyn GuestUi, va: u32, value: u8) -> bool {
    let Some(word) = g.read32(va & !3) else {
        return false;
    };
    let shift = 8 * (va & 3);
    g.write32(
        va & !3,
        (word & !(0xFF << shift)) | (u32::from(value) << shift),
    )
}

fn fade_ready(g: &dyn GuestUi, page: PageRef) -> bool {
    let fade = f32::from_bits(g.read32(page.0 + PAGE_FADE).unwrap_or(0));
    let ready = f32::from_bits(g.read32(FADE_READY).unwrap_or(u32::MAX));
    fade >= ready
}

fn player_state(g: &dyn GuestUi, player: u32) -> Option<u32> {
    g.read32(PLAYER_STATE + 4 * player)
}

/// `0x190EB0`: the profile name is not already taken by another player slot with the same owner.
fn name_free(g: &dyn GuestUi, player: u32, owner: u32, name: u32) -> bool {
    (0..4).filter(|slot| *slot != player).all(|slot| {
        g.read32(SLOT_OWNER + 4 * slot) != Some(owner)
            || !names_equal(g, SLOT_NAME + SLOT_NAME_BYTES * slot, name)
    })
}

/// The C string compare of `0x190EB0` (unbounded in the original; slot names hold at most 30 bytes).
fn names_equal(g: &dyn GuestUi, a: u32, b: u32) -> bool {
    for i in 0..SLOT_NAME_BYTES {
        let (x, y) = (read_u8(g, a + i), read_u8(g, b + i));
        if x != y || x.is_none() {
            return false;
        }
        if x == Some(0) {
            return true;
        }
    }
    false
}

/// `0x190E80`: copy the profile name into the player's slot name.
fn set_slot_name(g: &mut dyn GuestUi, player: u32, name: u32) {
    let slot = SLOT_NAME + SLOT_NAME_BYTES * player;
    for i in 0..SLOT_NAME_BYTES {
        let byte = read_u8(g, name + i).unwrap_or(0);
        write_u8(g, slot + i, byte);
        if byte == 0 {
            break;
        }
    }
}

/// True for the page and player state the delete flow belongs to.
pub fn owns(g: &dyn GuestUi, page: PageRef, state: u32) -> bool {
    page.builder(g) == BUILDER
        && g.read32(page.0 + PAGE_PLAYER)
            .and_then(|player| player_state(g, player))
            == Some(state)
}

/// Y in the list state: request the delete of the profile under the cursor (`0x1928E3`). `None` when
/// the page is not the profile list, so the caller keeps the generic behaviour.
pub fn delete(g: &mut dyn GuestUi, page: PageRef) -> Option<Outcome> {
    if !owns(g, page, STATE_LIST) {
        return None;
    }
    if !fade_ready(g, page) {
        return Some(Outcome::NotReady);
    }
    let player = g.read32(page.0 + PAGE_PLAYER)?;
    let id = g.read32(page.0 + PAGE_CURSOR_ID).unwrap_or(0) as i32;
    let cursor = g.call(CURSOR_OBJECT, &[page.0]).unwrap_or(0);
    let disabled = cursor != 0 && g.read32(cursor + NODE_FLAGS).unwrap_or(0) & 1 != 0;
    let entry = PROFILES.wrapping_add((id as u32).wrapping_mul(PROFILE_STRIDE));
    let refused = disabled
        || id == ITEM_CREATE
        || id < 0
        || !name_free(
            g,
            player,
            g.read32(entry + PROFILE_OWNER).unwrap_or(0),
            entry,
        );
    if refused {
        if g.read32(SOUND_READY).unwrap_or(0) != 0
            && let Some(sound) = g.read32(SOUND_TABLE).and_then(|t| g.read32(t + 0xC))
        {
            g.call(PLAY_SOUND, &[sound]);
        }
        return Some(Outcome::Denied);
    }
    let owner = g.read32(entry + PROFILE_OWNER).unwrap_or(0);
    g.write32(SLOT_OWNER + 4 * player, owner);
    set_slot_name(g, player, entry);
    g.write32(PLAYER_STATE + 4 * player, STATE_CONFIRM_DELETE);
    g.call(PAD_NEUTRAL, &[player]);
    Some(Outcome::Activated {
        row_handler: false,
        page_handler: true,
        kind: false,
    })
}

/// A on the profile page in a state whose update reads the A button itself: the delete confirm
/// panel (state 15), the storage message (state 8, e.g. "unable to delete") and the error panel
/// that returns to the list (state 10), and the acknowledge of state 11 (T3772). `None` for every
/// other page and state, which keep the generic activation.
pub fn accept(g: &mut dyn GuestUi, page: PageRef) -> Option<Outcome> {
    if page.builder(g) != BUILDER {
        return None;
    }
    let player = g.read32(page.0 + PAGE_PLAYER)?;
    let state = player_state(g, player)?;
    if !matches!(
        state,
        STATE_CONFIRM_DELETE | STATE_MESSAGE | STATE_ERROR | STATE_ACKNOWLEDGE
    ) {
        return None;
    }
    if !fade_ready(g, page) {
        return Some(Outcome::NotReady);
    }
    match state {
        // 0x192D65: Yes moves to the delete state, any other item cancels.
        STATE_CONFIRM_DELETE
            if g.read32(page.0 + PAGE_CURSOR_ID).unwrap_or(0) as i32 == ITEM_YES =>
        {
            g.write32(PLAYER_STATE + 4 * player, STATE_DELETE);
        }
        STATE_CONFIRM_DELETE => {
            write_u8(g, SLOT_NAME + SLOT_NAME_BYTES * player, 0);
            g.write32(SLOT_OWNER + 4 * player, u32::MAX);
            g.write32(PLAYER_STATE + 4 * player, STATE_LIST);
            g.write32(LIST_DIRTY, 1);
        }
        // 0x192C5F: back to the state the message was raised from.
        STATE_MESSAGE => {
            let back = g.read32(MESSAGE_RETURN_STATE).unwrap_or(STATE_LIST);
            g.write32(PLAYER_STATE + 4 * player, back);
            g.write32(PANEL_TIMER + 4 * player, 0);
        }
        // 0x192CBA: acknowledged; memory unit profiles load (state 6), the others go on through 0x190FD0.
        STATE_ACKNOWLEDGE => {
            acknowledge(g, player);
            g.write32(PANEL_TIMER + 4 * player, 0);
        }
        // 0x192BE0: forget the slot and rebuild the list.
        _ => {
            write_u8(g, SLOT_NAME + SLOT_NAME_BYTES * player, 0);
            g.write32(PLAYER_STATE + 4 * player, STATE_LIST);
            g.write32(SLOT_OWNER + 4 * player, u32::MAX);
            g.write32(LIST_DIRTY, 1);
            g.write32(PANEL_TIMER + 4 * player, 0);
        }
    }
    g.call(PAD_NEUTRAL, &[player]);
    Some(Outcome::Activated {
        row_handler: false,
        page_handler: true,
        kind: false,
    })
}

/// The A branch of state 11 (`0x192CBA`) up to the common tail (`0x192D3F`: panel timer and pad neutral, in [`accept`]).
fn acknowledge(g: &mut dyn GuestUi, player: u32) {
    if (g.read32(SLOT_OWNER + 4 * player).unwrap_or(0) as i32) < UNIT_OWNER_LIMIT {
        g.write32(UNIT_LOAD_REQUEST, 0);
        g.write32(UNIT_LOAD_PENDING, 1);
        g.write32(PLAYER_STATE + 4 * player, STATE_UNIT_LOAD);
        return;
    }
    g.call(
        APPLY_PROFILE,
        &[player, SLOT_NAME + SLOT_NAME_BYTES * player, 0],
    );
    g.call(PROFILE_EVENT, &[player, 0xB, 0]);
    player_done(g, player);
}

/// `0x190FD0` (player in ECX): the player stops choosing; while others still choose it waits (state 14), the last one
/// aggregates the profiles and opens the next page. The third stack word of `0x79B70` is the argument `0x69A60` left
/// on the stack (the original releases both calls' arguments together).
fn player_done(g: &mut dyn GuestUi, player: u32) {
    let choosing = g.read32(PLAYERS_CHOOSING).unwrap_or(0) & !(1u32 << (player & 31));
    g.write32(PLAYERS_CHOOSING, choosing);
    if choosing != 0 {
        g.write32(LIST_DIRTY, 1);
        g.write32(PLAYER_STATE + 4 * player, STATE_WAIT_OTHERS);
        return;
    }
    let slots = g.read32(SUMMARY_SLOTS).unwrap_or(0);
    g.call(AGGREGATE_STATS, &[slots]);
    let context = g.read32(MENU_CONTEXT).unwrap_or(0);
    g.call(PUSH_CHILD, &[context, DONE_DESCRIPTOR, slots]);
    g.write32(PLAYERS_CHOOSING, 0);
}

/// The player of a profile page instance and its save menu state.
pub fn page_player(g: &dyn GuestUi, page: PageRef) -> Option<(u32, u32)> {
    let player = g.read32(page.0 + PAGE_PLAYER)?;
    Some((player, player_state(g, player)?))
}

/// The controller port player `player` plays on (`None` while it has none).
pub fn player_port(g: &dyn GuestUi, player: u32) -> Option<u8> {
    let port = g.read32(PLAYER_PORT + 4 * player)? as i32;
    u8::try_from(port).ok()
}

/// True while the player of `page` waits for a controller (state 1).
pub fn waits_for_join(g: &dyn GuestUi, page: PageRef) -> bool {
    page.builder(g) == BUILDER && page_player(g, page).is_some_and(|(_, state)| state == STATE_JOIN)
}

/// START on port `port` while the page's player waits for a controller (state 1, `0x1927A3`): the port joins as that
/// player unless it is absent (`0x42710`) or already plays (`0x74E914` of a player below the count). The original
/// polls the raw START bit of every port (`0x42C30`); the press edge comes from the native page input instead.
/// `None` when the page is not a waiting profile page.
pub fn join(g: &mut dyn GuestUi, page: PageRef, port: u8) -> Option<Outcome> {
    if !waits_for_join(g, page) {
        return None;
    }
    if !fade_ready(g, page) {
        return Some(Outcome::NotReady);
    }
    let port = u32::from(port);
    let count = g.read32(PLAYER_COUNT).unwrap_or(0) as i32;
    let taken = (0..count.max(0) as u32).any(|p| g.read32(PLAYER_PORT + 4 * p) == Some(port));
    if taken || g.call(PAD_PRESENT, &[port]).unwrap_or(0) == 0 {
        return Some(Outcome::Denied);
    }
    let player = g.read32(page.0 + PAGE_PLAYER)?;
    g.write32(PLAYER_PORT + 4 * player, port);
    g.call(PAD_MAP_SET, &[player, port]);
    g.call(PAD_NEUTRAL, &[player]);
    g.write32(PLAYER_STATE + 4 * player, STATE_LIST);
    g.write32(LIST_DIRTY, 1);
    Some(Outcome::Activated {
        row_handler: false,
        page_handler: true,
        kind: false,
    })
}

/// Every pad read of the update `0x192770` and the Rust branch answering it (guarded by
/// `tests/test_t3770_native_page_raw_pad.py` against `tools/data/native_page_raw_pad.json`).
pub const RAW_PAD_BRANCHES: [(u32, u32, &str); 7] = [
    (STATE_JOIN, PAD_PRESENT, "join"),
    (STATE_JOIN, PAD_RAW_BUTTONS, "join"),
    (STATE_LIST, PAD_CURRENT_BUTTONS, "delete"),
    (STATE_MESSAGE, PAD_CURRENT_BUTTONS, "accept"),
    (STATE_ERROR, PAD_CURRENT_BUTTONS, "accept"),
    (STATE_ACKNOWLEDGE, PAD_CURRENT_BUTTONS, "accept"),
    (STATE_CONFIRM_DELETE, PAD_CURRENT_BUTTONS, "accept"),
];
/// `0x427B0(player)`: the player's current buttons through its port map.
const PAD_CURRENT_BUTTONS: u32 = 0x4_27B0;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu_state::layout::page;
    use crate::native_page::{CommandQueue, NativeCommand, service_on};
    use std::collections::HashMap;

    #[derive(Default)]
    struct Fake {
        mem: HashMap<u32, u32>,
        calls: Vec<(u32, Vec<u32>)>,
    }
    impl GuestUi for Fake {
        fn read32(&self, va: u32) -> Option<u32> {
            Some(*self.mem.get(&va).unwrap_or(&0))
        }
        fn write32(&mut self, va: u32, value: u32) -> bool {
            self.mem.insert(va, value);
            true
        }
        fn call(&mut self, va: u32, args: &[u32]) -> Option<u32> {
            self.calls.push((va, args.to_vec()));
            Some(if va == CURSOR_OBJECT { 0 } else { 1 })
        }
    }

    const PLAYER: u32 = 1;
    const OWNER: u32 = 0x10;

    fn put_name(f: &mut Fake, at: u32, name: &[u8]) {
        let mut bytes = name.to_vec();
        bytes.push(0);
        for (i, byte) in bytes.iter().enumerate() {
            write_u8(f, at + i as u32, *byte);
        }
    }

    /// Player 1's profile page in the list state with the cursor on profile 1 ("ANNA", owner 0x10).
    fn page_on_list(cursor_id: i32) -> (Fake, PageRef) {
        let mut f = Fake::default();
        let p = PageRef::slot(3);
        f.mem.insert(p.0, 0x4E_8D54);
        f.mem.insert(p.0 + page::BUILDER, BUILDER);
        f.mem.insert(p.0 + page::STATE, 2);
        f.mem.insert(p.0 + PAGE_PLAYER, PLAYER);
        f.mem.insert(p.0 + PAGE_CURSOR_ID, cursor_id as u32);
        f.mem.insert(p.0 + PAGE_FADE, 1.0f32.to_bits());
        f.mem.insert(FADE_READY, 0.5f32.to_bits());
        f.mem.insert(PLAYER_STATE + 4 * PLAYER, STATE_LIST);
        f.mem.insert(SLOT_OWNER, u32::MAX);
        let entry = PROFILES + PROFILE_STRIDE;
        f.mem.insert(entry + PROFILE_OWNER, OWNER);
        put_name(&mut f, entry, b"ANNA");
        (f, p)
    }

    fn slot_name(f: &Fake, player: u32) -> Vec<u8> {
        (0..SLOT_NAME_BYTES)
            .map(|i| read_u8(f, SLOT_NAME + SLOT_NAME_BYTES * player + i).unwrap())
            .take_while(|b| *b != 0)
            .collect()
    }

    #[test]
    fn delete_on_a_profile_opens_the_confirm_panel_for_that_profile() {
        let (mut f, p) = page_on_list(1);
        assert!(matches!(delete(&mut f, p), Some(Outcome::Activated { .. })));
        assert_eq!(f.mem[&(PLAYER_STATE + 4 * PLAYER)], STATE_CONFIRM_DELETE);
        assert_eq!(f.mem[&(SLOT_OWNER + 4 * PLAYER)], OWNER);
        assert_eq!(slot_name(&f, PLAYER), b"ANNA");
        assert!(f.calls.contains(&(PAD_NEUTRAL, vec![PLAYER])));
    }

    #[test]
    fn delete_is_refused_on_the_create_row_and_on_a_name_in_use() {
        let (mut f, p) = page_on_list(ITEM_CREATE);
        f.mem.insert(SOUND_READY, 1);
        f.mem.insert(SOUND_TABLE, 0x9000);
        f.mem.insert(0x900C, 0x77);
        assert_eq!(delete(&mut f, p), Some(Outcome::Denied));
        assert_eq!(f.mem[&(PLAYER_STATE + 4 * PLAYER)], STATE_LIST);
        assert!(f.calls.contains(&(PLAY_SOUND, vec![0x77])));

        // Player 0 already plays as ANNA of the same memory unit: not deletable.
        let (mut f, p) = page_on_list(1);
        f.mem.insert(SLOT_OWNER, OWNER);
        put_name(&mut f, SLOT_NAME, b"ANNA");
        assert_eq!(delete(&mut f, p), Some(Outcome::Denied));
        assert_eq!(f.mem[&(PLAYER_STATE + 4 * PLAYER)], STATE_LIST);
    }

    #[test]
    fn delete_waits_for_the_fade_and_ignores_other_pages_and_states() {
        let (mut f, p) = page_on_list(1);
        f.mem.insert(p.0 + PAGE_FADE, 0.25f32.to_bits());
        assert_eq!(delete(&mut f, p), Some(Outcome::NotReady));
        let (mut f, p) = page_on_list(1);
        f.mem.insert(p.0 + page::BUILDER, 0x2D_2350);
        assert_eq!(delete(&mut f, p), None);
        let (mut f, p) = page_on_list(1);
        f.mem.insert(PLAYER_STATE + 4 * PLAYER, 5);
        assert_eq!(delete(&mut f, p), None);
        assert_eq!(accept(&mut f, p), None);
    }

    #[test]
    fn confirm_yes_deletes_and_any_other_item_cancels() {
        let (mut f, p) = page_on_list(ITEM_YES);
        f.mem
            .insert(PLAYER_STATE + 4 * PLAYER, STATE_CONFIRM_DELETE);
        assert!(accept(&mut f, p).is_some());
        assert_eq!(f.mem[&(PLAYER_STATE + 4 * PLAYER)], STATE_DELETE);

        let (mut f, p) = page_on_list(-12);
        f.mem
            .insert(PLAYER_STATE + 4 * PLAYER, STATE_CONFIRM_DELETE);
        f.mem.insert(SLOT_OWNER + 4 * PLAYER, OWNER);
        put_name(&mut f, SLOT_NAME + SLOT_NAME_BYTES * PLAYER, b"ANNA");
        assert!(accept(&mut f, p).is_some());
        assert_eq!(f.mem[&(PLAYER_STATE + 4 * PLAYER)], STATE_LIST);
        assert_eq!(f.mem[&(SLOT_OWNER + 4 * PLAYER)], u32::MAX);
        assert!(slot_name(&f, PLAYER).is_empty());
        assert_eq!(f.mem[&LIST_DIRTY], 1);
    }

    /// The native page queue drives the whole flow: Delete, then Accept on Yes, and the generic
    /// activation (page veto `0x7B0AE` path) is not run for the confirm panel.
    #[test]
    fn native_queue_runs_delete_then_confirm() {
        let (mut f, p) = page_on_list(1);
        let queue = CommandQueue::default();
        let mut out = Vec::new();
        queue.lock().unwrap().push_back(NativeCommand::Delete);
        service_on(&mut f, Some(p), &queue, &mut out);
        assert_eq!(f.mem[&(PLAYER_STATE + 4 * PLAYER)], STATE_CONFIRM_DELETE);
        f.mem.insert(p.0 + PAGE_CURSOR_ID, ITEM_YES as u32);
        f.calls.clear();
        queue.lock().unwrap().push_back(NativeCommand::Accept);
        service_on(&mut f, Some(p), &queue, &mut out);
        assert_eq!(f.mem[&(PLAYER_STATE + 4 * PLAYER)], STATE_DELETE);
        assert_eq!(out.len(), 2);
        assert!(
            f.calls.iter().all(|(va, _)| *va != 0x7_3DE0),
            "the generic page veto and handlers do not run: {:?}",
            f.calls
        );
        assert!(f.calls.contains(&(PAD_NEUTRAL, vec![PLAYER])));
    }

    #[test]
    fn accept_dismisses_storage_messages_and_error_panels() {
        let (mut f, p) = page_on_list(0);
        f.mem.insert(PLAYER_STATE + 4 * PLAYER, STATE_MESSAGE);
        f.mem.insert(MESSAGE_RETURN_STATE, 6);
        f.mem.insert(PANEL_TIMER + 4 * PLAYER, 2.0f32.to_bits());
        assert!(accept(&mut f, p).is_some());
        assert_eq!(f.mem[&(PLAYER_STATE + 4 * PLAYER)], 6);
        assert_eq!(f.mem[&(PANEL_TIMER + 4 * PLAYER)], 0);

        let (mut f, p) = page_on_list(0);
        f.mem.insert(PLAYER_STATE + 4 * PLAYER, STATE_ERROR);
        f.mem.insert(SLOT_OWNER + 4 * PLAYER, OWNER);
        assert!(accept(&mut f, p).is_some());
        assert_eq!(f.mem[&(PLAYER_STATE + 4 * PLAYER)], STATE_LIST);
        assert_eq!(f.mem[&(SLOT_OWNER + 4 * PLAYER)], u32::MAX);
        assert_eq!(f.mem[&LIST_DIRTY], 1);

        // The list state keeps the generic row activation (profile rows have their own handlers).
        let (mut f, p) = page_on_list(1);
        assert_eq!(accept(&mut f, p), None);
    }

    /// T3772: A in state 11. A memory unit profile (owner below 8) asks for the unit load (state 6).
    #[test]
    fn acknowledge_loads_memory_unit_profiles() {
        let (mut f, p) = page_on_list(0);
        f.mem.insert(PLAYER_STATE + 4 * PLAYER, STATE_ACKNOWLEDGE);
        f.mem.insert(SLOT_OWNER + 4 * PLAYER, 3);
        f.mem.insert(UNIT_LOAD_REQUEST, 9);
        f.mem.insert(PANEL_TIMER + 4 * PLAYER, 2.0f32.to_bits());
        assert!(accept(&mut f, p).is_some());
        assert_eq!(f.mem[&(PLAYER_STATE + 4 * PLAYER)], STATE_UNIT_LOAD);
        assert_eq!(f.mem[&UNIT_LOAD_REQUEST], 0);
        assert_eq!(f.mem[&UNIT_LOAD_PENDING], 1);
        assert_eq!(f.mem[&(PANEL_TIMER + 4 * PLAYER)], 0);
        assert!(f.calls.contains(&(PAD_NEUTRAL, vec![PLAYER])));
        assert!(!f.calls.iter().any(|(va, _)| *va == APPLY_PROFILE));
    }

    /// T3772: A in state 11 on a hard disk profile: apply it, then `0x190FD0` waits for the other players (state 14)
    /// or, for the last one, aggregates and opens the next page.
    #[test]
    fn acknowledge_applies_the_profile_then_waits_or_finishes() {
        let (mut f, p) = page_on_list(0);
        f.mem.insert(PLAYER_STATE + 4 * PLAYER, STATE_ACKNOWLEDGE);
        f.mem.insert(SLOT_OWNER + 4 * PLAYER, 8);
        f.mem.insert(PLAYERS_CHOOSING, 0b11);
        assert!(accept(&mut f, p).is_some());
        let name = SLOT_NAME + SLOT_NAME_BYTES * PLAYER;
        assert_eq!(f.calls[0], (APPLY_PROFILE, vec![PLAYER, name, 0]));
        assert_eq!(f.calls[1], (PROFILE_EVENT, vec![PLAYER, 0xB, 0]));
        assert_eq!(f.mem[&PLAYERS_CHOOSING], 0b01, "player 1 stops choosing");
        assert_eq!(f.mem[&(PLAYER_STATE + 4 * PLAYER)], STATE_WAIT_OTHERS);
        assert_eq!(f.mem[&LIST_DIRTY], 1);
        assert!(!f.calls.iter().any(|(va, _)| *va == PUSH_CHILD));

        let (mut f, p) = page_on_list(0);
        f.mem.insert(PLAYER_STATE + 4 * PLAYER, STATE_ACKNOWLEDGE);
        f.mem.insert(SLOT_OWNER + 4 * PLAYER, OWNER);
        f.mem.insert(PLAYERS_CHOOSING, 0b10);
        f.mem.insert(SUMMARY_SLOTS, 0x55);
        f.mem.insert(MENU_CONTEXT, 0x7000);
        assert!(accept(&mut f, p).is_some());
        assert!(f.calls.contains(&(AGGREGATE_STATS, vec![0x55])));
        assert!(
            f.calls
                .contains(&(PUSH_CHILD, vec![0x7000, DONE_DESCRIPTOR, 0x55]))
        );
        assert_eq!(f.mem[&PLAYERS_CHOOSING], 0);
        assert_eq!(f.mem[&(PLAYER_STATE + 4 * PLAYER)], STATE_ACKNOWLEDGE);
    }

    /// T3772 guard: every pad read of `0x192770` has its Rust branch, and the branch answers in that state.
    #[test]
    fn every_raw_pad_read_of_the_update_has_a_native_branch() {
        for (state, _call, handler) in RAW_PAD_BRANCHES {
            let (mut f, p) = page_on_list(ITEM_YES);
            f.mem.insert(PLAYER_STATE + 4 * PLAYER, state);
            f.mem.insert(PLAYER_PORT + 4 * PLAYER, u32::MAX);
            f.mem.insert(PLAYER_COUNT, 2);
            let outcome = match handler {
                "join" => join(&mut f, p, 0),
                "delete" => delete(&mut f, p),
                "accept" => accept(&mut f, p),
                other => panic!("unknown branch {other}"),
            };
            assert!(
                outcome.is_some(),
                "state {state}: {handler} does not answer"
            );
        }
    }
}

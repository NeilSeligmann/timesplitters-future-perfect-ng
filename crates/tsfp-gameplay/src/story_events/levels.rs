//! Level descriptor table, the per-level hook `[level+0x110]` and the special level ids (T1844).
//!
//! MEASURED in `build/default.xbe` (SHA-256 `3cfd001a...1816bc`, static reading only, no xemu run):
//!
//! * The level table starts at `0x515FFC` (stride 0x80, level id in the dword at `+4`, ends at an entry whose
//!   word at `+0` is 0). `+0x20` points to the story-side level struct and `+0x24` to the other-mode struct
//!   (`0x6A810` returns `+0x20` for the story mode 10; `0x6A7B0` returns `+0x20` only when the level id is at
//!   most 0x64 and `[0x7DE494]` is non-zero, else `+0x24`; `0x6A850` caches the answer per level and mode).
//!   `[0x7DE494]` is read by two functions and written by none, so it keeps its initial 0: outside the story
//!   mode the `+0x24` struct is always used.
//! * The struct word `+0x110` is the level hook, a plain function pointer stored in the image (not touched by the
//!   fixup loops of `0x69F30`). [`LEVEL_HOOKS`] lists, per level id, the hook of the `+0x20` and the `+0x24`
//!   struct (0 = no struct or a null hook). Level `0x64`'s pointers are the BSS struct `0x78CC00` whose
//!   `+0x110` is never stored (T1845, MEASURED): the hook is null, [`HookSource::None`].
//! * The hook is called by `0x228870` (the per-frame static slot update), see [`frame_hook_dispatch`].
//!
//! The hook bodies (25 distinct functions) are ported in `hooks_audio`, `hooks_story` and `hooks_big` (T1845);
//! [`HOOK_PORTS`] says where each lives and how far it is decoded.

use super::mode::ModeContext;

/// Level ids with special handling in the event engine (MEASURED).
pub const LEVEL_NO_RESTORE_GATE: u32 = 0x38;
/// Level 0x64: no restore gate, restore deferral is never applied for levels above it, hook struct is run time filled.
pub const LEVEL_ARCADE_BOUNDARY: u32 = 0x64;
/// Level 0x65: the event pool is allocated with zero entries (`0x2374A0`).
pub const LEVEL_NO_EVENTS: u32 = 0x65;
/// Level 0x66: takes the story restore gate even outside the story mode.
pub const LEVEL_STORY_LIKE: u32 = 0x66;

/// Where the hook of a level comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookSource {
    /// A static function pointer in the image: the address.
    Static(u32),
    /// No struct or a null hook.
    None,
    /// The level struct is in BSS and filled at run time (level 0x64).
    RuntimeFilled,
}

/// `(level id, hook of the +0x20 struct, hook of the +0x24 struct)`, read from the level table at `0x515FFC`.
pub const LEVEL_HOOKS: &[(u32, u32, u32)] = &[
    (0x1, 0x1ddc90, 0x0),
    (0x2, 0x21ad00, 0x0),
    (0x3, 0x0, 0x0),
    (0x4, 0x1e40d0, 0x0),
    (0x5, 0x220600, 0x0),
    (0x6, 0x20b630, 0x0),
    (0x7, 0x0, 0x0),
    (0x8, 0x0, 0x0),
    (0x9, 0x224160, 0x0),
    (0xa, 0x0, 0x0),
    (0xb, 0x1f5a10, 0x0),
    (0xc, 0x0, 0x0),
    (0xd, 0x0, 0x0),
    (0xe, 0x205bc0, 0x0),
    (0xf, 0x0, 0x207580),
    (0x10, 0x0, 0x0),
    (0x11, 0x213f50, 0x0),
    (0x12, 0x1fa9c0, 0x0),
    (0x13, 0x0, 0x0),
    (0x14, 0x0, 0x21af90),
    (0x16, 0x20fbd0, 0x0),
    (0x17, 0x0, 0x0),
    (0x18, 0x0, 0x0),
    (0x19, 0x0, 0x0),
    (0x1a, 0x221c70, 0x0),
    (0x1b, 0x0, 0x389b0),
    (0x1c, 0x0, 0x0),
    (0x1d, 0x2084a0, 0x2084a0),
    (0x1e, 0x0, 0x0),
    (0x1f, 0x0, 0x0),
    (0x20, 0x0, 0x0),
    (0x21, 0x1f6620, 0x0),
    (0x22, 0x1f6620, 0x0),
    (0x23, 0x0, 0x0),
    (0x24, 0x0, 0x0),
    (0x25, 0x0, 0x0),
    (0x26, 0x0, 0x0),
    (0x27, 0x222d90, 0x222d90),
    (0x28, 0x0, 0x0),
    (0x29, 0x0, 0x0),
    (0x2b, 0x0, 0x0),
    (0x2c, 0x0, 0x0),
    (0x2d, 0x0, 0x0),
    (0x2e, 0x222b60, 0x222b60),
    (0x2f, 0x0, 0x0),
    (0x30, 0x220dc0, 0x220dc0),
    (0x31, 0x225a10, 0x225a10),
    (0x32, 0x0, 0x0),
    (0x33, 0x0, 0x0),
    (0x34, 0x0, 0x226600),
    (0x35, 0x2210a0, 0x2210a0),
    (0x36, 0x0, 0x0),
    (0x38, 0x389b0, 0x0),
    (0x39, 0x0, 0x0),
    (0x3a, 0x0, 0x0),
    (0x3b, 0x0, 0x225c50),
    (0x3c, 0x222d90, 0x222d90),
    (0x3d, 0x1d7b80, 0x1d7b80),
    (0x3e, 0x0, 0x0),
    (0x3f, 0x222b60, 0x222b60),
    (0x40, 0x21ad00, 0x21ad00),
    (0x41, 0x0, 0x0),
    (0x42, 0x0, 0x0),
    (0x43, 0x0, 0x0),
    (0x44, 0x2210a0, 0x2210a0),
    (0x45, 0x0, 0x0),
    (0x46, 0x0, 0x0),
    (0x47, 0x225a10, 0x225a10),
    (0x48, 0x207580, 0x207580),
    (0x49, 0x0, 0x0),
    (0x4a, 0x222d90, 0x222d90),
    (0x4b, 0x0, 0x0),
    (0x4c, 0x0, 0x0),
    (0x4d, 0x2210a0, 0x2210a0),
    (0x4e, 0x0, 0x0),
    (0x4f, 0x21af90, 0x21af90),
    (0x50, 0x1e40d0, 0x1e40d0),
    (0x51, 0x0, 0x0),
    (0x52, 0x0, 0x0),
    (0x64, 0x0, 0x0),
    (0x65, 0x0, 0x0),
    (0x66, 0x0, 0x0),
    (0x67, 0x0, 0x0),
    (0x68, 0x0, 0x0),
    (0x69, 0x0, 0x0),
    (0x6a, 0x0, 0x0),
    (0x6b, 0x0, 0x0),
];

/// The hook source for `level_id`, story side (`story == true`, `+0x20` struct) or the other modes (`+0x24` struct).
pub fn hook_source(level_id: u32, story: bool) -> HookSource {
    // T1845: level 0x64's struct (BSS 0x78CC00) is a zero-initialised progress block whose +0x110 nothing stores,
    // so its hook is null (MEASURED, docs/t-story-event-arcade-runtime.md). `RuntimeFilled` is kept for API
    // compatibility and is no longer returned.
    match LEVEL_HOOKS.iter().find(|row| row.0 == level_id) {
        Some(&(_, primary, secondary)) => match if story { primary } else { secondary } {
            0 => HookSource::None,
            address => HookSource::Static(address),
        },
        None => HookSource::None,
    }
}

/// `0x6A7B0` / `0x6A810` / `0x6A850`: which struct a mode reads. Story (mode 10) always uses `+0x20`; the other
/// modes use it only for levels up to 0x64 with `[0x7DE494] != 0` (never set in the image).
pub fn uses_story_struct(mode_id: u8, level_id: u32, alt_flag: bool) -> bool {
    mode_id == super::mode::MODE_STORY || (level_id <= LEVEL_ARCADE_BOUNDARY && alt_flag)
}

/// The static hook address for the context, if any.
pub fn level_hook(context: &ModeContext) -> HookSource {
    hook_source(
        context.level_id,
        uses_story_struct(context.mode_id, context.level_id, false),
    )
}

/// How `0x228870` reaches the hook this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookDispatch {
    /// The whole function returns without running anything (`[0x4E7944]` step count 0, or the descriptor path
    /// with neither bit 2 nor bit 0x40).
    Skip,
    /// The descriptor path with bit 0x40 and no bit 2: only the hook runs (a tail jump), no slot poll.
    HookOnly,
    /// The normal path: slot poll, mode objective (`0x232270` in mode 3, `0x2284D0` in mode 5), then the hook.
    AfterObjective,
}

/// `0x228870`: `steps` is `[0x4E7944]`, `game_flags` `[0x7DE458]`, `descriptor_flags` the dword `+0x18` of the
/// entry `0x230510()` returns (read only when `game_flags & 0x40`).
pub fn frame_hook_dispatch(steps: i32, game_flags: u32, descriptor_flags: u32) -> HookDispatch {
    if steps == 0 {
        return HookDispatch::Skip;
    }
    if game_flags & 0x40 != 0 && descriptor_flags & 2 == 0 {
        return if descriptor_flags & 0x40 != 0 {
            HookDispatch::HookOnly
        } else {
            HookDispatch::Skip
        };
    }
    HookDispatch::AfterObjective
}

/// `0x2374A0`: the instance pool of level 0x65 is empty whatever the definition count.
pub fn pool_size(level_id: u32, definitions: usize) -> usize {
    if level_id == LEVEL_NO_EVENTS {
        0
    } else {
        definitions
    }
}

/// `0x23765B..0x2376C1`: for a persistent verb 3 object (record flag 0x10) in the snapshot restore, true when the
/// per-type gate applies: game flag 0x40 clear, level not 0x64 or 0x38, and (story mode or level 0x66). When it
/// applies and the object type's class flag `0x2000000` (class table `0x791D4C`, stride 0x30, `+0x28`) is set the
/// object is not recreated (the deferral path `0x23772C`).
pub fn restore_gate_applies(context: &ModeContext) -> bool {
    context.game_flags & 0x40 == 0
        && context.level_id != LEVEL_ARCADE_BOUNDARY
        && context.level_id != LEVEL_NO_RESTORE_GATE
        && (context.mode_id == super::mode::MODE_STORY || context.level_id == LEVEL_STORY_LIKE)
}

/// Class flag that blocks the persistent restore when [`restore_gate_applies`].
pub const CLASS_FLAG_NO_RESTORE: u32 = 0x200_0000;

/// `0x237770`: deferred persistent restores are applied only for levels up to 0x64.
pub fn pending_restore_enabled(level_id: u32) -> bool {
    level_id <= LEVEL_ARCADE_BOUNDARY
}

/// Where each distinct hook address (table value) is ported (T1845). `Full` = the hook body is modelled,
/// `Partial` = the top-level logic is modelled and some callees are `Unsupported` (see the named doc).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortStatus {
    Full,
    Partial,
}

/// `(hook address, port function, module, status)`.
pub const HOOK_PORTS: &[(u32, &str, &str, PortStatus)] = &[
    (
        0x1DDC90,
        "hooks_story::level1_hook",
        "hooks_story",
        PortStatus::Partial,
    ),
    (
        0x21AD00,
        "hooks_story::level2_hook",
        "hooks_story",
        PortStatus::Partial,
    ),
    (
        0x1E40D0,
        "hooks_story::level4_hook",
        "hooks_story",
        PortStatus::Partial,
    ),
    (
        0x220600,
        "hooks_story::level5_hook",
        "hooks_story",
        PortStatus::Partial,
    ),
    (
        0x224160,
        "hooks_story::level9_hook",
        "hooks_story",
        PortStatus::Partial,
    ),
    (
        0x205BC0,
        "hooks_story::level0e_hook",
        "hooks_story",
        PortStatus::Partial,
    ),
    (
        0x1FA9C0,
        "hooks_story::level12_hook",
        "hooks_story",
        PortStatus::Partial,
    ),
    (
        0x221C70,
        "hooks_story::level1a_hook",
        "hooks_story",
        PortStatus::Partial,
    ),
    (
        0x20B630,
        "hooks_big::level06_hook",
        "hooks_big",
        PortStatus::Partial,
    ),
    (
        0x1F5A10,
        "hooks_big::level0b_hook",
        "hooks_big",
        PortStatus::Partial,
    ),
    (
        0x213F50,
        "hooks_big::level11_hook",
        "hooks_big",
        PortStatus::Partial,
    ),
    (
        0x1F6620,
        "hooks_big::strip_scroll_speed",
        "hooks_big",
        PortStatus::Full,
    ),
    (
        0x222D90,
        "hooks_audio (MaxMix)",
        "hooks_audio",
        PortStatus::Full,
    ),
    (
        0x2210A0,
        "hooks_audio (outfit crossfade)",
        "hooks_audio",
        PortStatus::Full,
    ),
    (
        0x225A10,
        "hooks_audio (mean crossfade)",
        "hooks_audio",
        PortStatus::Full,
    ),
    (
        0x222B60,
        "hooks_audio (periodic one-shot)",
        "hooks_audio",
        PortStatus::Full,
    ),
    (
        0x220DC0,
        "hooks_audio (lightning strike)",
        "hooks_audio",
        PortStatus::Full,
    ),
    (
        0x21AF90,
        "hooks_audio (random one-shot)",
        "hooks_audio",
        PortStatus::Full,
    ),
    (
        0x226600,
        "hooks_audio (weighted crossfade)",
        "hooks_audio",
        PortStatus::Full,
    ),
    (
        0x225C50,
        "hooks_audio (class loop)",
        "hooks_audio",
        PortStatus::Full,
    ),
    (
        0x1D7B80,
        "hooks_audio (random thunder)",
        "hooks_audio",
        PortStatus::Full,
    ),
    (
        0x207580,
        "hooks_audio (crossfade 0x207450)",
        "hooks_audio",
        PortStatus::Full,
    ),
    (
        0x2084A0,
        "hooks_audio (level 0x1D crossfade)",
        "hooks_audio",
        PortStatus::Partial,
    ),
    (
        0x20FBD0,
        "hooks_audio (level 0x16 timeout and loop)",
        "hooks_audio",
        PortStatus::Partial,
    ),
    (
        0x389B0,
        "no-op (a single return)",
        "levels",
        PortStatus::Full,
    ),
];

/// Thunk to body: the table holds link thunks for several audio hooks (T1846). Returns the body address the
/// hook port dispatch uses, other addresses unchanged.
pub fn audio_alias(address: u32) -> u32 {
    match address {
        0x222D90 => 0x222C90,
        0x225A10 => 0x225840,
        0x2210A0 => 0x220F90,
        0x207580 => 0x207450,
        0x226600 => 0x2264F0,
        0x1D7B80 => 0x1D7920,
        0x220DC0 => 0x220AF0,
        other => other,
    }
}

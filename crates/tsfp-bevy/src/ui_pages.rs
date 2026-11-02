// SPDX-License-Identifier: GPL-3.0-or-later
//! T2311 UI-13: the data-driven front-end page registry.
//!
//! Every registered page (a [`PageDescriptor`]: builder VA and name) is handled the same way, no
//! per-page code:
//!
//! * **Content from the live page record.** At every frame boundary on the guest thread
//!   ([`service`], called from `ui_live::tsfp_ui_frame_service`) the top live page slot is read
//!   through `tsfp_ui_input::live_page` (rows in creation order, text ids from the string banks,
//!   runtime text, fonts, layout boxes, option entries, cursor row) and turned into a model page.
//!   The Rust drawer gets that page, so rows unlocked, renamed or added at run time are drawn as
//!   the guest has them, not as a captured dump had them.
//! * **Drawn inside the engine frame** by the T2309 `PageLayer` with the original skin, and the
//!   original draw of the page is skipped per page by the `sub_00076810` hook
//!   (`ui_overlay::tsfp_ui_page_draw_owned` asks [`draw_owned`]). A page is drawn by Rust only
//!   when every row draw callback it uses has a Rust implementation ([`RUST_ROW_DRAWS`]); a page
//!   with an unported custom row draw (Story level rows, Challenge set rows, the Progress summary)
//!   keeps the original draw, and the reason is logged once (`ui-pages: ... legacy draw`).
//! * **Native input.** Keys and pad on a registered page become native `Step`, `Accept` and `Back`
//!   commands (`ui_live::native_input_page`), the mouse focuses the row under the pointer and
//!   left/right click accept/back ([`pointer_commands`]); all are run at the frame boundary by
//!   `native_page::service`, which calls the original page and row handlers. No pad presses.
//! * **F9** gives drawing and input back to the original (`ui_live::USER_HIDDEN`).
//!
//! The main menu is registered too: its content now also comes from the live record (its
//! interactive focus model, `LiveMenu`, stays in ui_live.rs).
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use tsfp_ui_input::live_page::{self, LivePage, TextIndex};
use tsfp_ui_input::menu_state::{PageState, layout as guest_layout};
use tsfp_ui_input::native_page::{GuestUi, NativeCommand};
use tsfp_ui_input::{ModelLayout, hit_test};
use tsfp_ui_input::{default_style_live, list_rows_live};
use tsfp_ui_model::default_style::DefaultPage;
use tsfp_ui_model::list_rows::{GuestImage, ListRowKind};
use tsfp_ui_model::progress_summary::PLAYER_ICON_SLOT;
use tsfp_ui_model::skin::PackedColor;
use tsfp_ui_model::{PageId, UiModel, WidgetId};
use tsfp_ui_wgpu::in_frame::LiveContent;

/// One registered front-end page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageDescriptor {
    pub builder: u32,
    pub name: &'static str,
}

/// T2311 batch 1 (docs/t-menu-catalogue.md builder identities): the main menu and the row-list
/// pages reached from it. Options and Arcade are T2310's.
pub const PAGES: &[PageDescriptor] = &[
    PageDescriptor {
        builder: 0x2C_A230,
        name: "Arcade type",
    },
    PageDescriptor {
        builder: 0x2C_BFC0,
        name: "Settings",
    },
    PageDescriptor {
        builder: 0x2D_0BE0,
        name: "Main Menu",
    },
    PageDescriptor {
        builder: 0x2C_B090,
        name: "Story",
    },
    // T2323: selector pages use the same typed live rows and native commands.
    PageDescriptor {
        builder: 0x2D_62F0,
        name: "Story difficulty",
    },
    PageDescriptor {
        builder: 0x2C_AC30,
        name: "Story briefing",
    },
    PageDescriptor {
        builder: 0x2D_6250,
        name: "Arcade mode",
    },
    PageDescriptor {
        builder: 0x2C_A960,
        name: "Arcade level",
    },
    PageDescriptor {
        builder: 0x2E_FA40,
        name: "Unlock list selector",
    },
    PageDescriptor {
        builder: 0x2E_FB40,
        name: "Setup option selector",
    },
    PageDescriptor {
        builder: 0x2E_FBF0,
        name: "Setup category selector",
    },
    PageDescriptor {
        builder: 0x2C_9AE0,
        name: "Arcade league",
    },
    PageDescriptor {
        builder: 0x2C_9480,
        name: "Arcade match",
    },
    PageDescriptor {
        builder: 0x2C_9040,
        name: "Challenge sign-in warning",
    },
    PageDescriptor {
        builder: 0x2C_C170,
        name: "Movies",
    },
    PageDescriptor {
        builder: 0x2C_8940,
        name: "Map Maker",
    },
    PageDescriptor {
        builder: 0x2C_8E00,
        name: "Challenge",
    },
    PageDescriptor {
        builder: 0x2D_6810,
        name: "Player Progress",
    },
    PageDescriptor {
        builder: 0x2D_08C0,
        name: "Extras",
    },
    PageDescriptor {
        builder: 0x1A_2730,
        name: "Xbox Live / System Link",
    },
    PageDescriptor {
        builder: 0x2D_2350,
        name: "Players",
    },
    // T2319: profile storage is a separate page from the Map Maker picker.
    // Descriptor 0x4E8D74; native framework commands own focus/activate/back.
    PageDescriptor {
        builder: 0x19_33C0,
        name: "Profile storage and save status",
    },
    PageDescriptor {
        builder: 0x19_4980,
        name: "Profile selection",
    },
    // T2191 (in-game, docs/t2191-ingame-ui.md "Native in-game pages"): the pause item list (Story, Arcade, Challenge
    // and the Map Maker preview, T2185 captures), its confirm dialogs and the options pages reached from it.
    PageDescriptor {
        builder: 0x2F_5520,
        name: "Pause",
    },
    PageDescriptor {
        builder: 0x2E_B9E0,
        name: "In-game confirm",
    },
    PageDescriptor {
        builder: 0x2E_BE90,
        name: "Leave match confirm",
    },
    // T2315 increment 4: the pages of the in-game BACK action (0x1CE7EB -> 0x2E6980): the Arcade scoreboard (descriptor
    // 0x524750) and the Story objectives (0x524740).
    PageDescriptor {
        builder: 0x2F_5920,
        name: "Scoreboard",
    },
    PageDescriptor {
        builder: 0x2F_5E60,
        name: "Objectives",
    },
    PageDescriptor {
        builder: 0x2E_A8E0,
        name: "Controls",
    },
    PageDescriptor {
        builder: 0x2E_9EC0,
        name: "Controls accept changes",
    },
    PageDescriptor {
        builder: 0x2C_3E20,
        name: "Audio/Video",
    },
    // T1916 (Map Maker UI, docs/t-system-mapmaker-ui.md "Native Map Maker UI"): the editor menu and pages it opens
    // (T2185 routes ui-editor-menu/load/save; descriptors in docs/t1714-editor-pages.md). T2408/T1690 registers the
    // selected-entry properties page for native cursor navigation and typed actions. Its measured kind-A/default-style
    // draw candidate is admitted only under TSFP_MAPMAKER_UI=rust; frame parity remains open.
    PageDescriptor {
        builder: 0x31_3010,
        name: "Map Maker editor menu",
    },
    PageDescriptor {
        builder: 0x31_33E0,
        name: "Map Maker map settings",
    },
    // T1714 identifies this Assault Options route as a Map Maker editor page.
    // The typed page model preserves measured action order and opaque runtime
    // choice IDs; original gameplay callbacks remain its temporary effect seam.
    PageDescriptor {
        builder: 0x31_3FE0,
        name: "Map Maker assault options",
    },
    // T1714 measures the Assault start-point assignment page and its two
    // actionable rows; selection mutation remains behind the typed request.
    PageDescriptor {
        builder: crate::mapmaker_assault_phase_assignment_live::BUILDER,
        name: "Map Maker Assault phase start point",
    },
    // T1714: measured Logic Summary row order with duplicate Trigger/Action
    // labels resolved by their page position and generation-scoped identity.
    PageDescriptor {
        builder: 0x32_9DE0,
        name: "Map Maker logic summary",
    },
    // T2408's Action Picker model and typed input adapter are admitted only
    // for the exact measured T1714 page shape; dynamic action effects remain
    // at the original callback seam.
    PageDescriptor {
        builder: tsfp_ui_input::mapmaker_action_picker::BUILDER,
        name: "Map Maker Select Actions",
    },
    // T1714's trigger editor page: dynamic rows remain live while measured
    // selector controls use generation-scoped Rust actions.
    PageDescriptor {
        builder: tsfp_ui_input::mapmaker_trigger_selection::BUILDER,
        name: "Map Maker trigger selection",
    },
    // T1714 shared profile-list helpers; registered only after the matching
    // typed Map Settings action and exact runtime row capacity are observed.
    PageDescriptor {
        builder: 0x2B_F830,
        name: "Map Maker Bot Set list",
    },
    PageDescriptor {
        builder: 0x2C_1830,
        name: "Map Maker Weapon Set list",
    },
    PageDescriptor {
        builder: 0x31_2270,
        name: "Map Maker editor settings",
    },
    PageDescriptor {
        builder: 0x31_5720,
        name: "Map Maker controls",
    },
    PageDescriptor {
        builder: 0x31_4420,
        name: "Map Maker preview",
    },
    // T1714 records this text/type-6 help page and its ordered editor command
    // labels. Its caller is not established, so admit it only through the
    // Map Maker page gate; dynamic row content and handlers remain live.
    PageDescriptor {
        builder: 0x31_4650,
        name: "Map Maker editor help",
    },
    // T2408: Award Flags has a generation-scoped native action model; retain
    // its exact four-row static contract and route the page through the same
    // guarded cursor path as the other Map Maker editor pages.
    PageDescriptor {
        builder: tsfp_ui_input::mapmaker_award_flags::BUILDER,
        name: "Map Maker Award Flags",
    },
    // T1714 measures the kind chooser's labels but not its caller or callback
    // effects. Keep the page in the guarded Map Maker path; live rows and any
    // unsupported callback drawing continue through the existing seam.
    PageDescriptor {
        builder: 0x32_23A0,
        name: "Map Maker object kind chooser",
    },
    // T1714 records the Awards chooser labels, but no direct caller or row
    // effects. Keep it on the guarded generic page/cursor seam.
    PageDescriptor {
        builder: 0x32_2BB0,
        name: "Map Maker awards chooser",
    },
    // T1714 records the ordered Award Medal Settings labels; dynamic trigger
    // rows and callback effects remain at the typed page seam.
    PageDescriptor {
        builder: tsfp_ui_input::mapmaker_award_medal::BUILDER,
        name: "Map Maker Award Medal settings",
    },
    // T1714 measures a dynamic Awards table with a trailing extra item; keep
    // every live row in order and leave row effects at the callback seam.
    PageDescriptor {
        builder: tsfp_ui_input::mapmaker_award_table::BUILDER,
        name: "Map Maker Awards table",
    },
    PageDescriptor {
        builder: 0x32_8D20,
        name: "Map Maker selected-entry properties",
    },
    // T1714 measures a mode-filtered runtime kind table; row values and
    // callback effects remain opaque at the Rust-owned input boundary.
    PageDescriptor {
        builder: tsfp_ui_input::mapmaker_kind_list::BUILDER,
        name: "Map Maker mode-filtered kind list",
    },
    PageDescriptor {
        builder: 0x32_B6A0,
        name: "Map Maker load/save storage",
    },
    PageDescriptor {
        builder: 0x32_BEA0,
        name: "Map Maker map list",
    },
    PageDescriptor {
        builder: 0x32_AEE0,
        name: "Map Maker storage status",
    },
    // T2408 owner route (2026-10-19): dynamic Map Maker routes observed at
    // 0x2BF770 and 0x192110. They overlap general frontend/save helpers, so
    // service admits them only while the Rust Map Maker path is enabled. The
    // mode prompt also admits its exact measured row pair before UI_MODE is set.
    PageDescriptor {
        builder: 0x2B_F770,
        name: "Map Maker mode selection",
    },
    PageDescriptor {
        builder: 0x19_2110,
        name: "Map Maker local map list",
    },
    // Logical identity: descriptor-less type-2 children are matched by popup_descriptor.
    PageDescriptor {
        builder: 0x7_A080,
        name: "Map Maker confirm",
    },
];

/// T1916: the Map Maker pages of [`PAGES`]. They are neither front-end nor in-game pages for the in-game session flag.
pub const MAPMAKER_PAGES: &[u32] = tsfp_ui_input::mapmaker_handlers::NATIVE_CURSOR_PAGES;

fn is_mapmaker_alias(builder: u32) -> bool {
    matches!(builder, 0x2B_F770 | 0x19_2110)
}

fn requires_registered_mapmaker_input(builder: u32) -> bool {
    is_mapmaker_alias(builder)
        || builder == tsfp_ui_input::mapmaker_logic_summary::BUILDER
        || builder == tsfp_ui_input::mapmaker_trigger_selection::BUILDER
}

fn mapmaker_page_enabled_for(
    g: &dyn GuestUi,
    builder: u32,
    slot: u32,
    mapmaker_ui_enabled: bool,
    ui_mode: Option<u32>,
) -> bool {
    if is_mapmaker_alias(builder) {
        return mapmaker_alias_enabled_for(g, builder, slot, mapmaker_ui_enabled, ui_mode);
    }
    if builder == tsfp_ui_input::mapmaker_logic_summary::BUILDER {
        return mapmaker_ui_enabled
            && ui_mode == Some(tsfp_ui_input::editor_pointer::addr::MODE_MAPMAKER);
    }
    if builder == tsfp_ui_input::mapmaker_trigger_selection::BUILDER {
        return mapmaker_ui_enabled
            && ui_mode == Some(tsfp_ui_input::editor_pointer::addr::MODE_MAPMAKER);
    }
    true
}

fn mapmaker_page_enabled(g: &dyn GuestUi, builder: u32, slot: u32) -> bool {
    mapmaker_page_enabled_for(
        g,
        builder,
        slot,
        crate::storage_live::mapmaker_ui_enabled(),
        g.read32(tsfp_ui_input::editor_pointer::addr::UI_MODE),
    )
}

fn mapmaker_alias_enabled_for(
    g: &dyn GuestUi,
    builder: u32,
    slot: u32,
    mapmaker_ui_enabled: bool,
    ui_mode: Option<u32>,
) -> bool {
    if !is_mapmaker_alias(builder) || !mapmaker_ui_enabled {
        return false;
    }
    match builder {
        // The owner route can reach this prompt before the editor publishes
        // UI_MODE. Its exact two-row callback identity is sufficient proof of
        // the prompt; an empty or unrelated 0x2BF770 page remains rejected.
        0x2B_F770 => {
            ui_mode == Some(tsfp_ui_input::editor_pointer::addr::MODE_MAPMAKER)
                || mapmaker_mode_prompt_rows(g, slot)
        }
        // The Local list alias still needs the established Map Maker mode
        // guard; its ambiguous save/profile role is not inferred from rows.
        0x19_2110 => ui_mode == Some(tsfp_ui_input::editor_pointer::addr::MODE_MAPMAKER),
        _ => false,
    }
}

fn mapmaker_alias_enabled(g: &dyn GuestUi, builder: u32, slot: u32) -> bool {
    mapmaker_alias_enabled_for(
        g,
        builder,
        slot,
        crate::storage_live::mapmaker_ui_enabled(),
        g.read32(tsfp_ui_input::editor_pointer::addr::UI_MODE),
    )
}

/// Row draw callbacks the Rust skin draws itself. T2311 increment 2: the divider 0x2D82F0
/// (`tsfp_ui_model::skin::row_draws`); increment 3: the Story level row 0x2CA9F0 and the Challenge
/// set row 0x2C8B50 with their icon draw 0x2BEAC0 (`tsfp_ui_model::list_rows`, data read by
/// `tsfp_ui_input::list_rows_live`), and the Player Progress summary 0x2CC850
/// (`tsfp_ui_model::progress_summary`, stats read by `list_rows_live::read_stats`). A page using
/// any other row draw keeps the original draw.
pub const RUST_ROW_DRAWS: &[u32] = &[
    tsfp_ui_model::skin::DIVIDER_ROW_DRAW,
    tsfp_ui_model::list_rows::LEVEL_ROW_DRAW,
    tsfp_ui_model::list_rows::CHALLENGE_ROW_DRAW,
    tsfp_ui_model::progress_summary::PROGRESS_SUMMARY_DRAW,
    // T2315 increment 3: the Audio/Video slider and icon rows (`tsfp_ui_model::option_rows`).
    tsfp_ui_model::option_rows::SLIDER_ROW_DRAW,
    tsfp_ui_model::option_rows::ICON_ROW_DRAW,
    // T2315 increment 3: the Controls button picture and turn speed rows.
    tsfp_ui_model::option_rows::BUTTON_ROW_DRAW,
    tsfp_ui_model::option_rows::TURN_SPEED_ROW_DRAW,
    // T2315 increment 4: the scoreboard and Story objectives rows (`tsfp_ui_model::ingame_rows`); a mode whose branches
    // are not ported keeps the original draw (`tsfp_ui_input::ingame_live` returns None, see `service`).
    tsfp_ui_model::ingame_rows::SCOREBOARD_ROW_DRAW,
    tsfp_ui_model::ingame_rows::OBJECTIVES_ROW_DRAW,
];

/// Page draws (descriptor draw, page `+0x10`) the Rust drawer draws itself. T2315 increment 3: the Audio/Video page
/// draw 0x2C4190 (the safe-area frame of the screen position mode, `tsfp_ui_model::option_rows`).
pub const RUST_PAGE_DRAWS: &[u32] = &[
    tsfp_ui_model::option_rows::AV_PAGE_DRAW,
    tsfp_ui_model::mapmaker_controls::PAGE_DRAW,
];

const MAPMAKER_PALETTE_PROPERTIES_BUILDER: u32 = 0x32_8D20;
const MAPMAKER_PALETTE_PROPERTIES_DRAW: u32 = 0x31_6B30;

fn page_draw_supported(builder: u32, page_draw: u32, palette_properties_candidate: bool) -> bool {
    RUST_PAGE_DRAWS.contains(&page_draw)
        || (palette_properties_candidate
            && builder == MAPMAKER_PALETTE_PROPERTIES_BUILDER
            && page_draw == MAPMAKER_PALETTE_PROPERTIES_DRAW)
}
/// T2191: the pages that open the in-game menu (the game is paused under them). Pause hands the options pages
/// (Controls, Audio/Video) the slot when it opens them, so "in game" is a session flag ([`ingame`]): set while one
/// of these is the top page, cleared when no page is live (the menu closed, gameplay runs) or a front-end page is.
pub const INGAME_PAGES: &[u32] = &[0x2F_5520, 0x2E_B9E0, 0x2E_BE90, 0x2F_5920, 0x2F_5E60];

/// The in-game menu is open (see [`INGAME_PAGES`]).
static INGAME: AtomicBool = AtomicBool::new(false);

/// The in-game menu is open. Physical pad START retains the original pause toggle;
/// keyboard Escape resumes on the root and backs on subpages (T2327).
pub fn ingame() -> bool {
    INGAME.load(Ordering::Acquire)
}

/// Session flag update from the top live page's builder (`None`: no page live), once per shell frame from the C
/// page observer's view (`ui_overlay::top_live_page`, slots it reports closed are not live).
pub fn track_ingame(top: Option<u32>) {
    match top {
        None => {
            if INGAME.swap(false, Ordering::AcqRel) {
                eprintln!("tsfp-bevy: ui-pages: in-game menu closed");
            }
        }
        Some(builder) if INGAME_PAGES.contains(&builder) => {
            if !INGAME.swap(true, Ordering::AcqRel) {
                eprintln!("tsfp-bevy: ui-pages: in-game menu open ({builder:#x})");
            }
        }
        // A front-end page on top (quit to the menus without a page-less frame in between) ends the session too.
        Some(builder)
            if descriptor(builder).is_some()
                && !PAUSE_OPTION_PAGES.contains(&builder)
                && !MAPMAKER_PAGES.contains(&builder) =>
        {
            if INGAME.swap(false, Ordering::AcqRel) {
                eprintln!("tsfp-bevy: ui-pages: in-game menu closed ({builder:#x} on top)");
            }
        }
        Some(_) => {}
    }
}

/// T2191: registered pages the pause menu opens that also exist in the front end (Controls, its accept page,
/// Audio/Video): they keep the in-game flag as they found it.
pub const PAUSE_OPTION_PAGES: &[u32] = &[0x2E_A8E0, 0x2E_9EC0, 0x2C_3E20];

#[cfg(not(tsfp_no_host))]
unsafe extern "C" {
    // src/xbox/guest_mem.c: the guest address of a synthetic physical address (D3D `Data` words).
    fn kmem_virtual_from_physical(physical: u32, out_address: *mut u32) -> bool;
    // src/host/rust_lane_call.c (T2024 slice 3): the guest address holding `length` texel bytes of a D3D `Data` word, the
    // way the render trace finds them (`0x40000000 | Data` first, the Register mask of the game heap), 0 when none.
    fn tsfp_render_trace_virtual_of_physical(physical: u32, length: u32) -> u32;
}
#[cfg(tsfp_no_host)]
unsafe fn kmem_virtual_from_physical(_physical: u32, _out_address: *mut u32) -> bool {
    false
}
#[cfg(tsfp_no_host)]
unsafe fn tsfp_render_trace_virtual_of_physical(_physical: u32, _length: u32) -> u32 {
    0
}

/// T2311: the pixels of a slot texture (`0x81220`, not in the disc's named texture table) from the
/// guest's texture memory: its D3D `Data` word mapped to a guest address, the base level decoded by
/// the replayer's NV2A decoder. Read only.
fn slot_image(g: &dyn GuestUi, texture: &list_rows_live::SlotTexture) -> Option<GuestImage> {
    let length = tsfp_wgpu::texture::data_length(texture.format, texture.size)?;
    if length == 0 || length > 1 << 20 {
        return None;
    }
    // T2315: the texels live at `0x40000000 | Data` (the game heap's Register mask, as the render trace reads them); the
    // plain physical lookup of T2311 lands on zero bytes for the player icon (slot 0x16, `mm_cursor23`).
    // SAFETY: lookups in the host's region table under its own lock; `address` is a plain out value.
    let mut address =
        unsafe { tsfp_render_trace_virtual_of_physical(texture.data & 0x0FFF_FFFF, length) };
    if address == 0 && !unsafe { kmem_virtual_from_physical(texture.data, &mut address) } {
        return None;
    }
    let mut bytes = Vec::with_capacity(length as usize + 3);
    for offset in (0..length).step_by(4) {
        bytes.extend_from_slice(&g.read32(address + offset)?.to_le_bytes());
    }
    // An all-transparent result is no picture (T2311 measured all-zero bytes through the physical lookup alone).
    let decoded = tsfp_wgpu::texture::decode(texture.format, texture.size, &bytes)?;
    if !decoded.rgba.chunks(4).any(|texel| texel[3] != 0) {
        if std::env::var_os("TSFP_UI_DRAW_LOG").is_some() {
            eprintln!(
                "tsfp-bevy: ui-pages:   slot texture {:#x} (data {:#x} at {address:#x}, format {:#x}): no texels readable, icon not drawn",
                texture.handle, texture.data, texture.format
            );
        }
        return None;
    }
    Some(GuestImage {
        key: texture.key(),
        width: decoded.width,
        height: decoded.height,
        rgba: Arc::new(decoded.rgba),
    })
}

/// T1916 increment 2: the default page style of the page in `slot` (`tsfp_ui_input::default_style_live`), with the
/// pulse constants of `0x72B70` and the pointer picture (`page +0x4C`).
fn default_page(
    g: &dyn GuestUi,
    slot: u32,
    live: &LivePage,
    controls: Option<tsfp_ui_model::mapmaker_controls::ControlsDiagram>,
) -> Option<DefaultPage> {
    use tsfp_ui_model::default_style as ds;
    let words = default_style_live::read(g, slot)?;
    let float = |va: u32| g.read32(va).map(f32::from_bits).filter(|v| v.is_finite());
    // 0x72B70: index = frame / [0x4E8CEC] * [0x475C94] * [0x475CE0] + [0x475CD4] into the 2048-entry sine table,
    // alpha factor (sin + 1) * [0x4765E0] + [0x475CD4].
    let period_word = g
        .read32(default_style_live::addr::PULSE_PERIOD)
        .unwrap_or(0) as i32;
    let step = float(0x47_5C94).unwrap_or(0.0) * float(0x47_5CE0).unwrap_or(0.0);
    let pulse_period = if period_word > 0 && step > 0.0 {
        2048.0 * period_word as f32 / step
    } else {
        0.0
    };
    let highlight = ds::Highlight {
        bar: PackedColor(words.bar),
        edge_top: PackedColor(words.edge_top),
        edge_bottom: PackedColor(words.edge_bottom),
        full_width: words.bar_left.map(|left| (left, words.page_box.x1)),
        pulse_base: float(0x47_5CD4).unwrap_or(1.0),
        pulse_amplitude: float(0x47_65E0).unwrap_or(0.0),
        pulse_period,
    };
    let mut style = ds::DefaultStyle {
        page_box: words.page_box,
        frame: words
            .frame_shown()
            .then(|| ds::FrameColours(words.frame_colours())),
        highlight,
        text: ds::TextColours {
            normal: ds::TEXT_NORMAL,
            focused: ds::TEXT_FOCUSED,
            greyed: ds::TEXT_GREYED,
        },
        text_inset: ds::TEXT_INSET,
        text_drop: ds::TEXT_DROP,
        // Widget ids are the row indices (`live_page::widget_id`).
        greyed: live
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.flags & guest_layout::row::FLAG_GREYED != 0)
            .map(|(index, _)| live_page::widget_id(index))
            .collect(),
    };
    // The pointer stays the original's: MEASURED live (increment 2, first build), the guest pointer is still drawn with
    // this page's original draw skipped (the pointer sprite of the parent page's draw), so a Rust pointer here would be a
    // second one. The model's pointer path (`default_style::pointer`) is for the Rust-owned pointer.
    let pointer: Option<ds::PointerImage> = None;
    if std::env::var_os("TSFP_UI_DRAW_LOG").is_some() {
        eprintln!(
            "tsfp-bevy: mapmaker-ui: default style slot {slot}: style {:#x} word {:#x} flags {:#x} box {:?} fade {} colours {:#x} frame {:x?} bar {:#x} top {:#x} bottom {:#x} bar_left {:?} pulse base {} amp {} period {} (word {period_word}, step {step}) consts c78 {:?} cc0 {:?} pointer {:?}",
            words.style,
            words.style_word,
            words.flags,
            words.page_box,
            words.fade,
            words.colours,
            words.frame,
            words.bar,
            words.edge_top,
            words.edge_bottom,
            words.bar_left,
            style.highlight.pulse_base,
            style.highlight.pulse_amplitude,
            pulse_period,
            float(0x47_5C78),
            float(0x47_5CC0),
            pointer.as_ref().map(|p| (p.image.key.clone(), p.size)),
        );
    }
    let mut widgets = tsfp_ui_input::default_widgets_live::read(g, live, slot, words.colours);
    if controls.is_some() {
        // Scope2 retains the original child framework background and later
        // parent chrome. A native child frame at the late layer would cover
        // that chrome (rejected live experiment, T1916). Controls labels also
        // extend outside the generic row viewport.
        style.frame = None;
        widgets.clip = None;
        widgets.scroll_bar = None;
    }
    widgets.controls = controls;
    // Slot textures of the widgets (the tile light bulb): their pixels from the guest's texture memory.
    for name in widgets.texture_names() {
        if let Some(slot_index) = name
            .strip_prefix("slot:")
            .and_then(|n| n.parse::<u32>().ok())
            && let Some(image) =
                list_rows_live::slot_texture(g, slot_index).and_then(|t| slot_image(g, &t))
        {
            widgets.images.push(image);
        }
    }
    Some(DefaultPage {
        style,
        pointer,
        widgets,
    })
}

/// T1916 increment 2: the Map Maker pointer of this guest frame (design units), `None` when the guest does not show
/// it (UI context `+0xF8` not 1 or 3). Bits of x and y, `u64::MAX` for none.
static POINTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(u64::MAX);

/// The pointer to draw over a default-style page (see [`POINTER`]).
pub fn pointer() -> Option<(f32, f32)> {
    let bits = POINTER.load(Ordering::Acquire);
    (bits != u64::MAX).then(|| {
        (
            f32::from_bits((bits >> 32) as u32),
            f32::from_bits(bits as u32),
        )
    })
}

/// T1916: the guest frame counter `[0x7497A8]` at the last frame boundary (the default style's pulse clock).
static GUEST_FRAME: AtomicU32 = AtomicU32::new(0);

pub fn guest_frame() -> u64 {
    u64::from(GUEST_FRAME.load(Ordering::Acquire))
}

fn track_pointer(g: &dyn GuestUi) {
    GUEST_FRAME.store(
        g.read32(default_style_live::addr::FRAME_COUNTER)
            .unwrap_or(0),
        Ordering::Release,
    );
    let shown = tsfp_ui_input::editor_pointer::current_context(g)
        .and_then(|c| g.read32(c + default_style_live::addr::CONTEXT_POINTER_SHOWN))
        .is_some_and(|state| state == 1 || state == 3);
    let bits = match tsfp_ui_input::editor_pointer::read(g).filter(|_| shown) {
        Some((x, y)) => (u64::from(x.to_bits()) << 32) | u64::from(y.to_bits()),
        None => u64::MAX,
    };
    POINTER.store(bits, Ordering::Release);
}

pub fn descriptor(builder: u32) -> Option<&'static PageDescriptor> {
    PAGES.iter().find(|p| p.builder == builder)
}

/// The registered page that takes native input, other than the main menu (it has its own
/// `LiveMenu` path in ui_live.rs).
pub fn input_page(builder: u32) -> Option<&'static str> {
    input_page_with_footer(builder, crate::ui_back::enabled())
}
fn input_page_with_footer(builder: u32, footer: bool) -> Option<&'static str> {
    descriptor(builder)
        .filter(|_| {
            !requires_registered_mapmaker_input(builder)
                || (TOP.load(Ordering::Acquire) == builder
                    && current().is_some_and(|page| page.live.builder == builder))
        })
        .filter(|p| u64::from(p.builder) != tsfp_ui_model::frontend::MAIN_MENU_BUILDER)
        .filter(|_| !matches!(builder, 0x2C_A230 | 0x2C_BFC0) || footer)
        .map(|p| p.name)
}

/// Why a registered page keeps the original draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrawBlock {
    /// Rows with these draw callbacks have no Rust implementation yet.
    RowDraws(Vec<u32>),
    NoRows,
    /// T1916: the page record has its own page draw (descriptor draw, page `+0x10`), not ported.
    PageDraw(u32),
    /// T1916 increment 3: default-style rows whose widget (`(row type, kind)`) has no Rust draw yet.
    Widgets(Vec<(u32, u32)>),
}

/// Whether the Rust drawer can draw `live` in full.
pub fn drawable(live: &LivePage) -> Result<(), DrawBlock> {
    drawable_with(live, &[])
}

/// [`drawable`] with the extra row draws `extra` (the default style's widget draws, `default_widgets_live::ROW_DRAWS`).
pub fn drawable_with(live: &LivePage, extra: &[u32]) -> Result<(), DrawBlock> {
    if live.rows.is_empty() {
        return Err(DrawBlock::NoRows);
    }
    let missing: Vec<u32> = live
        .draw_callbacks()
        .into_iter()
        .filter(|d| !RUST_ROW_DRAWS.contains(d) && !extra.contains(d))
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(DrawBlock::RowDraws(missing))
    }
}

/// T1916: the descriptor's page draw in the page record (descriptor `{builder, gate, draw, thunk}` copied from
/// `+0x08`; INFERRED offset, logged live).
pub const PAGE_DRAW: u32 = 0x10;
/// T1916: style record callbacks read by 0x76810 (MEASURED from the lift): `+0xC4` page frame `(page)`, `+0xD8` focused
/// row highlight `(page, row, rect)`; also `+0xC8`, `+0xCC`, `+0xD0` (pointer). Front-end style: 0x2D8060, 0x2D7AE0
/// (`+0xCC`), 0x2D9390 (MEASURED live).
pub const STYLE_FRAME: u32 = 0xC4;
pub const STYLE_HIGHLIGHT: u32 = 0xD8;

/// The top registered page, as read at the last frame boundary.
#[derive(Clone)]
pub struct Current {
    pub descriptor: PageDescriptor,
    pub slot: u32,
    pub live: LivePage,
    pub content: Arc<LiveContent>,
    pub focused: Option<WidgetId>,
    /// Typed Rust operation attached to the registered Map Maker progress page.
    pub mapmaker_storage_progress: Option<tsfp_mapmaker::ui::StorageProgress>,
    /// Drawn by Rust (every row draw has a Rust implementation).
    pub drawable: bool,
    /// Cursor-row nodes in cursor order (node types 4, 7, 8 on the `+0x8` chain).
    pub cursor_nodes: Vec<u32>,
    pub hit: Option<Arc<ModelLayout>>,
    /// Guest words outside the page record the content was built from ([`volatile_words`]).
    pub volatile: [u32; 10],
}

struct Registry {
    controls: tsfp_ui_input::mapmaker_controls_live::ControlsSession,
    texts: TextIndex,
    current: Option<Current>,
    /// Builders whose legacy-draw reason was logged.
    logged: Vec<u32>,
}

static REGISTRY: Mutex<Option<Registry>> = Mutex::new(None);
/// The builder of the top registered page (0 = none), for lock-free queries.
static TOP: AtomicU32 = AtomicU32::new(0);

/// The page slot record address of `slot` (the widget 0x76810 receives for a page).
pub fn slot_record(slot: u32) -> u32 {
    guest_layout::PAGE_TABLE + slot * guest_layout::PAGE_STRIDE
}

/// Top active page slot; a modal can reuse a lower slot than its parent (T3853).
fn top_slot(g: &dyn GuestUi) -> Option<u32> {
    tsfp_ui_input::native_page::top_page(g)
        .and_then(|_| tsfp_ui_input::native_page::input_page(g))
        .map(|p| (p.0 - guest_layout::PAGE_TABLE) / guest_layout::PAGE_STRIDE)
}

/// `0x7A080` allocates a type-2 child with no builder descriptor. Recognise only
/// its measured Yes/No activation pair over a registered Map Maker parent.
fn popup_descriptor(g: &dyn GuestUi, slot: u32) -> Option<&'static PageDescriptor> {
    let page = tsfp_ui_input::native_page::PageRef(slot_record(slot));
    if tsfp_ui_input::mapmaker_handlers::is_mode_choice_popup(g, page) {
        return descriptor(0x7_A080);
    }
    if g.read32(tsfp_ui_input::editor_pointer::addr::UI_MODE)
        != Some(tsfp_ui_input::editor_pointer::addr::MODE_MAPMAKER)
    {
        return None;
    }
    // T2333's direct Clear Map prompt belongs to the editor menu, has text
    // key 0xAE1, and installs only the measured Yes callback 0x311AA0; No is
    // the zero callback. Keep it separate from Editor Settings' mode-change
    // prompt, whose Yes/No pair is 0x312120 / 0x361710.
    if let Some(clear_map_popup) = tsfp_ui_input::native_page::popup_over(g, &[0x31_3010])
        && clear_map_popup.0 == slot_record(slot)
        && g.read32(clear_map_popup.0) == Some(2)
    {
        if popup_handlers(g, clear_map_popup)
            .is_some_and(|handlers| is_direct_clear_map_popup_handlers(&handlers))
        {
            return descriptor(0x7_A080);
        }
    }
    // T1688/T2333: Editor Settings' mode-change confirmation is a descriptor-less
    // child with its separate exact Yes/No callback pair.
    if let Some(editor_settings_popup) = tsfp_ui_input::native_page::popup_over(g, &[0x31_2270])
        && editor_settings_popup.0 == slot_record(slot)
        && g.read32(editor_settings_popup.0) == Some(2)
    {
        if popup_handlers(g, editor_settings_popup).is_some_and(|handlers| {
            handlers == [0x31_2120, 0x36_1710] || handlers == [0x36_1710, 0x31_2120]
        }) {
            return descriptor(0x7_A080);
        }
    }
    let popup = tsfp_ui_input::native_page::popup_over(g, MAPMAKER_PAGES)?;
    if popup.0 != slot_record(slot) || g.read32(popup.0) != Some(2) {
        return None;
    }
    (popup_handlers(g, popup)? == [0x7_9730, 0x7_9790])
        .then(|| descriptor(0x7_A080))
        .flatten()
}

/// The row activation handlers of `popup` in row order; `None` for a cycle or a truncated walk (not a recognised
/// dialog).
fn popup_handlers(g: &dyn GuestUi, popup: tsfp_ui_input::native_page::PageRef) -> Option<Vec<u32>> {
    let mut node = g.read32(popup.0 + guest_layout::page::FIRST_NODE)?;
    let mut seen = Vec::new();
    let mut handlers = Vec::new();
    while node != 0 && seen.len() < 256 && !seen.contains(&node) {
        seen.push(node);
        let handler = g.read32(node + guest_layout::row::ON_ACTIVATE).unwrap_or(0);
        if handler != 0 {
            handlers.push(handler);
        }
        node = g.read32(node + 4).unwrap_or(0);
    }
    (node == 0).then_some(handlers)
}

fn is_direct_clear_map_popup_handlers(handlers: &[u32]) -> bool {
    handlers == [0x31_1AA0]
}

/// Guest thread, frame boundary: read the top page if it is registered.
pub fn service(g: &mut dyn GuestUi) {
    track_pointer(g);
    let Ok(mut guard) = REGISTRY.lock() else {
        return;
    };
    let registry = guard.get_or_insert_with(|| Registry {
        controls: Default::default(),
        texts: TextIndex::default(),
        current: None,
        logged: Vec::new(),
    });
    let top = top_slot(g)
        .and_then(|slot| {
            let builder = g.read32(slot_record(slot) + guest_layout::page::BUILDER)?;
            descriptor(builder)
                .filter(|_| mapmaker_page_enabled(g, builder, slot))
                .filter(|_| {
                    !crate::mapmaker_set_lists_live::is_candidate_builder(builder)
                        || crate::mapmaker_set_lists_live::registered(builder)
                })
                .filter(|_| !matches!(builder, 0x2C_A230 | 0x2C_BFC0) || crate::ui_back::enabled())
                .or_else(|| popup_descriptor(g, slot))
                .map(|d| (slot, *d))
        })
        .or_else(|| {
            if !crate::ui_back::enabled() {
                return None;
            }
            let parents: Vec<u32> = PAGES
                .iter()
                .map(|p| p.builder)
                .chain(
                    crate::ui_live::NATIVE_INPUT_PAGES
                        .iter()
                        .map(|(builder, _)| *builder),
                )
                .collect();
            let popup = tsfp_ui_input::native_page::popup_over(g, &parents)?;
            let slot = (0..tsfp_ui_input::native_page::PAGE_SLOTS)
                .find(|slot| slot_record(*slot) == popup.0)?;
            let builder = g.read32(popup.0 + guest_layout::page::BUILDER)?;
            Some((
                slot,
                PageDescriptor {
                    builder,
                    name: "Native popup",
                },
            ))
        });
    let observed = top.map(|(slot, page)| (page.builder, slot));
    crate::mapmaker_editor_live::observe_top(observed);
    crate::mapmaker_action_picker_live::observe_top(observed);
    crate::mapmaker_awards_live::observe_top(observed);
    crate::mapmaker_award_flags_live::observe_top(observed);
    crate::mapmaker_award_table_live::observe_top(observed);
    crate::mapmaker_kind_list_live::observe_top(observed);
    crate::mapmaker_award_medal_live::observe_top(observed);
    crate::mapmaker_object_kind_live::observe_top(observed);
    crate::mapmaker_kind_picker_live::observe_top(observed);
    crate::mapmaker_selected_entry_live::observe_top(observed);
    crate::mapmaker_controls_ui_live::observe_top(observed);
    crate::mapmaker_assault_options_live::observe_top(observed);
    crate::mapmaker_assault_phase_assignment_live::observe_top(observed);
    crate::mapmaker_logic_summary_live::observe_top(observed);
    crate::mapmaker_trigger_selection_live::observe_top(observed);
    crate::mapmaker_tool_menu_live::observe_top(observed);
    crate::mapmaker_sample_map_menu_live::observe_top(observed);
    crate::mapmaker_preview_live::observe_top(observed);
    crate::mapmaker_palette_grid_live::observe_top(observed);
    crate::mapmaker_editor_settings_live::observe_top(observed);
    crate::mapmaker_disabled_modes_live::observe_top(observed);
    crate::mapmaker_palette_properties_live::observe_top(observed);
    crate::mapmaker_settings_live::observe_top(observed);
    crate::mapmaker_description_live::observe_guest(g);
    crate::mapmaker_set_lists_live::observe_top(observed);
    let advanced_default_page = top.and_then(|(slot, page)| {
        let page_ref = tsfp_ui_input::native_page::PageRef(slot_record(slot));
        let direct_prompt = tsfp_ui_input::mapmaker_handlers::is_mode_choice_popup(g, page_ref);
        let alias_prompt = page.builder == 0x2B_F770
            && mapmaker_alias_enabled(g, page.builder, slot)
            && mapmaker_mode_prompt_rows(g, slot);
        (direct_prompt || alias_prompt).then_some(slot_record(slot))
    });
    crate::ui_live::queue_mapmaker_advanced_default(advanced_default_page);
    if top.is_none()
        && let Some(slot) = top_slot(g)
        && g.read32(tsfp_ui_input::editor_pointer::addr::UI_MODE)
            == Some(tsfp_ui_input::editor_pointer::addr::MODE_MAPMAKER)
    {
        // T1916: unregistered pages seen in the Map Maker (the next ones to register), once each.
        let builder = g
            .read32(slot_record(slot) + guest_layout::page::BUILDER)
            .unwrap_or(0);
        if !registry.logged.contains(&builder) {
            registry.logged.push(builder);
            eprintln!(
                "tsfp-bevy: mapmaker-ui: unregistered page builder {builder:#x} slot {slot} id {:#x}",
                g.read32(slot_record(slot) + guest_layout::page::ID)
                    .unwrap_or(0)
            );
        }
    }
    let Some((slot, descriptor)) = top else {
        crate::ui_extensions::track_settings_page(None);
        registry.controls.clear();
        registry.current = None;
        TOP.store(0, Ordering::Release);
        return;
    };
    crate::ui_extensions::track_settings_page(Some(descriptor.builder));
    registry.texts.refresh(g);
    let Some(mut live) = live_page::read_page(g, &registry.texts, slot) else {
        registry.controls.clear();
        registry.current = None;
        TOP.store(0, Ordering::Release);
        return;
    };
    selector_geometry(g, &mut live);
    let cursor_nodes: Vec<u32> = PageState::load(g, slot)
        .map(|p| p.rows.iter().map(|r| r.node).collect())
        .unwrap_or_default();
    if descriptor.builder == tsfp_ui_input::mapmaker_logic_summary::BUILDER
        && !crate::mapmaker_logic_summary_live::supported(&live, &cursor_nodes)
    {
        registry.current = None;
        registry.controls.clear();
        TOP.store(0, Ordering::Release);
        return;
    }
    if descriptor.builder == tsfp_ui_input::mapmaker_action_picker::BUILDER
        && !crate::mapmaker_action_picker_live::supported(&live, &cursor_nodes)
    {
        registry.current = None;
        registry.controls.clear();
        TOP.store(0, Ordering::Release);
        return;
    }
    if descriptor.builder == tsfp_ui_input::mapmaker_kind_picker::BUILDER
        && !crate::mapmaker_kind_picker_live::supported(&live, &cursor_nodes)
    {
        registry.current = None;
        registry.controls.clear();
        TOP.store(0, Ordering::Release);
        return;
    }
    if descriptor.builder == tsfp_ui_input::mapmaker_trigger_selection::BUILDER
        && !crate::mapmaker_trigger_selection_live::supported(&live, &cursor_nodes)
    {
        registry.current = None;
        registry.controls.clear();
        TOP.store(0, Ordering::Release);
        return;
    }
    if descriptor.builder == crate::mapmaker_assault_phase_assignment_live::BUILDER
        && !crate::mapmaker_assault_phase_assignment_live::supported(&live, &cursor_nodes)
    {
        registry.current = None;
        registry.controls.clear();
        TOP.store(0, Ordering::Release);
        return;
    }
    if descriptor.builder == tsfp_ui_input::mapmaker_awards::BUILDER
        && !crate::mapmaker_awards_live::supported(&live, &cursor_nodes)
    {
        registry.current = None;
        registry.controls.clear();
        TOP.store(0, Ordering::Release);
        return;
    }
    if descriptor.builder == tsfp_ui_input::mapmaker_award_flags::BUILDER
        && !crate::mapmaker_award_flags_live::supported(&live, &cursor_nodes)
    {
        registry.current = None;
        registry.controls.clear();
        TOP.store(0, Ordering::Release);
        return;
    }
    if descriptor.builder == tsfp_ui_input::mapmaker_award_table::BUILDER
        && !crate::mapmaker_award_table_live::supported(&live, &cursor_nodes)
    {
        registry.current = None;
        registry.controls.clear();
        TOP.store(0, Ordering::Release);
        return;
    }
    if descriptor.builder == tsfp_ui_input::mapmaker_kind_list::BUILDER
        && !crate::mapmaker_kind_list_live::supported(&live, &cursor_nodes)
    {
        registry.current = None;
        registry.controls.clear();
        TOP.store(0, Ordering::Release);
        return;
    }
    if descriptor.builder == tsfp_ui_input::mapmaker_award_medal::BUILDER
        && !crate::mapmaker_award_medal_live::supported(&live, &cursor_nodes)
    {
        registry.current = None;
        registry.controls.clear();
        TOP.store(0, Ordering::Release);
        return;
    }
    if descriptor.builder == tsfp_ui_input::mapmaker_object_kind::BUILDER
        && !crate::mapmaker_object_kind_live::supported(&live, &cursor_nodes)
    {
        registry.current = None;
        registry.controls.clear();
        TOP.store(0, Ordering::Release);
        return;
    }
    if descriptor.builder == tsfp_ui_input::mapmaker_tool_menu::BUILDER
        && !crate::mapmaker_tool_menu_live::supported(&live, &cursor_nodes)
    {
        registry.current = None;
        registry.controls.clear();
        TOP.store(0, Ordering::Release);
        return;
    }
    if descriptor.builder == tsfp_ui_input::mapmaker_sample_map_menu::BUILDER
        && !crate::mapmaker_sample_map_menu_live::supported(&live, &cursor_nodes)
    {
        registry.current = None;
        registry.controls.clear();
        TOP.store(0, Ordering::Release);
        return;
    }
    let native_settings = crate::ui_extensions::prepare_settings_page(descriptor.builder, g);
    if native_settings {
        for (cursor_row, node) in cursor_nodes.iter().enumerate() {
            let Some(value) =
                crate::ui_extensions::settings_row_value(descriptor.builder, cursor_row)
            else {
                continue;
            };
            let Some(row) = live.rows.iter_mut().find(|row| row.node == *node) else {
                continue;
            };
            let value = match value {
                tsfp_ui_input::UiValue::Bool(value) => u32::from(value),
                tsfp_ui_input::UiValue::Index(value) => value,
                tsfp_ui_input::UiValue::Int(value) if value >= 0 => value as u32,
                tsfp_ui_input::UiValue::Int(_) => continue,
                tsfp_ui_input::UiValue::Float(_) | tsfp_ui_input::UiValue::None => continue,
            };
            row.selected = row
                .options
                .iter()
                .position(|option| option.id == value)
                .unwrap_or(value as usize)
                .min(u32::MAX as usize) as u32;
        }
    }
    TOP.store(descriptor.builder, Ordering::Release);
    // T2316: opt-in until a real callback objective has frame/trace evidence.
    let progress = if std::env::var("TSFP_UI_OBJECTIVE_CALLBACKS").is_ok_and(|v| v == "1") {
        tsfp_ui_input::ingame_live::sample_progress(g, &live)
    } else {
        None
    };
    let mut volatile = volatile_words(g, slot);
    if native_settings {
        volatile[4] ^= crate::ui_extensions::settings_fingerprint(descriptor.builder);
    }
    volatile[5] ^= progress.as_ref().map_or(0, |p| p.checksum());
    let highlight_edges = live_page::highlight_edges(g, slot);
    let page_draw = g.read32(slot_record(slot) + PAGE_DRAW).unwrap_or(0);
    let controls_draw = page_draw == tsfp_ui_model::mapmaker_controls::PAGE_DRAW;
    let controls = if controls_draw {
        registry.controls.sample(g, slot_record(slot), &live)
    } else {
        registry.controls.clear();
        None
    };
    if !controls_draw
        && registry.current.as_ref().is_some_and(|c| {
            c.slot == slot
                && c.live == live
                && c.volatile == volatile
                && !crate::mapmaker_disabled_modes_live::should_refresh(descriptor.builder, slot)
                && c.content
                    .model
                    .page(c.content.page)
                    .is_some_and(|page| page.highlight_edges == highlight_edges)
        })
    {
        return;
    }
    let page_draw = g.read32(slot_record(slot) + PAGE_DRAW).unwrap_or(0);
    let style = g
        .read32(slot_record(slot) + guest_layout::page::STYLE)
        .unwrap_or(0);
    let style_draws = [STYLE_FRAME, STYLE_HIGHLIGHT].map(|o| g.read32(style + o).unwrap_or(0));
    // T1916 increment 2: a style with no frame or highlight callback (the Map Maker pages) is drawn with the framework
    // default style (`tsfp_ui_model::default_style`), not the front-end skin.
    let mut default_page = if style_draws == [0, 0] {
        default_page(g, slot, &live, controls.clone())
    } else {
        None
    };
    // Final paired readback retains a parent-chrome mismatch (T1916). Keep
    // this measured candidate opt-in until the attachment contract is fixed.
    let controls_admitted = std::env::var("TSFP_UI_MAPMAKER_CONTROLS").is_ok_and(|v| v == "1");
    let palette_properties_candidate = default_page.is_some()
        && crate::mapmaker_palette_properties_live::native_draw_candidate(
            &live,
            slot,
            &cursor_nodes,
        );
    let mut verdict = if controls_draw && (!controls_admitted || controls.is_none()) {
        Err(DrawBlock::PageDraw(page_draw))
    } else if page_draw != 0
        && !page_draw_supported(descriptor.builder, page_draw, palette_properties_candidate)
    {
        Err(DrawBlock::PageDraw(page_draw))
    } else if style_draws == [0, 0] && default_page.is_none() {
        Err(DrawBlock::NoRows)
    } else if controls.is_some() {
        // Exact 18 type-7/kind-1 rows were validated by the complete custom
        // diagram reader; its label and selected value replace generic kind 1.
        drawable_with(&live, tsfp_ui_input::default_widgets_live::ROW_DRAWS)
    } else if default_page.is_some() {
        let missing = tsfp_ui_input::default_widgets_live::unsupported(&live);
        if missing.is_empty() {
            drawable_with(&live, tsfp_ui_input::default_widgets_live::ROW_DRAWS)
        } else {
            Err(DrawBlock::Widgets(missing))
        }
    } else {
        drawable(&live)
    };
    if let Err(block) = &verdict
        && !registry.logged.contains(&descriptor.builder)
    {
        registry.logged.push(descriptor.builder);
        eprintln!(
            "tsfp-bevy: ui-pages: {} {:#x}: legacy draw ({block:?}), native input",
            descriptor.name, descriptor.builder
        );
    }
    let page_id = PageId(u64::from(descriptor.builder));
    let (mut page, dynamic) = live.to_model_page(page_id);
    page.panel_bounds = live_page::panel_bounds(g, slot);
    page.highlight_edges = highlight_edges;
    if default_page.is_some() {
        // T1916 increment 3: group boxes (row type 5) draw their own title (`default_widgets`), not the row text.
        for (index, row) in live.rows.iter().enumerate() {
            if row.node_type == 5
                && let Some(w) = page
                    .widgets
                    .iter_mut()
                    .find(|w| w.id == live_page::widget_id(index))
            {
                w.text = tsfp_ui_model::model::TextRef::None;
            }
        }
    }
    if controls.is_some() {
        // Labels/options of the first 18 nodes are drawn by the curve callback,
        // not by the framework generic text or focused-row bar.
        for w in &mut page.widgets {
            if w.id.0 < 18 {
                w.text = tsfp_ui_model::model::TextRef::None;
                w.options.clear();
            }
        }
    }
    let mut list = if controls.is_some() {
        None
    } else {
        list_rows_live::read_list_rows_with_progress(g, &live, slot, progress.as_ref())
    };
    if native_settings
        && let Some(list) = &mut list
        && let Some(options) = &mut list.options
    {
        if descriptor.builder == 0x2C_3E20
            && let Some(audio) = crate::ui_extensions::settings_audio_video()
        {
            for (live_row, draw_row) in live
                .rows
                .iter()
                .filter(|row| {
                    row.draw == tsfp_ui_model::option_rows::SLIDER_ROW_DRAW
                        || row.draw == tsfp_ui_model::option_rows::ICON_ROW_DRAW
                })
                .zip(options.rows.iter_mut())
            {
                let value = match live_row.id {
                    1 => Some(audio.music_volume as f32 / 16.0),
                    3 => Some(audio.sound_volume as f32 / 16.0),
                    _ => None,
                };
                if let (Some(value), Some(slider)) = (value, draw_row.slider.as_mut()) {
                    slider.value = value;
                }
            }
        }
        if descriptor.builder == 0x2E_A8E0
            && let Some(tsfp_ui_input::UiValue::Float(speed)) =
                crate::ui_extensions::settings_row_value(descriptor.builder, 9)
            && let Some(slider) = options.text_sliders.first_mut()
        {
            slider.value = speed;
        }
    }
    // T2315 increment 4: the in-game rows are drawn by Rust only in the modes their reader ports.
    let ingame_draws = [
        tsfp_ui_model::ingame_rows::SCOREBOARD_ROW_DRAW,
        tsfp_ui_model::ingame_rows::OBJECTIVES_ROW_DRAW,
    ];
    if verdict.is_ok()
        && live.rows.iter().any(|r| ingame_draws.contains(&r.draw))
        && list.as_ref().is_none_or(|l| l.ingame.is_none())
    {
        verdict = Err(DrawBlock::RowDraws(
            live.rows
                .iter()
                .map(|r| r.draw)
                .filter(|d| ingame_draws.contains(d))
                .collect(),
        ));
        if !registry.logged.contains(&descriptor.builder) {
            registry.logged.push(descriptor.builder);
            eprintln!(
                "tsfp-bevy: ui-pages: {} {:#x}: legacy draw (mode not ported), native input",
                descriptor.name, descriptor.builder
            );
        }
    }
    if let Some(list) = &mut list
        && list.summary.as_ref().is_some_and(|s| s.icon.is_some())
        && let Some(image) =
            list_rows_live::slot_texture(g, PLAYER_ICON_SLOT).and_then(|t| slot_image(g, &t))
    {
        list.images.push(image);
    }
    if let Some(list) = &mut list
        && let Some(options) = &list.options
    {
        // T2315: Controls button pictures whose handle is not in the named texture table come from texture memory.
        let keys: Vec<String> = options
            .pictures
            .iter()
            .flat_map(|p| p.textures.iter().map(|(_, name)| name.clone()))
            .filter(|k| k.starts_with("slot:"))
            .collect();
        for key in keys {
            let Some(slot) = key
                .strip_prefix("slot:")
                .and_then(|n| n.parse::<u32>().ok())
            else {
                continue;
            };
            if let Some(image) =
                list_rows_live::handle_texture(g, slot.wrapping_sub(0x1_0000), slot)
                    .and_then(|t| slot_image(g, &t))
                && !list.images.iter().any(|i| i.key == image.key)
            {
                list.images.push(image);
            }
        }
    }
    if let Some(list) = &list {
        // Level names are drawn upper-cased (`0x40F00` in `0x2CA9F0`).
        for row in &list.rows {
            if matches!(row.kind, ListRowKind::Level { .. })
                && let Some(w) = page.widgets.iter_mut().find(|w| w.id == row.widget)
            {
                w.uppercase = true;
            }
        }
    }
    let cursor_nodes: Vec<u32> = PageState::load(g, slot)
        .map(|p| p.rows.iter().map(|r| r.node).collect())
        .unwrap_or_default();
    if let Some(owned_page) = crate::mapmaker_editor_live::project_owned_editor_menu_page(
        &live,
        slot,
        &cursor_nodes,
        live.focused(),
        page.clone(),
    ) {
        page = owned_page;
    }
    if descriptor.builder == 0x7_A080
        && let Some(owned_page) =
            crate::mapmaker_editor_live::project_owned_clear_map_confirmation_page(
                &live,
                &cursor_nodes,
                page.clone(),
            )
    {
        page = owned_page;
    }
    if descriptor.builder == 0x7_A080
        && let Some(owned_page) =
            crate::mapmaker_editor_live::project_owned_quit_map_confirmation_page(
                &live,
                &cursor_nodes,
                page.clone(),
            )
    {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_assault_options_live::project_owned_page(
        &live,
        slot,
        &cursor_nodes,
        live.focused(),
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_assault_phase_assignment_live::project_owned_page(
        &live,
        slot,
        &cursor_nodes,
        live.focused(),
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_logic_summary_live::project_owned_page(
        &live,
        slot,
        &cursor_nodes,
        live.focused(),
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_action_picker_live::project_owned_page(
        &live,
        slot,
        &cursor_nodes,
        live.focused(),
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_kind_picker_live::project_owned_page(
        &live,
        slot,
        &cursor_nodes,
        live.focused(),
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_trigger_selection_live::project_owned_page(
        &live,
        slot,
        &cursor_nodes,
        live.focused(),
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_awards_live::project_owned_page(
        &live,
        slot,
        &cursor_nodes,
        live.focused(),
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_award_flags_live::project_owned_page(
        &live,
        slot,
        &cursor_nodes,
        live.focused(),
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_award_table_live::project_owned_page(
        &live,
        slot,
        &cursor_nodes,
        live.focused(),
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_kind_list_live::project_owned_page(
        &live,
        slot,
        &cursor_nodes,
        live.focused(),
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_award_medal_live::project_owned_page(
        &live,
        slot,
        &cursor_nodes,
        live.focused(),
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_object_kind_live::project_owned_page(
        &live,
        slot,
        &cursor_nodes,
        live.focused(),
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_tool_menu_live::project_owned_page(
        &live,
        slot,
        &cursor_nodes,
        live.focused(),
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_sample_map_menu_live::project_owned_page(
        &live,
        slot,
        &cursor_nodes,
        live.focused(),
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_preview_live::project_owned_page(
        &live,
        slot,
        &cursor_nodes,
        live.focused(),
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_palette_grid_live::project_owned_page(
        &live,
        slot,
        &cursor_nodes,
        live.focused(),
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_palette_properties_live::project_owned_page(
        &live,
        slot,
        &cursor_nodes,
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(owned_page) = crate::mapmaker_settings_live::project_owned_map_settings_page(
        &live,
        &cursor_nodes,
        page.clone(),
    ) {
        page = owned_page;
    }
    if let Some(default_page) = default_page.as_mut() {
        crate::mapmaker_disabled_modes_live::project_owned_page(
            g,
            descriptor.builder,
            slot,
            &live,
            &cursor_nodes,
            default_page,
        );
    }
    if let Some(owned_page) =
        crate::mapmaker_editor_settings_live::project_owned_editor_settings_page(
            &live,
            slot,
            &cursor_nodes,
            page.clone(),
        )
    {
        page = owned_page;
    }
    if descriptor.builder == 0x7_A080
        && let Some(owned_page) =
            crate::mapmaker_editor_settings_live::project_owned_editor_kind_confirmation_page(
                &live,
                &cursor_nodes,
                page.clone(),
            )
    {
        page = owned_page;
    }
    if crate::mapmaker_set_lists_live::is_candidate_builder(descriptor.builder) {
        let Some(owned_page) = crate::mapmaker_set_lists_live::project_owned_page(
            descriptor.builder,
            slot,
            &live,
            &cursor_nodes,
            page.clone(),
        ) else {
            registry.current = None;
            TOP.store(0, Ordering::Release);
            return;
        };
        page = owned_page;
    }
    let owned_storage_progress = if descriptor.builder == 0x32_AEE0 {
        crate::storage_live::project_live_mapmaker_storage_progress(page.clone())
    } else {
        None
    };
    let mapmaker_storage_progress = owned_storage_progress.map(|progress| {
        page = progress.page;
        progress.progress
    });
    let hit = ModelLayout::from_page(&page, 640.0, 480.0).map(Arc::new);
    let mut model = UiModel::new();
    model.pages.insert(page_id, page);
    let first = registry
        .current
        .as_ref()
        .is_none_or(|c| c.descriptor != descriptor || c.slot != slot);
    if std::env::var_os("TSFP_UI_ROW_GEOM").is_some()
        && let Some(list) = &list
    {
        // T2315 item 4 diagnostic: the list view band and the scroll arrows at every change of the page content.
        let record = slot_record(slot);
        let style = g.read32(record + 0x1C).unwrap_or(0);
        eprintln!(
            "tsfp-bevy: ui-pages: scroll {} band {:?} arrows {:?} style {:#x} word0 {:#x} hidden {}",
            f32::from_bits(g.read32(record + 0x84).unwrap_or(0)),
            list.clip,
            list.arrows,
            style,
            g.read32(style).unwrap_or(0),
            g.read32(record + 0x40).unwrap_or(0)
        );
    }
    if first {
        eprintln!(
            "tsfp-bevy: ui-pages: {} {:#x} slot {slot} live: {} rows, {} cursor rows, {}",
            descriptor.name,
            descriptor.builder,
            live.rows.len(),
            cursor_nodes.len(),
            if verdict.is_ok() {
                "drawn by Rust in the engine frame"
            } else {
                "original draw"
            }
        );
        if std::env::var_os("TSFP_UI_ROW_DUMP").is_some() {
            // T1916 increment 3 diagnostic: every row of a newly shown page (type, kind, draw callback, rect, text).
            for r in &live.rows {
                eprintln!(
                    "tsfp-bevy: ui-pages:   row {:#x} type {} kind {} draw {:#x} flags {:#x} id {} rect {:?} text {:?} sel {} opts {} parent {:#x}",
                    r.node,
                    r.node_type,
                    r.kind,
                    r.draw,
                    r.flags,
                    r.id,
                    r.rect,
                    r.text,
                    r.selected,
                    r.options.len(),
                    g.read32(r.node + 0x88).unwrap_or(0)
                );
            }
        }
        if std::env::var_os("TSFP_UI_ROW_WORDS").is_some() {
            // T2315 diagnostic: the text placement fields of the default row draw 0x76150 (+0x30 align, +0x3C x, +0x40 y
            // offsets, +0x1C font, +0x6C width, +0x70 height, +0xC0) and its unit globals.
            let float = |va: u32| f32::from_bits(g.read32(va).unwrap_or(0));
            eprintln!(
                "tsfp-bevy: ui-pages: units [0x7BA038] {} [0x7BA188] {}",
                float(0x7B_A038),
                float(0x7B_A188)
            );
            for r in &live.rows {
                eprintln!(
                    "tsfp-bevy: ui-pages:   place {:#x} type {} align30 {} x3c {} y40 {} w6c {} h70 {} c0 {} font {} rect {:?}",
                    r.node,
                    r.node_type,
                    g.read32(r.node + 0x30).unwrap_or(0),
                    float(r.node + 0x3C),
                    float(r.node + 0x40),
                    float(r.node + 0x6C),
                    float(r.node + 0x70),
                    g.read32(r.node + 0xC0).unwrap_or(0),
                    r.font,
                    r.rect
                );
            }
        }
        if std::env::var_os("TSFP_UI_ROW_GEOM").is_some() {
            // T2315 item 4 diagnostic: the row geometry of 0x71E90 (row +0x64..+0x70 units, parent +0x88, its +0x6C/+0x80/
            // +0x84, page +0xA8, [0x7BA184]) and the scrolled-page fields of 0x76810 (+0x5C, +0x6C, +0x70).
            let float = |va: u32| f32::from_bits(g.read32(va).unwrap_or(0));
            let record = slot_record(slot);
            eprintln!(
                "tsfp-bevy: ui-pages: geom page style {:#x} style+c8 {:#x} style+d8 {:#x} a8 {} 5c {} 6c {} 70 {} 80 {} 84 {} e4..f0 {} {} {} {} y0 [0x7BA184] {} consts {} {} {}",
                g.read32(record + 0x1C).unwrap_or(0),
                g.read32(g.read32(record + 0x1C).unwrap_or(0) + 0xC8)
                    .unwrap_or(0),
                g.read32(g.read32(record + 0x1C).unwrap_or(0) + 0xD8)
                    .unwrap_or(0),
                float(record + 0xA8),
                float(record + 0x5C),
                float(record + 0x6C),
                float(record + 0x70),
                float(record + 0x80),
                float(record + 0x84),
                float(record + 0xE4),
                float(record + 0xE8),
                float(record + 0xEC),
                float(record + 0xF0),
                float(0x7B_A184),
                float(0x47_5C78),
                float(0x47_5D18),
                float(0x47_5DB4)
            );
            for r in &live.rows {
                let parent = g.read32(r.node + 0x88).unwrap_or(0);
                let colors = g.read32(r.node + 0x90).unwrap_or(0);
                let words: Vec<String> = (0..12u32)
                    .map(|i| format!("{:08x}", g.read32(colors + 4 * i).unwrap_or(0)))
                    .collect();
                eprintln!(
                    "tsfp-bevy: ui-pages:   colors {:#x} at {colors:#x}: {}",
                    r.node,
                    words.join(" ")
                );
                // T2315 3b: the value box and placement fields of `0x7B520` / `0x7B7B0` (units) and their globals.
                eprintln!(
                    "tsfp-bevy: ui-pages:   value {:#x} box {} {} {} {} dc {} e0 {} c0 {} b4 {} b8 {:#x} bc {} c4 {} c8 {} [0x7BA180] {} [0x7BA18C] {}",
                    r.node,
                    f32::from_bits(g.read32(r.node + 0xE4).unwrap_or(0)),
                    f32::from_bits(g.read32(r.node + 0xE8).unwrap_or(0)),
                    f32::from_bits(g.read32(r.node + 0xEC).unwrap_or(0)),
                    f32::from_bits(g.read32(r.node + 0xF0).unwrap_or(0)),
                    f32::from_bits(g.read32(r.node + 0xDC).unwrap_or(0)),
                    f32::from_bits(g.read32(r.node + 0xE0).unwrap_or(0)),
                    g.read32(r.node + 0xC0).unwrap_or(0),
                    g.read32(r.node + 0xB4).unwrap_or(0),
                    g.read32(r.node + 0xB8).unwrap_or(0),
                    g.read32(r.node + 0xBC).unwrap_or(0),
                    g.read32(r.node + 0xC4).unwrap_or(0) as i32,
                    g.read32(r.node + 0xC8).unwrap_or(0) as i32,
                    f32::from_bits(g.read32(0x7B_A180).unwrap_or(0)),
                    f32::from_bits(g.read32(0x7B_A18C).unwrap_or(0))
                );
                eprintln!(
                    "tsfp-bevy: ui-pages:   geom {:#x} flags28 {:#x} xywh {} {} {} {} parent {:#x} p6c {} p80 {} p84 {} box {} {} {} {}",
                    r.node,
                    g.read32(r.node + 0x28).unwrap_or(0),
                    float(r.node + 0x64),
                    float(r.node + 0x68),
                    float(r.node + 0x6C),
                    float(r.node + 0x70),
                    parent,
                    float(parent + 0x6C),
                    float(parent + 0x80),
                    float(parent + 0x84),
                    float(r.node + 0x78),
                    float(r.node + 0x7C),
                    float(r.node + 0x80),
                    float(r.node + 0x84)
                );
            }
        }
        if std::env::var_os("TSFP_UI_DRAW_LOG").is_some() {
            // T1916: the page style record (`+0x1C`) and its draw callbacks, read by 0x76810: +0xC4 page frame,
            // +0xC8, +0xCC, +0xD0, +0xD8 row highlight (`(page, row, rect)`).
            let style = g
                .read32(slot_record(slot) + guest_layout::page::STYLE)
                .unwrap_or(0);
            let calls: Vec<String> = [0xC4u32, 0xC8, 0xCC, 0xD0, 0xD4, 0xD8, 0xDC]
                .iter()
                .map(|o| format!("+{o:#x}={:#x}", g.read32(style + o).unwrap_or(0)))
                .collect();
            eprintln!(
                "tsfp-bevy: ui-pages: {:#x} style {style:#x} {}",
                descriptor.builder,
                calls.join(" ")
            );
        }
        if MAPMAKER_PAGES.contains(&descriptor.builder)
            && std::env::var_os("TSFP_UI_DRAW_LOG").is_some()
        {
            let record = slot_record(slot);
            eprintln!(
                "tsfp-bevy: mapmaker-ui: page {:#x} flags {:#x} page draw {page_draw:#x} gate {:#x} accept {:#x} back {:#x} pointer {:?} verdict {verdict:?}",
                descriptor.builder,
                g.read32(record + guest_layout::page::FLAGS).unwrap_or(0),
                g.read32(record + 0x0C).unwrap_or(0),
                g.read32(record + guest_layout::page::ON_ACCEPT)
                    .unwrap_or(0),
                g.read32(record + guest_layout::page::ON_BACK).unwrap_or(0),
                tsfp_ui_input::editor_pointer::read(g),
            );
            for r in &live.rows {
                eprintln!(
                    "tsfp-bevy: mapmaker-ui:   row {:#x} type {} id {} flags {:#x} draw {:#x} act {:#x} rect {:?} text {:?} font {} opts {}",
                    r.node,
                    r.node_type,
                    r.id,
                    r.flags,
                    r.draw,
                    r.activate,
                    r.rect,
                    r.text,
                    r.font,
                    r.options.len()
                );
            }
        }
        if let Some(list) = &list
            && std::env::var_os("TSFP_UI_DRAW_LOG").is_some()
        {
            eprintln!(
                "tsfp-bevy: ui-pages: list rows {}: text unit {} medal unit {} margin {} plain {} clip {:?} padlock {:?} glow {:?}",
                list.rows.len(),
                list.text_unit,
                list.medal_unit,
                list.text_margin,
                list.plain_alpha,
                list.clip,
                list.padlock,
                list.glow
            );
            for r in &live.rows {
                eprintln!(
                    "tsfp-bevy: ui-pages:   live row {:#x} type {} id {} flags {:#x} draw {:#x} rect {:?} text {:?} font {}",
                    r.node, r.node_type, r.id, r.flags, r.draw, r.rect, r.text, r.font
                );
                if std::env::var_os("TSFP_UI_ROW_WORDS").is_some() {
                    let words: Vec<String> = (0..0x10Cu32)
                        .step_by(4)
                        .map(|o| format!("{o:x}:{:x}", g.read32(r.node + o).unwrap_or(0)))
                        .collect();
                    eprintln!("tsfp-bevy: ui-pages:     words {}", words.join(" "));
                    let options = g.read32(r.node + 0xD8).unwrap_or(0);
                    if options != 0 {
                        let words: Vec<String> = (0..0x28u32)
                            .step_by(4)
                            .map(|o| format!("{:x}", g.read32(options + o).unwrap_or(0)))
                            .collect();
                        let text = g.read32(options + 4).unwrap_or(0);
                        let bytes: Vec<u8> = (0..16)
                            .map(|i| {
                                (g.read32((text + i) & !3).unwrap_or(0) >> (((text + i) & 3) * 8))
                                    as u8
                            })
                            .collect();
                        eprintln!(
                            "tsfp-bevy: ui-pages:     option words {} text {:?}",
                            words.join(" "),
                            String::from_utf8_lossy(&bytes)
                        );
                    }
                }
            }
            for row in &list.rows {
                eprintln!(
                    "tsfp-bevy: ui-pages:   row {:?} {:?} locked {} bright {} h {} {:?}",
                    row.widget, row.area, row.locked, row.brightness, row.row_height, row.kind
                );
            }
            if let Some(summary) = &list.summary {
                eprintln!("tsfp-bevy: ui-pages:   summary {summary:?}");
            }
            for image in &list.images {
                eprintln!(
                    "tsfp-bevy: ui-pages:   guest image {} {}x{}",
                    image.key, image.width, image.height
                );
            }
        }
    }
    registry.current = Some(Current {
        descriptor,
        slot,
        focused: live.focused(),
        mapmaker_storage_progress,
        content: Arc::new(LiveContent {
            model,
            page: page_id,
            dynamic,
            list,
            default_page,
        }),
        drawable: verdict.is_ok()
            && tsfp_ui_input::native_page::top_page(g)
                == Some(tsfp_ui_input::native_page::PageRef(slot_record(slot))),
        cursor_nodes,
        hit,
        live,
        volatile,
    });
}

/// The alternate live Map Maker route observed by the owner has a normal page
/// builder instead of the descriptor-less popup. Accept it as the mode prompt
/// only when its live rows carry the exact measured Beginner/Advanced callback
/// pair; an empty or unrelated page is never auto-selected.
fn mapmaker_mode_prompt_rows(g: &dyn GuestUi, slot: u32) -> bool {
    let page = slot_record(slot);
    let Some(mut node) = g.read32(page + guest_layout::page::FIRST_NODE) else {
        return false;
    };
    let mut seen = Vec::new();
    let mut handlers = Vec::new();
    while node != 0 && seen.len() < 256 && !seen.contains(&node) {
        seen.push(node);
        let handler = g.read32(node + guest_layout::row::ON_ACTIVATE).unwrap_or(0);
        if handler != 0 {
            handlers.push(handler);
        }
        node = g.read32(node + guest_layout::row::NEXT).unwrap_or(0);
    }
    node == 0 && handlers == [0x31_2B60, 0x31_2B60]
}

/// Whether the active Map Maker page is the measured Beginner/Advanced prompt.
/// The descriptor-less child is the canonical REA route; the 0x2BF770 alias
/// is accepted only with the exact callback pair required by its admission gate.
pub(crate) fn active_mapmaker_experience_prompt(g: &dyn GuestUi) -> bool {
    let Some(slot) = top_slot(g) else {
        return false;
    };
    let page = slot_record(slot);
    let builder = g
        .read32(page + guest_layout::page::BUILDER)
        .unwrap_or_default();
    if builder == 0x2B_F770 {
        return mapmaker_alias_enabled(g, builder, slot) && mapmaker_mode_prompt_rows(g, slot);
    }
    tsfp_ui_input::mapmaker_handlers::is_mode_choice_popup(
        g,
        tsfp_ui_input::native_page::PageRef(page),
    )
}

/// T2315 increment 3: guest words the page content depends on besides the live page record: the Audio/Video slider
/// values (`[0x5236BC]`, `[0x5236C0]`), the Controls turn speed, the page state (`+0x18C`, its cursor id and screen position mode) and the lit
/// side arrows (`+0xC8` bits 4 and 8). A change rebuilds the content.
fn volatile_words(g: &dyn GuestUi, slot: u32) -> [u32; 10] {
    use list_rows_live::av;
    let record = slot_record(slot);
    let settings = g
        .read32(av::SETTINGS_INDEX)
        .unwrap_or(0)
        .wrapping_mul(av::SETTINGS_STRIDE);
    [
        g.read32(av::TURN_SPEED.wrapping_add(settings)).unwrap_or(0),
        g.read32(av::VALUE_ROW1).unwrap_or(0),
        g.read32(av::VALUE_OTHER).unwrap_or(0),
        g.read32(record + av::PAGE_STATE).unwrap_or(0),
        g.read32(record + av::PAGE_ARROW_BITS).unwrap_or(0)
            & (av::LEFT_ARROW_LIT | av::RIGHT_ARROW_LIT),
        // T2315 increment 4: scores, ranks and objective states of the in-game BACK pages.
        tsfp_ui_input::ingame_live::checksum(g),
        // The original page frame can resize without its row content changing.
        g.read32(record + 0xE4).unwrap_or(0),
        g.read32(record + 0xE8).unwrap_or(0),
        g.read32(record + 0xEC).unwrap_or(0),
        g.read32(record + 0xF0).unwrap_or(0),
    ]
}

/// The top registered page read at the last frame boundary.
pub fn current() -> Option<Current> {
    REGISTRY.lock().ok()?.as_ref()?.current.clone()
}

/// The builder of the top registered page, 0 for none.
pub fn top_builder() -> u32 {
    TOP.load(Ordering::Acquire)
}

/// The C draw switch (`sub_00076810`): the Rust drawer owns `widget` when it is the slot record of
/// the top registered page, that page is drawable and Rust drawing is on (`enabled`: in-frame layer
/// drawing, not hidden by F9). Returns the content to draw.
pub fn draw_owned(widget: u32, builder: u32, enabled: bool) -> Option<Current> {
    // T3799: under an open option list the original draws the page, so the list (original) stays on top of it.
    if !enabled || crate::ui_live::option_list_open() {
        return None;
    }
    current().filter(|c| c.drawable && c.live.builder == builder && slot_record(c.slot) == widget)
}

/// Pointer on a registered page: the row under `pos` (640x480 design space) is focused when the
/// pointer moved onto it, a left press accepts, a right press backs. `moved` is false when the
/// pointer is still (so keys keep the focus).
pub fn pointer_commands(
    current: &Current,
    pos: Option<(f32, f32)>,
    moved: bool,
    left_pressed: bool,
    right_pressed: bool,
) -> Vec<NativeCommand> {
    let mut out = Vec::new();
    let row = pos
        .zip(current.hit.as_deref())
        .and_then(|((x, y), layout)| {
            let widget = hit_test(layout, x, y)?;
            current
                .live
                .cursor_index(widget.0 as usize, &current.cursor_nodes)
        });
    if let Some(index) = row {
        if moved
            && current.focused.and_then(|w| {
                current
                    .live
                    .cursor_index(w.0 as usize, &current.cursor_nodes)
            }) != Some(index)
        {
            out.push(NativeCommand::Focus(index));
        }
        if left_pressed {
            out.push(NativeCommand::Select(index));
        }
    }
    if right_pressed {
        out.push(NativeCommand::Back);
    }
    out
}

/// Pointer route used by the live page registry. Keeping the descriptor gate
/// here lets tests exercise the same registered-page seam used by mouse input.
pub fn registered_pointer_commands(
    current: &Current,
    pos: Option<(f32, f32)>,
    moved: bool,
    left_pressed: bool,
    right_pressed: bool,
) -> Vec<NativeCommand> {
    if current.descriptor.builder != current.live.builder
        || descriptor(current.live.builder).is_none()
        || current.live.builder == 0x19_2110
        || (matches!(current.live.builder, 0x2C_A230 | 0x2C_BFC0) && !crate::ui_back::enabled())
    {
        return Vec::new();
    }
    pointer_commands(current, pos, moved, left_pressed, right_pressed)
}

/// Custom selector drawers can leave the generic clipped box inverted. Read the
/// existing 0x71E90 geometry contract for non-scrolled rows; scrolled-out rows
/// retain their missing box so they never gain invisible hit targets.
fn selector_geometry(g: &dyn GuestUi, live: &mut LivePage) {
    if !matches!(
        live.builder,
        0x2D_62F0
            | 0x2C_AC30
            | 0x2D_6250
            | 0x2C_A960
            | 0x2C_9AE0
            | 0x2C_9480
            | 0x2C_9040
            | 0x2C_C170
            | 0x2C_8940
            | 0x32_BEA0
            | 0x2B_F770
            | 0x2E_FA40
            | 0x2E_FB40
            | 0x2E_FBF0
    ) {
        return;
    }
    for row in &mut live.rows {
        if row.rect.is_none() && row.flags & live_page::draw_fields::FLAG_SCROLLED == 0 {
            row.rect = live_page::row_geometry(g, row.node).filter(|r| r[2] > r[0] && r[3] > r[1]);
        }
    }
}

/// A wheel event is bounded view-only scrolling, never cursor movement or activation. Pixel and line events
/// share direction; cap bursts so a delayed event cannot flood the guest queue.
pub fn wheel_commands(delta: f32) -> Vec<NativeCommand> {
    if !delta.is_finite() || delta == 0.0 {
        return Vec::new();
    }
    let notches = delta.abs().ceil().min(8.0) as i32 * if delta > 0.0 { 1 } else { -1 };
    vec![NativeCommand::Scroll(notches)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_ui_input::live_page::{LiveRow, LiveText};

    fn row(node: u32, top: f32, draw: u32) -> LiveRow {
        LiveRow {
            node,
            node_type: 7,
            id: 0,
            flags: 0,
            text: LiveText::Id(1),
            font: 4,
            value_font: 0,
            align: 0,
            rect: Some([176.0, top, 464.0, top + 30.0]),
            clip: None,
            value: None,
            activate: 0x10,
            draw,
            kind: 0,
            selected: 0,
            options: Vec::new(),
            brightness: 0.0,
            height: 30.0,
            text_align: 0,
            text_x: 0.0,
            text_y: 0.0,
        }
    }

    fn current(rows: Vec<LiveRow>, cursor: u32) -> Current {
        let live = LivePage {
            slot: 2,
            builder: 0x2D_08C0,
            state: 2,
            cursor_node: cursor,
            rows,
            units: (6.4, 4.8),
        };
        let id = PageId(1);
        let (page, dynamic) = live.to_model_page(id);
        let hit = ModelLayout::from_page(&page, 640.0, 480.0).map(Arc::new);
        let mut model = UiModel::new();
        model.pages.insert(id, page);
        Current {
            descriptor: *descriptor(0x2D_08C0).unwrap(),
            slot: 2,
            focused: live.focused(),
            mapmaker_storage_progress: None,
            cursor_nodes: live.rows.iter().map(|r| r.node).collect(),
            drawable: drawable(&live).is_ok(),
            content: Arc::new(LiveContent {
                model,
                page: id,
                dynamic,
                list: None,
                default_page: None,
            }),
            hit,
            live,
            volatile: [0; 10],
        }
    }

    #[test]
    fn navigation_footer_dispatches_back_on_every_registered_page_and_measured_resume() {
        let mut c = current(vec![row(0x1000, 100.0, 0)], 0x1000);
        for page in PAGES {
            c.descriptor = *page;
            if page.builder != 0x2F_5520 {
                let expected = if matches!(page.builder, 0x2F_5920 | 0x2F_5E60) {
                    NativeCommand::ResumeGame
                } else {
                    NativeCommand::Back
                };
                assert_eq!(crate::ui_back::command(&c), Some(expected), "{}", page.name);
            }
        }
        c.descriptor = *descriptor(0x2F_5520).unwrap();
        assert_eq!(
            crate::ui_back::command(&c),
            None,
            "missing Resume never becomes an arbitrary selection"
        );
        c.live.rows[0].activate = 0x2E_BB80;
        assert_eq!(crate::ui_back::command(&c), Some(NativeCommand::Select(0)));
        c.cursor_nodes.clear();
        assert_eq!(
            crate::ui_back::command(&c),
            None,
            "Resume must belong to the cursor list"
        );
    }

    #[test]
    fn mapmaker_confirm_requires_child_link_type_and_yes_no_handlers() {
        use std::collections::HashMap;
        struct Memory(HashMap<u32, u32>);
        impl GuestUi for Memory {
            fn read32(&self, a: u32) -> Option<u32> {
                self.0.get(&a).copied()
            }
            fn write32(&mut self, _: u32, _: u32) -> bool {
                false
            }
            fn call(&mut self, _: u32, _: &[u32]) -> Option<u32> {
                None
            }
        }
        let parent = slot_record(0);
        let child = slot_record(1);
        let mut m = Memory(HashMap::from([
            (tsfp_ui_input::editor_pointer::addr::UI_MODE, 0x66),
            (parent, 1),
            (parent + guest_layout::page::BUILDER, 0x31_3010),
            (parent + guest_layout::page::ACTIVE_CHILD, child),
            (child, 2),
            (child + guest_layout::page::FIRST_NODE, 0x9000),
            (0x9004, 0x9100),
            (0x9038, 0x7_9730),
            (0x9138, 0x7_9790),
        ]));
        assert_eq!(popup_descriptor(&m, 1).map(|d| d.builder), Some(0x7_A080));
        m.0.insert(0x9104, 0x9000);
        assert_eq!(
            popup_descriptor(&m, 1),
            None,
            "cyclic children are not admitted"
        );
        m.0.remove(&0x9104);
        m.0.insert(0x9138, 0x1234);
        assert_eq!(
            popup_descriptor(&m, 1),
            None,
            "unrelated children are not admitted"
        );
        m.0.insert(0x9138, 0x7_9790);
        m.0.insert(parent + guest_layout::page::ACTIVE_CHILD, 0);
        assert_eq!(
            popup_descriptor(&m, 1),
            None,
            "requires the active-child edge"
        );
    }

    #[test]
    fn active_mapmaker_experience_prompt_requires_measured_popup_callbacks() {
        use std::collections::HashMap;
        struct Memory(HashMap<u32, u32>);
        impl GuestUi for Memory {
            fn read32(&self, address: u32) -> Option<u32> {
                self.0.get(&address).copied()
            }
            fn write32(&mut self, _: u32, _: u32) -> bool {
                false
            }
            fn call(&mut self, _: u32, _: &[u32]) -> Option<u32> {
                None
            }
        }
        let parent = slot_record(0);
        let child = slot_record(1);
        let mut memory = Memory(HashMap::from([
            (parent, 1),
            (parent + guest_layout::page::BUILDER, 0x31_3010),
            (parent + guest_layout::page::ACTIVE_CHILD, child),
            (child, 2),
            (child + guest_layout::page::FIRST_NODE, 0x9000),
            (0x9004, 0x9100),
            (0x9038, 0x31_2B60),
            (0x9138, 0x31_2B60),
        ]));

        assert!(active_mapmaker_experience_prompt(&memory));
        memory.0.insert(0x9138, 0x31_2120);
        assert!(!active_mapmaker_experience_prompt(&memory));
    }

    #[test]
    fn editor_settings_mode_confirmation_requires_its_measured_yes_no_pair() {
        use std::collections::HashMap;
        struct Memory(HashMap<u32, u32>);
        impl GuestUi for Memory {
            fn read32(&self, a: u32) -> Option<u32> {
                self.0.get(&a).copied()
            }
            fn write32(&mut self, _: u32, _: u32) -> bool {
                false
            }
            fn call(&mut self, _: u32, _: &[u32]) -> Option<u32> {
                None
            }
        }
        let parent = slot_record(0);
        let child = slot_record(1);
        let mut memory = Memory(HashMap::from([
            (tsfp_ui_input::editor_pointer::addr::UI_MODE, 0x66),
            (parent, 1),
            (parent + guest_layout::page::BUILDER, 0x31_2270),
            (parent + guest_layout::page::ACTIVE_CHILD, child),
            (child, 2),
            (child + guest_layout::page::FIRST_NODE, 0x9000),
            (0x9004, 0x9100),
            (0x9038, 0x31_2120),
            (0x9138, 0x36_1710),
        ]));
        assert_eq!(
            popup_descriptor(&memory, 1).map(|descriptor| descriptor.builder),
            Some(0x7_A080)
        );
        memory.0.insert(0x9038, 0x36_1710);
        memory.0.insert(0x9138, 0x31_2120);
        assert_eq!(
            popup_descriptor(&memory, 1).map(|descriptor| descriptor.builder),
            Some(0x7_A080),
            "action identity is bound independently of button order"
        );
        memory.0.insert(0x9038, 0x31_2120);
        memory.0.insert(0x9138, 0x36_1710);
        memory
            .0
            .insert(parent + guest_layout::page::BUILDER, 0x31_3010);
        assert_eq!(popup_descriptor(&memory, 1), None);
        memory
            .0
            .insert(parent + guest_layout::page::BUILDER, 0x31_2270);
        memory.0.insert(0x9138, 0x31_2B60);
        assert_eq!(popup_descriptor(&memory, 1), None);
    }

    #[test]
    fn direct_clear_map_popup_uses_its_editor_menu_yes_callback_contract() {
        use std::collections::HashMap;
        struct Memory(HashMap<u32, u32>);
        impl GuestUi for Memory {
            fn read32(&self, address: u32) -> Option<u32> {
                self.0.get(&address).copied()
            }
            fn write32(&mut self, _: u32, _: u32) -> bool {
                false
            }
            fn call(&mut self, _: u32, _: &[u32]) -> Option<u32> {
                None
            }
        }
        let parent = slot_record(0);
        let child = slot_record(1);
        let mut memory = Memory(HashMap::from([
            (tsfp_ui_input::editor_pointer::addr::UI_MODE, 0x66),
            (parent, 1),
            (parent + guest_layout::page::BUILDER, 0x31_3010),
            (parent + guest_layout::page::ACTIVE_CHILD, child),
            (child, 2),
            (child + guest_layout::page::FIRST_NODE, 0x9000),
            (0x9004, 0x9100),
            (0x9038, 0x31_1AA0),
        ]));
        assert_eq!(
            popup_descriptor(&memory, 1).map(|descriptor| descriptor.builder),
            Some(0x7_A080)
        );
        memory.0.insert(0x9038, 0x31_2120);
        assert_eq!(
            popup_descriptor(&memory, 1),
            None,
            "Editor Settings' Yes handler is not direct Clear Map"
        );
        memory.0.insert(0x9038, 0x31_1AA0);
        memory
            .0
            .insert(parent + guest_layout::page::BUILDER, 0x31_2270);
        assert_eq!(
            popup_descriptor(&memory, 1),
            None,
            "the callback must belong to the editor menu popup"
        );
        memory.0.insert(0x9038, 0x31_2120);
        memory.0.insert(0x9138, 0x36_1710);
        assert_eq!(top_slot(&memory), Some(1));
        assert_eq!(
            popup_descriptor(&memory, 1).map(|d| d.builder),
            Some(0x7_A080)
        );
        // Owner settings run: parent slot 2, warning reuses the editor's freed slot 1.
        memory.0.insert(parent, 0);
        let parent = slot_record(2);
        memory.0.insert(parent, 1);
        memory
            .0
            .insert(parent + guest_layout::page::BUILDER, 0x31_2270);
        memory
            .0
            .insert(parent + guest_layout::page::ACTIVE_CHILD, child);
        assert_eq!(
            top_slot(&memory),
            Some(1),
            "render the warning, not settings"
        );
        assert_eq!(
            popup_descriptor(&memory, 1).map(|d| d.builder),
            Some(0x7_A080)
        );
    }

    #[test]
    fn mapmaker_select_mode_popup_is_registered_by_its_measured_choice_handlers() {
        use std::collections::HashMap;
        struct Memory(HashMap<u32, u32>);
        impl GuestUi for Memory {
            fn read32(&self, a: u32) -> Option<u32> {
                self.0.get(&a).copied()
            }
            fn write32(&mut self, _: u32, _: u32) -> bool {
                false
            }
            fn call(&mut self, _: u32, _: &[u32]) -> Option<u32> {
                None
            }
        }
        let parent = slot_record(0);
        let child = slot_record(1);
        let mut m = Memory(HashMap::from([
            (parent, 1),
            (parent + guest_layout::page::BUILDER, 0x2C_8940),
            (parent + guest_layout::page::ACTIVE_CHILD, child),
            (child, 2),
            (child + guest_layout::page::FIRST_NODE, 0x9000),
            (0x9004, 0x9100),
            (0x9038, 0x31_2B60),
            (0x9138, 0x31_2B60),
        ]));
        assert_eq!(popup_descriptor(&m, 1).map(|d| d.builder), Some(0x7_A080));
        m.0.insert(0x9138, 0x7_9790);
        assert_eq!(
            popup_descriptor(&m, 1),
            None,
            "other dialogs stay unregistered"
        );
        m.0.insert(0x9138, 0x31_2B60);
        m.0.insert(parent + guest_layout::page::ACTIVE_CHILD, 0);
        assert_eq!(
            popup_descriptor(&m, 1),
            None,
            "requires the active-child edge"
        );
    }

    #[test]
    fn registry_holds_batch_one_and_excludes_the_main_menu_from_page_input() {
        assert_eq!(PAGES.len(), 57);
        for builder in [
            0x2D_62F0, 0x2C_AC30, 0x2D_6250, 0x2C_9AE0, 0x2C_9480, 0x2C_9040, 0x2C_C170, 0x2C_8940,
            0x2C_A960, 0x2E_FA40, 0x2E_FB40, 0x2E_FBF0,
        ] {
            assert!(
                input_page(builder).is_some(),
                "selector {builder:#x} owns native input"
            );
        }
        assert!(input_page(tsfp_ui_input::mapmaker_award_table::BUILDER).is_some());
        assert!(input_page(tsfp_ui_input::mapmaker_kind_list::BUILDER).is_some());
        let mut builders: Vec<_> = PAGES.iter().map(|p| p.builder).collect();
        builders.sort_unstable();
        builders.dedup();
        assert_eq!(builders.len(), PAGES.len());
        assert_eq!(
            input_page(0x19_33C0),
            Some("Profile storage and save status")
        );
        assert_eq!(input_page(0x19_4980), Some("Profile selection"));
        assert_eq!(input_page(0x31_3010), Some("Map Maker editor menu"));
        assert_eq!(input_page(0x31_4650), Some("Map Maker editor help"));
        assert!(MAPMAKER_PAGES.contains(&0x31_4650));
        assert_eq!(
            input_page(tsfp_ui_input::mapmaker_award_flags::BUILDER),
            Some("Map Maker Award Flags")
        );
        assert!(MAPMAKER_PAGES.contains(&tsfp_ui_input::mapmaker_award_flags::BUILDER));
        assert_eq!(
            input_page(tsfp_ui_input::mapmaker_action_picker::BUILDER),
            Some("Map Maker Select Actions")
        );
        assert!(MAPMAKER_PAGES.contains(&tsfp_ui_input::mapmaker_action_picker::BUILDER));
        assert_eq!(input_page(0x32_23A0), Some("Map Maker object kind chooser"));
        assert!(MAPMAKER_PAGES.contains(&0x32_23A0));
        assert_eq!(input_page(0x32_2BB0), Some("Map Maker awards chooser"));
        assert!(MAPMAKER_PAGES.contains(&0x32_2BB0));
        assert_eq!(
            input_page(0x32_7A70),
            Some("Map Maker Award Medal settings")
        );
        assert!(MAPMAKER_PAGES.contains(&0x32_7A70));
        assert_eq!(
            input_page(0x32_8D20),
            Some("Map Maker selected-entry properties")
        );
        assert_eq!(input_page(0x32_BEA0), Some("Map Maker map list"));
        assert!(MAPMAKER_PAGES.iter().all(|b| descriptor(*b).is_some()));
        assert!(
            MAPMAKER_PAGES.contains(&0x32_8D20),
            "selected-entry properties uses the native cursor path"
        );
        assert_eq!(input_page(0x2F_5520), Some("Pause"));
        assert_eq!(input_page(0x2D_08C0), Some("Extras"));
        assert_eq!(input_page(0x2D_0BE0), None, "main menu input is LiveMenu's");
        assert_eq!(
            input_page_with_footer(0x2C_BFC0, false),
            None,
            "original pointer ownership unchanged"
        );
        assert_eq!(input_page_with_footer(0x2C_BFC0, true), Some("Settings"));
        assert_eq!(input_page_with_footer(0x2C_A230, false), None);
        assert_eq!(input_page_with_footer(0x2C_A230, true), Some("Arcade type"));
        assert!(descriptor(0x2D_0BE0).is_some());
    }

    /// T3723: every registered page reaches the keyboard and the mouse, natively or through the fallback pad buttons
    /// (arrows, Enter or numpad Enter, Backspace or Escape, left and right click). Pads are the original path.
    #[test]
    fn every_registered_page_accepts_keyboard_and_mouse() {
        use tsfp_ui_input::keyboard::{MouseFallback, fallback_button_on_page};
        assert!(PAGES.len() >= 50);
        for page in PAGES {
            for (key, pad) in [
                (0x4000_0052, 11),
                (0x4000_0051, 12),
                (0x4000_0050, 13),
                (0x4000_004f, 14),
                (13, 0),
                (0x4000_0058, 0),
                (8, 1),
            ] {
                assert_eq!(
                    fallback_button_on_page(key, page.builder),
                    Some(pad),
                    "{} key {key:#x}",
                    page.name
                );
            }
            assert!(
                fallback_button_on_page(27, page.builder).is_some(),
                "{} Escape",
                page.name
            );
        }
        let mut mouse = MouseFallback::new();
        assert_eq!(
            (mouse.step(0, true, true), mouse.step(1, true, true)),
            (Some(0), Some(1))
        );
    }

    #[test]
    fn ingame_flag_spans_the_pause_session_only() {
        assert!(!ingame());
        track_ingame(Some(0x2D_08C0));
        assert!(!ingame(), "front-end pages are not in game");
        track_ingame(Some(0x2F_5520));
        assert!(ingame(), "pause opens the in-game menu");
        track_ingame(Some(0x2E_A8E0));
        assert!(ingame(), "Controls opened from pause stays in game");
        track_ingame(None);
        assert!(!ingame(), "resume: no page live");
        track_ingame(Some(0x2E_B9E0));
        assert!(ingame(), "a confirm dialog is in game");
        track_ingame(Some(0x2D_0BE0));
        assert!(!ingame(), "quit straight to the main menu ends the session");
        track_ingame(Some(0x2F_5520));
        track_ingame(Some(0x32_B6A0));
        assert!(
            ingame(),
            "a Map Maker storage page keeps the flag as it found it"
        );
        track_ingame(None);
    }

    #[test]
    fn unported_row_draws_keep_the_original_draw() {
        let plain = LivePage {
            slot: 1,
            builder: 1,
            state: 2,
            cursor_node: 0,
            rows: vec![row(1, 100.0, 0)],
            units: (6.4, 4.8),
        };
        assert_eq!(drawable(&plain), Ok(()));
        let divider = LivePage {
            rows: vec![row(1, 100.0, 0x2D_82F0), row(2, 140.0, 0)],
            ..plain.clone()
        };
        assert_eq!(
            drawable(&divider),
            Ok(()),
            "the divider is Rust drawn (increment 2)"
        );
        let lists = LivePage {
            rows: vec![row(1, 100.0, 0x2C_A9F0), row(2, 140.0, 0x2C_8B50)],
            ..plain.clone()
        };
        assert_eq!(
            drawable(&lists),
            Ok(()),
            "Story and Challenge list rows are Rust drawn (increment 3)"
        );
        let progress = LivePage {
            rows: vec![row(1, 100.0, 0x2D_82F0), row(2, 140.0, 0x2C_C850)],
            ..plain.clone()
        };
        assert_eq!(
            drawable(&progress),
            Ok(()),
            "the Progress summary is Rust drawn (increment 3)"
        );
        let custom = LivePage {
            rows: vec![row(1, 100.0, 0x2D_82F0), row(2, 140.0, 0x2E_D710)],
            ..plain.clone()
        };
        assert_eq!(drawable(&custom), Err(DrawBlock::RowDraws(vec![0x2E_D710])));
        let empty = LivePage {
            rows: Vec::new(),
            ..plain
        };
        assert_eq!(drawable(&empty), Err(DrawBlock::NoRows));
    }

    #[test]
    fn palette_custom_page_draw_admission_is_scoped_to_owned_kind_a_page() {
        assert!(page_draw_supported(
            MAPMAKER_PALETTE_PROPERTIES_BUILDER,
            MAPMAKER_PALETTE_PROPERTIES_DRAW,
            true
        ));
        assert!(!page_draw_supported(
            MAPMAKER_PALETTE_PROPERTIES_BUILDER,
            MAPMAKER_PALETTE_PROPERTIES_DRAW,
            false
        ));
        assert!(!page_draw_supported(
            0x31_6920,
            MAPMAKER_PALETTE_PROPERTIES_DRAW,
            true
        ));
        assert!(!page_draw_supported(
            MAPMAKER_PALETTE_PROPERTIES_BUILDER,
            0x31_6920,
            true
        ));
        assert!(page_draw_supported(0x31_3010, RUST_PAGE_DRAWS[0], false));
    }

    #[test]
    fn palette_light_cell_uses_existing_typed_default_row_draw() {
        let live = LivePage {
            slot: 1,
            builder: MAPMAKER_PALETTE_PROPERTIES_BUILDER,
            state: 2,
            cursor_node: 1,
            rows: vec![
                row(
                    1,
                    100.0,
                    tsfp_ui_input::default_widgets_live::addr::LABEL_DRAW,
                ),
                row(
                    2,
                    140.0,
                    tsfp_ui_input::default_widgets_live::addr::LIGHT_DRAW,
                ),
            ],
            units: (6.4, 4.8),
        };
        assert_eq!(
            drawable_with(&live, tsfp_ui_input::default_widgets_live::ROW_DRAWS),
            Ok(())
        );
    }

    #[test]
    fn pointer_focuses_the_row_under_it_and_clicks_accept_or_back() {
        let c = current(vec![row(0x100, 100.0, 0), row(0x200, 140.0, 0)], 0x100);
        // Over row 1 (y 150), moving: focus 1; pressing: select 1.
        assert_eq!(
            pointer_commands(&c, Some((320.0, 150.0)), true, false, false),
            vec![NativeCommand::Focus(1)]
        );
        assert_eq!(
            pointer_commands(&c, Some((320.0, 150.0)), false, true, false),
            vec![NativeCommand::Select(1)]
        );
        // Over the focused row: no focus command. Outside the rows: only back on right click.
        assert!(pointer_commands(&c, Some((320.0, 110.0)), true, false, false).is_empty());
        assert_eq!(
            pointer_commands(&c, Some((10.0, 10.0)), true, true, true),
            vec![NativeCommand::Back]
        );
    }

    #[test]
    fn mapmaker_select_mode_popup_routes_mouse_hover_and_click_to_choice() {
        let mut page = current(vec![row(0x100, 100.0, 0), row(0x200, 140.0, 0)], 0x100);
        page.descriptor = *descriptor(0x7_A080).unwrap();
        page.live.builder = 0; // the original type-2 popup has no guest builder
        assert_eq!(
            pointer_commands(&page, Some((320.0, 150.0)), true, true, false),
            vec![NativeCommand::Focus(1), NativeCommand::Select(1)],
            "hover and click must select Advanced on the descriptor-less prompt"
        );
    }

    fn assert_mapmaker_family_mouse_input(builder: u32) {
        let mut page = current(vec![row(0x100, 100.0, 0), row(0x200, 140.0, 0)], 0x100);
        page.descriptor = *descriptor(builder).expect("registered Map Maker page family");
        page.live.builder = builder;
        assert_eq!(
            registered_pointer_commands(&page, Some((320.0, 150.0)), true, true, false),
            vec![NativeCommand::Focus(1), NativeCommand::Select(1)],
            "Map Maker page {builder:#x} should route pointer hover and click"
        );
    }

    #[test]
    fn mapmaker_editor_menu_routes_mouse_focus_and_select() {
        assert_mapmaker_family_mouse_input(0x31_3010);
    }

    #[test]
    fn mapmaker_editor_configuration_routes_mouse_focus_and_select() {
        for builder in [0x31_33E0, 0x31_3FE0, 0x31_2270, 0x31_5720, 0x31_4420] {
            assert_mapmaker_family_mouse_input(builder);
        }
    }

    #[test]
    fn mapmaker_storage_pages_route_mouse_focus_and_select() {
        for builder in [0x32_B6A0, 0x32_BEA0, 0x32_AEE0] {
            assert_mapmaker_family_mouse_input(builder);
        }
    }

    #[test]
    fn mapmaker_mode_prompt_alias_is_registered_and_routes_mouse_events() {
        let builder = 0x2B_F770;
        assert!(tsfp_ui_input::mapmaker_handlers::INFERRED_CURSOR_PAGE_ALIASES.contains(&builder));
        assert_mapmaker_family_mouse_input(builder);
    }

    #[test]
    fn mapmaker_mode_prompt_alias_is_admitted_before_mode_publish_only_for_measured_rows() {
        use std::collections::HashMap;
        struct Memory(HashMap<u32, u32>);
        impl GuestUi for Memory {
            fn read32(&self, address: u32) -> Option<u32> {
                self.0.get(&address).copied()
            }
            fn write32(&mut self, _: u32, _: u32) -> bool {
                false
            }
            fn call(&mut self, _: u32, _: &[u32]) -> Option<u32> {
                None
            }
        }

        let slot = 2;
        let page = slot_record(slot);
        let first = 0x9000;
        let second = 0x9100;
        let mut memory = Memory(HashMap::from([
            (page + guest_layout::page::FIRST_NODE, first),
            (first + guest_layout::row::NEXT, second),
            (second + guest_layout::row::NEXT, 0),
            (first + guest_layout::row::ON_ACTIVATE, 0x31_2B60),
            (second + guest_layout::row::ON_ACTIVATE, 0x31_2B60),
        ]));

        assert!(mapmaker_alias_enabled_for(
            &memory, 0x2B_F770, slot, true, None,
        ));
        assert!(!mapmaker_alias_enabled_for(
            &memory, 0x2B_F770, slot, false, None,
        ));
        assert!(!mapmaker_alias_enabled_for(
            &memory, 0x19_2110, slot, true, None,
        ));

        memory
            .0
            .insert(second + guest_layout::row::ON_ACTIVATE, 0x7_9790);
        assert!(!mapmaker_alias_enabled_for(
            &memory, 0x2B_F770, slot, true, None,
        ));
        assert!(mapmaker_alias_enabled_for(
            &memory,
            0x19_2110,
            slot,
            true,
            Some(tsfp_ui_input::editor_pointer::addr::MODE_MAPMAKER),
        ));
    }

    #[test]
    fn mapmaker_logic_summary_page_requires_rust_switch_and_mapmaker_mode() {
        struct Empty;
        impl GuestUi for Empty {
            fn read32(&self, _: u32) -> Option<u32> {
                None
            }
            fn write32(&mut self, _: u32, _: u32) -> bool {
                false
            }
            fn call(&mut self, _: u32, _: &[u32]) -> Option<u32> {
                None
            }
        }
        let builder = tsfp_ui_input::mapmaker_logic_summary::BUILDER;
        let mode = tsfp_ui_input::editor_pointer::addr::MODE_MAPMAKER;
        assert!(mapmaker_page_enabled_for(
            &Empty,
            builder,
            0,
            true,
            Some(mode)
        ));
        assert!(!mapmaker_page_enabled_for(
            &Empty,
            builder,
            0,
            false,
            Some(mode)
        ));
        assert!(!mapmaker_page_enabled_for(
            &Empty,
            builder,
            0,
            true,
            Some(0)
        ));
    }

    #[test]
    fn mapmaker_trigger_selection_page_requires_rust_switch_and_mapmaker_mode() {
        struct Empty;
        impl GuestUi for Empty {
            fn read32(&self, _: u32) -> Option<u32> {
                None
            }
            fn write32(&mut self, _: u32, _: u32) -> bool {
                false
            }
            fn call(&mut self, _: u32, _: &[u32]) -> Option<u32> {
                None
            }
        }
        let builder = tsfp_ui_input::mapmaker_trigger_selection::BUILDER;
        let mode = tsfp_ui_input::editor_pointer::addr::MODE_MAPMAKER;
        assert!(mapmaker_page_enabled_for(
            &Empty,
            builder,
            0,
            true,
            Some(mode)
        ));
        assert!(!mapmaker_page_enabled_for(
            &Empty,
            builder,
            0,
            false,
            Some(mode)
        ));
        assert!(!mapmaker_page_enabled_for(
            &Empty,
            builder,
            0,
            true,
            Some(0)
        ));
    }

    #[test]
    fn mapmaker_local_list_alias_is_registered_without_native_mouse_actions() {
        let builder = 0x19_2110;
        assert!(tsfp_ui_input::mapmaker_handlers::INFERRED_CURSOR_PAGE_ALIASES.contains(&builder));
        let mut page = current(vec![row(0x100, 100.0, 0), row(0x200, 140.0, 0)], 0x100);
        page.descriptor = *descriptor(builder).expect("registered Map Maker page alias");
        page.live.builder = builder;
        assert!(
            registered_pointer_commands(&page, Some((320.0, 150.0)), true, true, false).is_empty()
        );
    }

    #[test]
    fn mapmaker_confirmation_routes_mouse_focus_and_select() {
        assert_mapmaker_family_mouse_input(0x7_A080);
    }

    #[test]
    fn mapmaker_map_list_missing_box_gets_hit_geometry_and_hidden_scrolled_row_does_not() {
        use std::collections::BTreeMap;
        use tsfp_ui_input::live_page::draw_fields as f;
        struct Memory(BTreeMap<u32, u32>);
        impl GuestUi for Memory {
            fn read32(&self, va: u32) -> Option<u32> {
                self.0.get(&va).copied()
            }
            fn write32(&mut self, _: u32, _: u32) -> bool {
                false
            }
            fn call(&mut self, _: u32, _: &[u32]) -> Option<u32> {
                None
            }
        }
        let mut g = Memory(BTreeMap::new());
        g.0.insert(0x100 + f::PARENT, 0x1000);
        for (va, value) in [
            (f::UNIT_X, 6.4f32),
            (f::UNIT_Y, 4.8),
            (0x100 + f::GEOM_X, 10.0),
            (0x100 + f::GEOM_Y, 20.0),
            (0x100 + f::GEOM_W, 80.0),
            (0x100 + f::GEOM_H, 10.0),
            (0x1000 + f::PAGE_X, 0.0),
            (0x1000 + f::PARENT_TOP, 0.0),
            (f::ORIGIN_Y, 0.0),
        ] {
            g.0.insert(va, value.to_bits());
        }
        let mut r = row(0x100, 0.0, 0x2C_AD80);
        r.rect = None;
        let mut live = current(vec![r], 0x100).live;
        live.builder = 0x2D_62F0;
        selector_geometry(&g, &mut live);
        let rect = live.rows[0].rect.unwrap();
        assert!((rect[0] - 64.0).abs() < 0.01 && (rect[1] - 96.0).abs() < 0.01);
        assert!(rect[2] > rect[0] && rect[3] > rect[1]);
        let c = current(live.rows.clone(), 0x100);
        assert_eq!(
            pointer_commands(&c, Some((100.0, 110.0)), false, true, false),
            vec![NativeCommand::Select(0)]
        );
        live.builder = 0x32_BEA0;
        live.rows[0].rect = None;
        selector_geometry(&g, &mut live);
        assert!(
            live.rows[0].rect.is_some(),
            "map-list row gets a mouse hit box"
        );
        let mut map_list = current(live.rows.clone(), 0x100);
        map_list.descriptor = *descriptor(0x32_BEA0).unwrap();
        map_list.live.builder = 0x32_BEA0;
        assert_eq!(
            pointer_commands(&map_list, Some((100.0, 110.0)), false, true, false),
            vec![NativeCommand::Select(0)],
            "local-storage map rows route clicks through the native page command"
        );
        live.rows[0].rect = None;
        live.rows[0].flags |= f::FLAG_SCROLLED;
        selector_geometry(&g, &mut live);
        assert!(live.rows[0].rect.is_none());
        live.rows[0].flags = 0;
        g.0.insert(0x100 + f::GEOM_W, 0.0f32.to_bits());
        selector_geometry(&g, &mut live);
        assert!(live.rows[0].rect.is_none());
    }

    #[test]
    fn wheel_scrolls_view_without_cursor_steps_or_activation_and_bounds_bursts() {
        assert_eq!(wheel_commands(1.0), vec![NativeCommand::Scroll(1)]);
        assert_eq!(wheel_commands(-2.0), vec![NativeCommand::Scroll(-2)]);
        assert_eq!(wheel_commands(999.0), vec![NativeCommand::Scroll(8)]);
        for delta in [0.0, f32::NAN, f32::INFINITY] {
            assert!(wheel_commands(delta).is_empty());
        }
    }

    #[test]
    fn draw_switch_owns_only_the_top_drawable_slot_record() {
        assert_eq!(slot_record(2), 0x6F_EBE8 + 2 * 0x23C);
        // Nothing serviced: never owned.
        assert!(draw_owned(slot_record(2), 0x2D_08C0, true).is_none());
    }
}

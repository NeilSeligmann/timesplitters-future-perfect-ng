//! T2312 increment 8: the weapon slot panel as native Rust (`0x141E30`, called once per frame by the HUD frame
//! `0x14F0B0`, no arguments).
//!
//! The original (MEASURED from the XBE disassembly of `0x141E30`, `0x141BF0`, `0x141650`, `0x141590`, `0x561C0`,
//! `0x81B70`, `0x12CC70`; docs/t2312-native-hud.md, "Increment 8"):
//!
//! * The owned weapons (the order list `0x4FC888`, 70 ids; owned flag `player extension + 0x54C + id`, hidden flag
//!   bit 0x10 of the weapon table `0x4FA128 + 0x3C * id`, id 1 hidden while extension `+0x584`) are grouped by the
//!   table's slot group `+0x38`. Each group is one column, 14.625 % of the HUD rectangle wide, the columns centred
//!   on 50 %; the panel's vertical place (`slide`, % of the HUD height) is 4 % (mode `0x749144` 0), `4 - +0x175C`
//!   (mode 1, the panel record's animated slide; the drawer stores the target in `+0x1760`) or `50 - selected`.
//! * A nine-slice panel (`0x12CC70(x0, y0, x1, y1, alpha, 3)`, texture `newHud_cornerGradient`, alpha
//!   `min(2 * +0x1758, 1)` while the fade state `+0x1750` is set) spans the shown icons plus 4 %, 10.5 % high, and
//!   the icons are drawn clipped to it (`0x1A010`).
//! * Per group the remembered weapon (`viewer + 0xE3C + group`) is drawn by `0x141650`: its icon `gunim_*_b`
//!   (icon table `0x4F9BB0 + 20 * id`: icon, overlay, x and y scale) in a dark colour, then the overlay
//!   `gunim_*_out_b` additively in the slot colour (selected, empty, normal), a glow while the switch flash
//!   `+0x1754` runs, and a small arrow (`xbdpad` atlas `[0x4D6C7C]`) above groups holding more than one weapon.
//!   While cycling inside the selected group (`+0x1764`, -1..1) the outgoing weapon slides out of the panel.
//!
//! [`WeaponPanel::read`] replays it on typed reads; the shell draws the result. The panel's only guest write
//! (the mode 1 slide target `+0x1760`, read by the fade updater `0x141340`) is returned as Rust state
//! ([`WeaponPanelDraw::slide_target`], held by the shell's [`SlideTargets`]) and stored by the gate.
use crate::hud_native::{GuestCall, GuestRead};

/// Guest addresses (MEASURED from the disassembly, see the module doc).
pub mod weapon_addr {
    /// The drawer (cdecl, no arguments).
    pub const FN_PANEL: u32 = 0x0014_1e30;
    /// Viewer (`+4` slot, `+0x14` object; object `+0x7C` the player extension) and the per-group remembered weapon.
    pub const VIEWER: u32 = 0x007b_0c7c;
    pub const OBJECT_EXTENSION: u32 = 0x7c;
    pub const REMEMBERED: u32 = 0xe3c;
    /// Player extension: current weapon id, owned flags (bytes by id), the punch-hidden flag.
    pub const CURRENT: u32 = 0x94;
    pub const OWNED: u32 = 0x54c;
    pub const PUNCH_HIDDEN: u32 = 0x584;
    /// HUD records (`0x177C` per slot) and the panel fields: fade state, flash timer, alpha, slide, slide target,
    /// cycle offset.
    pub const HUD_RECORDS: u32 = 0x007a_6160;
    pub const HUD_RECORD_SIZE: u32 = 0x177c;
    pub const STATE: u32 = 0x1750;
    pub const FLASH: u32 = 0x1754;
    pub const ALPHA: u32 = 0x1758;
    pub const SLIDE: u32 = 0x175c;
    pub const SLIDE_TARGET: u32 = 0x1760;
    pub const CYCLE: u32 = 0x1764;
    /// Weapon order list (70 ids), weapon table (flags `+0`, group `+0x38`), icon table (20 bytes per id).
    pub const ORDER: u32 = 0x004f_c888;
    pub const ORDER_LEN: u32 = 70;
    pub const WEAPONS: u32 = 0x004f_a128;
    pub const WEAPON_SIZE: u32 = 0x3c;
    pub const WEAPON_GROUP: u32 = 0x38;
    pub const ICONS: u32 = 0x004f_9bb0;
    pub const ICON_SIZE: u32 = 20;
    /// Panel placement mode (0 fixed, 1 animated slide with the target write, else centred on the selection).
    pub const MODE: u32 = 0x0074_9144;
    /// Viewport (`+0` x0, `+4` x1, `+0x14` y0, `+0x18` y1) and the HUD rectangle margins (left, top, right, bottom).
    pub const VIEWPORT_PTR: u32 = 0x007e_71c8;
    pub const MARGIN_LEFT: u32 = 0x007a_6140;
    pub const MARGIN_TOP: u32 = 0x007a_6144;
    pub const MARGIN_RIGHT: u32 = 0x007a_6148;
    pub const MARGIN_BOTTOM: u32 = 0x007a_614c;
    /// Texture table rows (`0x80F20`: handle at `+0`), texture records (size `+0x26` / `+0x2A`), the arrow atlas
    /// handle (record 673 of the table).
    pub const TEXTURE_ROWS: u32 = 0x004d_2d64;
    pub const TEXTURE_RECORDS_PTR: u32 = 0x004b_85c8;
    pub const ARROW_HANDLE: u32 = 0x004d_6c7c;
    /// Weapon queries called through the host's guest call (side-effect free reads of the weapon system):
    /// slot empty (`0x1C0080(extension, id)`), previous and next weapon of the group (`(extension, id, 1)`).
    pub const FN_EMPTY: u32 = 0x001c_0080;
    pub const FN_PREVIOUS: u32 = 0x001b_a9b0;
    pub const FN_NEXT: u32 = 0x001b_a8f0;
    /// Net player count (`0x1CDAE0`) and the split-height switches of `0x141590`.
    pub const FN_NET_PLAYERS: u32 = 0x001c_dae0;
    pub const MODE_BYTE: u32 = 0x007d_e455;
    pub const LEVEL_PLAYERS: u32 = 0x0079_0950;
    pub const SESSION: u32 = 0x007b_0c48;
    /// Render state the drawer leaves set: state group `0x19650(0)`, blend `0x19D70(1)`.
    pub const FN_STATE_GROUP: u32 = 0x0001_9650;
    pub const FN_BLEND: u32 = 0x0001_9d70;
    /// Primitive draws the shadow check observes: panel, faded quad, arrow.
    pub const FN_PANEL_QUAD: u32 = 0x0012_cc70;
    pub const FN_FADED_QUAD: u32 = 0x0005_61c0;
    pub const FN_ARROW: u32 = 0x0008_1b70;
}

/// Constants of the drawer (f32 values of the XBE data, addresses in brackets).
pub mod k {
    /// Column pitch (`0x47DB6C`), selected group extra (`0x47DB74`), selected centre (`0x47DB78`), group step
    /// (`0x47DB70`), the centre (`0x475D28`), the fixed slide (`0x475DB4`).
    pub const PITCH: f32 = 14.625;
    pub const SELECTED_EXTRA: f32 = 3.375;
    pub const SELECTED_CENTRE: f32 = 6.9375;
    pub const GROUP_STEP: f32 = 10.5;
    pub const CENTRE: f32 = 50.0;
    pub const FIXED_SLIDE: f32 = 4.0;
    /// Mode 1 target: limit (`0x47DB68`), value (`0x47DB64`), offset (`0x47D7DC`).
    pub const TARGET_LIMIT: f32 = 64.4;
    pub const TARGET_VALUE: f32 = -78.125;
    pub const TARGET_OFFSET: f32 = 46.0;
    /// Icon widths (% of the HUD width): normal (`0x47D014`) and wide textures (128 texels, `0x47DB5C`); the slot
    /// widths they are centred in (`0x47DB60`, `0x47DB6C`).
    pub const ICON_WIDTH: f32 = 7.5;
    pub const WIDE_ICON_WIDTH: f32 = 14.25;
    pub const SLOT_WIDTH: f32 = 11.25;
    pub const WIDE_SLOT_WIDTH: f32 = 14.625;
    pub const WIDE_TEXELS: u16 = 0x80;
    /// Panel margin (`0x47D66C`), panel height (`0x41280000` pushed), minimum size (`0x475DB4`).
    pub const PANEL_MARGIN: f32 = 0.04;
    pub const HEIGHT: f32 = 10.5;
    pub const MIN_SIZE: f32 = 4.0;
    /// Selected slot drop per cycle cube (`0x47DB58`).
    pub const SELECTED_DROP: f32 = 13.875;
    /// Vertical fade band of `0x561C0` (% of the HUD height, pushed constants).
    pub const FADE: [f32; 4] = [1.0, 4.0, 96.0, 99.0];
    /// Arrow: half width (`0x47DB50` of the HUD width), top and bottom over the icon top (`0x47DB54`, `0x47DAEC`
    /// of the half height), atlas units (1/16 texel).
    /// Exact XBE words (`0x3C54FDF3`, `0x3F0CCCCC`): one unit below the nearest f32 of 0.013 and 0.55.
    pub const ARROW_HALF_WIDTH: f32 = f32::from_bits(0x3c54_fdf3);
    pub const ARROW_TOP: f32 = f32::from_bits(0x3f0c_cccc);
    pub const ARROW_BOTTOM: f32 = -0.15;
    pub const ARROW_UNITS: [u32; 4] = [0x600, 0x200, 0x7f0, 0x3f0];
    /// Glow growth (`0x475CC8` x, `0x475CDC` y) and alpha (`0x47CFF4`).
    pub const GLOW_X: f32 = 0.6;
    pub const GLOW_Y: f32 = 0.3;
    pub const GLOW_ALPHA: f32 = 30.0;
}

/// Texture names by texture id (`0x80F20` index; the name is in the row before, as for the reticles), the icons
/// and overlays the icon table uses (ids 0x1D..=0x8C, all in `pak/gun.pak`) and the fallback id 0x119.
pub const WEAPON_TEXTURES: [(u32, &str); 113] = [
    (0x1d, "gunim_assaultrifle_b"),
    (0x1e, "gunim_assaultrifle_out_b"),
    (0x1f, "gunim_machinepistol_b"),
    (0x20, "gunim_machinepistol_out_b"),
    (0x21, "gunim_machinepistol_double_b"),
    (0x22, "gunim_machinepistol_double_out_b"),
    (0x23, "gunim_pistol9mm_b"),
    (0x24, "gunim_pistol9mm_out_b"),
    (0x25, "gunim_pistol9mm_double_b"),
    (0x26, "gunim_pistol9mm_double_out_b"),
    (0x27, "gunim_assaultshotgun_b"),
    (0x28, "gunim_assaultshotgun_out_b"),
    (0x29, "gunim_rlauncher_b"),
    (0x2a, "gunim_rlauncher_out_b"),
    (0x2b, "gunim_frocket_b"),
    (0x2c, "gunim_frocket_out_b"),
    (0x2d, "gunim_frocket_alt_b"),
    (0x2e, "gunim_frocket_alt_out_b"),
    (0x2f, "gunim_fsniper_b"),
    (0x30, "gunim_fsniper_out_b"),
    (0x31, "gunim_fsniper_alt_out_b"),
    (0x32, "gunim_plasma_b"),
    (0x33, "gunim_plasma_out_b"),
    (0x34, "gunim_remotemine_b"),
    (0x35, "gunim_remotemine_out_b"),
    (0x36, "gunim_detonator_b"),
    (0x37, "gunim_detonator_out_b"),
    (0x38, "gunim_proximitymine_b"),
    (0x39, "gunim_proximitymine_out_b"),
    (0x3a, "gunim_timedmine_b"),
    (0x3b, "gunim_timedmine_out_b"),
    (0x3c, "gunim_uplink_b"),
    (0x3d, "gunim_uplink_out_b"),
    (0x3e, "gunim_punch_b"),
    (0x3f, "gunim_punch_out_b"),
    (0x40, "gunim_uplink_map_b"),
    (0x41, "gunim_uplink_map_out_b"),
    (0x42, "gunim_grenade_b"),
    (0x43, "gunim_grenade_out_b"),
    (0x44, "gunim_flare_b"),
    (0x45, "gunim_flare_out_b"),
    (0x46, "gunim_flare_double_b"),
    (0x47, "gunim_flare_double_out_b"),
    (0x48, "gunim_baseballbat_b"),
    (0x49, "gunim_baseballbat_out_b"),
    (0x4a, "gunim_electrotool_b"),
    (0x4b, "gunim_electrotool_out_b"),
    (0x4c, "gunim_electrotool_alt_b"),
    (0x4d, "gunim_electrotool_alt_out_b"),
    (0x4e, "gunim_flamethrower_b"),
    (0x4f, "gunim_flamethrower_out_b"),
    (0x50, "gunim_doubleshotgun_b"),
    (0x51, "gunim_doubleshotgun_out_b"),
    (0x52, "gunim_ghostgun_b"),
    (0x53, "gunim_ghostgun_out_b"),
    (0x54, "gunim_luger_b"),
    (0x55, "gunim_luger_out_b"),
    (0x56, "gunim_magnum_b"),
    (0x57, "gunim_magnum_out_b"),
    (0x58, "gunim_tnt_b"),
    (0x59, "gunim_tnt_out_b"),
    (0x5a, "gunim_minigun_b"),
    (0x5b, "gunim_minigun_out_b"),
    (0x5c, "gunim_minigun_spin_b"),
    (0x5d, "gunim_minigun_spin_out_b"),
    (0x5e, "gunim_nemosmg_b"),
    (0x5f, "gunim_nemosmg_out_b"),
    (0x60, "gunim_nemosmg_double_b"),
    (0x61, "gunim_nemosmg_double_out_b"),
    (0x62, "gunim_scifihandgun_b"),
    (0x63, "gunim_scifihandgun_out_b"),
    (0x64, "gunim_scifihandgun_bounce_b"),
    (0x65, "gunim_scifihandgun_bounce_out_b"),
    (0x66, "gunim_scifihandgun_double_b"),
    (0x67, "gunim_scifihandgun_double_out_b"),
    (0x68, "gunim_scifihandgun_double_bounce_b"),
    (0x69, "gunim_scifihandgun_double_bounce_out_b"),
    (0x6a, "gunim_sniperbolt_b"),
    (0x6b, "gunim_sniperbolt_out_b"),
    (0x6c, "gunim_sniper_b"),
    (0x6d, "gunim_sniper_out_b"),
    (0x6e, "gunim_dartgun_b"),
    (0x6f, "gunim_dartgun_out_b"),
    (0x70, "gunim_fshotgun_b"),
    (0x71, "gunim_fshotgun_out_b"),
    (0x72, "gunim_lugerx2_b"),
    (0x73, "gunim_lugerx2_out_b"),
    (0x74, "gunim_sluger_b"),
    (0x75, "gunim_sluger_out_b"),
    (0x76, "gunim_monkeygun_b"),
    (0x77, "gunim_monkeygun_out_b"),
    (0x78, "gunim_monkeygunx2_b"),
    (0x79, "gunim_monkeygunx2_out_b"),
    (0x7a, "gunim_magcharger_b"),
    (0x7b, "gunim_magcharger_out_b"),
    (0x7c, "gunim_magcharger_thermal_b"),
    (0x7d, "gunim_magcharger_thermal_out_b"),
    (0x7e, "gunim_nemosmg_rocket_b"),
    (0x7f, "gunim_nemosmg_rocket_out_b"),
    (0x80, "gunim_spistol9mm_b"),
    (0x81, "gunim_spistol9mm_out_b"),
    (0x82, "gunim_sbp500_b"),
    (0x83, "gunim_sbp500_out_b"),
    (0x84, "gunim_harpoon_b"),
    (0x85, "gunim_harpoon_out_b"),
    (0x86, "gunim_glock_b"),
    (0x87, "gunim_glock_out_b"),
    (0x88, "gunim_glock_silencer_b"),
    (0x89, "gunim_glock_out_silencer_b"),
    (0x8a, "gunim_glock_double_b"),
    (0x8b, "gunim_glock_double_out_b"),
    (0x8c, "gunim_brick_b"),
    (0x119, "crosshair_fixedgun"),
];

/// Candidate names of the arrow atlas (`[0x4D6C7C]`, texture record 673); the shell resolves the name the running
/// table gives and logs it.
pub const ARROW_TEXTURES: [&str; 2] = ["xbbuttons", "xbdpad"];

/// The texture name of texture id `id` ([`WEAPON_TEXTURES`]).
pub fn texture_name(id: u32) -> Option<&'static str> {
    WEAPON_TEXTURES
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, n)| *n)
}

/// The panel: nine-slice pieces (`0x12CC70` geometry, [`crate::hud_rank::panel_pieces_scaled`]), the vertex alpha
/// byte, and the original call's arguments (shadow check).
#[derive(Clone, Debug, PartialEq)]
pub struct PanelDraw {
    pub pieces: Vec<crate::hud_rank::RowPiece>,
    pub alpha_byte: u8,
    pub call: [f32; 5],
}

/// One weapon icon (`0x141650` replayed): rects in guest pixels, colour words R G B A high to low (0x80 = 1).
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponSlotDraw {
    pub weapon: u32,
    pub icon: u32,
    pub overlay: Option<u32>,
    /// Icon rectangle x0, y0, x1, y1 and the vertical fade band (`0x561C0`: alpha ramps from 0 at `[0]` to 1 at
    /// `[1]`, back to 0 from `[2]` to `[3]`).
    pub rect: [f32; 4],
    pub fade: [f32; 4],
    /// Texture coordinates of the icon draw (1/16 texel units of the icon's record over its record size): the
    /// overlay reuses the icon's units, normalised by the overlay's record.
    pub uv_units: [u32; 2],
    /// Record sizes (texels) of the icon and the overlay texture.
    pub icon_size: [u16; 2],
    pub overlay_size: Option<[u16; 2]>,
    pub shadow_colour: u32,
    pub overlay_colour: u32,
    /// The switch flash glow (rect, colour), drawn additively with the overlay texture.
    pub glow: Option<([f32; 4], u32)>,
    /// The group arrow (rect, colour), atlas `[0x4D6C7C]`, clipped to the HUD rectangle instead of the panel.
    pub arrow: Option<([f32; 4], u32)>,
    pub selected: bool,
    pub empty: bool,
}

/// Everything one call draws.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponPanelDraw {
    pub panel: Option<PanelDraw>,
    /// The scissor the icons are drawn with (`0x1A010` integer rect), `None` without a panel (left as it was).
    pub clip: Option<[f32; 4]>,
    /// The HUD rectangle (the arrows' scissor).
    pub hud_clip: [f32; 4],
    pub slots: Vec<WeaponSlotDraw>,
    /// Mode 1: the slide target the original stores in the panel record `+0x1760` (Rust state, the gate writes it).
    pub slide_target: Option<f32>,
    /// HUD record of the call (the `+0x1760` address base).
    pub record: u32,
    pub mode: u32,
    pub slide: f32,
    /// The arrow atlas: its name as the running texture table gives it (one of [`ARROW_TEXTURES`]) and record size.
    pub arrow_texture: Option<&'static str>,
    pub arrow_size: [u16; 2],
}

/// The call's outcome.
#[derive(Clone, Debug, PartialEq)]
pub enum WeaponCall {
    Draw(WeaponPanelDraw),
    /// No viewer slot: the original returns at once.
    Nothing,
    /// Not replayable (unreadable state): the original draws.
    Legacy(&'static str),
}

/// The mode 1 slide targets per HUD slot as Rust-owned state (the original keeps it in record `+0x1760`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SlideTargets {
    pub target: [Option<f32>; 4],
}

impl SlideTargets {
    /// Record the target of a replayed call; returns the value the gate stores for the guest's updater.
    pub fn update(&mut self, slot: usize, draw: &WeaponPanelDraw) -> Option<f32> {
        let target = draw.slide_target?;
        if let Some(entry) = self.target.get_mut(slot) {
            *entry = Some(target);
        }
        Some(target)
    }
}

struct Viewport {
    x0: i32,
    x1: i32,
    y0: i32,
    y1: i32,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl Viewport {
    fn read<G: GuestRead + ?Sized>(g: &G) -> Option<Self> {
        use weapon_addr::*;
        let vp = g.read32(VIEWPORT_PTR).filter(|&p| p != 0)?;
        let i = |va: u32| g.read32(va).map(|v| v as i32);
        Some(Viewport {
            x0: i(vp)?,
            x1: i(vp + 4)?,
            y0: i(vp + 0x14)?,
            y1: i(vp + 0x18)?,
            left: i(MARGIN_LEFT)?,
            top: i(MARGIN_TOP)?,
            right: i(MARGIN_RIGHT)?,
            bottom: i(MARGIN_BOTTOM)?,
        })
    }
    /// HUD rectangle width and height (integer, as the drawer subtracts them).
    fn width(&self) -> f32 {
        self.x1
            .wrapping_sub(self.x0)
            .wrapping_sub(self.left)
            .wrapping_sub(self.right) as f32
    }
    fn height(&self) -> f32 {
        self.y1
            .wrapping_sub(self.y0)
            .wrapping_sub(self.bottom)
            .wrapping_sub(self.top) as f32
    }
    fn left_edge(&self) -> f32 {
        self.x0.wrapping_add(self.left) as f32
    }
    fn top_edge(&self) -> f32 {
        self.y0.wrapping_add(self.top) as f32
    }
}

/// `0x141590(percent)`: `percent` % of the HUD height, doubled in the split layouts.
fn height_percent<G: GuestCall + ?Sized>(g: &G, vp: &Viewport, percent: f32) -> Option<f32> {
    use weapon_addr::*;
    let net = || g.call(FN_NET_PLAYERS, &[]);
    let split_candidate = net()? == 2 || {
        let viewer = g.read32(VIEWER)?;
        net()? == 3 && viewer != 0 && g.read32(viewer + 4)? == 0
    };
    let doubled = split_candidate && {
        let mode = g.read32(MODE_BYTE & !3)? >> ((MODE_BYTE & 3) * 8) & 0xff;
        if mode != 10 || (g.read32(LEVEL_PLAYERS)? as i32) <= 1 {
            true
        } else {
            let session = g.read32(SESSION)?;
            let player = if session == 0 {
                0
            } else {
                g.read32(session + 0x158c)?
            };
            player != 0 && g.read32(player + 0x21c)? & 1 != 0
        }
    };
    let h = vp.height() * percent * 0.01;
    Some(if doubled { h * 2.0 } else { h })
}

/// Texture record size (texels) of texture id `id` (`0x80F20` then the record table).
fn texture_size<G: GuestRead + ?Sized>(g: &G, id: u32) -> Option<[u16; 2]> {
    use weapon_addr::*;
    let handle = g.read32(TEXTURE_ROWS + id.checked_mul(24)?)?;
    texture_size_by_handle(g, handle)
}

fn texture_size_by_handle<G: GuestRead + ?Sized>(g: &G, handle: u32) -> Option<[u16; 2]> {
    let record = g
        .read32(g.read32(weapon_addr::TEXTURE_RECORDS_PTR)?)?
        .checked_add(handle.checked_mul(0x34)?)?;
    Some([g.read16(record + 0x26)?, g.read16(record + 0x2a)?])
}

/// The weapons of the order list that the panel shows (owned, not hidden).
struct Owned {
    ids: Vec<(u32, u32)>,
}

impl Owned {
    fn read<G: GuestRead + ?Sized>(g: &G, extension: u32) -> Option<Self> {
        use weapon_addr::*;
        let mut ids = Vec::new();
        for i in 0..ORDER_LEN {
            let id = g.read32(ORDER + 4 * i)?;
            if id == 0 {
                continue;
            }
            let group = g.read32(WEAPONS + id.checked_mul(WEAPON_SIZE)? + WEAPON_GROUP)?;
            let owned = g.read32((extension + OWNED + id) & !3)?
                >> (((extension + OWNED + id) & 3) * 8)
                & 0xff;
            if owned == 0 || g.read32(WEAPONS + id * WEAPON_SIZE)? & 0x10 != 0 {
                continue;
            }
            if id == 1
                && g.read32((extension + PUNCH_HIDDEN) & !3)?
                    >> (((extension + PUNCH_HIDDEN) & 3) * 8)
                    & 0xff
                    != 0
            {
                continue;
            }
            ids.push((id, group));
        }
        Some(Owned { ids })
    }
}

fn byte<G: GuestRead + ?Sized>(g: &G, va: u32) -> Option<u32> {
    Some(g.read32(va & !3)? >> ((va & 3) * 8) & 0xff)
}

/// x87 `_ftol2` truncation of the colour arithmetic. The products and sums are rounded to single precision as the
/// running host's x87 does (MEASURED by the shadow check: double precision put one green byte in 36 of 1,440 one
/// below the original's).
fn ftol(v: f32) -> i32 {
    v as i32
}

/// The slot colours of `0x141650`: shadow (dark) and overlay words, and the glow's rgb.
fn slot_colours(selected: bool, empty: bool, alpha: f32, flash: f32) -> (u32, u32, u32) {
    let (a, f) = (alpha, flash);
    let mut shadow = (ftol(a * 40.0) as u32 & 0xff) | 0x1414_1400;
    let mut overlay = (ftol(a * 80.0) as u32 & 0xff) | 0x554b_4100;
    let mut rgb = 0;
    if selected {
        let full = ftol(a * 127.0) as u32;
        shadow = (full & 0xff) | 0x1414_1400;
        // Red is `(flash + 1) * 110` either way (the x87 multiply comes before the empty test).
        let r = ftol((f + 1.0) * 110.0);
        let (g, b) = if empty {
            (ftol(f * 20.0 + 50.0), ftol(f * 5.0 + 20.0))
        } else {
            (ftol(f * 50.0 + 100.0), ftol(f * 15.0 + 90.0))
        };
        rgb = ((((r as u32) << 8) | (g as u32 & 0xff)) << 8 | (b as u32 & 0xff)) << 8;
        overlay = rgb | full;
    } else if empty {
        overlay = (ftol(a * 80.0) as u32 & 0xff) | 0x551e_1400;
    }
    (shadow, overlay, rgb)
}

/// The per-slot arguments of `0x141650` the panel computes.
struct SlotArgs {
    selected: bool,
    empty: bool,
    icon: u32,
    overlay: Option<u32>,
    x_percent: f32,
    y_percent: f32,
    width_percent: f32,
    scale: [f32; 2],
    alpha: f32,
    flash: f32,
    glow: f32,
    arrow: bool,
}

/// `0x141650` replayed.
fn slot_draw<G: GuestCall + ?Sized>(
    g: &G,
    vp: &Viewport,
    cycle: f32,
    weapon: u32,
    a: &SlotArgs,
) -> Option<WeaponSlotDraw> {
    let hud_w = vp.width();
    let half_w = hud_w * a.width_percent * 0.005;
    let half_h = height_percent(g, vp, k::HEIGHT * 0.5)?;
    let cx = hud_w * a.x_percent * 0.01 + vp.left_edge() + half_w;
    let hud_h = vp.height();
    let top = vp.top_edge();
    let cy = hud_h * a.y_percent * 0.01 + top + half_h;
    let (sw, sh) = (half_w * a.scale[0], half_h * a.scale[1]);
    let rect = [cx - sw, cy - sh, sw + cx, sh + cy];
    let fade = k::FADE.map(|p| hud_h * p * 0.01 + top);
    let icon_size = texture_size(g, a.icon)?;
    let uv_units = [u32::from(icon_size[0]) << 4, u32::from(icon_size[1]) << 4];
    let overlay_size = match a.overlay {
        Some(id) => Some(texture_size(g, id)?),
        None => None,
    };
    let (shadow_colour, overlay_colour, rgb) = slot_colours(a.selected, a.empty, a.alpha, a.flash);
    let glow = (a.overlay.is_some() && a.glow > 0.0).then(|| {
        let gx = (a.glow * k::GLOW_X + 1.0) * half_w * a.scale[0];
        let gy = (a.glow * k::GLOW_Y + 1.0) * half_h * a.scale[1];
        let alpha = ((1.0 - a.glow) * k::GLOW_ALPHA) as i32 as u32;
        ([cx - gx, cy - gy, gx + cx, gy + cy], alpha | rgb)
    });
    let arrow = (a.arrow && (cycle == 0.0 || !a.selected)).then(|| {
        let ax = hud_w * k::ARROW_HALF_WIDTH;
        (
            [
                cx - ax,
                rect[1] - half_h * k::ARROW_TOP,
                ax + cx,
                rect[1] - half_h * k::ARROW_BOTTOM,
            ],
            overlay_colour,
        )
    });
    Some(WeaponSlotDraw {
        weapon,
        icon: a.icon,
        overlay: a.overlay,
        rect,
        fade,
        uv_units,
        icon_size,
        overlay_size,
        shadow_colour,
        overlay_colour,
        glow,
        arrow,
        selected: a.selected,
        empty: a.empty,
    })
}

pub struct WeaponPanel;

impl WeaponPanel {
    /// Replays `0x141E30` on the live state.
    pub fn read<G: GuestCall + ?Sized>(g: &G) -> WeaponCall {
        match Self::replay(g, None) {
            Some(Some(draw)) => WeaponCall::Draw(draw),
            Some(None) => WeaponCall::Nothing,
            None => WeaponCall::Legacy("unreadable state"),
        }
    }

    /// Native renderer entry: lifecycle comes from owned state, never the HUD guest record.
    /// World inventory, original tables and viewport remain read-only neighbouring inputs.
    pub fn read_owned<G: GuestCall + ?Sized>(
        g: &G,
        animation: &crate::hud_producers::WeaponAnimation,
    ) -> WeaponCall {
        match Self::replay(g, Some(animation)) {
            Some(Some(draw)) => WeaponCall::Draw(draw),
            Some(None) => WeaponCall::Nothing,
            None => WeaponCall::Legacy("unreadable neighbouring inputs"),
        }
    }

    fn replay<G: GuestCall + ?Sized>(
        g: &G,
        animation: Option<&crate::hud_producers::WeaponAnimation>,
    ) -> Option<Option<WeaponPanelDraw>> {
        use weapon_addr::*;
        let viewer = g.read32(VIEWER)?;
        let slot = g.read32(viewer + 4)? as i32;
        if slot < 0 {
            return Some(None);
        }
        let record = HUD_RECORDS.wrapping_add((slot as u32).wrapping_mul(HUD_RECORD_SIZE));
        let animation = match animation {
            Some(value) => *value,
            None => crate::hud_producers::WeaponAnimation {
                phase: g.read32(record + STATE)?,
                flash: g.read_f32(record + FLASH)?,
                alpha: g.read_f32(record + ALPHA)?,
                slide: g.read_f32(record + SLIDE)?,
                target: g.read_f32(record + SLIDE_TARGET)?,
                cycle: g.read_f32(record + CYCLE)?,
            },
        };
        let extension = g.read32(g.read32(viewer + 0x14)? + OBJECT_EXTENSION)?;
        let owned = Owned::read(g, extension)?;
        let current = g.read32(extension + CURRENT)?;
        let current_group = g.read32(WEAPONS + current.checked_mul(WEAPON_SIZE)? + WEAPON_GROUP)?;
        // `0x141BF0` and the first walk: columns by group in order of first appearance, the selected one wider.
        let mut index = [0u32; 0x46];
        let mut count = [0u32; 0x46];
        let (mut pos, mut selected_pos, mut columns) = (0.0f32, 0.0f32, 0u32);
        for &(_, group) in &owned.ids {
            let g_index = group as usize;
            if g_index >= 0x46 {
                return None;
            }
            if count[g_index] == 0 {
                index[g_index] = columns;
                if current_group == group {
                    selected_pos = pos + k::SELECTED_CENTRE;
                    pos += k::SELECTED_EXTRA;
                }
                columns += 1;
                pos += k::GROUP_STEP;
            }
            count[g_index] += 1;
        }
        let total = (columns as f32 - 1.0) * 0.0 + columns as f32 * k::PITCH;
        let start = k::CENTRE - total * 0.5;
        let mode = g.read32(MODE)?;
        let mut slide_target = None;
        let slide = match mode {
            0 => k::FIXED_SLIDE,
            1 => {
                let target = if selected_pos <= k::TARGET_LIMIT || selected_pos.is_nan() {
                    0.0
                } else if selected_pos > k::TARGET_VALUE {
                    k::TARGET_VALUE
                } else {
                    selected_pos - k::TARGET_OFFSET
                };
                slide_target = Some(target);
                k::FIXED_SLIDE - animation.slide
            }
            _ => k::CENTRE - selected_pos,
        };
        let vp = Viewport::read(g)?;
        let (hud_w, hud_h) = (vp.width(), vp.height());
        let y0 = hud_h * slide * 0.01 + vp.top_edge();
        let y1 = height_percent(g, &vp, k::HEIGHT)? + (hud_h * slide * 0.01 + vp.top_edge());
        let (mut x0, mut x1) = (vp.left_edge(), vp.x1.wrapping_sub(vp.right) as f32);
        // The second walk: the extent of the shown icons.
        let mut first = false;
        let icon_of = |id: u32| -> Option<u32> {
            let icon = g.read32(ICONS + id.checked_mul(ICON_SIZE)?)?;
            Some(if icon == u32::MAX { 0x119 } else { icon })
        };
        let wide =
            |icon: u32| -> Option<bool> { Some(texture_size(g, icon)?[0] == k::WIDE_TEXELS) };
        for &(id, group) in &owned.ids {
            if byte(g, viewer + REMEMBERED + group)? != id {
                continue;
            }
            let icon = icon_of(id)?;
            let (w, s) = if wide(icon)? {
                (k::WIDE_ICON_WIDTH, k::PITCH)
            } else {
                (k::ICON_WIDTH, k::SLOT_WIDTH)
            };
            let column = index[group as usize] as f32 * k::PITCH + start;
            let left = ((s - w) + (k::PITCH - s)) * 0.5 + column;
            if first {
                x1 = vp.left_edge() + hud_w * left * 0.01 + w * hud_w * 0.01;
            } else {
                x0 = vp.left_edge() + hud_w * left * 0.01;
                x1 = x0 + w * hud_w * 0.01;
                first = true;
            }
        }
        let margin = hud_w * k::PANEL_MARGIN;
        let base = vp.left_edge();
        x0 = if base > x0 - margin {
            base
        } else {
            x0 - margin
        };
        let limit = vp.x1.wrapping_sub(vp.right) as f32;
        x1 = if margin + x1 > limit {
            limit
        } else {
            x1 + margin
        };
        let state = animation.phase;
        let (mut panel, mut clip) = (None, None);
        if x1 - x0 > k::MIN_SIZE && y1 - y0 > k::MIN_SIZE {
            let fade = if state != 0 {
                let v = animation.alpha * 2.0;
                1.0 - if v > 1.0 { 1.0 } else { v }
            } else {
                1.0
            };
            let alpha = 1.0 - fade;
            let scale_x = g.read_f32(crate::hud_rank::rank_addr::SCALE_X)?;
            panel = Some(PanelDraw {
                pieces: crate::hud_rank::panel_pieces_scaled(scale_x, [x0, y0, x1, y1]),
                alpha_byte: ((alpha * crate::hud_rank::row::PANEL_ALPHA) as i32 & 0xff) as u8,
                call: [x0, y0, x1, y1, alpha],
            });
            clip = Some([
                (x0 as i32 + 1) as f32,
                (y0 as i32 + 1) as f32,
                (x1 as i32 - 1) as f32,
                (y1 as i32 - 1) as f32,
            ]);
        }
        // The icons.
        let cycle = animation.cycle;
        let panel_alpha = animation.alpha;
        let mut slots = Vec::new();
        for &(id, group) in &owned.ids {
            let icon = icon_of(id)?;
            let column = index[group as usize] as f32 * k::PITCH + start;
            let mut next = id;
            let mut cycling = false;
            if count[group as usize] > 1 {
                let previous = g.call(FN_PREVIOUS, &[extension, id, 1])?;
                next = g.call(FN_NEXT, &[extension, id, 1])?;
                if current_group == group
                    && ((0.0 > cycle && previous == current) || (cycle > 0.0 && next == current))
                {
                    cycling = true;
                }
            }
            if byte(g, viewer + REMEMBERED + group)? != id && !cycling {
                continue;
            }
            let cube = cycle * cycle * cycle;
            let (width, slot_w) = if wide(icon)? {
                (k::WIDE_ICON_WIDTH, k::PITCH)
            } else {
                (k::ICON_WIDTH, k::SLOT_WIDTH)
            };
            let x_base = (k::PITCH - slot_w) * 0.5 + column;
            let mut y_percent = slide;
            let mut alpha = panel_alpha;
            if current == id {
                y_percent = cube * k::SELECTED_DROP + slide;
            } else if cycling {
                let shifted = if 0.0 > cube { cube + 1.0 } else { cube - 1.0 };
                alpha *= cube.abs();
                y_percent = shifted * k::PITCH + slide;
            }
            let (mut flash, mut glow) = (0.0f32, 0.0f32);
            if current == id && animation.phase == 1 {
                let t = animation.flash;
                let f = (0.5 - t) * 2.0;
                flash = if 0.0 > f { 0.0 } else { f };
                let v = t * 4.0;
                glow = if v > 1.0 { 1.0 } else { v };
            }
            let overlay = g.read32(ICONS + id * ICON_SIZE + 4)?;
            let args = SlotArgs {
                selected: current == id,
                empty: g.call(FN_EMPTY, &[extension, id])? != 0,
                icon,
                overlay: (overlay != u32::MAX).then_some(overlay),
                x_percent: (slot_w - width) * 0.5 + x_base,
                y_percent,
                width_percent: width,
                scale: [
                    g.read_f32(ICONS + id * ICON_SIZE + 8)?,
                    g.read_f32(ICONS + id * ICON_SIZE + 12)?,
                ],
                alpha,
                flash,
                glow,
                arrow: next != id,
            };
            slots.push(slot_draw(g, &vp, cycle, id, &args)?);
        }
        let hud_clip = [
            vp.left_edge(),
            vp.top_edge(),
            vp.x1.wrapping_sub(vp.right) as f32,
            vp.y1.wrapping_sub(vp.bottom) as f32,
        ];
        // The arrow atlas `[0x4D6C7C]`: the handle of a texture table record whose name pointer sits 8 bytes before it.
        let arrow_handle = g.read32(ARROW_HANDLE)?;
        let arrow_name = g
            .read32(ARROW_HANDLE - 8)
            .and_then(|p| crate::hud_rank::guest_string(g, p));
        let arrow_texture = arrow_name.and_then(|n| {
            ARROW_TEXTURES
                .into_iter()
                .find(|t| t.as_bytes() == n.as_slice())
        });
        let arrow_size = if slots.iter().any(|s| s.arrow.is_some()) {
            texture_size_by_handle(g, arrow_handle)?
        } else {
            [0, 0]
        };
        Some(Some(WeaponPanelDraw {
            panel,
            clip,
            hud_clip,
            slots,
            slide_target,
            record,
            mode,
            slide,
            arrow_texture,
            arrow_size,
        }))
    }
}

/// One primitive call the original makes inside `0x141E30` (shadow check): the function and its stack words.
#[derive(Clone, Debug, PartialEq)]
pub struct PrimitiveCall {
    pub function: u32,
    pub args: Vec<u32>,
}

impl WeaponPanelDraw {
    /// The primitive calls the original makes for this draw, in order: the panel `0x12CC70`, per icon the faded
    /// quad `0x561C0` (icon, then overlay) and the arrow `0x81B70`. The glow (`0x1B7B0`) is not listed: the same
    /// function draws inside `0x561C0` and `0x81B70`, so the observer cannot tell them apart.
    pub fn primitive_calls(&self) -> Vec<PrimitiveCall> {
        let f = |v: f32| v.to_bits();
        let mut calls = Vec::new();
        if let Some(panel) = &self.panel {
            let [x0, y0, x1, y1, alpha] = panel.call;
            calls.push(PrimitiveCall {
                function: weapon_addr::FN_PANEL_QUAD,
                args: vec![f(x0), f(y0), f(x1), f(y1), f(alpha), 3],
            });
        }
        for slot in &self.slots {
            let quad = |colour: u32| {
                let [x0, y0, x1, y1] = slot.rect;
                let [a, b, c, d] = slot.fade;
                vec![
                    f(x0),
                    f(y0),
                    f(x1),
                    f(y1),
                    0,
                    0,
                    slot.uv_units[0],
                    slot.uv_units[1],
                    colour,
                    f(a),
                    f(b),
                    f(c),
                    f(d),
                ]
            };
            calls.push(PrimitiveCall {
                function: weapon_addr::FN_FADED_QUAD,
                args: quad(slot.shadow_colour),
            });
            if slot.overlay.is_some() {
                calls.push(PrimitiveCall {
                    function: weapon_addr::FN_FADED_QUAD,
                    args: quad(slot.overlay_colour),
                });
            }
            if let Some(([x0, y0, x1, y1], colour)) = slot.arrow {
                calls.push(PrimitiveCall {
                    function: weapon_addr::FN_ARROW,
                    args: vec![f(x0), f(y0), f(x1), f(y1), colour],
                });
            }
        }
        calls
    }
}

/// Number of stack words the shadow observer reads for each primitive.
pub fn primitive_arity(function: u32) -> Option<usize> {
    match function {
        weapon_addr::FN_PANEL_QUAD => Some(6),
        weapon_addr::FN_FADED_QUAD => Some(13),
        weapon_addr::FN_ARROW => Some(5),
        _ => None,
    }
}

/// Texture coordinates `[u0, v0, u1, v1]` (0..1) of 1/16-texel units on a record of `size` texels.
pub fn units_uv(units: [u32; 4], size: [u16; 2]) -> [f32; 4] {
    let (w, h) = (
        f32::from(size[0].max(1)) * 16.0,
        f32::from(size[1].max(1)) * 16.0,
    );
    [
        units[0] as f32 / w,
        units[1] as f32 / h,
        units[2] as f32 / w,
        units[3] as f32 / h,
    ]
}

/// Clip a rect with texture coordinates to `clip` (x0, y0, x1, y1), the scissor `0x1A010` sets; None when nothing
/// is left.
pub fn clip_rect(
    rect: [f32; 4],
    uv: [f32; 4],
    clip: Option<[f32; 4]>,
) -> Option<([f32; 4], [f32; 4])> {
    let [x0, y0, x1, y1] = rect;
    if !(x1 > x0 && y1 > y0) {
        return None;
    }
    let Some([cx0, cy0, cx1, cy1]) = clip else {
        return Some((rect, uv));
    };
    let (nx0, ny0, nx1, ny1) = (x0.max(cx0), y0.max(cy0), x1.min(cx1), y1.min(cy1));
    if !(nx1 > nx0 && ny1 > ny0) {
        return None;
    }
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let (tx0, tx1) = ((nx0 - x0) / (x1 - x0), (nx1 - x0) / (x1 - x0));
    let (ty0, ty1) = ((ny0 - y0) / (y1 - y0), (ny1 - y0) / (y1 - y0));
    Some((
        [nx0, ny0, nx1, ny1],
        [
            lerp(uv[0], uv[2], tx0),
            lerp(uv[1], uv[3], ty0),
            lerp(uv[0], uv[2], tx1),
            lerp(uv[1], uv[3], ty1),
        ],
    ))
}

/// The faded quad `0x561C0` as alpha-weighted pieces: the full-alpha middle of the band, and the two ramps cut
/// into [`RAMP_STRIPS`] strips each at the strip's mid alpha (INFERRED approximation of the per-vertex ramp the
/// original draws with `0x1BA70`; the renderer has one colour per quad). Each piece: rect, uv, alpha factor.
pub const RAMP_STRIPS: usize = 4;
pub fn faded_pieces(
    rect: [f32; 4],
    uv: [f32; 4],
    fade: [f32; 4],
) -> Vec<([f32; 4], [f32; 4], f32)> {
    let [_, y0, _, y1] = rect;
    let [a, b, c, d] = fade;
    let mut bands: Vec<(f32, f32, f32)> = Vec::new();
    let ramp = |from: f32, to: f32, up: bool, bands: &mut Vec<(f32, f32, f32)>| {
        if to <= from {
            return;
        }
        let step = (to - from) / RAMP_STRIPS as f32;
        for i in 0..RAMP_STRIPS {
            let (s0, s1) = (from + step * i as f32, from + step * (i + 1) as f32);
            let t = (i as f32 + 0.5) / RAMP_STRIPS as f32;
            bands.push((s0, s1, if up { t } else { 1.0 - t }));
        }
    };
    ramp(a, b, true, &mut bands);
    bands.push((b, c, 1.0));
    ramp(c, d, false, &mut bands);
    bands
        .into_iter()
        .filter_map(|(s0, s1, alpha)| {
            let (top, bottom) = (y0.max(s0), y1.min(s1));
            if bottom <= top {
                return None;
            }
            let band = [rect[0], top, rect[2], bottom];
            let (r, u) = clip_rect(rect, uv, Some([f32::MIN, band[1], f32::MAX, band[3]]))?;
            Some((r, u, alpha))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Tier 0, FABRICATED guest values on the real offsets.
    use super::*;
    use std::collections::HashMap;
    use weapon_addr::*;

    struct Guest {
        words: HashMap<u32, u32>,
        calls: HashMap<(u32, u32), u32>,
    }
    impl GuestRead for Guest {
        fn read32(&self, va: u32) -> Option<u32> {
            Some(self.words.get(&va).copied().unwrap_or(0))
        }
    }
    impl GuestCall for Guest {
        fn call(&self, va: u32, args: &[u32]) -> Option<u32> {
            match va {
                FN_NET_PLAYERS => Some(1),
                FN_EMPTY => Some(0),
                FN_PREVIOUS | FN_NEXT => {
                    Some(self.calls.get(&(va, args[1])).copied().unwrap_or(args[1]))
                }
                _ => None,
            }
        }
    }

    fn set_byte(m: &mut HashMap<u32, u32>, va: u32, v: u8) {
        let w = m.entry(va & !3).or_insert(0);
        *w = (*w & !(0xff << ((va & 3) * 8))) | (u32::from(v) << ((va & 3) * 8));
    }

    /// One player at 640 x 480, no margins; weapons 2 (group 0) and 22, 14 (group 3, 22 current).
    fn guest(mode: u32) -> Guest {
        let mut m = HashMap::new();
        let (viewer, object, ext, vp, records) = (0x100, 0x200, 0x1000, 0x300, 0x5000);
        m.insert(VIEWER, viewer);
        m.insert(viewer + 4, 0);
        m.insert(viewer + 0x14, object);
        m.insert(object + OBJECT_EXTENSION, ext);
        m.insert(ext + CURRENT, 22);
        for (i, id) in [2u32, 22, 14].into_iter().enumerate() {
            m.insert(ORDER + 4 * i as u32, id);
            set_byte(&mut m, ext + OWNED + id, 1);
            m.insert(ICONS + id * ICON_SIZE, 0x23);
            m.insert(ICONS + id * ICON_SIZE + 4, 0x24);
            m.insert(ICONS + id * ICON_SIZE + 8, 1.0f32.to_bits());
            m.insert(ICONS + id * ICON_SIZE + 12, 1.0f32.to_bits());
        }
        m.insert(WEAPONS + 22 * WEAPON_SIZE + WEAPON_GROUP, 3);
        m.insert(WEAPONS + 14 * WEAPON_SIZE + WEAPON_GROUP, 3);
        set_byte(&mut m, viewer + REMEMBERED, 2);
        set_byte(&mut m, viewer + REMEMBERED + 3, 22);
        m.insert(MODE, mode);
        m.insert(VIEWPORT_PTR, vp);
        m.insert(vp + 4, 640);
        m.insert(vp + 0x18, 480);
        m.insert(TEXTURE_RECORDS_PTR, 0x400);
        m.insert(0x400, records);
        m.insert(TEXTURE_ROWS + 0x23 * 24, 1);
        m.insert(records + 0x34 + 0x24, 64 << 16);
        m.insert(records + 0x34 + 0x28, 32 << 16);
        m.insert(crate::hud_rank::rank_addr::SCALE_X, 1.0f32.to_bits());
        let record = HUD_RECORDS;
        m.insert(record + STATE, 2);
        m.insert(record + ALPHA, 0.25f32.to_bits());
        let mut calls = HashMap::new();
        calls.insert((FN_NEXT, 22), 14);
        calls.insert((FN_PREVIOUS, 22), 14);
        calls.insert((FN_NEXT, 14), 22);
        calls.insert((FN_PREVIOUS, 14), 22);
        Guest { words: m, calls }
    }

    #[test]
    fn owned_animation_does_not_read_guest_lifecycle() {
        struct Inputs(Guest);
        impl GuestRead for Inputs {
            fn read32(&self, va: u32) -> Option<u32> {
                assert!(
                    !(HUD_RECORDS + STATE..=HUD_RECORDS + CYCLE).contains(&va),
                    "guest lifecycle read {va:x}"
                );
                self.0.read32(va)
            }
        }
        impl GuestCall for Inputs {
            fn call(&self, va: u32, args: &[u32]) -> Option<u32> {
                self.0.call(va, args)
            }
        }
        let g = guest(1);
        let expected = WeaponPanel::read(&g);
        let animation = crate::hud_producers::WeaponAnimation {
            phase: 2,
            alpha: 0.25,
            ..Default::default()
        };
        assert_eq!(WeaponPanel::read_owned(&Inputs(g), &animation), expected);
    }

    #[test]
    fn columns_panel_and_icons_replay_the_drawer() {
        let WeaponCall::Draw(draw) = WeaponPanel::read(&guest(0)) else {
            panic!("no draw")
        };
        // Two columns: 2 * 14.625 wide, centred: start 35.375 %; slide 4 %: y 19.2 .. 19.2 + 50.4.
        assert_eq!(draw.slide, 4.0);
        let panel = draw.panel.as_ref().unwrap();
        assert!((panel.call[1] - 19.2).abs() < 1e-4 && (panel.call[3] - 69.6).abs() < 1e-3);
        // Alpha min(2 * 0.25, 1) = 0.5.
        assert!((panel.call[4] - 0.5).abs() < 1e-6);
        assert_eq!(panel.alpha_byte, 36);
        // The remembered weapons of each group: 2 and 22 (the selected one), not 14.
        assert_eq!(
            draw.slots.iter().map(|s| s.weapon).collect::<Vec<_>>(),
            vec![2, 22]
        );
        let selected = &draw.slots[1];
        assert!(selected.selected && selected.arrow.is_some());
        assert_eq!(draw.slots[0].arrow, None);
        // Icon 7.5 % of 640 wide = 48, 10.5 % of 480 high = 50.4; record 64 x 32 texels.
        let [x0, y0, x1, y1] = selected.rect;
        assert!((x1 - x0 - 48.0).abs() < 1e-3 && (y1 - y0 - 50.4).abs() < 1e-3);
        assert_eq!(selected.uv_units, [1024, 512]);
        // Selected colours: shadow alpha trunc(0.25 * 127) = 31, overlay rgb (110, 100, 90) and the same alpha.
        assert_eq!(selected.shadow_colour, 0x1414_141f);
        assert_eq!(selected.overlay_colour, 0x6e64_5a1f);
        // Unselected: shadow trunc(0.25 * 40) = 10, overlay 0x554B41 at trunc(0.25 * 80) = 20.
        assert_eq!(
            (draw.slots[0].shadow_colour, draw.slots[0].overlay_colour),
            (0x1414_140a, 0x554b_4114)
        );
        assert_eq!(draw.slide_target, None);
        // Primitive list: panel, two quads per icon, one arrow.
        let calls = draw.primitive_calls();
        assert_eq!(
            calls.iter().map(|c| c.function).collect::<Vec<_>>(),
            vec![
                FN_PANEL_QUAD,
                FN_FADED_QUAD,
                FN_FADED_QUAD,
                FN_FADED_QUAD,
                FN_FADED_QUAD,
                FN_ARROW
            ]
        );
    }

    #[test]
    fn mode_one_keeps_the_slide_target_as_rust_state() {
        let mut g = guest(1);
        g.words.insert(HUD_RECORDS + SLIDE, 1.5f32.to_bits());
        let WeaponCall::Draw(draw) = WeaponPanel::read(&g) else {
            panic!("no draw")
        };
        assert_eq!(draw.slide, 2.5);
        // Selected centre 10.5 + 6.9375 = 17.4375 <= 64.4: target 0.
        assert_eq!(draw.slide_target, Some(0.0));
        let mut targets = SlideTargets::default();
        assert_eq!(targets.update(0, &draw), Some(0.0));
        assert_eq!(targets.target[0], Some(0.0));
    }

    #[test]
    fn cycling_slides_the_outgoing_weapon_and_no_slot_draws_nothing() {
        let mut g = guest(0);
        g.words.insert(HUD_RECORDS + CYCLE, 0.5f32.to_bits());
        let WeaponCall::Draw(draw) = WeaponPanel::read(&g) else {
            panic!("no draw")
        };
        // 14 is drawn too (next of the selected 22 while the cycle is positive? previous == current is the
        // negative case; with +0.5 the next of 14 is 22): alpha times |0.125|, y 4 + (0.125 - 1) * 14.625.
        let outgoing = draw
            .slots
            .iter()
            .find(|s| s.weapon == 14)
            .expect("outgoing weapon drawn");
        assert!(!outgoing.selected);
        assert_eq!(
            outgoing.shadow_colour & 0xff,
            (0.25f64 * 0.125 * 40.0) as u32
        );
        g.words.insert(0x104, u32::MAX);
        assert_eq!(WeaponPanel::read(&g), WeaponCall::Nothing);
    }

    #[test]
    fn clip_and_fade_pieces() {
        let (r, uv) = clip_rect(
            [0.0, 0.0, 10.0, 10.0],
            [0.0, 0.0, 1.0, 1.0],
            Some([5.0, -1.0, 20.0, 8.0]),
        )
        .unwrap();
        assert_eq!(r, [5.0, 0.0, 10.0, 8.0]);
        assert_eq!(uv, [0.5, 0.0, 1.0, 0.8]);
        assert_eq!(
            clip_rect(
                [0.0, 0.0, 10.0, 10.0],
                [0.0; 4],
                Some([11.0, 0.0, 20.0, 8.0])
            ),
            None
        );
        // Inside the full band: one piece at alpha 1.
        let pieces = faded_pieces(
            [0.0, 10.0, 4.0, 20.0],
            [0.0, 0.0, 1.0, 1.0],
            [0.0, 4.0, 96.0, 100.0],
        );
        assert_eq!(
            pieces,
            vec![([0.0, 10.0, 4.0, 20.0], [0.0, 0.0, 1.0, 1.0], 1.0)]
        );
        // Across the top ramp: strips below 4 with rising alpha.
        let pieces = faded_pieces(
            [0.0, 0.0, 4.0, 8.0],
            [0.0, 0.0, 1.0, 1.0],
            [0.0, 4.0, 96.0, 100.0],
        );
        assert_eq!(pieces.len(), 5);
        assert!(pieces[0].2 < pieces[3].2 && pieces[4].2 == 1.0);
        assert_eq!(
            units_uv(k::ARROW_UNITS, [128, 64]),
            [0.75, 0.5, 0.9921875, 0.984375]
        );
        assert_eq!(texture_name(0x23), Some("gunim_pistol9mm_b"));
    }
}

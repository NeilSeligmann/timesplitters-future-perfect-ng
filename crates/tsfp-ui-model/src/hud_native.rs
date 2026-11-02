//! T2312 UI-14: the in-game HUD as a native Rust system.
//!
//! Three parts, all host-free and readable:
//!
//! 1. **Elements and ownership** ([`HudElement`], [`HudOwnership`]): every HUD element the native HUD
//!    can draw names the legacy drawers it replaces (`tools/data/hud_native_gates.json`, checked by a
//!    test). The build generates one gate per drawer (`tools/play/hud_gate.py`). A drawer call is a
//!    *draw request*: the original decided that the element is visible this frame. When the element is
//!    owned the gate skips the legacy body and the native HUD draws it; otherwise legacy draws it.
//! 2. **State** ([`HudFrame`]): the typed values the HUD shows, read once per frame from the game
//!    through [`GuestRead`] (named offsets below, sources and confidence per field).
//! 3. **Layout** ([`HudLayout`], [`layout`]): quads, sprites and text runs in window pixels for the
//!    `tsfp-ui-renderer` contract. Geometry is data ([`HudLayout::original`]), anchored to the
//!    4:3 picture (original preset) or to the window safe area (widescreen option).
//!
//! No game strings, textures or bytes live here: texture and font references are names and indices
//! resolved at run time from the owner's disc (T2307 skin layer).
use tsfp_ui_renderer::{BlendMode, Color, LayoutError, Quad, Rect, Vec2};

/// Read access to the running game (the shell's `GuestUi::read32`). `None` = unmapped.
pub trait GuestRead {
    fn read32(&self, va: u32) -> Option<u32>;
    fn read_f32(&self, va: u32) -> Option<f32> {
        self.read32(va).map(f32::from_bits)
    }
    fn read16(&self, va: u32) -> Option<u16> {
        let word = self.read32(va & !3)?;
        Some((word >> ((va & 2) * 8)) as u16)
    }
}

/// A HUD element the native HUD can own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HudElement {
    /// The whole legacy HUD frame (diagnostic: suppress everything).
    Whole,
    /// Weapon ammo row: reserve, icon, loaded (bottom right).
    Ammo,
    /// `newHud` bag base label with bar.
    Bar,
    /// Split-screen health fraction bars.
    SplitBars,
    /// Status icons and amount bars (detonation, portrait).
    Status,
    /// `newHud` target and cross textures.
    Target,
    /// The weapon reticle at the aim point (reticle table 0x4E6900, targeting overlay 0x14C970 in the view pass).
    Crosshair,
    /// Per-reticle extra drawers of the reticle table (old crosshair, sci-fi overlay, sniper back):
    /// suppress-only, no native implementation yet.
    ReticleExtra,
    /// Health (left) and armour (right) arcs, shown after damage (wrapper 0x14D3A0 of 0x13AB60),
    /// with the hit-sector indicator (Rust-owned timer: setter 0x13AB10 gated, level reset 0x148400 seen).
    Vitals,
    /// Arcade rank box (`0x12EC20`, register arguments, increment 4; in `core` since increment 5, when the
    /// frame and panel textures were corrected): frame, panel, label and ordinal ([`crate::hud_rank`]); the C
    /// gate keeps the row cursor advance.
    Rank,
    /// The radar (`0x133AB0`, increment 5): ring, sweep, own dot, blips and rim markers, centre pulse
    /// ([`crate::hud_radar`]); blips, sweep angle and pulse restart are Rust-owned.
    Radar,
    /// The button prompt panel (`0x1476F0`, increment 6, Story weapon help): rounded frosted panel, texts built
    /// from string ids with button glyphs and inline icons ([`crate::hud_prompts`]); the lifecycle is Rust-owned
    /// (show `0x147180`, dismiss `0x147300` and level init `0x147250` observed).
    Prompts,
    /// The Arcade effect icons under the rank box (`0x12F670`, increment 6, `xmm0` alpha and a stack row cursor):
    /// cloak, speed, shrink and damage, dim or coloured by the effect's strength ([`crate::hud_effects`]).
    Effects,
    /// The HUD message queue (`0x14AC80`, increment 7): pickup lines ("Got the ..."), slide-in titles; loop,
    /// conflicts and stacking in Rust. Typed presenters cover kinds 1-5, 7-10 and kind 6 is the measured no-draw,
    /// no-weight case ([`crate::hud_messages`]); unsupported entries still fall back to their original drawer.
    /// The complete lifecycle switch remains opt-in until its route adoption gate passes.
    Messages,
    /// The weapon slot panel (`0x141E30`, increment 8): the owned weapons by slot group in a panel that fades in on a
    /// weapon switch, icons, overlays, glow and group arrows ([`crate::hud_weapons`]); the mode 1 slide target is Rust
    /// state. `0x12CC70`, `0x561C0` and `0x81B70` are observed pass-through for the shadow check.
    Weapons,
    /// The Challenge points row (`0x12ECA0`, increment 8, register `esi` row cursor): the labelled row of the rank box
    /// with the label and value from the Challenge text lines ([`crate::hud_rank::points_row`]).
    Points,
    /// Census only (T2312 increment 3): other drawers the HUD frame `0x14F0B0` calls, gated
    /// pass-through so the exit report counts which ones run in a route. Never skipped.
    Census,
}

impl HudElement {
    pub const ALL: [HudElement; 17] = [
        HudElement::Whole,
        HudElement::Ammo,
        HudElement::Bar,
        HudElement::SplitBars,
        HudElement::Status,
        HudElement::Target,
        HudElement::Crosshair,
        HudElement::ReticleExtra,
        HudElement::Vitals,
        HudElement::Rank,
        HudElement::Radar,
        HudElement::Prompts,
        HudElement::Effects,
        HudElement::Messages,
        HudElement::Weapons,
        HudElement::Points,
        HudElement::Census,
    ];

    /// The core set the default play variant hands to the native HUD.
    pub const CORE: [HudElement; 10] = [
        HudElement::Ammo,
        HudElement::Crosshair,
        HudElement::Vitals,
        HudElement::Rank,
        HudElement::Radar,
        HudElement::Prompts,
        HudElement::Effects,
        HudElement::Messages,
        HudElement::Weapons,
        HudElement::Points,
    ];

    /// Elements with a native implementation (`TSFP_HUD_NATIVE=all`). Owning any other element
    /// only suppresses its legacy drawer (diagnostic, named explicitly).
    pub const NATIVE: [HudElement; 10] = [
        HudElement::Ammo,
        HudElement::Crosshair,
        HudElement::Vitals,
        HudElement::Rank,
        HudElement::Radar,
        HudElement::Prompts,
        HudElement::Effects,
        HudElement::Messages,
        HudElement::Weapons,
        HudElement::Points,
    ];

    /// Name used in `tools/data/hud_native_gates.json` and `TSFP_HUD_NATIVE`.
    pub fn name(self) -> &'static str {
        match self {
            HudElement::Whole => "whole",
            HudElement::Ammo => "ammo",
            HudElement::Bar => "bar",
            HudElement::SplitBars => "split_bars",
            HudElement::Status => "status",
            HudElement::Target => "target",
            HudElement::Crosshair => "crosshair",
            HudElement::ReticleExtra => "reticle_extra",
            HudElement::Vitals => "vitals",
            HudElement::Rank => "rank",
            HudElement::Radar => "radar",
            HudElement::Prompts => "prompts",
            HudElement::Effects => "effects",
            HudElement::Messages => "messages",
            HudElement::Weapons => "weapons",
            HudElement::Points => "points",
            HudElement::Census => "census",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.name() == name)
    }

    /// Legacy drawers of this element (guest addresses of the original, INFERRED names).
    pub fn drawers(self) -> &'static [u32] {
        match self {
            HudElement::Whole => &[0x0014_f0b0],
            HudElement::Ammo => &[0x0013_34d0],
            HudElement::Bar => &[0x0013_0700],
            HudElement::SplitBars => &[0x0013_be30],
            HudElement::Status => &[0x0014_b930],
            HudElement::Target => &[0x0012_f230],
            HudElement::Crosshair => &[0x0014_c970],
            HudElement::ReticleExtra => &[0x0014_bff0, 0x0013_5f30, 0x0013_25d0],
            HudElement::Vitals => &[0x0014_d3a0, hit_addr::FN_SET, hit_addr::FN_RESET],
            HudElement::Rank => &[crate::hud_rank::rank_addr::FN_RANK_BOX],
            HudElement::Radar => &[crate::hud_radar::radar_addr::FN_RADAR],
            HudElement::Prompts => &PROMPT_DRAWERS,
            HudElement::Effects => &[crate::hud_effects::effect_addr::FN_EFFECTS],
            HudElement::Messages => &[
                crate::hud_messages::message_addr::FN_QUEUE,
                crate::hud_messages::message_addr::FN_OUTLINED,
            ],
            HudElement::Weapons => &WEAPON_DRAWERS,
            HudElement::Points => &[crate::hud_rank::points_addr::FN_POINTS_ROW],
            HudElement::Census => &CENSUS_DRAWERS,
        }
    }

    /// The element a drawer belongs to.
    pub fn of_drawer(va: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.drawers().contains(&va))
    }

    fn bit(self) -> u32 {
        1 << (self as u32)
    }
}

/// HUD frame (`0x14F0B0`) drawers without a native element yet, counted per route (census). Names in
/// `tools/data/hud_native_gates.json` (INFERRED, tools/data/function_names.csv).
pub const CENSUS_DRAWERS: [u32; 27] = [
    // Draw probe (increment 8): leaf primitives (rect `0x1B7B0`, integer quad `0x1F090`, vertex-colour quad `0x1BA70`,
    // plain string `0x41DC0`), observed pass-through; each call is charged to the census drawer that ran last.
    0x0001_b7b0,
    0x0001_f090,
    0x0001_ba70,
    0x0004_1dc0,
    0x0012_e3f0,
    0x0012_e9f0,
    0x0012_eab0,
    0x0012_eba0,
    0x0012_ed10,
    0x0012_ede0,
    0x0012_ef00,
    0x0012_eff0,
    0x0013_0d20,
    0x0013_91a0,
    0x0013_92c0,
    0x0013_96b0,
    0x0013_c060,
    0x0013_c290,
    0x0013_c560,
    0x0013_d4c0,
    0x0014_3d20,
    0x0014_46e0,
    0x0014_cc80,
    0x0014_d6e0,
    0x0014_d980,
    0x0034_bae0,
    0x0013_8590,
];

/// The weapon slot panel's gates: the drawer, then the primitive draws observed pass-through by the shadow check.
pub const WEAPON_DRAWERS: [u32; 4] = [
    crate::hud_weapons::weapon_addr::FN_PANEL,
    crate::hud_weapons::weapon_addr::FN_PANEL_QUAD,
    crate::hud_weapons::weapon_addr::FN_FADED_QUAD,
    crate::hud_weapons::weapon_addr::FN_ARROW,
];

/// The prompt panel's gates: the drawer, then the pass-through observers (show, dismiss, level init).
pub const PROMPT_DRAWERS: [u32; 4] = [
    crate::hud_prompts::prompt_addr::FN_PROMPTS,
    crate::hud_prompts::prompt_addr::FN_SHOW,
    crate::hud_prompts::prompt_addr::FN_DISMISS,
    crate::hud_prompts::prompt_addr::FN_INIT,
];

/// Which elements the native HUD draws (the legacy drawer is skipped for exactly these).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HudOwnership(u32);

impl HudOwnership {
    pub const NONE: HudOwnership = HudOwnership(0);

    pub fn of(elements: &[HudElement]) -> Self {
        HudOwnership(elements.iter().fold(0, |bits, e| bits | e.bit()))
    }

    /// `TSFP_HUD_NATIVE`: `off` (legacy draws everything), `core`, `all` (every element with a
    /// native implementation, [`HudElement::NATIVE`]), or a comma list of element names (any
    /// element, including suppress-only ones for diagnostics). Unknown names are errors.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.trim() {
            "" | "off" | "legacy" | "0" => Ok(Self::NONE),
            "core" | "1" | "on" => Ok(Self::of(&HudElement::CORE)),
            "all" => Ok(Self::of(&HudElement::NATIVE)),
            list => list
                .split(',')
                .map(|name| {
                    HudElement::from_name(name.trim())
                        .ok_or_else(|| format!("unknown HUD element {name:?}"))
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|elements| Self::of(&elements)),
        }
    }

    pub fn owns(self, element: HudElement) -> bool {
        self.0 & element.bit() != 0
    }

    /// The gate's question: is this legacy drawer replaced by the native HUD?
    pub fn owns_drawer(self, va: u32) -> bool {
        HudElement::of_drawer(va).is_some_and(|e| self.owns(e))
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn elements(self) -> impl Iterator<Item = HudElement> {
        HudElement::ALL.into_iter().filter(move |e| self.owns(*e))
    }
}

/// One legacy draw request seen this frame: the element, the drawer and its first stack arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawRequest {
    pub element: HudElement,
    pub drawer: u32,
    pub args: [u32; 5],
    /// The reticle the original would draw at this call ([`Reticle::read`], crosshair requests only).
    pub reticle: Option<Reticle>,
    /// The ammo row of this call ([`AmmoRow::read`] on the call's arguments, ammo requests only).
    pub ammo: Option<AmmoRow>,
    /// The health and armour arcs of this call ([`VitalsArcs::read`], vitals requests only).
    pub vitals: Option<VitalsArcs>,
    /// The hit-sector indicator of this vitals call (Rust-owned timer, [`HitSectors::step`]).
    pub hit_sector: Option<HitSectorQuad>,
    /// The rank box of this call ([`crate::hud_rank::RankBox::read`], rank requests only).
    pub rank: Option<crate::hud_rank::RankBox>,
    /// The radar pieces of this call ([`crate::hud_radar::RadarState::read`], radar requests only).
    pub radar: Option<Vec<crate::hud_radar::RadarPiece>>,
    /// The prompt panel of this call ([`crate::hud_prompts::PromptPanels::read`], prompt requests only).
    pub prompts: Option<crate::hud_prompts::PromptDraw>,
    /// The effect icons of this call ([`crate::hud_effects::EffectIcons::read`], effect requests only).
    pub effects: Option<crate::hud_effects::EffectIcons>,
    /// The message queue texts of this call ([`crate::hud_messages::replay`], message requests only).
    pub messages: Option<crate::hud_messages::QueueDraw>,
    /// The weapon slot panel of this call ([`crate::hud_weapons::WeaponPanel::read`], weapon requests only).
    pub weapons: Option<crate::hud_weapons::WeaponPanelDraw>,
}

/// Health and armour arcs (HUD frame `0x14F0B0` -> wrapper `0x14D3A0` -> `0x13AB60`, MEASURED from the
/// disassembly, T2312 increment 2). The left arc is health, the right arc armour; each is 20 bands
/// cut from the half-arc textures by the band tables, coloured per band and lit per band.
pub mod arc_addr {
    /// Object field: pointer to the HUD flag block (lit flags, transition timers, flash timer).
    pub const OBJ_FLAG_BLOCK: u32 = 0x268;
    /// Block layout: health lit flags `+0` (20 words), armour lit flags `+0x50`, health timers
    /// `+0xA0` (20 f32), armour timers `+0xF0`, flash timer `+0x140` (f32, arcs shown while > 0).
    pub const BLOCK_ARMOUR: u32 = 0x50;
    pub const BLOCK_TIMERS: u32 = 0xa0;
    pub const BLOCK_FLASH: u32 = 0x140;
    /// Band tables of 21 rows of 0x14 bytes: threshold f32 `+0`, band position f32 `+4` (0 top, 1
    /// bottom), red `+8`, green `+0xC`, blue `+0x10` (ints, 0x80 = 1.0).
    pub const HEALTH_BANDS: u32 = 0x004e_6d80;
    pub const ARMOUR_BANDS: u32 = 0x004e_6f28;
    pub const BAND_ROW: u32 = 0x14;
    /// Guest hit-sector fade timers `0x7A5DA4 + 8 * index` (set to 1.0 on a hit by `0x13AB10`, decayed by
    /// the drawer itself). With the setter gated they stay 0 and the timer is Rust-owned ([`HitSectors`]);
    /// a nonzero guest timer means the original keeps that call (it writes the timer).
    pub const HIT_TIMERS: u32 = 0x007a_5da4;
    pub const HIT_TIMER_STRIDE: u32 = 8;
    /// Viewport fields the drawer reads: centre x `+8`, centre y `+0x1C` (ints).
    pub const VP_CENTRE_X: u32 = 0x8;
    pub const VP_CENTRE_Y: u32 = 0x1c;
    /// Net effective player count (`0x1CDAE0`, no arguments).
    pub const FN_PLAYER_COUNT: u32 = 0x001c_dae0;
}

/// Constants of `0x13AB60` and its wrapper (f32 values read from the XBE, addresses in the comments).
mod arc_const {
    /// Width scale for more than two players (0x47DB30) and otherwise (0x478674).
    pub const SCALE_MANY: f32 = 1.08;
    pub const SCALE_FEW: f32 = 1.2;
    /// Inner width of each arc over the half width (0x47CFE0).
    pub const INNER: f32 = 0.65;
    /// Transition timer to alpha (0x475CEC, 1 / 0.7).
    pub const TIMER_TO_ALPHA: f32 = 1.428_571_5;
    /// The bar pass dims unlit bands to this fraction of the band colour (0x475CDC).
    pub const DIM: f32 = 0.3;
    /// Texture v edge nudge (0x475D1C).
    pub const V_NUDGE: f32 = 0.01;
    /// Outline pass target colour of lit bands, red, green, blue (0x47D1E0, 0x47DAB0, 0x4785C8).
    pub const OUTLINE_LIT: [f32; 3] = [64.0, 24.0, 100.0];
    /// Wrapper: flash timer to fade `min(1, sqrt(t * 0.5) * 1.2)`, then the four alpha arguments
    /// fade * 127 (bar lit and glow, 0x475CC0), * 50 (bar unlit, 0x475D28), * 60 and * 20 (outline, 0x4760AC, 0x4782C4).
    pub const FADE_GAIN: f32 = 1.2;
    pub const ALPHA_LIT: f32 = 127.0;
    pub const ALPHA_UNLIT: f32 = 50.0;
    pub const ALPHA_OUTLINE_HIGH: f32 = 60.0;
    pub const ALPHA_OUTLINE_LOW: f32 = 20.0;
}

/// One band row of a band table.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ArcBandRow {
    pub threshold: f32,
    /// Vertical position of the band's top edge, 0 at the arc top, 1 at the bottom.
    pub position: f32,
    pub rgb: [i32; 3],
}

/// One arc (health or armour): band rows, lit flags and transition timers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcSide {
    pub rows: [ArcBandRow; 21],
    pub lit: [bool; 20],
    pub timers: [f32; 20],
    /// Band index where the texture's v mapping mirrors (the row at position 0.5): 8 health, 10 armour.
    pub mirror_band: usize,
}

impl ArcSide {
    /// Band alpha: lit bands fade in and unlit ones fade out over their transition timer.
    pub fn band_alpha(&self, band: usize) -> f32 {
        let timer = self.timers[band];
        if timer > 0.0 {
            let t = (timer * arc_const::TIMER_TO_ALPHA).min(1.0);
            if self.lit[band] { 1.0 - t } else { t }
        } else if self.lit[band] {
            1.0
        } else {
            0.0
        }
    }

    /// Alpha of a band with half the next band's alpha as a floor (the soft leading edge).
    pub fn soft_alpha(&self, band: usize) -> f32 {
        let own = self.band_alpha(band);
        if band + 1 < 20 {
            own.max(0.5 * self.band_alpha(band + 1))
        } else {
            own
        }
    }
}

/// The two arcs as `0x13AB60` would draw them this call, in 640 x 480 guest pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VitalsArcs {
    /// The wrapper's alpha arguments: bar lit and glow, bar unlit, outline high and low.
    pub alpha: [i32; 4],
    pub centre: [f32; 2],
    pub half_width: f32,
    pub inner_width: f32,
    pub half_height: f32,
    /// Health (left) and armour (right).
    pub sides: [ArcSide; 2],
    /// Hit-sector entry of this viewer (the viewer record's first word, `0x13AB60` `ebx`).
    pub hit_index: u32,
}

/// What a vitals drawer call does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VitalsCall {
    /// No flag block or no flash: the wrapper returns without drawing.
    Nothing,
    Draw(VitalsArcs),
    /// A hit-sector indicator is fading (the drawer writes its timer) or the state is unreadable.
    Legacy,
}

/// One textured band quad of an arc pass with a vertical colour gradient (top and bottom colour
/// words as the original builds them: red, green, blue, alpha on the 0x80 = 1.0 scale).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcQuad {
    pub texture: u32,
    pub additive: bool,
    /// x0, y0, x1, y1 in guest pixels.
    pub rect: [f32; 4],
    /// u at x0 and x1, v at y0 and y1.
    pub uv: [f32; 4],
    pub top: [i32; 4],
    pub bottom: [i32; 4],
}

/// Texture ids of the three passes (outline additive, bar alpha, glow additive); names via the
/// name-table row (id - 1): `arc_outline`, `arc_bar`, `arc_glow`.
pub const ARC_PASSES: [(u32, bool, &str); 3] = [
    (0x321, true, "arc_outline"),
    (0x323, false, "arc_bar"),
    (0x322, true, "arc_glow"),
];

impl VitalsArcs {
    /// Replay the wrapper `0x14D3A0` and the geometry and state reads of `0x13AB60`.
    pub fn read<G: GuestCall + ?Sized>(g: &G) -> VitalsCall {
        use arc_addr::*;
        let Some(viewer) = g.read32(addr::VIEWER_RECORD_PTR).filter(|&v| v != 0) else {
            return VitalsCall::Legacy;
        };
        let Some(object) = g.read32(viewer + addr::RECORD_OBJECT) else {
            return VitalsCall::Legacy;
        };
        let block = g.read32(object + OBJ_FLAG_BLOCK).unwrap_or(0);
        if block == 0 {
            return VitalsCall::Nothing;
        }
        let Some(flash) = g.read_f32(block + BLOCK_FLASH) else {
            return VitalsCall::Legacy;
        };
        if !(flash > 0.0) {
            return VitalsCall::Nothing;
        }
        // The guest hit-sector timers stay 0 while the native HUD owns the hits (the setter `0x13AB10`
        // is gated and the timer lives in [`HitSectors`]). A nonzero guest timer (a hit before the
        // native HUD was in the frame) is decayed by the drawer itself (a guest write): keep the original then.
        let hit_index = g.read32(viewer).unwrap_or(u32::MAX);
        match g.read_f32(HIT_TIMERS.wrapping_add(hit_index.wrapping_mul(HIT_TIMER_STRIDE))) {
            Some(t) if t == 0.0 => {}
            _ => return VitalsCall::Legacy,
        }
        let (Some(players), Some(slot)) = (
            g.call(FN_PLAYER_COUNT, &[]),
            g.read32(viewer + addr::RECORD_SLOT),
        ) else {
            return VitalsCall::Legacy;
        };
        let Some(viewport) = g.read32(addr::VIEWPORT_PTR).filter(|&v| v != 0) else {
            return VitalsCall::Legacy;
        };
        let fade = ((flash * 0.5).sqrt() * arc_const::FADE_GAIN).min(1.0);
        let alpha = [
            (fade * arc_const::ALPHA_LIT) as i32,
            (fade * arc_const::ALPHA_UNLIT) as i32,
            (fade * arc_const::ALPHA_OUTLINE_HIGH) as i32,
            (fade * arc_const::ALPHA_OUTLINE_LOW) as i32,
        ];
        let first_of_three = players == 3 && slot == 0;
        let scale = if players > 2 && !first_of_three {
            arc_const::SCALE_MANY
        } else {
            arc_const::SCALE_FEW
        };
        let width = if players < 3 || first_of_three {
            512.0
        } else {
            300.0
        };
        let height = if players == 1 { 340.0 } else { 220.0 };
        let half_width = width * scale * 0.5;
        let int = |va: u32| g.read32(va).unwrap_or(0) as i32 as f32;
        let side = |table: u32, offset: u32, mirror_band: usize| {
            let mut s = ArcSide {
                rows: [ArcBandRow::default(); 21],
                lit: [false; 20],
                timers: [0.0; 20],
                mirror_band,
            };
            for (i, row) in s.rows.iter_mut().enumerate() {
                let at = table + i as u32 * BAND_ROW;
                *row = ArcBandRow {
                    threshold: g.read_f32(at).unwrap_or(0.0),
                    position: g.read_f32(at + 4).unwrap_or(0.0),
                    rgb: [
                        int(at + 8) as i32,
                        int(at + 0xc) as i32,
                        int(at + 0x10) as i32,
                    ],
                };
            }
            for i in 0..20 {
                s.lit[i] = g.read32(block + offset + 4 * i as u32).unwrap_or(0) != 0;
                s.timers[i] = g
                    .read_f32(block + BLOCK_TIMERS + offset + 4 * i as u32)
                    .unwrap_or(0.0);
            }
            s
        };
        VitalsCall::Draw(VitalsArcs {
            alpha,
            centre: [int(viewport + VP_CENTRE_X), int(viewport + VP_CENTRE_Y)],
            half_width,
            inner_width: half_width * arc_const::INNER,
            half_height: height * 0.5,
            sides: [
                side(HEALTH_BANDS, 0, 8),
                side(ARMOUR_BANDS, BLOCK_ARMOUR, 10),
            ],
            hit_index,
        })
    }

    /// The band quads in the original's draw order: pass (outline, bar, glow), side (health, armour), band.
    pub fn quads(&self) -> Vec<ArcQuad> {
        let left = self.centre[0] - self.half_width;
        let right = self.centre[0] + self.half_width;
        let top = self.centre[1] - self.half_height;
        let height = 2.0 * self.half_height;
        let [lit_alpha, unlit_alpha, outline_high, outline_low] = self.alpha;
        let mut out = Vec::with_capacity(120);
        for (pass, &(texture, additive, _)) in ARC_PASSES.iter().enumerate() {
            for (s, side) in self.sides.iter().enumerate() {
                // Health spans left .. left + inner (u 0 to 1), armour right - inner .. right (u 1 to 0).
                let (x0, x1, u0, u1) = if s == 0 {
                    (left, left + self.inner_width, 0.0, 1.0)
                } else {
                    (right - self.inner_width, right, 1.0, 0.0)
                };
                for band in 0..20 {
                    let (row, next) = (side.rows[band], side.rows[band + 1]);
                    let y0 = row.position * height + top;
                    let y1 = next.position * height + top;
                    let (top_color, bottom_color) = match pass {
                        0 => {
                            let edge = |p: f32| 1.0 - (p - 0.5).abs() * 2.0;
                            let soft = side.soft_alpha(band);
                            let span = (outline_high - outline_low) as f32;
                            let outline =
                                |a: f32, p: f32| ((span * a + outline_low as f32) * edge(p)) as i32;
                            let toward = |rgb: [i32; 3]| -> [i32; 3] {
                                let mut c = [0; 3];
                                for k in 0..3 {
                                    let v = rgb[k] as f32 * (1.0 - soft)
                                        + soft * arc_const::OUTLINE_LIT[k];
                                    c[k] = v as i32;
                                }
                                c
                            };
                            let t = toward(row.rgb);
                            let b = toward(next.rgb);
                            let top_a = outline(soft, row.position);
                            let bottom_a = if band < 19 {
                                outline(side.soft_alpha(band + 1), next.position)
                            } else {
                                top_a
                            };
                            ([t[0], t[1], t[2], top_a], [b[0], b[1], b[2], bottom_a])
                        }
                        1 => {
                            let a = side.band_alpha(band);
                            let alpha = if side.lit[band] {
                                lit_alpha
                            } else {
                                unlit_alpha
                            };
                            let dim = |rgb: [i32; 3]| -> [i32; 3] {
                                let mut c = [0; 3];
                                for k in 0..3 {
                                    let floor = (rgb[k] as f32 * arc_const::DIM) as i32;
                                    c[k] = ((rgb[k] - floor) as f32 * a + floor as f32) as i32;
                                }
                                c
                            };
                            let (t, b) = (dim(row.rgb), dim(next.rgb));
                            ([t[0], t[1], t[2], alpha], [b[0], b[1], b[2], alpha])
                        }
                        _ => {
                            let top_a = (lit_alpha as f32 * side.soft_alpha(band)) as i32;
                            let bottom_a = if band < 19 {
                                (lit_alpha as f32 * side.soft_alpha(band + 1)) as i32
                            } else {
                                top_a
                            };
                            let (t, b) = (row.rgb, next.rgb);
                            ([t[0], t[1], t[2], top_a], [b[0], b[1], b[2], bottom_a])
                        }
                    };
                    // v: the texture holds the upper half of the arc; the lower half mirrors it.
                    let m = side.mirror_band;
                    let (mut v0, mut v1);
                    if band < m {
                        v0 = 1.0 - row.position * 2.0;
                        v1 = 1.0 - next.position * 2.0;
                        if band == 0 {
                            v0 -= arc_const::V_NUDGE;
                        } else if band == m - 1 {
                            v1 += arc_const::V_NUDGE;
                        }
                    } else {
                        v0 = (row.position - 0.5) * 2.0;
                        v1 = (next.position - 0.5) * 2.0;
                        if band == m {
                            v0 += arc_const::V_NUDGE;
                        }
                    }
                    out.push(ArcQuad {
                        texture,
                        additive,
                        rect: [x0, y0, x1, y1],
                        uv: [u0, u1, v0, v1],
                        top: top_color,
                        bottom: bottom_color,
                    });
                }
            }
        }
        out
    }
}

/// The hit-sector indicator (first part of `0x13AB60`, setter `0x13AB10`, reset in `0x148400`;
/// MEASURED from the XBE disassembly, T2312 increment 3). A red sector texture (`hitsector`, id
/// `0x126`) rotated towards the last hit, fading over a timer.
pub mod hit_addr {
    /// Entries (angle f32, timer f32) at `0x7A5DA0 + 8 * index`, four of them (cleared by `0x148400`).
    pub const ENTRIES: usize = 4;
    /// `0x13AB10(index, direction *f32[3])`: timer = 1.0, angle = atan2(x, z) - player yaw (degrees).
    /// Gated: while the native HUD owns the vitals the hit goes to [`super::HitSectors`] instead.
    pub const FN_SET: u32 = 0x0013_ab10;
    /// HUD layout reset at level start (`0x148400`): passes through, the Rust state resets with it.
    pub const FN_RESET: u32 = 0x0014_8400;
    /// Player records `*0x7B0C48 + index * 0x1584`, yaw in degrees at `+0x100` (f32).
    pub const PLAYERS_PTR: u32 = 0x007b_0c48;
    pub const PLAYER_STRIDE: u32 = 0x1584;
    pub const PLAYER_YAW: u32 = 0x100;
    /// Guest sine table of 2048 f32 (`0x7A38A0`, cosine = index + 0x200).
    pub const SIN_TABLE: u32 = 0x007a_38a0;
    /// Decay inputs: time scale f32 `0x4E794C`, frame rate int `0x4E8CEC`, double step flag `0x7497B4`.
    pub const TIME_SCALE: u32 = 0x004e_794c;
    pub const FRAME_RATE: u32 = 0x004e_8cec;
    pub const DOUBLE_STEP: u32 = 0x0074_97b4;
    /// Viewport fields: centre x `+8`, width `+0xC`, centre y `+0x1C`, height `+0x20` (ints).
    pub const VP_WIDTH: u32 = 0xc;
    pub const VP_HEIGHT: u32 = 0x20;
}

/// Constants of the hit-sector code (f32 values from the XBE, addresses in the comments).
mod hit_const {
    /// Degrees to radians (0x475D0C).
    pub const DEG_TO_RAD: f32 = 0.017_453_292;
    /// Radians to sine-table index (0x475CE0, 2048 / 2 pi) and rounding (0x475CD4).
    pub const TO_INDEX: f32 = 325.949_3;
    pub const ROUND: f32 = 0.5;
    /// Quarter turn for the second axis (0x475CE4).
    pub const QUARTER: f32 = 1.570_796_4;
    /// Ring radius: viewport width * 0.3688 (0x47DB28) - 16 (0x475DC0); aspect uses height * 0.4196 (0x47DB2C).
    pub const RADIUS_SCALE: f32 = 0.3688;
    pub const RADIUS_INSET: f32 = 16.0;
    pub const HEIGHT_SCALE: f32 = 0.4196;
    /// Half size of the quad (0x475D08).
    pub const HALF: f32 = 32.0;
    /// Decay step factor (0x475D10).
    pub const DECAY: f32 = 2.0;
    /// Red channel = timer * 31 (0x47DAA4); alpha word 0x32; blend mode 2 (additive).
    pub const RED: f32 = 31.0;
    pub const ALPHA: i32 = 0x32;
}

/// One drawn hit sector: corners in guest pixels (strip order: texture top-left, top-right,
/// bottom-left, bottom-right) and the colour word (red, green, blue, alpha; 0x80 = 1.0), additive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitSectorQuad {
    pub corners: [[f32; 2]; 4],
    pub color: [i32; 4],
}

/// Texture name of the hit sector (id `0x126`, name-table row `0x125`).
pub const HIT_SECTOR_TEXTURE: &str = "hitsector";

/// Rust-owned hit-sector state (replaces the guest entries at `0x7A5DA0`): the hit angle and the
/// fade timer of each entry. Hits arrive from the gated setter, the vitals drawer call decays them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HitSectors {
    pub angle: [f32; hit_addr::ENTRIES],
    pub timer: [f32; hit_addr::ENTRIES],
}

impl HitSectors {
    /// `0x13AB10`: a hit from `direction` (x, y, z) on entry `index` whose player faces `yaw` degrees.
    pub fn hit(&mut self, index: u32, direction: [f32; 3], yaw: f32) {
        let i = index as usize;
        if i >= hit_addr::ENTRIES {
            return;
        }
        self.timer[i] = 1.0;
        // fpatan on x87: atan2(x, z), minus yaw * (pi / 180), stored as f32.
        let angle = (direction[0] as f64).atan2(direction[2] as f64)
            - yaw as f64 * hit_const::DEG_TO_RAD as f64;
        self.angle[i] = angle as f32;
    }

    /// `0x13AB10` replayed from its guest arguments (`index`, pointer to the direction).
    pub fn hit_from_call<G: GuestRead + ?Sized>(
        &mut self,
        g: &G,
        index: u32,
        direction: u32,
    ) -> bool {
        let read = |va: u32| g.read_f32(va);
        let (Some(x), Some(y), Some(z)) =
            (read(direction), read(direction + 4), read(direction + 8))
        else {
            return false;
        };
        let Some(players) = g.read32(hit_addr::PLAYERS_PTR).filter(|&p| p != 0) else {
            return false;
        };
        let Some(yaw) = g
            .read_f32(players + index.wrapping_mul(hit_addr::PLAYER_STRIDE) + hit_addr::PLAYER_YAW)
        else {
            return false;
        };
        self.hit(index, [x, y, z], yaw);
        true
    }

    /// `0x148400` (level start): every entry cleared.
    pub fn reset(&mut self) {
        *self = HitSectors::default();
    }

    /// One `0x13AB60` call for entry `index`: the quad drawn with the timer before this call's decay
    /// (nothing while the timer is 0, the original's alpha-0 quad adds nothing), then the decay.
    pub fn step<G: GuestRead + ?Sized>(&mut self, g: &G, index: u32) -> Option<HitSectorQuad> {
        use hit_addr::*;
        use hit_const::*;
        let i = index as usize;
        if i >= ENTRIES {
            return None;
        }
        let timer = self.timer[i];
        let quad = if timer > 0.0 {
            self.quad(g, timer, self.angle[i])
        } else {
            None
        };
        // Decay: timer -= (1 / rate) * scale * 2, once more without the scale when the double-step flag is set.
        let rate = g.read32(FRAME_RATE).map_or(60.0, |r| r as i32 as f32);
        let step = 1.0 / rate;
        let mut t = timer - step * g.read_f32(TIME_SCALE).unwrap_or(1.0) * DECAY;
        if g.read32(DOUBLE_STEP).unwrap_or(0) != 0 {
            t -= step * DECAY;
        }
        self.timer[i] = if 0.0 > t { 0.0 } else { t };
        quad
    }

    fn quad<G: GuestRead + ?Sized>(&self, g: &G, timer: f32, angle: f32) -> Option<HitSectorQuad> {
        use hit_addr::*;
        use hit_const::*;
        let viewport = g.read32(addr::VIEWPORT_PTR).filter(|&v| v != 0)?;
        let int = |offset: u32| g.read32(viewport + offset).map(|v| v as i32 as f32);
        let (cx, width, cy, height) = (
            int(arc_addr::VP_CENTRE_X)?,
            int(VP_WIDTH)?,
            int(arc_addr::VP_CENTRE_Y)?,
            int(VP_HEIGHT)?,
        );
        let aspect = height * HEIGHT_SCALE / (width * RADIUS_SCALE);
        let table = |index: i32| {
            let entry = (index & 0x7ff) as u32;
            g.read_f32(SIN_TABLE + 4 * entry)
                .unwrap_or_else(|| (entry as f32 * std::f32::consts::TAU / 2048.0).sin())
        };
        let first = (angle * TO_INDEX + ROUND) as i32;
        let second = ((angle + QUARTER) * TO_INDEX + ROUND) as i32;
        let (sin_a, cos_a) = (table(first), table(first + 0x200));
        let (sin_b, cos_b) = (table(second), table(second + 0x200));
        let radius = width * RADIUS_SCALE - RADIUS_INSET;
        let centre = [radius * sin_a + cx, radius * cos_a * aspect + cy];
        // Axis a points from the screen centre towards the hit, axis b across it.
        let a = [sin_a * HALF, cos_a * HALF * aspect];
        let b = [sin_b * HALF, cos_b * HALF * aspect];
        let at = |sa: f32, sb: f32| {
            [
                centre[0] + sa * a[0] + sb * b[0],
                centre[1] + sa * a[1] + sb * b[1],
            ]
        };
        let red = ((timer as f64 * RED as f64) as i32) & 0xff;
        Some(HitSectorQuad {
            corners: [at(-1.0, -1.0), at(-1.0, 1.0), at(1.0, -1.0), at(1.0, 1.0)],
            color: [red, 0, 0, ALPHA],
        })
    }
}

/// Colour of an arc vertex word (red, green, blue, alpha; 0x80 = 1.0).
pub fn arc_color(c: [i32; 4]) -> Color {
    let channel = |v: i32| (v.clamp(0, 255) as f32 / 128.0).min(1.0);
    Color([channel(c[0]), channel(c[1]), channel(c[2]), channel(c[3])])
}

/// Which side of the picture an ammo row sits on (argument 4 of 0x1334D0: 0 left, 1 right).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmmoSide {
    /// Second hand of a dual wield, mirrored to the left edge.
    Left,
    Right,
}

/// One 0x1334D0 ammo row (MEASURED: the 0x14F0B0 sites pass `(character_ext, 0, hand_state, hand, side)`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AmmoRow {
    pub side: AmmoSide,
    /// Original per-corner stacking cursor, captured at the drawer call.
    pub stack_index: u8,
    pub weapon_record: u32,
    pub ammo_type: u32,
    pub clip_size: u32,
    pub loaded: u32,
    pub reserve: u32,
    /// Viewport bottom at the call (`[[0x7E71C8] + 0x18]`: 480 full screen, 240 top of a split), the row anchor.
    pub viewport_bottom: i32,
}

/// What an ammo-row drawer call does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AmmoCall {
    /// The hand holds no weapon record (the drawer returns without drawing).
    Nothing,
    Draw(AmmoRow),
    /// Not reproduced (side 2, the vehicle call with no character): legacy draws it.
    Legacy,
}

impl AmmoRow {
    /// Read the row from the drawer's five stack arguments.
    pub fn read<G: GuestRead + ?Sized>(g: &G, args: [u32; 5]) -> AmmoCall {
        Self::read_input(g, args, None)
    }

    /// Native row allocation supplies the byte index; inventory and hand facts stay read-only inputs.
    pub fn read_owned<G: GuestRead + ?Sized>(g: &G, args: [u32; 5], stack_index: u8) -> AmmoCall {
        Self::read_input(g, args, Some(stack_index))
    }

    fn read_input<G: GuestRead + ?Sized>(
        g: &G,
        args: [u32; 5],
        owned_index: Option<u8>,
    ) -> AmmoCall {
        use addr::*;
        let [character, zero, _hand_state, hand, side] = args;
        let side = match side {
            0 => AmmoSide::Left,
            1 => AmmoSide::Right,
            _ => return AmmoCall::Legacy,
        };
        if character == 0 || zero != 0 || hand == 0 {
            return AmmoCall::Legacy;
        }
        let Some(index) = g.read32(hand + HAND_RECORD_INDEX) else {
            return AmmoCall::Legacy;
        };
        if index == u32::MAX {
            return AmmoCall::Nothing;
        }
        if index >= WEAPON_RECORD_COUNT {
            return AmmoCall::Legacy;
        }
        let record = WEAPON_RECORDS + index * WEAPON_RECORD_SIZE;
        let ammo_type = g.read32(record + WEAPON_AMMO_TYPE).unwrap_or(0) & 0xff;
        // Only the ordinary numeric bullet row is reconstructed; other styles and secondary fire stay
        // together in the original drawer (T2312), so a partial replay cannot misplace their elements.
        let Some(secondary) = g.read32(record + 0x1c) else {
            return AmmoCall::Legacy;
        };
        if secondary > 0 && secondary < 44 {
            return AmmoCall::Legacy;
        }
        if ammo_type == 0 || ammo_type >= 44 {
            return AmmoCall::Nothing;
        }
        if g.read32(0x004e_a404 + ammo_type * 0xe0) != Some(10) {
            return AmmoCall::Legacy;
        }
        let viewport = g.read32(addr::VIEWPORT_PTR).filter(|&v| v != 0); // T3787: read unconditionally
        let Some(viewport_bottom) = viewport.and_then(|v| g.read32(v + 0x18)) else {
            return AmmoCall::Legacy;
        };
        let stack_index = match owned_index {
            Some(index) => index,
            None => match AmmoStackCursor::read(g, side) {
                Some(cursor) => cursor.index,
                None => return AmmoCall::Legacy,
            },
        };
        AmmoCall::Draw(AmmoRow {
            side,
            stack_index,
            weapon_record: index,
            ammo_type,
            clip_size: g.read32(record + WEAPON_CLIP_SIZE).unwrap_or(0),
            loaded: g
                .read16(hand + HAND_LOADED_BY_TYPE + 2 * ammo_type)
                .map_or(0, u32::from),
            reserve: g
                .read16(character + EXT_RESERVE_BY_TYPE + 2 * ammo_type)
                .map_or(0, u32::from),
            viewport_bottom: viewport_bottom as i32,
        })
    }
}

/// Per-corner row allocation shared with secondary ammo; guest storage is a TEMPORARY-SHIM (removal: T2312).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmmoStackCursor {
    pub word_address: u32,
    pub word: u32,
    pub shift: u32,
    pub index: u8,
}

impl AmmoStackCursor {
    pub fn read<G: GuestRead + ?Sized>(g: &G, side: AmmoSide) -> Option<Self> {
        let viewer = g.read32(addr::VIEWER_RECORD_PTR)?;
        let slot = g.read32(viewer + addr::RECORD_SLOT)? as i32;
        if slot < 0 {
            return None;
        }
        let word_address = reticle_addr::PLAYER_SLOTS
            .checked_add((slot as u32).checked_mul(reticle_addr::PLAYER_SLOT_SIZE)?)?
            .checked_add(0x174c)?;
        let word = g.read32(word_address)?;
        let shift = if side == AmmoSide::Right { 8 } else { 0 };
        Some(Self {
            word_address,
            word,
            shift,
            index: (word >> shift) as u8,
        })
    }

    /// Preserve both the other corner and the adjacent HUD bytes (the original increments a byte).
    pub fn advanced_word(self) -> u32 {
        (self.word & !(0xff << self.shift)) | (u32::from(self.index.wrapping_add(1)) << self.shift)
    }
}

/// Read access plus cdecl calls of original predicates (the shell's guest API). `call` returns `None`
/// when calls are unavailable; the reader then leaves the case to the original.
pub trait GuestCall: GuestRead {
    fn call(&self, va: u32, args: &[u32]) -> Option<u32>;
}

/// The weapon reticle at the aim point: table, viewer fields and the drawer's globals
/// (targeting overlay 0x14C970, MEASURED from the disassembly, T2312).
pub mod reticle_addr {
    /// Records of 0x1C bytes: `+0` `+4` `+8` extra drawers, `+0xC` texture-table index (-1 none),
    /// `+0x10` half size (f32, design pixels), `+0x14` colour word, `+0x18` blend mode.
    pub const TABLE: u32 = 0x004e_6900;
    pub const RECORD_SIZE: u32 = 0x1c;
    pub const TEXTURE: u32 = 0xc;
    pub const HALF_SIZE: u32 = 0x10;
    pub const COLOR: u32 = 0x14;
    pub const BLEND: u32 = 0x18;
    /// Player slots `0x7A6160 + slot * 0x177C`; `+0` is the reticle index.
    pub const PLAYER_SLOTS: u32 = 0x007a_6160;
    pub const PLAYER_SLOT_SIZE: u32 = 0x177c;
    /// Viewer record fields: `+8` scope object and `+0xC` flags bit 0x20 (sniper bar drawn), `+0x114`
    /// aim state, `+0x1500` -> `+0x74` and `+0xF5C` (reticle shown), `+0xF6C` / `+0xF70` aim factors
    /// (f32), `+0xF84` target candidate object.
    pub const VIEWER_SCOPE_OBJECT: u32 = 0x8;
    pub const VIEWER_FLAGS: u32 = 0xc;
    pub const FLAG_SCOPE: u32 = 0x20;
    pub const VIEWER_AIM_STATE: u32 = 0x114;
    pub const VIEWER_SUB: u32 = 0x1500;
    pub const SUB_SHOW: u32 = 0x74;
    pub const VIEWER_SHOW: u32 = 0xf5c;
    pub const VIEWER_AIM_X: u32 = 0xf6c;
    pub const VIEWER_AIM_Y: u32 = 0xf70;
    pub const VIEWER_TARGET: u32 = 0xf84;
    /// Global that also shows the reticle when it equals 8.
    pub const SHOW_STATE: u32 = 0x006d_60d8;
    /// Viewport: x `+0`, width `+0x10`, y `+0x14`, height `+0x24` (as the drawer reads them).
    pub const VP_X: u32 = 0x0;
    pub const VP_W: u32 = 0x10;
    pub const VP_Y: u32 = 0x14;
    pub const VP_H: u32 = 0x24;
    /// Aim point factor (f32 at 0x47D5B0) and the quad scale x/y (f32 at 0x4E68F4 / 0x4E68F8; MEASURED:
    /// the drawer adds the 0x4E68F8 product to y, the reticle is 1.25 times wider than tall).
    pub const AIM_FACTOR: u32 = 0x0047_d5b0;
    pub const SCALE_X: u32 = 0x004e_68f4;
    pub const SCALE_Y: u32 = 0x004e_68f8;
    /// Tint tables indexed by tint 1 (target in range) and 2 (special aim states): blend, colour.
    pub const TINT_BLEND: u32 = 0x004e_7390;
    pub const TINT_COLOR: u32 = 0x004e_739c;
    /// Game mode word (0x64: aim states 0x20 and 0x40 hide the reticle).
    pub const GAME_MODE: u32 = 0x0079_094c;
    /// Original predicates the drawer calls: candidate alive and eligible (viewer, object), within
    /// range (viewer, object), recolouring game modes (no arguments).
    pub const FN_TARGET_ELIGIBLE: u32 = 0x000d_00f0;
    pub const FN_TARGET_IN_RANGE: u32 = 0x000d_0160;
    pub const FN_RECOLOUR_MODE: u32 = 0x0012_cb40;
    /// HUD rectangle update the drawer runs first (writes 0x7A6140..0x7A614C); the gate runs it too.
    pub const FN_HUD_RECT: u32 = 0x0014_af00;
    /// Reticle indices with their own drawers (homing lock-on 0x1F, lock-on box 0xD, sniper 0x27).
    pub const OWN_DRAWER_INDICES: [u32; 3] = [0x1f, 0xd, 0x27];
    /// Shown even without the show flags.
    pub const ALWAYS_SHOWN_INDEX: u32 = 0x11;
}

/// One reticle draw: what 0x14C970 would draw this call, in 640 x 480 guest pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reticle {
    pub index: u32,
    /// Texture-table index (name via [`reticle_texture_name`]).
    pub texture: u32,
    pub centre: [f32; 2],
    /// Half extents (half size times the quad scale).
    pub half: [f32; 2],
    /// 0 plain, 1 target in range, 2 special aim state.
    pub tint: u32,
    /// Original colour word (R, G, B, A from the high byte, see [`Reticle::rgba`]).
    pub color: u32,
    pub blend: u32,
}

/// What a reticle drawer call does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReticleCall {
    /// No HUD slot: the drawer returns at once (nothing drawn, no state touched).
    NoSlot,
    /// Nothing is drawn (hidden, no texture); the drawer still updated the HUD rectangle.
    Nothing,
    /// The plain reticle quad.
    Draw(Reticle),
    /// A case the native HUD does not reproduce (lock-on and sniper drawers, the sniper bar,
    /// recolouring modes, unreadable state): legacy draws it.
    Legacy,
}

impl Reticle {
    /// Replay the decisions and arithmetic of 0x14C970 on the guest state.
    pub fn read<G: GuestCall + ?Sized>(g: &G) -> ReticleCall {
        use reticle_addr::*;
        let Some(viewer) = g.read32(addr::VIEWER_RECORD_PTR).filter(|&v| v != 0) else {
            return ReticleCall::Legacy;
        };
        let Some(slot) = g.read32(viewer + addr::RECORD_SLOT) else {
            return ReticleCall::Legacy;
        };
        if (slot as i32) < 0 {
            return ReticleCall::NoSlot;
        }
        if slot >= 4 {
            return ReticleCall::Legacy;
        }
        let Some(index) = g.read32(PLAYER_SLOTS + slot * PLAYER_SLOT_SIZE) else {
            return ReticleCall::Legacy;
        };
        let viewer_word = |offset: u32| g.read32(viewer + offset).unwrap_or(0);
        // Tint and visibility by aim state; otherwise a live target candidate in range tints 1.
        let (mut tint, mut visible) = (0, true);
        match viewer_word(VIEWER_AIM_STATE) {
            0x80 | 0x1_0000 => tint = 2,
            0x20 | 0x40 => {
                visible = g.read32(GAME_MODE) != Some(0x64);
                tint = 2;
            }
            _ => {
                let target = viewer_word(VIEWER_TARGET);
                if target != 0 {
                    let in_range = match g.call(FN_TARGET_ELIGIBLE, &[viewer, target]) {
                        Some(0) => Some(0),
                        Some(_) => g.call(FN_TARGET_IN_RANGE, &[viewer, target]),
                        None => None,
                    };
                    match in_range {
                        None => return ReticleCall::Legacy,
                        Some(0) => {}
                        Some(_) => tint = 1,
                    }
                }
            }
        }
        if OWN_DRAWER_INDICES.contains(&index) {
            return ReticleCall::Legacy;
        }
        let sub_show = g
            .read32(viewer + VIEWER_SUB)
            .filter(|&s| s != 0)
            .and_then(|s| g.read32(s + SUB_SHOW))
            .unwrap_or(0);
        let shown = sub_show != 0
            || viewer_word(VIEWER_SHOW) != 0
            || g.read32(SHOW_STATE) == Some(8)
            || index == ALWAYS_SHOWN_INDEX;
        if !shown || !visible {
            return ReticleCall::Nothing;
        }
        let record = TABLE.wrapping_add(index.wrapping_mul(RECORD_SIZE));
        let Some(texture) = g.read32(record + TEXTURE) else {
            return ReticleCall::Legacy;
        };
        if texture == u32::MAX {
            return ReticleCall::Nothing;
        }
        // Recolouring modes (plain tint) and the sniper bar after the quad stay with the original.
        if tint == 0 && g.call(FN_RECOLOUR_MODE, &[]) != Some(0) {
            return ReticleCall::Legacy;
        }
        if viewer_word(VIEWER_SCOPE_OBJECT) != 0 && viewer_word(VIEWER_FLAGS) & FLAG_SCOPE != 0 {
            return ReticleCall::Legacy;
        }
        let (Some(viewport), Some(factor)) = (
            g.read32(addr::VIEWPORT_PTR).filter(|&v| v != 0),
            g.read_f32(AIM_FACTOR),
        ) else {
            return ReticleCall::Legacy;
        };
        let word = |va: u32| g.read32(va).unwrap_or(0);
        let int = |va: u32| word(va) as i32;
        let float = |va: u32| g.read_f32(va).unwrap_or(0.0);
        // x = x0 + w - trunc(trunc(w * factor) * aim_x), the same for y (cvttss2si truncation).
        let axis = |origin: i32, extent: i32, aim: f32| {
            let step = (extent as f32 * factor) as i32;
            (origin - (step as f32 * aim) as i32 + extent) as f32
        };
        let centre = [
            axis(
                int(viewport + VP_X),
                int(viewport + VP_W),
                float(viewer + VIEWER_AIM_X),
            ),
            axis(
                int(viewport + VP_Y),
                int(viewport + VP_H),
                float(viewer + VIEWER_AIM_Y),
            ),
        ];
        let (color, blend) = if tint == 0 {
            (word(record + COLOR), word(record + BLEND))
        } else {
            (word(TINT_COLOR + 4 * tint), word(TINT_BLEND + 4 * tint))
        };
        let half_size = float(record + HALF_SIZE);
        ReticleCall::Draw(Reticle {
            index,
            texture,
            centre,
            half: [half_size * float(SCALE_X), half_size * float(SCALE_Y)],
            tint,
            color,
            blend,
        })
    }

    /// Colour of the reticle quad: the word is R, G, B, A from the high byte down, on the PS2-heritage
    /// scale is byte / 255 followed by the original combiner's times-two mapping.
    /// T2307: trace reconstruction confirms the vertex program rearranges D3DCOLOR
    /// into these channels, then scales RGB and alpha by two. This replaces the
    /// earlier inferred byte / 128 approximation; it is not a fitted brightness gain.
    pub fn rgba(&self) -> Color {
        let channel = |shift: u32| (((self.color >> shift) & 0xff) as f32 * (2.0 / 255.0)).min(1.0);
        Color([channel(24), channel(16), channel(8), channel(0)])
    }

    /// The original blend selector (`0x19D70`): 2 is additive, the others draw with alpha.
    pub fn blend_mode(&self) -> BlendMode {
        if self.blend == 2 {
            BlendMode::Additive
        } else {
            BlendMode::Alpha
        }
    }
}

/// Reticle texture ids (the drawer's `texture` word, the argument of `0x80F20`) and their texture
/// names. MEASURED: id = name-table row + 1 (row `0x4D2D64 + 24 * n`); the plain Story reticle id
/// 0x113 is `crosshair_triangle`, the three-armed reticle of the legacy frame. Identifiers only.
pub const RETICLE_TEXTURES: [(u32, &str); 27] = [
    (0xc9, "gunReticules_futureSniper"),
    (0xca, "gunReticules_machineGun"),
    (0xcb, "gunReticules_magCharger"),
    (0xcc, "gunReticules_missile"),
    (0xcd, "gunReticules_rifle"),
    (0xce, "gunReticules_rocket"),
    (0xcf, "gunReticules_shotgun"),
    (0xd0, "gunReticules_sniper"),
    (0xd1, "gunReticules_uplink"),
    (0xd2, "gunReticules_customSniperRotate"),
    (0x10d, "crosshair_old"),
    (0x110, "quartring"),
    (0x111, "scoperotate"),
    (0x112, "crosshair_circle"),
    (0x113, "crosshair_triangle"),
    (0x114, "crosshair_shaded"),
    (0x115, "crosshair_olde"),
    (0x116, "crosshair_laser"),
    (0x117, "crosshair_scifi"),
    (0x118, "crosshair_plasma"),
    (0x119, "crosshair_fixedgun"),
    (0x11a, "aim"),
    (0x11d, "ring"),
    (0x11e, "homingcrosshair"),
    (0x124, "physicsgun"),
    (0x126, "hitsector"),
    (0x127, "objective_crosshair"),
];

/// The texture name of a reticle texture index.
pub fn reticle_texture_name(texture: u32) -> Option<&'static str> {
    RETICLE_TEXTURES
        .iter()
        .find(|(i, _)| *i == texture)
        .map(|(_, n)| *n)
}

/// Guest addresses the reader uses (MEASURED in T1891/T1642 unless marked, docs/t-hud-slice.md 3.1).
pub mod addr {
    /// Viewer record pointer; `+4` local HUD slot (negative: no HUD), `+0x14` controlled object.
    /// MEASURED: 0x14F0B0 reads exactly these.
    pub const VIEWER_RECORD_PTR: u32 = 0x007b_0c7c;
    pub const RECORD_SLOT: u32 = 0x4;
    pub const RECORD_OBJECT: u32 = 0x14;
    /// Object: health `+0x1CC`, maximum `+0x1D8` (f32), character/extension pointer `+0x7C`, class `+0x20`.
    pub const OBJ_HEALTH: u32 = 0x1cc;
    pub const OBJ_MAX_HEALTH: u32 = 0x1d8;
    pub const OBJ_CHARACTER: u32 = 0x7c;
    pub const OBJ_CLASS: u32 = 0x20;
    pub const CLASS_CHARACTER: u32 = 8;
    /// Character: armour pool `+0x78`, armour maximum `+0x7C` (INFERRED from table 0x4E6F28 use).
    pub const CHAR_ARMOR: u32 = 0x78;
    pub const CHAR_ARMOR_MAX: u32 = 0x7c;
    /// Extension: current weapon entry `+0x94`; hand B at `+0x1D4`: record index `+0x30`, loaded
    /// rounds by ammo type `+0x38 + 2 * type` (u16); reserve by type `+0x61E + 2 * type` (u16).
    pub const EXT_CURRENT_WEAPON_ENTRY: u32 = 0x94;
    pub const EXT_HAND_B: u32 = 0x1d4;
    pub const HAND_RECORD_INDEX: u32 = 0x30;
    pub const HAND_LOADED_BY_TYPE: u32 = 0x38;
    pub const EXT_RESERVE_BY_TYPE: u32 = 0x61e;
    /// Weapon records: `0x4EEF74 + index * 0x29C`, ammo type 1 `+0x14`, clip size 1 `+0x18` (INFERRED).
    pub const WEAPON_RECORDS: u32 = 0x004e_ef74;
    pub const WEAPON_RECORD_SIZE: u32 = 0x29c;
    pub const WEAPON_RECORD_COUNT: u32 = 0x42;
    pub const WEAPON_AMMO_TYPE: u32 = 0x14;
    pub const WEAPON_CLIP_SIZE: u32 = 0x18;
    /// Letterbox amount (f32); the HUD frame draws only when it equals the constant at 0x475CAC.
    pub const LETTERBOX_AMOUNT: u32 = 0x0074_90ec;
    pub const LETTERBOX_NONE: u32 = 0x0047_5cac;
    /// Viewport rectangle pointer: x0 `+0`, x1 `+4`, y0 `+0x14`, y1 `+0x18` (INFERRED names).
    pub const VIEWPORT_PTR: u32 = 0x007e_71c8;
}

/// Persistent Rust authority for the four local HUDs. Frame facts arrive through
/// a typed game boundary; animation and queue state survive those publications.
/// Native handlers mutate these values directly, and renderers borrow them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HudState {
    pub frames: [Option<HudFrame>; 4],
    pub weapons: [crate::hud_producers::WeaponAnimation; 4],
    pub messages: [crate::hud_producers::OwnedMessageQueue; 4],
    pub radar: [crate::hud_radar::RadarAnimation; 4],
    /// Original per-player +0x1710 input-progress contract.
    pub input_progress: [u32; 4],
    /// One typed eligible HUD action per player. The input owner publishes a
    /// fresh value each simulation step; the message scheduler consumes it once.
    pub message_action: [HudMessageAction; 4],
    /// Tracks the one-time guest-to-Rust queue handoff separately per player.
    imported_message_queues: [bool; 4],
}

/// Native input handoff for kind-7 HUD notices. Eligibility is decided by the
/// adjacent Rust input owner; the HUD stores no pad index or guest address.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HudMessageAction {
    eligible: bool,
    consumed: bool,
}

impl HudMessageAction {
    /// Publish the measured pad/mask intersection for one simulation step.
    fn publish(&mut self, eligible: bool) {
        self.eligible = eligible;
        self.consumed = false;
    }

    /// Consume the eligible action at most once, even when several notices are scheduled in one frame.
    pub fn take(&mut self) -> bool {
        if !self.eligible || self.consumed {
            return false;
        }
        self.consumed = true;
        true
    }
}

/// Width of one canonical message owner trace window. It matches the guest
/// span from queue cursor `H+0x30` through input progress `H+0x1710`; reserved
/// bytes and text after the first NUL are zeroed in both projections.
pub const HUD_MESSAGE_TRACE_BYTES: usize = crate::hud_producers::OWNED_MESSAGE_TRACE_BYTES;
pub const HUD_MESSAGE_TRACE_WORDS: usize = crate::hud_producers::OWNED_MESSAGE_TRACE_WORDS;

impl HudState {
    /// Import the measured legacy state once, before native queue operations
    /// begin. Later calls leave Rust-owned lifecycle state untouched.
    pub fn import_message_queue(
        &mut self,
        player: usize,
        snapshot: crate::hud_producers::MessageQueueSnapshot,
    ) -> bool {
        if player >= self.messages.len() {
            return false;
        }
        if self.imported_message_queues[player] {
            return true;
        }
        self.input_progress[player] = snapshot.input_progress;
        self.messages[player].import_snapshot(snapshot);
        self.imported_message_queues[player] = true;
        true
    }

    pub fn message_queue_imported(&self, player: usize) -> bool {
        self.imported_message_queues
            .get(player)
            .copied()
            .unwrap_or(false)
    }

    /// Publish neighbouring game facts without replacing UI lifecycle state.
    pub fn publish_frame(&mut self, frame: HudFrame) -> bool {
        let Some(entry) = self.frames.get_mut(frame.slot as usize) else {
            return false;
        };
        *entry = Some(frame);
        true
    }

    /// Native weapon-system attachment; no synthetic buttons or guest writes.
    pub fn weapon_event(
        &mut self,
        slot: usize,
        event: crate::hud_producers::WeaponPanelEvent,
        full: f32,
    ) -> bool {
        let Some(animation) = self.weapons.get_mut(slot) else {
            return false;
        };
        animation.apply_event(event, full);
        true
    }

    /// Direct text-system attachment for an original-preset notice. Invalid
    /// signed player indices are ignored, matching the original entry points.
    pub fn message_notice(
        &mut self,
        player: i32,
        kind: crate::hud_producers::NoticeKind,
        text: &[u8],
        opaque: u32,
        delay: f32,
        tuning: &crate::messages::MessageTuning,
    ) -> bool {
        let Some(queue) = usize::try_from(player)
            .ok()
            .and_then(|slot| self.messages.get_mut(slot))
        else {
            return false;
        };
        queue.push_notice(kind, text, opaque, delay, tuning);
        true
    }

    /// Direct generic-message attachment. Runtime table/metrics decoding supplies
    /// the preset; invalid players and suppressed duplicates leave all state intact.
    pub fn insert_message(
        &mut self,
        player: i32,
        kind: u32,
        text: &[u8],
        delay: f32,
        preset: &crate::hud_producers::GenericMessagePreset,
    ) -> crate::hud_producers::MessageInsertion {
        use crate::hud_producers::MessageInsertion;
        let Some(slot) = usize::try_from(player).ok().filter(|slot| *slot < 4) else {
            return MessageInsertion::Invalid;
        };
        let result = self.messages[slot].insert_generic(kind, text, delay, preset);
        if matches!(result, MessageInsertion::Inserted(_)) && preset.reset_input_progress {
            self.input_progress[slot] = 0;
        }
        result
    }

    /// Generic insertion and cue publication are one queue mutation, matching
    /// `0x148A10`: rejected or duplicate text leaves any existing cue untouched.
    pub fn insert_message_with_cue(
        &mut self,
        player: i32,
        kind: u32,
        text: &[u8],
        delay: f32,
        cue: i32,
        preset: &crate::hud_producers::GenericMessagePreset,
    ) -> crate::hud_producers::MessageAdmission {
        use crate::hud_producers::MessageAdmission;
        let Some(slot) = usize::try_from(player).ok().filter(|slot| *slot < 4) else {
            return MessageAdmission::Invalid;
        };
        let admission = self.messages[slot].admit_generic_with_cue(kind, text, delay, cue, preset);
        if matches!(admission, MessageAdmission::Inserted(_)) && preset.reset_input_progress {
            self.input_progress[slot] = 0;
        }
        admission
    }

    /// Post the typed kind-6 event into one player's owned queue. Caller-side
    /// object and player-count checks remain at the source attachment point.
    pub fn post_kind6_message(
        &mut self,
        player: i32,
        source_handle: u32,
        age_seed: f32,
        tuning: crate::hud_producers::Kind6MessageTuning,
    ) -> Option<crate::hud_producers::Kind6CueRequest> {
        let slot = usize::try_from(player).ok().filter(|slot| *slot < 4)?;
        self.messages[slot].post_kind6(source_handle, age_seed, tuning)
    }

    /// Direct message-system clear handler; no guest writes or pad emulation.
    pub fn clear_messages(&mut self, player: i32, kind: i32) -> bool {
        let Some(queue) = usize::try_from(player)
            .ok()
            .and_then(|slot| self.messages.get_mut(slot))
        else {
            return false;
        };
        queue.clear_kind(kind);
        true
    }

    /// Advance one player's Rust-owned message lifecycle at the simulation tick.
    /// The player is validated before indexing; queue expiry and audio effects are
    /// delegated to the typed queue contract, never to guest message storage.
    /// Verification: Tier 0 player-selection/expiry smoke coverage; the underlying
    /// queue tick is Tier 1 against lifted `0x14A5A0` on FABRICATED queue states.
    pub fn advance_messages(
        &mut self,
        player: i32,
        seconds: f32,
        frame_delta: i32,
        policy: &crate::hud_producers::MessagePolicy,
        audio: &mut impl crate::hud_producers::HudSounds,
    ) -> bool {
        let Some(queue) = usize::try_from(player)
            .ok()
            .and_then(|slot| self.messages.get_mut(slot))
        else {
            return false;
        };
        queue.advance(seconds, frame_delta, policy, audio);
        true
    }

    /// Publish the current pad state for one player's kind-7 action gate.
    /// The eligible mask belongs to that player's input context (`+0x20` in
    /// the lifted contract); no pad index or guest pointer is retained.
    pub fn publish_message_buttons(
        &mut self,
        player: i32,
        current_buttons: u32,
        eligible_mask: u32,
    ) -> bool {
        let Some(action) = usize::try_from(player)
            .ok()
            .and_then(|slot| self.message_action.get_mut(slot))
        else {
            return false;
        };
        action.publish(current_buttons & eligible_mask != 0);
        true
    }

    /// Schedule a Rust-owned player's messages and apply kind-7 input progress to that same player's typed
    /// state. The action callback is native input consumption; it receives no guest address or synthetic pad
    /// event. The queue scheduler preserves filtering, draw-before-retirement and weight accounting from the
    /// REA `0x14AC80` contract. Verification: Tier 0 player routing and progress/retirement smoke test; the
    /// live router-to-consumer attachment is opt-in and still awaits Tier 2 owner-route adoption.
    pub fn replay_messages_native(
        &mut self,
        player: i32,
        policy: &crate::hud_producers::MessagePolicy,
        only_kind_9: bool,
        show_split_notice: bool,
        draw: impl FnMut(&crate::messages::Message, f32, f32, u32),
    ) -> Option<[i32; 14]> {
        let slot = usize::try_from(player).ok().filter(|slot| *slot < 4)?;
        let queue = self.messages.get_mut(slot)?;
        let progress = self.input_progress.get_mut(slot)?;
        let action = self.message_action.get_mut(slot)?;
        Some(queue.replay_native(
            policy,
            only_kind_9,
            show_split_notice,
            progress,
            || action.take(),
            draw,
        ))
    }

    /// Run the native message scheduler transactionally around fallible
    /// presentation. Queue retirement, input consumption and shared progress
    /// commit together only after every draw has a supported Rust renderer.
    pub fn try_replay_messages_native(
        &mut self,
        player: i32,
        policy: &crate::hud_producers::MessagePolicy,
        only_kind_9: bool,
        show_split_notice: bool,
        mut draw: impl FnMut(&crate::messages::Message, f32, f32, u32) -> Result<(), &'static str>,
    ) -> Result<[i32; 14], &'static str> {
        let slot = usize::try_from(player)
            .ok()
            .filter(|slot| *slot < 4)
            .ok_or("invalid player")?;
        let mut queue = self.messages[slot].clone();
        let mut progress = self.input_progress[slot];
        let mut action = self.message_action[slot];
        let mut draw_error = None;
        let sums = queue.replay_native(
            policy,
            only_kind_9,
            show_split_notice,
            &mut progress,
            || action.take(),
            |message, slide, offset, progress| {
                if draw_error.is_none() {
                    draw_error = draw(message, slide, offset, progress).err();
                }
            },
        );
        if let Some(error) = draw_error {
            return Err(error);
        }
        self.messages[slot] = queue;
        self.input_progress[slot] = progress;
        self.message_action[slot] = action;
        Ok(sums)
    }

    /// Serialize one Rust-owned queue in the canonical T3397 trace layout.
    /// The window keeps the original 0x16c entry stride and field order while
    /// normalizing non-semantic padding and bytes after the text terminator.
    pub fn encode_message_trace(&self, player: usize, output: &mut [u32]) -> bool {
        if player >= self.messages.len() || output.len() < HUD_MESSAGE_TRACE_WORDS {
            return false;
        }
        let words = &mut output[..HUD_MESSAGE_TRACE_WORDS];
        if !self.messages[player].encode_trace_window(words) {
            return false;
        }
        words[HUD_MESSAGE_TRACE_BYTES / 4 - 1] = self.input_progress[player];
        true
    }

    /// Reset a local viewport at level/session transition.
    pub fn reset_slot(&mut self, slot: usize) -> bool {
        if slot >= self.frames.len() {
            return false;
        }
        self.frames[slot] = None;
        self.weapons[slot] = Default::default();
        self.messages[slot].reset();
        self.radar[slot] = Default::default();
        self.input_progress[slot] = 0;
        self.imported_message_queues[slot] = false;
        true
    }
}

/// The typed HUD state of the local viewer for one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HudFrame {
    /// Local HUD slot 0..3.
    pub slot: u32,
    pub health: f32,
    pub max_health: f32,
    /// Armour pool and maximum (characters only, else 0).
    pub armour: f32,
    pub armour_max: f32,
    /// Weapon record index of the held weapon; `None` for hands.
    pub weapon_record: Option<u32>,
    pub ammo_type: u32,
    pub clip_size: u32,
    pub ammo_loaded: u32,
    pub ammo_reserve: u32,
    /// Letterbox amount (cutscenes); the HUD hides while it is not the "none" constant.
    pub letterbox: f32,
    pub letterbox_none: f32,
    /// Guest viewport x0, x1, y0, y1 in 640 x 480 design pixels.
    pub viewport: [i32; 4],
}

impl HudFrame {
    /// Read the viewer's HUD state. `None` while there is no viewer (menus, loading) or a pointer is unmapped.
    pub fn read<G: GuestRead + ?Sized>(g: &G) -> Option<Self> {
        use addr::*;
        let record = g.read32(VIEWER_RECORD_PTR).filter(|&r| r != 0)?;
        let slot = g.read32(record + RECORD_SLOT)?;
        if slot as i32 >= 0x4 || (slot as i32) < 0 {
            return None;
        }
        let object = g.read32(record + RECORD_OBJECT).filter(|&o| o != 0)?;
        let character = g.read32(object + OBJ_CHARACTER).unwrap_or(0);
        let is_character = g.read32(object + OBJ_CLASS) == Some(CLASS_CHARACTER);
        let mut frame = HudFrame {
            slot,
            health: g.read_f32(object + OBJ_HEALTH)?,
            max_health: g.read_f32(object + OBJ_MAX_HEALTH)?,
            letterbox: g.read_f32(LETTERBOX_AMOUNT).unwrap_or(0.0),
            letterbox_none: g.read_f32(LETTERBOX_NONE).unwrap_or(0.0),
            ..HudFrame::default()
        };
        if let Some(viewport) = g.read32(VIEWPORT_PTR).filter(|&v| v != 0) {
            let word = |offset| g.read32(viewport + offset).map_or(0, |v| v as i32);
            frame.viewport = [word(0), word(4), word(0x14), word(0x18)];
        }
        if character != 0 {
            if is_character {
                frame.armour = g.read_f32(character + CHAR_ARMOR).unwrap_or(0.0);
                frame.armour_max = g.read_f32(character + CHAR_ARMOR_MAX).unwrap_or(0.0);
            }
            let hand = character + EXT_HAND_B;
            let index = g.read32(hand + HAND_RECORD_INDEX).unwrap_or(u32::MAX);
            if index < WEAPON_RECORD_COUNT {
                let record = WEAPON_RECORDS + index * WEAPON_RECORD_SIZE;
                frame.weapon_record = Some(index);
                frame.ammo_type = g.read32(record + WEAPON_AMMO_TYPE).unwrap_or(0) & 0xff;
                frame.clip_size = g.read32(record + WEAPON_CLIP_SIZE).unwrap_or(0);
                frame.ammo_loaded = g
                    .read16(hand + HAND_LOADED_BY_TYPE + 2 * frame.ammo_type)
                    .map_or(0, u32::from);
                frame.ammo_reserve = g
                    .read16(character + EXT_RESERVE_BY_TYPE + 2 * frame.ammo_type)
                    .map_or(0, u32::from);
            }
        }
        Some(frame)
    }

    /// The original HUD frame (0x14F0B0) draws elements only while no letterbox is up.
    pub fn hud_visible(&self) -> bool {
        self.letterbox == self.letterbox_none
    }
}

/// Where the HUD is anchored.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HudAnchor {
    /// Original preset: the 640 x 480 design space scaled into the 4:3 game picture.
    #[default]
    Picture,
    /// Widescreen option: corner elements move to the window's safe area, centre elements stay centred.
    SafeArea,
}

/// A text run of the native HUD: bytes in an original font, right/left/centre aligned in a design rect.
#[derive(Clone, Debug, PartialEq)]
pub struct HudText {
    pub element: HudElement,
    pub bytes: Vec<u8>,
    /// Font object index of the original font tables.
    pub font: u32,
    pub rect_px: Rect,
    /// Additional column bounds without changing the authored glyph pen or wrap width.
    pub clip_px: Option<Rect>,
    pub align: crate::model::Align,
    /// Alignment of the measured font line inside rect_px; numeric rows are vertically centred.
    pub vertical_align: crate::model::Align,
    pub color: Color,
    /// Glyph scale over the draw's `text_scale` (x, y); `[1.0, 1.0]` for most text.
    pub scale: [f32; 2],
    /// Drawn additively (the original's blend mode 2: message titles), after the alpha-blended texts.
    pub additive: bool,
}

/// A textured rectangle; `texture` is an original texture-table name resolved from the disc at run time.
#[derive(Clone, Debug, PartialEq)]
pub struct HudSprite {
    pub element: HudElement,
    pub texture: &'static str,
    pub rect_px: Rect,
    pub color: Color,
    /// Fit the texture inside `rect_px` keeping its aspect ratio, centred (icons with margins).
    pub keep_aspect: bool,
    /// How the sprite combines with the frame (the original's blend mode 2 is additive).
    pub blend: BlendMode,
    /// Texture coordinates `[u0, v0, u1, v1]` mapped to the rect's left-top and right-bottom
    /// (`u0 > u1` mirrors). [`FULL_UV`] for the whole texture.
    pub uv: [f32; 4],
}

/// A textured quad with free corners in window pixels (strip order: texture top-left, top-right,
/// bottom-left, bottom-right) and texture coordinates `[u0, v0, u1, v1]` over those corners.
#[derive(Clone, Debug, PartialEq)]
pub struct HudPoly {
    pub element: HudElement,
    pub texture: &'static str,
    pub corners_px: [Vec2; 4],
    pub color: Color,
    pub uv: [f32; 4],
    /// The hit sector is additive (the original's blend mode 2), the radar alpha (mode 1).
    pub blend: BlendMode,
}

/// The whole texture, unmirrored.
pub const FULL_UV: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// One piece of a sliced HUD frame texture: design box and texture coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FramePiece {
    pub at: DesignBox,
    pub uv: [f32; 4],
}

/// Everything one native HUD frame draws, in painter order per list (quads, sprites, then text).
#[derive(Clone, Debug, PartialEq)]
pub struct HudDraw {
    pub quads: Vec<Quad>,
    /// Rotated textured quads (the hit sector), additive, drawn before the sprites.
    pub polys: Vec<HudPoly>,
    pub sprites: Vec<HudSprite>,
    pub texts: Vec<HudText>,
    /// Sprites the original draws in the view pass (the weapon reticle, `0x14C970`), before the
    /// frame's closing colour grade: the renderer draws them into the grade's source surface
    /// (`FrameLayer::encode_before_grade`), else with the rest of the HUD.
    pub view_pass: Vec<HudSprite>,
    /// Glyph scale over the design-space font size (x, y); the original stretches HUD digits.
    pub text_scale: [f32; 2],
}

impl Default for HudDraw {
    fn default() -> Self {
        HudDraw {
            quads: Vec::new(),
            polys: Vec::new(),
            sprites: Vec::new(),
            texts: Vec::new(),
            view_pass: Vec::new(),
            text_scale: [1.0, 1.0],
        }
    }
}

impl HudDraw {
    pub fn is_empty(&self) -> bool {
        self.quads.is_empty()
            && self.polys.is_empty()
            && self.sprites.is_empty()
            && self.view_pass.is_empty()
            && self.texts.is_empty()
    }
}

/// One design-space box (640 x 480 units) with the corner it belongs to (for the safe-area option).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DesignBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Horizontal anchor: -1 left edge, 0 centre, 1 right edge.
    pub anchor_x: i8,
    /// Vertical anchor: -1 top, 0 centre, 1 bottom.
    pub anchor_y: i8,
}

/// HUD geometry and colours. [`HudLayout::original`] is the original preset.
#[derive(Clone, Debug, PartialEq)]
pub struct HudLayout {
    /// Reserve rounds, right aligned left of the icon.
    pub ammo_reserve: DesignBox,
    /// Weapon/ammo icon between the two numbers.
    pub ammo_icon: DesignBox,
    /// Loaded rounds, left aligned right of the icon.
    pub ammo_loaded: DesignBox,
    pub ammo_font: u32,
    pub ammo_color: Color,
    pub ammo_icon_texture: &'static str,
    /// Dark rounded frame around the icon: `newHud_bagBorder` sliced into a mirrored left cap, a
    /// stretched left arm and the right cap, with the original's gap right of the icon.
    pub ammo_frame: [FramePiece; 3],
    pub ammo_frame_texture: &'static str,
    pub ammo_frame_color: Color,
    /// Original numeric-row glyph factors, shared by measurement and atlas rasterization.
    pub ammo_text_scale: [f32; 2],
    /// Safe-area margin as a fraction of the window (widescreen option).
    pub safe_margin: f32,
}

impl HudLayout {
    /// The original preset. Geometry MEASURED from legacy frames (docs/t2312-native-hud.md, frame
    /// differences with and without the legacy drawer at the same gameplay frame).
    pub fn original() -> Self {
        HudLayout {
            // Two consecutive original uploads resolve the right-alignment edge to 542.
            ammo_reserve: DesignBox {
                x: 482.0,
                y: 424.5,
                w: 60.0,
                h: 16.0,
                anchor_x: 1,
                anchor_y: 1,
            },
            // The 32 x 32 icon texture stretched to 4:3 (42.67 x 32 design units), centred on (563, 432).
            ammo_icon: DesignBox {
                x: 541.67,
                y: 416.0,
                w: 42.67,
                h: 32.0,
                anchor_x: 1,
                anchor_y: 1,
            },
            // Original numeric glyph upload's first pen, rather than the rounded image-fit edge.
            ammo_loaded: DesignBox {
                x: 589.5625,
                y: 424.5,
                w: 60.0,
                h: 16.0,
                anchor_x: 1,
                anchor_y: 1,
            },
            // 0x1325D0 supersedes 0x1334D0's initial font selection with *0x4E7278.
            // Original preset: object 19 uses the Arial atlas at design scales 0.45/0.55.
            ammo_font: 19,
            ammo_color: Color([0.85, 0.85, 0.85, 0.9]),
            ammo_icon_texture: "newAmmo_small.rgb",
            ammo_frame: [
                FramePiece {
                    at: DesignBox {
                        x: 534.5,
                        y: 410.5,
                        w: 10.5,
                        h: 45.0,
                        anchor_x: 1,
                        anchor_y: 1,
                    },
                    uv: [54.0 / 64.0, 13.0 / 64.0, 33.0 / 64.0, 54.0 / 64.0],
                },
                FramePiece {
                    at: DesignBox {
                        x: 545.0,
                        y: 410.5,
                        w: 37.0,
                        h: 45.0,
                        anchor_x: 1,
                        anchor_y: 1,
                    },
                    uv: [30.0 / 64.0, 13.0 / 64.0, 20.0 / 64.0, 54.0 / 64.0],
                },
                FramePiece {
                    at: DesignBox {
                        x: 586.5,
                        y: 410.5,
                        w: 21.0,
                        h: 45.0,
                        anchor_x: 1,
                        anchor_y: 1,
                    },
                    uv: [12.0 / 64.0, 13.0 / 64.0, 54.0 / 64.0, 54.0 / 64.0],
                },
            ],
            ammo_frame_texture: "newHud_bagBorder",
            ammo_frame_color: Color([1.0, 1.0, 1.0, 0.286]),
            // Original glyph uploads: effective 0.5625 / 0.5892857 design pixels per atlas pixel.
            ammo_text_scale: [1.25, 15.0 / 14.0],
            safe_margin: 0.05,
        }
    }
}

/// Maps design boxes to window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudSpace {
    /// The 4:3 game picture inside the window, in window pixels.
    pub picture: Rect,
    /// The whole window.
    pub window: Vec2,
    pub anchor: HudAnchor,
    pub safe_margin: f32,
}

impl HudSpace {
    /// The picture letterboxed (pillarboxed) into the window, as the shell presents it.
    pub fn for_window(width: f32, height: f32, anchor: HudAnchor, safe_margin: f32) -> Self {
        let scale = (width / 640.0).min(height / 480.0);
        let size = Vec2::new(640.0 * scale, 480.0 * scale);
        let origin = Vec2::new((width - size.x) / 2.0, (height - size.y) / 2.0);
        HudSpace {
            picture: Rect { origin, size },
            window: Vec2::new(width, height),
            anchor,
            safe_margin,
        }
    }

    fn scale(&self) -> f32 {
        self.picture.size.x / 640.0
    }

    /// A design-space point (centre-anchored) in window pixels.
    pub fn point(&self, x: f32, y: f32) -> Vec2 {
        let s = self.scale();
        Vec2::new(self.picture.origin.x + x * s, self.picture.origin.y + y * s)
    }

    pub fn place(&self, b: DesignBox) -> Result<Rect, LayoutError> {
        use tsfp_ui_renderer::{AnchorSpace, AnchoredRect, Edge, Viewport};
        let viewport = Viewport::new(self.window, self.scale(), self.picture)?;
        let edge = |anchor| match anchor {
            -1 => Edge::Start,
            1 => Edge::End,
            _ => Edge::Center,
        };
        // Preserve each authored group, including its internal spacing. A safety
        // margin replaces the original reference inset rather than adding twice.
        let offset = |position: f32, extent: f32, authored: f32, anchor: i8, window: f32| {
            // Original has a 5% reference safe inset. Only add the amount by which
            // the window margin exceeds that reference; all pieces of a group move together.
            let extra = if self.anchor == HudAnchor::SafeArea {
                (window * self.safe_margin.max(0.0) / self.scale() - authored * 0.05).max(0.0)
            } else {
                0.0
            };
            match anchor {
                -1 => position + extra,
                1 => position - (authored - extent) - extra,
                _ => position - (authored - extent) * 0.5,
            }
        };
        viewport.resolve(AnchoredRect {
            space: if self.anchor == HudAnchor::SafeArea {
                AnchorSpace::Window
            } else {
                AnchorSpace::SafeArea4x3
            },
            horizontal: edge(b.anchor_x),
            vertical: edge(b.anchor_y),
            offset: Vec2::new(
                offset(b.x, b.w, 640.0, b.anchor_x, self.window.x),
                offset(b.y, b.h, 480.0, b.anchor_y, self.window.y),
            ),
            size: Vec2::new(b.w, b.h),
        })
    }
}

/// Decimal digits of a counter as the original prints it (no padding, no sign).
pub fn counter_text(value: u32) -> Vec<u8> {
    value.to_string().into_bytes()
}

/// Slices per band: the original shades each band with a top-to-bottom vertex gradient; the sprite
/// contract has one colour per sprite, so each band is drawn as this many slices with the colour
/// interpolated at the slice centre.
pub const ARC_SLICES: usize = 4;

/// The arcs as sprites (guest pixels are 640 x 480 design units).
fn arc_sprites(draw: &mut HudDraw, arcs: &VitalsArcs, space: &HudSpace) -> Result<(), LayoutError> {
    for quad in arcs.quads() {
        let [x0, y0, x1, y1] = quad.rect;
        let [u0, u1, v0, v1] = quad.uv;
        if !(y1 > y0) || !(x1 > x0) {
            continue;
        }
        let Some(&(_, _, texture)) = ARC_PASSES.iter().find(|p| p.0 == quad.texture) else {
            continue;
        };
        for slice in 0..ARC_SLICES {
            let (a, b) = (
                slice as f32 / ARC_SLICES as f32,
                (slice + 1) as f32 / ARC_SLICES as f32,
            );
            let mid = (a + b) * 0.5;
            let mut c = [0; 4];
            for k in 0..4 {
                c[k] = (quad.top[k] as f32 + (quad.bottom[k] - quad.top[k]) as f32 * mid).round()
                    as i32;
            }
            let at = DesignBox {
                x: x0,
                y: y0 + (y1 - y0) * a,
                w: x1 - x0,
                h: (y1 - y0) * (b - a),
                anchor_x: 0,
                anchor_y: 0,
            };
            draw.sprites.push(HudSprite {
                element: HudElement::Vitals,
                texture,
                rect_px: space.place(at)?,
                color: arc_color(c),
                keep_aspect: false,
                blend: if quad.additive {
                    BlendMode::Additive
                } else {
                    BlendMode::Alpha
                },
                uv: [u0, v0 + (v1 - v0) * a, u1, v0 + (v1 - v0) * b],
            });
        }
    }
    Ok(())
}

/// A design box mirrored to the other side of the 640-unit picture.
fn mirrored(b: DesignBox) -> DesignBox {
    DesignBox {
        x: 640.0 - b.x - b.w,
        anchor_x: -b.anchor_x,
        ..b
    }
}

/// One ammo row: reserve, the sliced frame, the icon and the loaded rounds; the left row mirrors the right.
fn ammo_row(
    draw: &mut HudDraw,
    row: &AmmoRow,
    table: &HudLayout,
    space: &HudSpace,
) -> Result<(), LayoutError> {
    use crate::model::Align;
    let left = row.side == AmmoSide::Left;
    // 0x1325D0: anchor = viewport bottom less inset (T3787); each occupied row raises it 49 * scale_y.
    let place = |mut b: DesignBox| {
        b.y -= f32::from(row.stack_index) * 49.0 + (480 - row.viewport_bottom) as f32;
        space.place(if left { mirrored(b) } else { b })
    };
    let (outer, inner) = if left {
        (Align::Start, Align::End)
    } else {
        (Align::End, Align::Start)
    };
    draw.texts.push(HudText {
        additive: false,
        element: HudElement::Ammo,
        bytes: counter_text(row.reserve),
        font: table.ammo_font,
        clip_px: None,
        rect_px: place(table.ammo_reserve)?,
        align: outer,
        vertical_align: Align::Center,
        color: table.ammo_color,
        scale: [1.0, 1.0],
    });
    for piece in &table.ammo_frame {
        let uv = if left {
            [piece.uv[2], piece.uv[1], piece.uv[0], piece.uv[3]]
        } else {
            piece.uv
        };
        draw.sprites.push(HudSprite {
            element: HudElement::Ammo,
            texture: table.ammo_frame_texture,
            rect_px: place(piece.at)?,
            color: table.ammo_frame_color,
            keep_aspect: false,
            blend: BlendMode::Alpha,
            uv,
        });
    }
    draw.sprites.push(HudSprite {
        element: HudElement::Ammo,
        texture: table.ammo_icon_texture,
        rect_px: place(table.ammo_icon)?,
        color: table.ammo_color,
        keep_aspect: false,
        blend: BlendMode::Alpha,
        uv: FULL_UV,
    });
    draw.texts.push(HudText {
        additive: false,
        element: HudElement::Ammo,
        bytes: counter_text(row.loaded),
        font: table.ammo_font,
        clip_px: None,
        rect_px: place(table.ammo_loaded)?,
        align: inner,
        vertical_align: Align::Center,
        color: table.ammo_color,
        scale: [1.0, 1.0],
    });
    Ok(())
}

/// Lay out the owned elements that the original requested this frame.
pub fn layout(
    frame: &HudFrame,
    requests: &[DrawRequest],
    owned: HudOwnership,
    table: &HudLayout,
    space: &HudSpace,
) -> Result<HudDraw, LayoutError> {
    let mut draw = HudDraw {
        text_scale: table.ammo_text_scale,
        ..HudDraw::default()
    };
    if !frame.hud_visible() {
        return Ok(draw);
    }
    // The reticle: one quad per crosshair request, as the original call would draw it.
    if owned.owns(HudElement::Crosshair) {
        for reticle in requests
            .iter()
            .filter(|r| r.element == HudElement::Crosshair)
            .filter_map(|r| r.reticle)
        {
            let Some(texture) = reticle_texture_name(reticle.texture) else {
                continue;
            };
            let at = DesignBox {
                x: reticle.centre[0] - reticle.half[0],
                y: reticle.centre[1] - reticle.half[1],
                w: 2.0 * reticle.half[0],
                h: 2.0 * reticle.half[1],
                anchor_x: 0,
                anchor_y: 0,
            };
            draw.view_pass.push(HudSprite {
                element: HudElement::Crosshair,
                texture,
                rect_px: space.place(at)?,
                color: reticle.rgba(),
                keep_aspect: false,
                blend: reticle.blend_mode(),
                uv: FULL_UV,
            });
        }
    }
    // The health and armour arcs: band quads with a vertical gradient, drawn as thin slices.
    if owned.owns(HudElement::Vitals) {
        for arcs in requests
            .iter()
            .filter(|r| r.element == HudElement::Vitals)
            .filter_map(|r| r.vitals)
        {
            arc_sprites(&mut draw, &arcs, space)?;
        }
        // The hit sector the same calls drew first (Rust-owned timer).
        for sector in requests
            .iter()
            .filter(|r| r.element == HudElement::Vitals)
            .filter_map(|r| r.hit_sector)
        {
            draw.polys.push(HudPoly {
                element: HudElement::Vitals,
                texture: HIT_SECTOR_TEXTURE,
                corners_px: sector.corners.map(|[x, y]| space.point(x, y)),
                color: arc_color(sector.color),
                uv: FULL_UV,
                blend: BlendMode::Additive,
            });
        }
    }
    // The ammo rows: one per drawer call that draws (right hand, and the left hand of a dual wield, mirrored).
    // A call without a row is the original drawer's own early-out (`AmmoCall::Nothing`: no record, or ammo type 0
    // as in the credits): it draws nothing, so the frame's held weapon is never a substitute (T3816).
    if owned.owns(HudElement::Ammo) {
        let rows: Vec<AmmoRow> = requests
            .iter()
            .filter(|r| r.element == HudElement::Ammo)
            .filter_map(|r| r.ammo)
            .collect();
        for row in rows {
            ammo_row(&mut draw, &row, table, space)?;
        }
    }
    // The radar: every piece as a quad in the original's painter order (blend mode 1).
    if owned.owns(HudElement::Radar) {
        for pieces in requests
            .iter()
            .filter(|r| r.element == HudElement::Radar)
            .filter_map(|r| r.radar.as_ref())
        {
            radar(&mut draw, pieces, space)?;
        }
    }
    // The rank box: frame, panel and the outlined label and value, as the original call would draw them.
    if owned.owns(HudElement::Rank) || owned.owns(HudElement::Points) {
        for rank in requests
            .iter()
            .filter(|r| {
                (r.element == HudElement::Rank || r.element == HudElement::Points)
                    && owned.owns(r.element)
            })
            .filter_map(|r| r.rank.as_ref())
        {
            rank_box(&mut draw, &rank.row, table, space)?;
        }
    }
    // The effect icons: four textured quads in the row under the rank box.
    if owned.owns(HudElement::Effects) {
        for icons in requests
            .iter()
            .filter(|r| r.element == HudElement::Effects)
            .filter_map(|r| r.effects.as_ref())
        {
            for icon in &icons.icons {
                let [x0, y0, x1, y1] = icon.rect;
                let at = DesignBox {
                    x: x0,
                    y: y0,
                    w: x1 - x0,
                    h: y1 - y0,
                    anchor_x: -1,
                    anchor_y: -1,
                };
                // `0x1BA70` byte-reverses its colour word: A B G R high to low is the reverse of the HUD colour word.
                draw.sprites.push(HudSprite {
                    element: HudElement::Effects,
                    texture: icon.texture,
                    rect_px: space.place(at)?,
                    color: word_color(icon.colour.swap_bytes()),
                    keep_aspect: false,
                    blend: BlendMode::Alpha,
                    uv: FULL_UV,
                });
            }
        }
    }
    // The button prompt panel: the rounded panel, the inline icons, then the texts (outlined ones with their passes).
    if owned.owns(HudElement::Prompts) {
        for panel in requests
            .iter()
            .filter(|r| r.element == HudElement::Prompts)
            .filter_map(|r| r.prompts.as_ref())
        {
            prompt_panel(&mut draw, panel, table, space)?;
        }
    }
    // The message queue: each outlined string as four outline passes and the text (titles additive).
    if owned.owns(HudElement::Messages) {
        for queue in requests
            .iter()
            .filter(|r| r.element == HudElement::Messages)
            .filter_map(|r| r.messages.as_ref())
        {
            message_texts(&mut draw, queue, table, space)?;
        }
    }
    // The weapon slot panel: panel, then per icon the dark icon, the additive overlay and glow, and the arrow.
    if owned.owns(HudElement::Weapons) {
        for panel in requests
            .iter()
            .filter(|r| r.element == HudElement::Weapons)
            .filter_map(|r| r.weapons.as_ref())
        {
            weapon_panel(&mut draw, panel, space)?;
        }
    }
    Ok(draw)
}

/// The weapon slot panel ([`crate::hud_weapons`], guest pixels). The panel as the rank box's (black at the vertex
/// alpha over the corner gradient; the screen-copy pass not replayed), the icons clipped to the panel's scissor.
fn weapon_panel(
    draw: &mut HudDraw,
    panel: &crate::hud_weapons::WeaponPanelDraw,
    space: &HudSpace,
) -> Result<(), LayoutError> {
    use crate::hud_weapons::{clip_rect, faded_pieces, texture_name, units_uv};
    let sprite = |draw: &mut HudDraw,
                  texture: &'static str,
                  rect: [f32; 4],
                  uv: [f32; 4],
                  color: Color,
                  blend: BlendMode|
     -> Result<(), LayoutError> {
        let [x0, y0, x1, y1] = rect;
        let at = DesignBox {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
            anchor_x: 0,
            anchor_y: -1,
        };
        draw.sprites.push(HudSprite {
            element: HudElement::Weapons,
            texture,
            rect_px: space.place(at)?,
            color,
            keep_aspect: false,
            blend,
            uv,
        });
        Ok(())
    };
    if let Some(p) = &panel.panel {
        let color = Color([0.0, 0.0, 0.0, (p.alpha_byte as f32 / 128.0).min(1.0)]);
        if color.0[3] > 0.0 {
            for piece in &p.pieces {
                let Some((at, uv)) = piece_box(piece) else {
                    continue;
                };
                sprite(
                    draw,
                    crate::hud_rank::row::PANEL_TEXTURE,
                    [at.x, at.y, at.x + at.w, at.y + at.h],
                    uv,
                    color,
                    BlendMode::Alpha,
                )?;
            }
        }
    }
    let scale = |c: Color, f: f32| Color([c.0[0], c.0[1], c.0[2], c.0[3] * f]);
    for slot in &panel.slots {
        let Some(icon) = texture_name(slot.icon) else {
            continue;
        };
        let units = [0, 0, slot.uv_units[0], slot.uv_units[1]];
        let icon_uv = units_uv(units, slot.icon_size);
        for (rect, uv, f) in faded_pieces(slot.rect, icon_uv, slot.fade) {
            if let Some((r, u)) = clip_rect(rect, uv, panel.clip) {
                sprite(
                    draw,
                    icon,
                    r,
                    u,
                    scale(word_color(slot.shadow_colour), f),
                    BlendMode::Alpha,
                )?;
            }
        }
        if let (Some(overlay), Some(size)) =
            (slot.overlay.and_then(texture_name), slot.overlay_size)
        {
            let overlay_uv = units_uv(units, size);
            for (rect, uv, f) in faded_pieces(slot.rect, overlay_uv, slot.fade) {
                if let Some((r, u)) = clip_rect(rect, uv, panel.clip) {
                    sprite(
                        draw,
                        overlay,
                        r,
                        u,
                        scale(word_color(slot.overlay_colour), f),
                        BlendMode::Additive,
                    )?;
                }
            }
            if let Some((rect, colour)) = slot.glow {
                if let Some((r, u)) = clip_rect(rect, overlay_uv, panel.clip) {
                    sprite(draw, overlay, r, u, word_color(colour), BlendMode::Additive)?;
                }
            }
        }
        if let (Some((rect, colour)), Some(texture)) = (slot.arrow, panel.arrow_texture) {
            let uv = units_uv(crate::hud_weapons::k::ARROW_UNITS, panel.arrow_size);
            if let Some((r, u)) = clip_rect(rect, uv, Some(panel.hud_clip)) {
                sprite(draw, texture, r, u, word_color(colour), BlendMode::Alpha)?;
            }
        }
    }
    Ok(())
}

/// The message queue's outlined strings ([`crate::hud_messages`], guest pixels): `0x41E30` draws the outline colour at
/// the four offsets, then the text; inline icons in the text colour.
fn message_texts(
    draw: &mut HudDraw,
    queue: &crate::hud_messages::QueueDraw,
    table: &HudLayout,
    space: &HudSpace,
) -> Result<(), LayoutError> {
    use crate::hud_prompts::k;
    use crate::model::Align;
    let base = table.ammo_text_scale;
    for text in &queue.texts {
        // The original message text has a horizontal viewer-column bound,
        // while its glyph ink is bounded by the HUD picture itself. A fixed
        // 40-design-pixel vertical scissor clipped tall runtime countdown
        // glyphs after FontTableShaper had already expanded their text row.
        let column_clip = space.place(DesignBox {
            x: text.left,
            y: 0.0,
            w: text.right - text.left,
            h: 480.0,
            anchor_x: -1,
            anchor_y: -1,
        })?;
        for icon in &text.icons {
            // Inline sprites share both authored text-column edges. Crop
            // geometry and UVs together so a partial icon is never squeezed.
            // Keep vertical bounds until their separate original contract is measured.
            let clip = [text.left, icon.rect[1], text.right, icon.rect[3]];
            let Some((rect, uv)) = crate::hud_weapons::clip_rect(icon.rect, icon.uv, Some(clip))
            else {
                continue;
            };
            let [x0, y0, x1, y1] = rect;
            let at = DesignBox {
                x: x0,
                y: y0,
                w: x1 - x0,
                h: y1 - y0,
                anchor_x: -1,
                anchor_y: -1,
            };
            let blend = if text.additive {
                BlendMode::Additive
            } else {
                BlendMode::Alpha
            };
            draw.sprites.push(HudSprite {
                element: HudElement::Messages,
                texture: icon.texture,
                rect_px: space.place(at)?,
                color: word_color(text.colour),
                keep_aspect: false,
                blend,
                uv,
            });
        }
        // FontMetrics places these runs with the same 480/448 vertical upload factor as prompts.
        let scale = [
            text.text_scale[0] * ORIGINAL_GLYPH_X / base[0],
            text.text_scale[1] * ORIGINAL_GLYPH_Y / base[1],
        ];
        let outline = word_color(text.outline);
        let passes = [
            (-k::OUTLINE_DX, 0.0, outline),
            (0.0, -k::OUTLINE_DY, outline),
            (k::OUTLINE_DX, 0.0, outline),
            (0.0, k::OUTLINE_DY, outline),
            (0.0, 0.0, word_color(text.colour)),
        ];
        for (dx, dy, color) in passes {
            if color.0[3] <= 0.0 {
                continue;
            }
            for run in &text.runs {
                let at = DesignBox {
                    x: run.x + dx,
                    y: run.y + dy,
                    w: (text.right - run.x).max(0.0),
                    h: 40.0,
                    anchor_x: -1,
                    anchor_y: -1,
                };
                draw.texts.push(HudText {
                    clip_px: Some(column_clip),
                    additive: text.additive,
                    element: HudElement::Messages,
                    bytes: run.bytes.clone(),
                    font: text.font,
                    rect_px: space.place(at)?,
                    align: Align::Start,
                    vertical_align: Align::Start,
                    color,
                    scale,
                });
            }
        }
    }
    Ok(())
}

/// The prompt panel's darkening. The original draws two black rounded rectangles (`0x2D7F90`: alphas `fade * 127`
/// and `fade * 64`, 0x80 = 1) with a screen-copy blur pass between them (texture -6, blend through the
/// destination alpha) that is not replayed. MEASURED on the legacy Story frames (presents 3400 and 3600, the panel's
/// bottom edge): the fully faded-in panel passes 0.27 to 0.36 of the background (mean 0.32). Natively the panel is
/// black at `composite(fade) / composite(1) * (1 - PROMPT_PANEL_TRANSMIT)` (INFERRED approximation, no blur).
pub const PROMPT_PANEL_TRANSMIT: f32 = 0.32;

/// MEASURED (rank box increment 4, prompt panel increment 6): at glyph scale 1 the HUD renderer's glyphs are
/// 1/1.25 the width of the original's at the same pen positions (the original measure, which the wrap and the pen
/// positions use, matches the legacy pictures); texts that replay the original's text system scale x by this.
pub const ORIGINAL_GLYPH_X: f32 = 1.25;

/// Original 480-line design glyph uploads use the text system's 1/448 height factor.
/// Captured menu, numeric and prompt glyphs independently confirm this multiplier.
pub const ORIGINAL_GLYPH_Y: f32 = 15.0 / 14.0;

/// The prompt panel ([`crate::hud_prompts`], guest pixels).
fn prompt_panel(
    draw: &mut HudDraw,
    panel: &crate::hud_prompts::PromptDraw,
    table: &HudLayout,
    space: &HudSpace,
) -> Result<(), LayoutError> {
    use crate::hud_prompts::k;
    use crate::model::Align;
    let composite = |a1: f32, a2: f32| 1.0 - (1.0 - a1) * (1.0 - a2);
    let [a1, a2] = panel.panel_alphas.map(|a| (a as f32 / 128.0).min(1.0));
    let full = composite(k::PANEL_ALPHA_1 / 128.0, k::PANEL_ALPHA_2 / 128.0);
    let alpha = (composite(a1, a2) / full * (1.0 - PROMPT_PANEL_TRANSMIT)).clamp(0.0, 1.0);
    let black = Color([0.0, 0.0, 0.0, alpha]);
    let boxed = |[x0, y0, x1, y1]: [f32; 4]| DesignBox {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
        anchor_x: -1,
        anchor_y: -1,
    };
    for piece in panel
        .panel
        .iter()
        .filter(|p| p.rect[2] > p.rect[0] && p.rect[3] > p.rect[1])
    {
        draw.sprites.push(HudSprite {
            element: HudElement::Prompts,
            texture: piece.texture,
            rect_px: space.place(boxed(piece.rect))?,
            color: black,
            keep_aspect: false,
            blend: BlendMode::Alpha,
            uv: piece.uv,
        });
    }
    for (icon, colour) in &panel.icons {
        let clip = [icon.rect[0], icon.rect[1], panel.right, icon.rect[3]];
        let Some((rect, uv)) = crate::hud_weapons::clip_rect(icon.rect, icon.uv, Some(clip)) else {
            continue;
        };
        draw.sprites.push(HudSprite {
            element: HudElement::Prompts,
            texture: icon.texture,
            rect_px: space.place(boxed(rect))?,
            color: word_color(*colour),
            keep_aspect: false,
            blend: BlendMode::Alpha,
            uv,
        });
    }
    let base = table.ammo_text_scale;
    for text in &panel.texts {
        let scale = [
            text.text_scale[0] * ORIGINAL_GLYPH_X / base[0],
            text.text_scale[1] * ORIGINAL_GLYPH_Y / base[1],
        ];
        let mut passes = Vec::with_capacity(5);
        if let Some(outline) = text.outline_alpha {
            let o = Color([0.0, 0.0, 0.0, (outline as f32 / 128.0).min(1.0)]);
            passes.extend([
                (-k::OUTLINE_DX, 0.0, o),
                (0.0, -k::OUTLINE_DY, o),
                (k::OUTLINE_DX, 0.0, o),
                (0.0, k::OUTLINE_DY, o),
            ]);
        }
        passes.push((0.0, 0.0, word_color(text.colour)));
        for (dx, dy, color) in passes {
            if color.0[3] <= 0.0 {
                continue;
            }
            for run in &text.runs {
                let at = DesignBox {
                    x: run.x + dx,
                    y: run.y + dy,
                    w: (text.right - run.x).max(0.0),
                    h: 40.0,
                    anchor_x: -1,
                    anchor_y: -1,
                };
                let clip = DesignBox {
                    x: run.x,
                    y: run.y + dy,
                    w: (text.right - run.x).max(0.0),
                    h: 40.0,
                    anchor_x: -1,
                    anchor_y: -1,
                };
                draw.texts.push(HudText {
                    clip_px: Some(space.place(clip)?),
                    additive: false,
                    element: HudElement::Prompts,
                    bytes: run.bytes.clone(),
                    font: text.font,
                    rect_px: space.place(at)?,
                    align: Align::Start,
                    vertical_align: Align::Start,
                    color,
                    scale,
                });
            }
        }
    }
    Ok(())
}

/// The radar pieces ([`crate::hud_radar`], guest pixels) as alpha-blended quads.
fn radar(
    draw: &mut HudDraw,
    pieces: &[crate::hud_radar::RadarPiece],
    space: &HudSpace,
) -> Result<(), LayoutError> {
    use crate::hud_radar::RadarPiece;
    // The original's arbitrary-aspect HUD unit-y stretches the radar at 21:9.
    // In the native window preset, use its ring as a typed geometry reference:
    // one uniform diameter for the ring, sweep, dots and rim markers together.
    let normalize_y = if space.anchor == HudAnchor::SafeArea {
        pieces.iter().find_map(|piece| match piece {
            RadarPiece::Rect {
                texture: "hudradar",
                rect: [l, t, r, b],
                ..
            } if r > l && b > t => Some((*t, (r - l) / (b - t))),
            _ => None,
        })
    } else {
        None
    };
    for piece in pieces {
        let (texture, corners, uv, color) = match piece {
            RadarPiece::Rect {
                texture,
                rect: [l, t, r, b],
                uv,
                color,
            } => (
                *texture,
                [[*l, *t], [*r, *t], [*l, *b], [*r, *b]],
                *uv,
                *color,
            ),
            RadarPiece::Quad {
                texture,
                corners,
                color,
            } => (*texture, *corners, FULL_UV, *color),
        };
        let mut corners_px = [Vec2::default(); 4];
        for (out, [x, y]) in corners_px.iter_mut().zip(corners) {
            let y = normalize_y.map_or(y, |(top, ratio)| top + (y - top) * ratio);
            *out = space
                .place(DesignBox {
                    x,
                    y,
                    w: 0.0,
                    h: 0.0,
                    anchor_x: 1,
                    anchor_y: -1,
                })?
                .origin;
        }
        draw.polys.push(HudPoly {
            element: HudElement::Radar,
            texture,
            corners_px,
            color: word_color(color),
            uv,
            blend: BlendMode::Alpha,
        });
    }
    Ok(())
}

/// Colour of a HUD colour word (R, G, B, A high to low; 0x80 = 1.0).
pub fn word_color(word: u32) -> Color {
    let b = word.to_be_bytes();
    arc_color([b[0] as i32, b[1] as i32, b[2] as i32, b[3] as i32])
}

/// One original piece rectangle (x0 > x1 or y0 > y1 mirrors) as a sprite box and texture coordinates.
fn piece_box(piece: &crate::hud_rank::RowPiece) -> Option<(DesignBox, [f32; 4])> {
    let [x0, y0, x1, y1] = piece.rect;
    let [u0, v0, u1, v1] = piece.uv;
    let (left, right, u) = if x0 <= x1 {
        (x0, x1, [u0, u1])
    } else {
        (x1, x0, [u1, u0])
    };
    let (top, bottom, v) = if y0 <= y1 {
        (y0, y1, [v0, v1])
    } else {
        (y1, y0, [v1, v0])
    };
    (right > left && bottom > top).then_some((
        DesignBox {
            x: left,
            y: top,
            w: right - left,
            h: bottom - top,
            anchor_x: -1,
            anchor_y: -1,
        },
        [u[0], v[0], u[1], v[1]],
    ))
}

/// The rank box ([`crate::hud_rank::labelled_row`]): panel, frame pieces, then each string as four
/// black outline passes (offsets of `0x41E30`) and the text.
fn rank_box(
    draw: &mut HudDraw,
    row: &crate::hud_rank::LabelledRowDraw,
    table: &HudLayout,
    space: &HudSpace,
) -> Result<(), LayoutError> {
    use crate::hud_rank::row as k;
    use crate::model::Align;
    // The panel's vertex colour is the alpha byte alone (black, `0x12CC70` pushes `trunc(alpha * 72)`), drawn with blend
    // 0xE (ONE / ZERO) and colour preset 3, then a screen-copy pass through the destination alpha (blend 6) that is
    // not replayed. Natively: black at that alpha over the gradient texture (INFERRED approximation of the darkening).
    let panel_color = Color([0.0, 0.0, 0.0, (row.panel_alpha as f32 / 128.0).min(1.0)]);
    for piece in &row.panel {
        let Some((at, uv)) = piece_box(piece) else {
            continue;
        };
        draw.sprites.push(HudSprite {
            element: HudElement::Rank,
            texture: k::PANEL_TEXTURE,
            rect_px: space.place(at)?,
            color: panel_color,
            keep_aspect: false,
            blend: BlendMode::Alpha,
            uv,
        });
    }
    let frame_color = word_color(row.frame_colour);
    for piece in &row.frame {
        let Some((at, uv)) = piece_box(piece) else {
            continue;
        };
        draw.sprites.push(HudSprite {
            element: HudElement::Rank,
            texture: k::FRAME_TEXTURE,
            rect_px: space.place(at)?,
            color: frame_color,
            keep_aspect: false,
            blend: BlendMode::Alpha,
            uv,
        });
    }
    // Preserve the original label fit and pens, including labels above the
    // frame, but constrain horizontal ink to the typed row geometry.
    let row_edges = row
        .panel
        .iter()
        .chain(&row.frame)
        .filter_map(piece_box)
        .fold(None::<(f32, f32)>, |edges, (at, _)| {
            Some(match edges {
                Some((left, right)) => (left.min(at.x), right.max(at.x + at.w)),
                None => (at.x, at.x + at.w),
            })
        });
    let base = table.ammo_text_scale;
    for text in &row.texts {
        // Row pens already include the original font line-top conversion. Glyph
        // height independently uses the shared 480/448 text raster factor.
        let scale = [text.scale_x / base[0], ORIGINAL_GLYPH_Y / base[1]];
        let outline = Color([0.0, 0.0, 0.0, (text.outline_alpha as f32 / 128.0).min(1.0)]);
        let passes = [
            (-k::OUTLINE_DX, 0.0, outline),
            (0.0, -k::OUTLINE_DY, outline),
            (k::OUTLINE_DX, 0.0, outline),
            (0.0, k::OUTLINE_DY, outline),
            (0.0, 0.0, word_color(text.colour)),
        ];
        for (dx, dy, color) in passes {
            if color.0[3] <= 0.0 {
                continue;
            }
            let at = DesignBox {
                x: text.x + dx,
                y: text.y + dy,
                w: 200.0,
                h: 40.0,
                anchor_x: -1,
                anchor_y: -1,
            };
            let clip_px = row_edges
                .map(|(left, right)| {
                    space.place(DesignBox {
                        x: left,
                        y: text.y + dy,
                        w: right - left,
                        h: 40.0,
                        anchor_x: -1,
                        anchor_y: -1,
                    })
                })
                .transpose()?;
            draw.texts.push(HudText {
                clip_px,
                additive: false,
                element: HudElement::Rank,
                bytes: text.bytes.clone(),
                font: row.font,
                rect_px: space.place(at)?,
                align: Align::Start,
                vertical_align: Align::Start,
                color,
                scale,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Tier 0, FABRICATED guest values (real offsets, made-up numbers).
    use super::*;
    use std::collections::HashMap;

    struct Guest(HashMap<u32, u32>);
    impl GuestRead for Guest {
        fn read32(&self, va: u32) -> Option<u32> {
            self.0.get(&va).copied()
        }
    }
    /// Predicates answer from fabricated words at `0xF000_0000 + va`.
    impl GuestCall for Guest {
        fn call(&self, va: u32, _args: &[u32]) -> Option<u32> {
            self.0.get(&(0xf000_0000 + va)).copied()
        }
    }

    #[test]
    fn message_owner_trace_uses_the_guest_abi_stride_and_typed_fields() {
        let mut state = HudState::default();
        let mut fresh_words = [u32::MAX; HUD_MESSAGE_TRACE_WORDS];
        assert!(state.encode_message_trace(0, &mut fresh_words));
        assert!(fresh_words.iter().all(|word| *word == 0));

        state.messages[2].push(
            crate::messages::Message {
                kind: 2,
                text: b"hud".to_vec(),
                age: 1.25,
                fade_start: 2.0,
                end: 4.0,
                evict_after: 3.0,
                weight: 7,
                opaque: 0x1234,
            },
            5.0,
            Some(9),
            None,
        );
        state.messages[2].progress[0] = 6;
        state.input_progress[2] = 3;

        let mut words = [u32::MAX; HUD_MESSAGE_TRACE_WORDS];
        assert!(state.encode_message_trace(2, &mut words));
        assert_eq!(words[0], 1); // cursor
        assert_eq!(words[2], 2); // slot zero kind
        assert_eq!(words[3], u32::from_le_bytes(*b"hud\0"));
        assert_eq!(words[2 + 0x144 / 4], 1.25f32.to_bits());
        assert_eq!(words[2 + 0x14c / 4], 5.0f32.to_bits());
        assert_eq!(words[2 + 0x15c / 4], 9);
        assert_eq!(words[2 + 0x160 / 4], u32::MAX);
        assert_eq!(words[2 + 0x164 / 4], 6);
        assert_eq!(words[2 + 0x168 / 4], 0x1234);
        assert_eq!(words[HUD_MESSAGE_TRACE_BYTES / 4 - 1], 3);
        assert!(words[HUD_MESSAGE_TRACE_WORDS - 2] == 0);
        assert!(!state.encode_message_trace(4, &mut words));
    }

    #[test]
    fn message_owner_imports_the_drained_guest_snapshot_once() {
        use crate::hud_producers::MessageQueueSnapshot;
        use crate::messages::Message;

        let mut entries: [Message; 16] = core::array::from_fn(|_| Message::default());
        entries[3] = Message {
            kind: 0,
            text: b"retired".to_vec(),
            age: 1.5,
            fade_start: 2.0,
            end: 4.0,
            evict_after: 3.0,
            weight: 2,
            opaque: 0x1234,
        };
        let snapshot = MessageQueueSnapshot {
            cursor: 7,
            entries,
            slides: [0.25; 16],
            cues: [0; 16],
            playing_handles: [-1; 16],
            progress: [9; 16],
            input_progress: 2,
        };
        let mut state = HudState::default();
        assert!(state.import_message_queue(0, snapshot));
        assert!(state.message_queue_imported(0));

        let mut words = [u32::MAX; HUD_MESSAGE_TRACE_WORDS];
        assert!(state.encode_message_trace(0, &mut words));
        let base = 2 + 3 * (0x16c / 4);
        assert_eq!(words[0], 7);
        assert_eq!(words[base], 0);
        assert_eq!(words[base + 1], u32::from_le_bytes(*b"reti"));
        assert_eq!(words[base + 0x14c / 4], 0.25f32.to_bits());
        assert_eq!(words[base + 0x15c / 4], 0);
        assert_eq!(words[base + 0x160 / 4], u32::MAX);
        assert_eq!(words[base + 0x164 / 4], 9);
        assert_eq!(words[HUD_MESSAGE_TRACE_BYTES / 4 - 1], 2);

        let replacement = MessageQueueSnapshot {
            cursor: 12,
            entries: core::array::from_fn(|_| Message::default()),
            slides: [0.0; 16],
            cues: [0; 16],
            playing_handles: [0; 16],
            progress: [0; 16],
            input_progress: 0,
        };
        assert!(state.import_message_queue(0, replacement));
        assert_eq!(state.messages[0].ring.cursor, 7);
        assert_eq!(state.input_progress[0], 2);
        assert!(!state.import_message_queue(
            4,
            MessageQueueSnapshot {
                cursor: 0,
                entries: core::array::from_fn(|_| Message::default()),
                slides: [0.0; 16],
                cues: [0; 16],
                playing_handles: [0; 16],
                progress: [0; 16],
                input_progress: 0,
            }
        ));
    }

    #[test]
    fn native_message_replay_owns_kind7_progress_per_player() {
        use crate::hud_producers::MessagePolicy;

        let mut state = HudState::default();
        let queue = &mut state.messages[1];
        queue.ring.cursor = 0;
        queue.push(
            crate::messages::Message {
                kind: 7,
                text: b"first".to_vec(),
                age: 0.5,
                weight: 1,
                ..Default::default()
            },
            0.0,
            None,
            None,
        );
        queue.push(
            crate::messages::Message {
                kind: 7,
                text: b"second".to_vec(),
                age: 0.5,
                weight: 1,
                ..Default::default()
            },
            0.0,
            None,
            None,
        );
        state.input_progress[1] = 1;
        assert!(state.publish_message_buttons(1, 0x08, 0x0c));
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };
        let mut draws = Vec::new();

        let sums = state
            .replay_messages_native(
                1,
                &policy,
                false,
                true,
                |message, _slide, _offset, progress| {
                    draws.push((message.text.clone(), progress));
                },
            )
            .expect("valid player has a native message queue");

        assert_eq!(draws, [(b"first".to_vec(), 2), (b"second".to_vec(), 2)]);
        assert_eq!(sums[7], 2);
        assert_eq!(state.messages[1].ring.entries[0].kind, 7);
        assert_eq!(state.messages[1].ring.entries[1].kind, 7);
        assert_eq!(state.input_progress[1], 2);
        assert_eq!(state.input_progress[0], 0);
        assert!(!state.message_action[1].take());
    }

    #[test]
    fn native_message_replay_rejects_invalid_player_before_input() {
        use crate::hud_producers::MessagePolicy;

        let mut state = HudState::default();
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };
        assert!(!state.publish_message_buttons(-1, 0x08, 0x0c));
        assert!(!state.publish_message_buttons(4, 0x08, 0x0c));
        assert!(state.publish_message_buttons(0, 0x08, 0x04));
        assert!(
            state
                .replay_messages_native(-1, &policy, false, true, |_, _, _, _| panic!(
                    "invalid player must not draw"
                ),)
                .is_none()
        );
        assert!(
            state
                .replay_messages_native(4, &policy, false, true, |_, _, _, _| panic!(
                    "invalid player must not draw"
                ),)
                .is_none()
        );
        assert!(!state.message_action[0].take());
    }

    #[test]
    fn native_message_action_requires_the_player_eligible_pad_mask() {
        let mut state = HudState::default();
        assert!(state.publish_message_buttons(2, 0x08, 0x0c));
        assert!(state.message_action[2].take());
        assert!(!state.message_action[2].take());
        assert!(state.publish_message_buttons(2, 0x08, 0x04));
        assert!(!state.message_action[2].take());
        assert!(!state.message_action[0].take());
    }

    #[test]
    fn native_message_renderer_failure_preserves_queue_progress_and_action() {
        use crate::hud_producers::MessagePolicy;

        let mut state = HudState::default();
        state.messages[0].ring.entries[0].kind = 7;
        state.messages[0].ring.entries[0].text = b"press to continue".to_vec();
        assert!(state.publish_message_buttons(0, 0x08, 0x08));
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };

        let result = state.try_replay_messages_native(0, &policy, false, true, |_, _, _, _| {
            Err("notice renderer unavailable")
        });

        assert_eq!(result, Err("notice renderer unavailable"));
        assert_eq!(state.messages[0].ring.entries[0].kind, 7);
        assert_eq!(state.input_progress[0], 0);
        assert!(state.message_action[0].take());
        assert!(!state.message_action[0].take());
    }

    fn gameplay_guest() -> Guest {
        use addr::*;
        let (record, object, character) = (0x0100_0000, 0x0200_0000, 0x0300_0000);
        let weapon = 14;
        let wrec = WEAPON_RECORDS + weapon * WEAPON_RECORD_SIZE;
        let mut m = HashMap::new();
        m.insert(VIEWER_RECORD_PTR, record);
        m.insert(record + RECORD_SLOT, 0);
        m.insert(reticle_addr::PLAYER_SLOTS + 0x174c, 0);
        m.insert(record + RECORD_OBJECT, object);
        m.insert(object + OBJ_HEALTH, 30.0f32.to_bits());
        m.insert(object + OBJ_MAX_HEALTH, 35.0f32.to_bits());
        m.insert(object + OBJ_CHARACTER, character);
        m.insert(object + OBJ_CLASS, CLASS_CHARACTER);
        m.insert(character + CHAR_ARMOR, 5.0f32.to_bits());
        m.insert(character + CHAR_ARMOR_MAX, 50.0f32.to_bits());
        m.insert(character + EXT_HAND_B + HAND_RECORD_INDEX, weapon);
        m.insert(wrec + WEAPON_AMMO_TYPE, 18);
        m.insert(wrec + WEAPON_CLIP_SIZE, 16);
        m.insert(wrec + 0x1c, 0);
        m.insert(0x004e_a404 + 18 * 0xe0, 10);
        // loaded (u16) at hand +0x38 + 36 = +0x5C: aligned word, low half.
        m.insert(character + EXT_HAND_B + HAND_LOADED_BY_TYPE + 36, 14);
        // reserve (u16) at +0x61E + 36 = +0x642: high half of the word at +0x640.
        m.insert(character + EXT_RESERVE_BY_TYPE + 36 - 2, 112 << 16);
        m.insert(LETTERBOX_AMOUNT, 0);
        m.insert(LETTERBOX_NONE, 0);
        // A full-screen viewport (x0, x1, y0, y1), as one player plays.
        let viewport = 0x0400_0000;
        m.insert(VIEWPORT_PTR, viewport);
        m.insert(viewport + 4, 640);
        m.insert(viewport + 0x14, 0);
        m.insert(viewport + 0x18, 480);
        Guest(m)
    }

    /// T3787 (I-T3787-1): in a two player split the top player's ammo rows sit at the bottom of the top viewport,
    /// not of the screen (where they covered player 2's HUD).
    #[test]
    fn split_screen_ammo_rows_anchor_to_their_viewport() {
        use addr::*;
        let character = 0x0300_0000;
        let hand_b = character + EXT_HAND_B;
        let mut top = gameplay_guest();
        top.0.insert(0x0400_0000 + 0x18, 240);
        let AmmoCall::Draw(top_row) = AmmoRow::read_owned(&top, [character, 0, 0, hand_b, 1], 0)
        else {
            panic!("top row")
        };
        let AmmoCall::Draw(full_row) =
            AmmoRow::read_owned(&gameplay_guest(), [character, 0, 0, hand_b, 1], 0)
        else {
            panic!("full-screen row")
        };
        assert_eq!(
            (top_row.viewport_bottom, full_row.viewport_bottom),
            (240, 480)
        );
        let mut no_viewport = gameplay_guest();
        no_viewport.0.remove(&VIEWPORT_PTR);
        assert_eq!(
            AmmoRow::read_owned(&no_viewport, [character, 0, 0, hand_b, 1], 0),
            AmmoCall::Legacy
        );
        let table = HudLayout::original();
        for (w, h) in [(640.0, 480.0), (1920.0, 1080.0)] {
            let space = HudSpace::for_window(w, h, HudAnchor::Picture, 0.05);
            let (mut upper, mut lower) = (HudDraw::default(), HudDraw::default());
            ammo_row(&mut upper, &top_row, &table, &space).unwrap();
            ammo_row(&mut lower, &full_row, &table, &space).unwrap();
            assert!(!lower.sprites.is_empty() && lower.sprites.len() == upper.sprites.len());
            let scale = space
                .place(DesignBox {
                    x: 0.0,
                    y: 0.0,
                    w: 1.0,
                    h: 1.0,
                    anchor_x: 0,
                    anchor_y: 0,
                })
                .unwrap()
                .size
                .y;
            for (a, b) in upper.sprites.iter().zip(&lower.sprites) {
                assert!((b.rect_px.origin.y - a.rect_px.origin.y - 240.0 * scale).abs() < 0.01);
            }
            for (a, b) in upper.texts.iter().zip(&lower.texts) {
                assert!((b.rect_px.origin.y - a.rect_px.origin.y - 240.0 * scale).abs() < 0.01);
            }
        }
    }

    #[test]
    fn gates_table_matches_the_elements() {
        let json = include_str!("../../../tools/data/hud_native_gates.json");
        let mut from_json: Vec<(u32, String)> = Vec::new();
        for line in json.lines().filter(|l| l.contains("\"va\"")) {
            let va = line.split("\"va\": \"0x").nth(1).unwrap()[..8].to_owned();
            let element = line.split("\"element\": \"").nth(1).unwrap();
            let element = element[..element.find('"').unwrap()].to_owned();
            from_json.push((u32::from_str_radix(&va, 16).unwrap(), element));
        }
        let from_rust: Vec<(u32, String)> = HudElement::ALL
            .iter()
            .flat_map(|e| e.drawers().iter().map(|va| (*va, e.name().to_owned())))
            .collect();
        assert!(!from_json.is_empty());
        let (mut a, mut b) = (from_json, from_rust);
        a.sort();
        b.sort();
        assert_eq!(a, b);
    }

    #[test]
    fn ownership_parses_and_gates_per_element() {
        assert_eq!(HudOwnership::parse("off").unwrap(), HudOwnership::NONE);
        let core = HudOwnership::parse("core").unwrap();
        assert!(core.owns_drawer(0x1334d0));
        assert!(!core.owns_drawer(0x14f0b0));
        assert!(core.owns_drawer(0x14c970), "the reticle is core");
        assert!(!core.owns_drawer(0x12345));
        let list = HudOwnership::parse("crosshair, ammo").unwrap();
        assert!(list.owns_drawer(0x14c970) && list.owns_drawer(0x1334d0));
        assert!(
            !list.owns_drawer(0x135f30),
            "reticle extras are a separate element"
        );
        assert!(!list.owns(HudElement::Bar));
        let all = HudOwnership::parse("all").unwrap();
        assert!(
            !all.owns(HudElement::Whole) && !all.owns(HudElement::Status),
            "all = native implementations only"
        );
        assert!(all.owns(HudElement::Ammo) && all.owns(HudElement::Crosshair));
        assert!(
            HudOwnership::parse("status")
                .unwrap()
                .owns(HudElement::Status),
            "suppress-only by name"
        );
        assert!(HudOwnership::parse("ammo,nope").is_err());
    }

    #[test]
    fn reads_the_viewer_state() {
        let frame = HudFrame::read(&gameplay_guest()).unwrap();
        assert_eq!((frame.health, frame.max_health), (30.0, 35.0));
        assert_eq!((frame.armour, frame.armour_max), (5.0, 50.0));
        assert_eq!(frame.weapon_record, Some(14));
        assert_eq!((frame.ammo_type, frame.clip_size), (18, 16));
        assert_eq!((frame.ammo_loaded, frame.ammo_reserve), (14, 112));
        assert!(frame.hud_visible());
    }

    #[test]
    fn no_viewer_no_frame() {
        let mut guest = gameplay_guest();
        guest.0.insert(addr::VIEWER_RECORD_PTR, 0);
        assert_eq!(HudFrame::read(&guest), None);
        let mut guest = gameplay_guest();
        guest.0.insert(0x0100_0000 + addr::RECORD_SLOT, u32::MAX);
        assert_eq!(HudFrame::read(&guest), None);
    }

    fn request(element: HudElement) -> DrawRequest {
        DrawRequest {
            element,
            drawer: element.drawers()[0],
            args: [0; 5],
            reticle: None,
            ammo: None,
            vitals: None,
            hit_sector: None,
            rank: None,
            radar: None,
            prompts: None,
            effects: None,
            messages: None,
            weapons: None,
        }
    }

    #[test]
    fn prompt_text_uses_the_remaining_original_column_at_both_aspects() {
        use crate::hud_prompts::{PromptDraw, PromptText, TextRun};
        let panel = PromptDraw {
            right: 300.0,
            panel: vec![],
            panel_alphas: [0; 2],
            icons: vec![],
            last_font: None,
            height: 40,
            texts: vec![PromptText {
                right: 300.0,
                runs: vec![TextRun {
                    bytes: b"long label".to_vec(),
                    x: 140.0,
                    y: 120.0,
                }],
                font: 5,
                colour: 0x7f7f7f7f,
                outline_alpha: Some(64),
                text_scale: [1.0; 2],
            }],
        };
        for (width, height) in [(1280.0, 960.0), (1280.0, 720.0)] {
            let space = HudSpace::for_window(width, height, HudAnchor::Picture, 0.0);
            let mut draw = HudDraw::default();
            prompt_panel(&mut draw, &panel, &HudLayout::original(), &space).unwrap();
            assert_eq!(
                draw.texts.len(),
                5,
                "outline passes retain the same column width"
            );
            for text in &draw.texts {
                let clip = text.clip_px.unwrap();
                assert!(
                    (clip.origin.x + clip.size.x
                        - space
                            .place(DesignBox {
                                x: 300.0,
                                y: 0.0,
                                w: 0.0,
                                h: 0.0,
                                anchor_x: -1,
                                anchor_y: -1
                            })
                            .unwrap()
                            .origin
                            .x)
                        .abs()
                        < 0.001
                );
            }
        }
    }

    #[test]
    fn prompt_inline_icons_share_the_column_without_using_stale_panel_height() {
        use crate::hud_prompts::{PlacedIcon, PromptDraw};
        let panel = PromptDraw {
            right: 300.0,
            panel: vec![],
            panel_alphas: [0; 2],
            texts: vec![],
            last_font: None,
            height: 0,
            icons: [280.0, 290.0, 300.0, 310.0]
                .into_iter()
                .map(|x| {
                    (
                        PlacedIcon {
                            texture: "test-icon",
                            rect: [x, 120.0, x + 20.0, 140.0],
                            uv: [0.25, 0.125, 0.75, 0.875],
                        },
                        0x7f7f7f7f,
                    )
                })
                .collect(),
        };
        for (width, height) in [(1280.0, 960.0), (1280.0, 720.0)] {
            let space = HudSpace::for_window(width, height, HudAnchor::Picture, 0.0);
            let mut draw = HudDraw::default();
            prompt_panel(&mut draw, &panel, &HudLayout::original(), &space).unwrap();
            assert_eq!(draw.sprites.len(), 2);
            assert_eq!(draw.sprites[0].uv, [0.25, 0.125, 0.75, 0.875]);
            assert_eq!(draw.sprites[1].uv, [0.25, 0.125, 0.5, 0.875]);
            for (sprite, design_width) in draw.sprites.iter().zip([20.0, 10.0]) {
                assert!((sprite.rect_px.size.x - design_width * space.scale()).abs() < 0.001);
                assert!((sprite.rect_px.size.y - 20.0 * space.scale()).abs() < 0.001);
            }
        }
    }

    #[test]
    fn centered_message_keeps_its_pen_but_clips_both_column_edges() {
        use crate::hud_messages::{MessageText, QueueDraw};
        use crate::hud_prompts::{PlacedIcon, TextRun};
        let queue = QueueDraw {
            texts: vec![MessageText {
                left: 100.0,
                right: 300.0,
                runs: vec![TextRun {
                    bytes: b"oversized centered name".to_vec(),
                    x: 20.0,
                    y: 120.0,
                }],
                icons: [80.0, 90.0, 100.0]
                    .into_iter()
                    .map(|x| PlacedIcon {
                        texture: "test-icon",
                        rect: [x, 120.0, x + 20.0, 140.0],
                        uv: [0.25, 0.125, 0.75, 0.875],
                    })
                    .collect(),
                font: 5,
                colour: 0x7f7f7f7f,
                outline: 0x7f7f7f7f,
                text_scale: [1.0; 2],
                additive: false,
            }],
            ..QueueDraw::default()
        };
        for (width, height) in [(1280.0, 960.0), (1280.0, 720.0)] {
            let space = HudSpace::for_window(width, height, HudAnchor::Picture, 0.0);
            let mut draw = HudDraw::default();
            message_texts(&mut draw, &queue, &HudLayout::original(), &space).unwrap();
            assert_eq!(draw.texts.len(), 5);
            for pass in &draw.texts {
                assert!(
                    (pass.rect_px.size.x - 280.0 * space.scale()).abs() < 0.001,
                    "all outline passes retain identical wrap width"
                );
                let clip = pass.clip_px.unwrap();
                assert!((clip.size.x - 200.0 * space.scale()).abs() < 0.001);
                assert!(
                    pass.rect_px.origin.x < clip.origin.x,
                    "centered pen is preserved"
                );
            }
            assert_eq!(draw.sprites.len(), 2);
            assert_eq!(draw.sprites[0].uv, [0.5, 0.125, 0.75, 0.875]);
            assert_eq!(draw.sprites[1].uv, [0.25, 0.125, 0.75, 0.875]);
            assert!((draw.sprites[0].rect_px.size.x - 10.0 * space.scale()).abs() < 0.001);
        }
    }

    #[test]
    fn message_runs_and_outlines_stay_in_the_viewer_column_at_both_aspects() {
        use crate::hud_messages::{MessageText, QueueDraw};
        use crate::hud_prompts::TextRun;
        let queue = QueueDraw {
            texts: vec![MessageText {
                left: 100.0,
                right: 300.0,
                runs: vec![
                    TextRun {
                        bytes: b"label".to_vec(),
                        x: 140.0,
                        y: 120.0,
                    },
                    TextRun {
                        bytes: b"outside".to_vec(),
                        x: 310.0,
                        y: 120.0,
                    },
                ],
                icons: vec![],
                font: 5,
                colour: 0x7f7f7f7f,
                outline: 0x7f7f7f7f,
                text_scale: [1.0; 2],
                additive: false,
            }],
            ..QueueDraw::default()
        };
        for (width, height) in [(1280.0, 960.0), (1280.0, 720.0)] {
            let space = HudSpace::for_window(width, height, HudAnchor::Picture, 0.0);
            let mut draw = HudDraw::default();
            message_texts(&mut draw, &queue, &HudLayout::original(), &space).unwrap();
            assert_eq!(draw.texts.len(), 10);
            for pass in draw.texts.chunks_exact(2) {
                let clip = pass[0].clip_px.unwrap();
                assert!((clip.origin.y - space.picture.origin.y).abs() < 0.001);
                assert!((clip.size.y - space.picture.size.y).abs() < 0.001);
                assert!(
                    (clip.origin.x + clip.size.x
                        - space
                            .place(DesignBox {
                                x: 300.0,
                                y: 0.0,
                                w: 0.0,
                                h: 0.0,
                                anchor_x: -1,
                                anchor_y: -1
                            })
                            .unwrap()
                            .origin
                            .x)
                        .abs()
                        < 0.001
                );
                assert_eq!(pass[1].rect_px.size.x, 0.0);
            }
        }
    }

    #[test]
    fn message_inline_icons_crop_at_column_edge_without_stretching_uvs() {
        use crate::hud_messages::{MessageText, QueueDraw};
        use crate::hud_prompts::PlacedIcon;
        for additive in [false, true] {
            let queue = QueueDraw {
                texts: vec![MessageText {
                    left: 100.0,
                    right: 300.0,
                    runs: vec![],
                    icons: [280.0, 290.0, 300.0, 310.0]
                        .into_iter()
                        .map(|x| PlacedIcon {
                            texture: "test-icon",
                            rect: [x, 120.0, x + 20.0, 140.0],
                            uv: [0.25, 0.125, 0.75, 0.875],
                        })
                        .collect(),
                    font: 5,
                    colour: 0x7f7f7f7f,
                    outline: 0,
                    text_scale: [1.0; 2],
                    additive,
                }],
                ..QueueDraw::default()
            };
            for (width, height) in [(1280.0, 960.0), (1280.0, 720.0)] {
                let space = HudSpace::for_window(width, height, HudAnchor::Picture, 0.0);
                let mut draw = HudDraw::default();
                message_texts(&mut draw, &queue, &HudLayout::original(), &space).unwrap();
                assert_eq!(
                    draw.sprites.len(),
                    2,
                    "icons beyond the column must not paint"
                );
                assert_eq!(draw.sprites[0].uv, [0.25, 0.125, 0.75, 0.875]);
                assert_eq!(draw.sprites[1].uv, [0.25, 0.125, 0.5, 0.875]);
                for (sprite, design_width) in draw.sprites.iter().zip([20.0, 10.0]) {
                    assert!((sprite.rect_px.size.x - design_width * space.scale()).abs() < 0.001);
                    assert!((sprite.rect_px.size.y - 20.0 * space.scale()).abs() < 0.001);
                    assert_eq!(
                        sprite.blend,
                        if additive {
                            BlendMode::Additive
                        } else {
                            BlendMode::Alpha
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn message_glyph_metrics_do_not_depend_on_the_ammo_layout_base() {
        use crate::hud_messages::{MessageText, QueueDraw};
        use crate::hud_prompts::TextRun;
        let queue = QueueDraw {
            texts: vec![MessageText {
                left: 100.0,
                right: 300.0,
                runs: vec![TextRun {
                    bytes: b"test".to_vec(),
                    x: 80.0,
                    y: 120.0,
                }],
                icons: vec![],
                font: 5,
                colour: 0x7f7f7f7f,
                outline: 0,
                text_scale: [0.8, 0.6],
                additive: true,
            }],
            ..QueueDraw::default()
        };
        let space = HudSpace::for_window(640.0, 480.0, HudAnchor::Picture, 0.0);
        for base in [[1.25, 15.0 / 14.0], [2.0, 3.0]] {
            let table = HudLayout {
                ammo_text_scale: base,
                ..HudLayout::original()
            };
            let mut draw = HudDraw::default();
            message_texts(&mut draw, &queue, &table, &space).unwrap();
            let text = &draw.texts[0];
            assert!((text.scale[0] * base[0] - 1.0).abs() < 1e-6);
            assert!((text.scale[1] * base[1] - 9.0 / 14.0).abs() < 1e-6);
            assert_eq!(
                (text.rect_px.origin.x, text.rect_px.origin.y),
                (80.0, 120.0)
            );
            assert!(text.additive);
        }
    }

    #[test]
    fn draws_only_owned_and_requested_elements() {
        let frame = HudFrame::read(&gameplay_guest()).unwrap();
        let table = HudLayout::original();
        let space = HudSpace::for_window(640.0, 480.0, HudAnchor::Picture, 0.05);
        let owned = HudOwnership::of(&[HudElement::Ammo]);
        let none = layout(&frame, &[], owned, &table, &space).unwrap();
        assert!(none.is_empty(), "not requested by the original this frame");
        let row = DrawRequest {
            ammo: right_row(&gameplay_guest()),
            ..request(HudElement::Ammo)
        };
        let draw = layout(&frame, &[row.clone()], owned, &table, &space).unwrap();
        let texts: Vec<&[u8]> = draw.texts.iter().map(|t| t.bytes.as_slice()).collect();
        assert_eq!(texts, [&b"112"[..], &b"14"[..]]);
        assert_eq!(draw.sprites.len(), 4, "three frame pieces and the icon");
        assert!(
            draw.sprites[0].uv[0] > draw.sprites[0].uv[2],
            "left cap mirrored"
        );
        let legacy = layout(&frame, &[row.clone()], HudOwnership::NONE, &table, &space).unwrap();
        assert!(legacy.is_empty(), "legacy keeps the element");
        let mut cut = frame;
        cut.letterbox = 0.5;
        assert!(
            layout(&cut, &[row], owned, &table, &space)
                .unwrap()
                .is_empty()
        );
    }

    /// The right-hand row the drawer call `(character, 0, 0, hand B, right)` carries.
    fn right_row(guest: &Guest) -> Option<AmmoRow> {
        let character = 0x0300_0000;
        match AmmoRow::read(guest, [character, 0, 0, character + addr::EXT_HAND_B, 1]) {
            AmmoCall::Draw(row) => Some(row),
            _ => None,
        }
    }

    /// T3816: in the credits the viewer holds weapon record 37 with ammo type 0. The original ammo drawer returns
    /// without drawing (`AmmoCall::Nothing`), so the native HUD must draw no ammo row, whatever the frame holds.
    #[test]
    fn an_ammo_call_that_draws_nothing_draws_no_row_in_the_credits() {
        use addr::*;
        let table = HudLayout::original();
        let space = HudSpace::for_window(640.0, 480.0, HudAnchor::Picture, 0.05);
        let owned = HudOwnership::of(&HudElement::CORE);
        let mut credits = gameplay_guest();
        let character = 0x0300_0000;
        let hand_b = character + EXT_HAND_B;
        credits.0.insert(hand_b + HAND_RECORD_INDEX, 37);
        let record = WEAPON_RECORDS + 37 * WEAPON_RECORD_SIZE;
        credits.0.insert(record + WEAPON_AMMO_TYPE, 0);
        credits.0.insert(record + 0x1c, 0);
        let frame = HudFrame::read(&credits).unwrap();
        assert_eq!((frame.weapon_record, frame.ammo_type), (Some(37), 0));
        assert!(frame.hud_visible(), "the credits frame has no letterbox");
        assert_eq!(
            AmmoRow::read(&credits, [character, 0, 0, hand_b, 1]),
            AmmoCall::Nothing
        );
        assert_eq!(right_row(&credits), None);
        let draw = layout(&frame, &[request(HudElement::Ammo)], owned, &table, &space).unwrap();
        assert!(draw.is_empty(), "no ammo row over the credits: {draw:?}");
        // A drawable weapon in the frame still draws nothing for a call that drew nothing.
        let armed = HudFrame::read(&gameplay_guest()).unwrap();
        assert!(right_row(&gameplay_guest()).is_some());
        let draw = layout(&armed, &[request(HudElement::Ammo)], owned, &table, &space).unwrap();
        assert!(
            draw.is_empty(),
            "the frame is not a substitute for the call: {draw:?}"
        );
    }

    /// Guard (class `hud-drawn-without-original-request`, T3816): every element draws only what the original's
    /// drawer call carries. A request without its payload (the drawer's own early-out), with a fully armed viewer
    /// in the frame, draws nothing, so no element can show on the front-end, the credits or a cutscene just because
    /// a player record exists.
    #[test]
    fn no_element_draws_from_the_frame_without_its_call_payload() {
        let table = HudLayout::original();
        let frame = HudFrame::read(&gameplay_guest()).unwrap();
        assert!(frame.weapon_record.is_some() && frame.health > 0.0);
        let all = HudOwnership::of(&HudElement::ALL);
        for anchor in [HudAnchor::Picture, HudAnchor::SafeArea] {
            let space = HudSpace::for_window(1280.0, 720.0, anchor, 0.05);
            for element in HudElement::ALL {
                let draw = layout(&frame, &[request(element)], all, &table, &space).unwrap();
                assert!(
                    draw.is_empty(),
                    "{element:?} drew from frame state: {draw:?}"
                );
            }
            let every: Vec<DrawRequest> = HudElement::ALL.iter().map(|&e| request(e)).collect();
            assert!(
                layout(&frame, &every, all, &table, &space)
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn dual_wield_rows_from_call_arguments() {
        use addr::*;
        let guest = gameplay_guest();
        let character = 0x0300_0000;
        let hand_b = character + EXT_HAND_B;
        let right = AmmoRow::read(&guest, [character, 0, 0, hand_b, 1]);
        let AmmoCall::Draw(row) = right else {
            panic!("right row")
        };
        assert_eq!(
            (row.side, row.loaded, row.reserve),
            (AmmoSide::Right, 14, 112)
        );
        // Native allocation remains authoritative with no readable guest cursor.
        let mut without_cursor = gameplay_guest();
        without_cursor
            .0
            .remove(&(reticle_addr::PLAYER_SLOTS + 0x174c));
        let AmmoCall::Draw(native) =
            AmmoRow::read_owned(&without_cursor, [character, 0, 0, hand_b, 1], 7)
        else {
            panic!("owned row must not read the guest cursor")
        };
        assert_eq!(
            (native.stack_index, native.loaded, native.reserve),
            (7, 14, 112)
        );

        assert_eq!(
            AmmoRow::read(&guest, [character, 0, 0, hand_b, 2]),
            AmmoCall::Legacy,
            "side 2"
        );
        assert_eq!(
            AmmoRow::read(&guest, [0, 0x0200_0000, 0, hand_b, 1]),
            AmmoCall::Legacy,
            "vehicle site"
        );
        let mut empty = gameplay_guest();
        empty.0.insert(hand_b + HAND_RECORD_INDEX, u32::MAX);
        assert_eq!(
            AmmoRow::read(&empty, [character, 0, 0, hand_b, 0]),
            AmmoCall::Nothing
        );
        let frame = HudFrame::read(&guest).unwrap();
        let table = HudLayout::original();
        let space = HudSpace::for_window(640.0, 480.0, HudAnchor::Picture, 0.05);
        let left_row = AmmoRow {
            side: AmmoSide::Left,
            ..row
        };
        let requests = [
            DrawRequest {
                ammo: Some(left_row),
                ..request(HudElement::Ammo)
            },
            DrawRequest {
                ammo: Some(row),
                ..request(HudElement::Ammo)
            },
        ];
        let draw = layout(
            &frame,
            &requests,
            HudOwnership::of(&[HudElement::Ammo]),
            &table,
            &space,
        )
        .unwrap();
        assert_eq!((draw.texts.len(), draw.sprites.len()), (4, 8));
        let (l, r) = (&draw.texts[0], &draw.texts[2]);
        assert_eq!(
            l.rect_px.origin.x,
            640.0 - r.rect_px.origin.x - r.rect_px.size.x,
            "mirrored reserve"
        );
        assert_eq!(
            (l.align, r.align),
            (crate::model::Align::Start, crate::model::Align::End)
        );
        assert!(
            draw.sprites[0].uv[0] < draw.sprites[0].uv[2],
            "left row's left cap unmirrored"
        );
    }

    #[test]
    fn unreconstructed_ammo_elements_remain_in_the_original_drawer() {
        let mut guest = gameplay_guest();
        let args = [0x0300_0000, 0, 0, 0x0300_0000 + addr::EXT_HAND_B, 0];
        let weapon = addr::WEAPON_RECORDS + 14 * addr::WEAPON_RECORD_SIZE;
        guest.0.insert(weapon + addr::WEAPON_AMMO_TYPE, 0);
        assert_eq!(
            AmmoRow::read(&guest, args),
            AmmoCall::Nothing,
            "unarmed has no row or cursor advance"
        );
        guest.0.insert(weapon + 0x1c, 5);
        assert_eq!(
            AmmoRow::read(&guest, args),
            AmmoCall::Legacy,
            "secondary fire must not be dropped"
        );
        guest.0.insert(weapon + 0x1c, 0);
        guest.0.insert(weapon + addr::WEAPON_AMMO_TYPE, 18);
        guest.0.insert(0x004e_a404 + 18 * 0xe0, 4);
        assert_eq!(
            AmmoRow::read(&guest, args),
            AmmoCall::Legacy,
            "icon-stack ammo is not a numeric bullet row"
        );
    }

    #[test]
    fn secondary_row_reserves_space_for_dual_wield() {
        let mut guest = gameplay_guest();
        // A legacy grenade drawer has consumed the first left row, right is untouched.
        guest
            .0
            .insert(reticle_addr::PLAYER_SLOTS + 0x174c, 0xabcd_0001);
        let left = AmmoStackCursor::read(&guest, AmmoSide::Left).unwrap();
        let right = AmmoStackCursor::read(&guest, AmmoSide::Right).unwrap();
        assert_eq!((left.index, right.index), (1, 0));
        assert_eq!(left.advanced_word(), 0xabcd_0002);
        assert_eq!(right.advanced_word(), 0xabcd_0101);
        let AmmoCall::Draw(row) = AmmoRow::read(
            &guest,
            [0x0300_0000, 0, 0, 0x0300_0000 + addr::EXT_HAND_B, 0],
        ) else {
            panic!("left row")
        };
        assert_eq!(row.stack_index, 1);
        let table = HudLayout::original();
        for (w, h) in [(640.0, 480.0), (1280.0, 960.0), (1920.0, 1080.0)] {
            let space = HudSpace::for_window(w, h, HudAnchor::Picture, 0.05);
            let mut bottom = HudDraw::default();
            let mut stacked = HudDraw::default();
            ammo_row(
                &mut bottom,
                &AmmoRow {
                    stack_index: 0,
                    ..row
                },
                &table,
                &space,
            )
            .unwrap();
            ammo_row(&mut stacked, &row, &table, &space).unwrap();
            let scale = h / 480.0;
            for (a, b) in stacked.sprites.iter().zip(&bottom.sprites) {
                assert!((b.rect_px.origin.y - a.rect_px.origin.y - 49.0 * scale).abs() < 0.001);
                assert!(
                    a.rect_px.origin.y + a.rect_px.size.y < b.rect_px.origin.y,
                    "49-pixel pitch leaves a gap around the 45-pixel frame"
                );
            }
            for (a, b) in stacked.texts.iter().zip(&bottom.texts) {
                assert!((b.rect_px.origin.y - a.rect_px.origin.y - 49.0 * scale).abs() < 0.001);
            }
        }
        guest
            .0
            .insert(reticle_addr::PLAYER_SLOTS + 0x174c, 0xabcd_12ff);
        assert_eq!(
            AmmoStackCursor::read(&guest, AmmoSide::Left)
                .unwrap()
                .advanced_word(),
            0xabcd_1200
        );
        guest.0.insert(0x0100_0000 + addr::RECORD_SLOT, u32::MAX);
        assert!(AmmoStackCursor::read(&guest, AmmoSide::Left).is_none());
    }

    #[test]
    fn picture_and_safe_area_anchoring() {
        let table = HudLayout::original();
        let picture = HudSpace::for_window(1280.0, 720.0, HudAnchor::Picture, 0.05);
        let r = picture.place(table.ammo_loaded).unwrap();
        // 4:3 picture of 960 x 720 centred: x offset 160, scale 1.5.
        assert_eq!(
            (r.origin.x, r.origin.y),
            (160.0 + 589.5625 * 1.5, 424.5 * 1.5)
        );
        let safe = HudSpace::for_window(1280.0, 720.0, HudAnchor::SafeArea, 0.05);
        let s = safe.place(table.ammo_loaded).unwrap();
        assert!(
            s.origin.x > r.origin.x,
            "right-anchored box moves towards the window edge"
        );
        let centre = DesignBox {
            x: 300.0,
            y: 220.0,
            w: 40.0,
            h: 40.0,
            anchor_x: 0,
            anchor_y: 0,
        };
        assert_eq!(
            safe.place(centre).unwrap(),
            picture.place(centre).unwrap(),
            "centre stays"
        );
    }

    #[test]
    fn wide_hud_groups_preserve_spacing_and_four_three_identity() {
        let table = HudLayout::original();
        for (w, h) in [
            (640.0, 480.0),
            (1280.0, 960.0),
            (1920.0, 1080.0),
            (2560.0, 1080.0),
        ] {
            let picture = HudSpace::for_window(w, h, HudAnchor::Picture, table.safe_margin);
            let safe = HudSpace::for_window(w, h, HudAnchor::SafeArea, table.safe_margin);
            let mut shift: Option<f32> = None;
            for b in [
                table.ammo_reserve,
                table.ammo_icon,
                table.ammo_loaded,
                table.ammo_frame[0].at,
                table.ammo_frame[1].at,
                table.ammo_frame[2].at,
            ] {
                let p = picture.place(b).unwrap();
                let s = safe.place(b).unwrap();
                let dx = s.origin.x - p.origin.x;
                if let Some(previous) = shift {
                    assert!((dx - previous).abs() < 0.001);
                }
                shift = Some(dx);
                assert_eq!(s.size, p.size);
                assert!((s.origin.y - p.origin.y).abs() < 0.001);
                if w / h == 4.0 / 3.0 {
                    assert!((dx).abs() < 0.001);
                }
            }
            let right = safe.place(table.ammo_icon).unwrap();
            let left = safe.place(mirrored(table.ammo_icon)).unwrap();
            assert!((left.origin.x + right.origin.x + right.size.x - w).abs() < 0.001);
        }
    }

    #[test]
    fn radar_rotated_corners_move_as_one_top_right_group() {
        use crate::hud_radar::RadarPiece;
        let piece = RadarPiece::Quad {
            texture: "radar",
            corners: [[550.0, 20.0], [600.0, 30.0], [540.0, 60.0], [590.0, 70.0]],
            color: 0xffffffff,
        };
        for (w, h) in [(640.0, 480.0), (1920.0, 1080.0), (2560.0, 1080.0)] {
            let p = HudSpace::for_window(w, h, HudAnchor::Picture, 0.05);
            let s = HudSpace::for_window(w, h, HudAnchor::SafeArea, 0.05);
            let mut before = HudDraw::default();
            let mut after = HudDraw::default();
            radar(&mut before, &[piece.clone()], &p).unwrap();
            radar(&mut after, &[piece.clone()], &s).unwrap();
            let dx = after.polys[0].corners_px[0].x - before.polys[0].corners_px[0].x;
            for (a, b) in after.polys[0]
                .corners_px
                .iter()
                .zip(before.polys[0].corners_px)
            {
                assert!((a.x - b.x - dx).abs() < 0.001);
                assert!((a.y - b.y).abs() < 0.001);
            }
            if w / h > 4.0 / 3.0 {
                assert!(dx > 0.0);
            } else {
                assert!(dx.abs() < 0.001);
            }
        }
    }

    #[test]
    fn native_radar_removes_guest_unit_y_stretch_only_when_opted_in() {
        let ring = crate::hud_radar::RadarPiece::Rect {
            texture: "hudradar",
            rect: [538.0, 24.0, 608.0, 146.5],
            uv: FULL_UV,
            color: 0xffffffff,
        };
        for anchor in [HudAnchor::Picture, HudAnchor::SafeArea] {
            let space = HudSpace::for_window(2240.0, 960.0, anchor, 0.05);
            let mut draw = HudDraw::default();
            radar(&mut draw, &[ring.clone()], &space).unwrap();
            let c = draw.polys[0].corners_px;
            let width = c[1].x - c[0].x;
            let height = c[2].y - c[0].y;
            let ratio = if anchor == HudAnchor::Picture {
                1.75
            } else {
                1.0
            };
            assert!((height / width - ratio).abs() < 0.001);
        }
    }

    #[test]
    fn labelled_hud_row_clips_long_ink_to_geometry_without_moving_its_title() {
        use crate::hud_rank::{LabelledRowDraw, RowPiece, RowText};
        let row = LabelledRowDraw {
            panel: vec![RowPiece {
                rect: [180.0, 100.0, 100.0, 180.0],
                uv: FULL_UV,
            }],
            panel_alpha: 64,
            frame: vec![],
            frame_colour: 0,
            texts: vec![RowText {
                bytes: b"oversized rank value".to_vec(),
                x: 60.0,
                y: 80.0,
                scale_x: 0.7,
                colour: 0x7f7f7f60,
                outline_alpha: 64,
            }],
            font: 19,
            advance: 56,
        };
        for (width, height) in [(1280.0, 960.0), (1280.0, 720.0)] {
            let space = HudSpace::for_window(width, height, HudAnchor::Picture, 0.0);
            let mut draw = HudDraw::default();
            rank_box(&mut draw, &row, &HudLayout::original(), &space).unwrap();
            for text in &draw.texts {
                let clip = text.clip_px.unwrap();
                assert!((clip.size.x - 80.0 * space.scale()).abs() < 0.001);
                assert!(text.rect_px.origin.x < clip.origin.x);
                assert_eq!(
                    text.rect_px.origin.y, clip.origin.y,
                    "the title above the frame keeps its vertical contract"
                );
            }
        }
    }

    #[test]
    fn labelled_hud_row_uses_original_glyph_height_without_moving_pens() {
        use crate::hud_rank::{LabelledRowDraw, RowText};
        let table = HudLayout::original();
        let row = LabelledRowDraw {
            panel: Vec::new(),
            panel_alpha: 0,
            frame: Vec::new(),
            frame_colour: 0,
            texts: vec![RowText {
                bytes: b"TEST".to_vec(),
                x: 54.0,
                y: 24.0,
                scale_x: 0.875,
                colour: 0x7f7f7f60,
                outline_alpha: 64,
            }],
            font: 19,
            advance: 56,
        };
        for unit in [1.0, 2.0, 3.0] {
            let space = HudSpace::for_window(640.0 * unit, 480.0 * unit, HudAnchor::Picture, 0.05);
            let mut draw = HudDraw::default();
            rank_box(&mut draw, &row, &table, &space).unwrap();
            assert_eq!(draw.texts.len(), 5);
            for text in &draw.texts {
                assert_eq!(
                    text.scale,
                    [
                        0.875 / table.ammo_text_scale[0],
                        ORIGINAL_GLYPH_Y / table.ammo_text_scale[1]
                    ]
                );
            }
            let fill = &draw.texts[4];
            assert_eq!(fill.rect_px.origin, Vec2::new(54.0 * unit, 24.0 * unit));
            assert_eq!(draw.texts[0].rect_px.origin.x, 52.0 * unit);
            assert_eq!(draw.texts[1].rect_px.origin.y, 23.0 * unit);
        }
    }

    #[test]
    fn reticle_replays_the_drawer() {
        use reticle_addr::*;
        let mut guest = gameplay_guest();
        let viewer = 0x0100_0000;
        let viewport = 0x0400_0000;
        let m = &mut guest.0;
        m.insert(viewer + VIEWER_SHOW, 1);
        m.insert(viewer + VIEWER_SUB, 0);
        m.insert(viewer + VIEWER_AIM_X, 1.0f32.to_bits());
        m.insert(viewer + VIEWER_AIM_Y, 1.0f32.to_bits());
        m.insert(PLAYER_SLOTS, 5);
        let record = TABLE + 5 * RECORD_SIZE;
        m.insert(record + TEXTURE, 0x113);
        m.insert(record + HALF_SIZE, 22.0f32.to_bits());
        m.insert(record + COLOR, 0x7f7f_7f20);
        m.insert(record + BLEND, 2);
        m.insert(TINT_COLOR + 4, 0x400a_0a7f);
        m.insert(TINT_BLEND + 4, 1);
        m.insert(addr::VIEWPORT_PTR, viewport);
        m.insert(viewport + VP_X, 0);
        m.insert(viewport + VP_W, 640);
        m.insert(viewport + VP_Y, 0);
        m.insert(viewport + VP_H, 480);
        m.insert(AIM_FACTOR, 0.5f32.to_bits());
        m.insert(SCALE_X, 1.0f32.to_bits());
        m.insert(SCALE_Y, 1.0f32.to_bits());
        m.insert(0xf000_0000 + FN_RECOLOUR_MODE, 0);
        let ReticleCall::Draw(r) = Reticle::read(&guest) else {
            panic!("reticle expected")
        };
        assert_eq!(
            (r.index, r.texture, r.tint, r.color, r.blend),
            (5, 0x113, 0, 0x7f7f_7f20, 2)
        );
        assert_eq!((r.centre, r.half), ([320.0, 240.0], [22.0, 22.0]));
        assert_eq!(reticle_texture_name(r.texture), Some("crosshair_triangle"));
        assert_eq!(
            (r.rgba().0[0], r.rgba().0[3], r.blend_mode()),
            (
                127.0 * (2.0 / 255.0),
                32.0 * (2.0 / 255.0),
                BlendMode::Additive
            )
        );
        let frame = HudFrame::read(&guest).unwrap();
        let table = HudLayout::original();
        let space = HudSpace::for_window(1280.0, 960.0, HudAnchor::Picture, 0.05);
        let req = DrawRequest {
            reticle: Some(r),
            ..request(HudElement::Crosshair)
        };
        let draw = layout(
            &frame,
            &[req],
            HudOwnership::of(&[HudElement::Crosshair]),
            &table,
            &space,
        )
        .unwrap();
        assert!(draw.sprites.is_empty(), "the reticle is a view-pass sprite");
        assert_eq!(draw.view_pass.len(), 1);
        assert_eq!(
            (
                draw.view_pass[0].rect_px.origin.x,
                draw.view_pass[0].rect_px.size.x
            ),
            (596.0, 88.0)
        );
        // A live candidate in range tints 1 (table colour); predicates unavailable: legacy.
        guest.0.insert(viewer + VIEWER_TARGET, 0x0500_0000);
        assert_eq!(
            Reticle::read(&guest),
            ReticleCall::Legacy,
            "no predicate answers"
        );
        guest.0.insert(0xf000_0000 + FN_TARGET_ELIGIBLE, 1);
        guest.0.insert(0xf000_0000 + FN_TARGET_IN_RANGE, 1);
        let ReticleCall::Draw(t) = Reticle::read(&guest) else {
            panic!("tinted reticle")
        };
        assert_eq!((t.tint, t.color, t.blend), (1, 0x400a_0a7f, 1));
        guest.0.insert(viewer + VIEWER_TARGET, 0);
        let mut other = guest;
        other.0.insert(viewer + VIEWER_SCOPE_OBJECT, 1);
        other.0.insert(viewer + VIEWER_FLAGS, FLAG_SCOPE);
        assert_eq!(
            Reticle::read(&other),
            ReticleCall::Legacy,
            "sniper bar stays legacy"
        );
        other.0.insert(viewer + VIEWER_FLAGS, 0);
        other.0.insert(PLAYER_SLOTS, 0x27);
        assert_eq!(
            Reticle::read(&other),
            ReticleCall::Legacy,
            "own drawer stays legacy"
        );
        other.0.insert(PLAYER_SLOTS, 5);
        other.0.insert(viewer + VIEWER_SHOW, 0);
        assert_eq!(Reticle::read(&other), ReticleCall::Nothing, "hidden");
    }

    #[test]
    fn vitals_arcs_replay_the_drawer() {
        use arc_addr::*;
        let mut guest = gameplay_guest();
        let (viewer, object, block, viewport) = (
            0x0100_0000u32,
            0x0200_0000u32,
            0x0600_0000u32,
            0x0400_0000u32,
        );
        let m = &mut guest.0;
        m.insert(viewer, 0);
        m.insert(object + OBJ_FLAG_BLOCK, block);
        m.insert(block + BLOCK_FLASH, 2.0f32.to_bits());
        m.insert(HIT_TIMERS, 0);
        m.insert(0xf000_0000 + FN_PLAYER_COUNT, 1);
        m.insert(addr::VIEWPORT_PTR, viewport);
        m.insert(viewport + VP_CENTRE_X, 320);
        m.insert(viewport + VP_CENTRE_Y, 240);
        for i in 0..21u32 {
            let p = i as f32 / 20.0;
            for (table, rgb) in [(HEALTH_BANDS, [127, 100, 20]), (ARMOUR_BANDS, [40, 34, 80])] {
                m.insert(table + i * BAND_ROW + 4, p.to_bits());
                for k in 0..3 {
                    m.insert(table + i * BAND_ROW + 8 + 4 * k, rgb[k as usize]);
                }
            }
        }
        // Health bands 10..19 lit (half health), band 9 fading in; armour all unlit.
        for i in 10..20u32 {
            m.insert(block + 4 * i, 1);
        }
        m.insert(block + 4 * 9, 1);
        m.insert(block + BLOCK_TIMERS + 4 * 9, 0.35f32.to_bits());
        let VitalsCall::Draw(arcs) = VitalsArcs::read(&guest) else {
            panic!("arcs expected")
        };
        assert_eq!(arcs.alpha, [127, 50, 60, 20], "full fade at flash 2.0");
        assert_eq!((arcs.half_width, arcs.half_height), (307.2, 170.0));
        let health = &arcs.sides[0];
        assert_eq!((health.band_alpha(5), health.band_alpha(12)), (0.0, 1.0));
        assert!(
            (health.band_alpha(9) - 0.5).abs() < 1e-6,
            "lit band half way through its fade-in"
        );
        assert_eq!(health.soft_alpha(8), 0.25, "half the next band's alpha");
        let quads = arcs.quads();
        assert_eq!(quads.len(), 3 * 2 * 20);
        let bar = quads.iter().find(|q| q.texture == 0x323).unwrap();
        assert_eq!(bar.rect[0], 320.0 - 307.2);
        assert_eq!(bar.top[3], 50, "unlit bar band uses the unlit alpha");
        assert_eq!(bar.top[0], 38, "unlit bands dim to 30%");
        let armour_bar = quads.iter().filter(|q| q.texture == 0x323).nth(20).unwrap();
        assert_eq!(
            (armour_bar.uv[0], armour_bar.uv[1]),
            (1.0, 0.0),
            "armour arc mirrored in u"
        );
        assert!(
            quads[0].uv[2] < 1.0 && quads[0].uv[2] > 0.98,
            "band 0 v nudged"
        );
        let frame = HudFrame::read(&guest).unwrap();
        let space = HudSpace::for_window(640.0, 480.0, HudAnchor::Picture, 0.05);
        let req = DrawRequest {
            vitals: Some(arcs),
            ..request(HudElement::Vitals)
        };
        let draw = layout(
            &frame,
            &[req],
            HudOwnership::of(&[HudElement::Vitals]),
            &HudLayout::original(),
            &space,
        )
        .unwrap();
        assert_eq!(draw.sprites.len(), 120 * ARC_SLICES);
        let mut hit = guest;
        hit.0.insert(HIT_TIMERS, 0.5f32.to_bits());
        assert_eq!(
            VitalsArcs::read(&hit),
            VitalsCall::Legacy,
            "hit sector fading: the original keeps the call"
        );
        hit.0.insert(block + BLOCK_FLASH, 0);
        assert_eq!(
            VitalsArcs::read(&hit),
            VitalsCall::Nothing,
            "no flash, no arcs"
        );
    }

    #[test]
    fn hit_sector_is_rust_owned_state() {
        use hit_addr::*;
        let mut guest = gameplay_guest();
        let (players, direction, viewport) = (0x0700_0000u32, 0x0710_0000u32, 0x0400_0000u32);
        let m = &mut guest.0;
        m.insert(PLAYERS_PTR, players);
        m.insert(players + PLAYER_STRIDE + PLAYER_YAW, 90.0f32.to_bits());
        // A hit from +x (right of a player facing +z) on entry 1, player turned 90 degrees.
        for (k, v) in [1.0f32, 0.0, 0.0].iter().enumerate() {
            m.insert(direction + 4 * k as u32, v.to_bits());
        }
        m.insert(addr::VIEWPORT_PTR, viewport);
        for (offset, value) in [
            (arc_addr::VP_CENTRE_X, 320),
            (VP_WIDTH, 640),
            (arc_addr::VP_CENTRE_Y, 240),
            (VP_HEIGHT, 480),
        ] {
            m.insert(viewport + offset, value);
        }
        m.insert(FRAME_RATE, 60);
        m.insert(TIME_SCALE, 1.0f32.to_bits());
        m.insert(DOUBLE_STEP, 0);
        let mut hits = HitSectors::default();
        assert_eq!(hits.step(&guest, 1), None, "no hit, nothing drawn");
        assert!(hits.hit_from_call(&guest, 1, direction));
        assert_eq!(hits.timer[1], 1.0);
        assert!(hits.angle[1].abs() < 1e-6, "atan2(1, 0) minus 90 degrees");
        // Angle 0: the sector sits below the centre (sin 0 = 0, cos 0 = 1), sine table computed (unmapped).
        let quad = hits.step(&guest, 1).expect("drawn at timer 1");
        assert_eq!(
            quad.color,
            [31, 0, 0, 0x32],
            "red = timer * 31 before the decay, alpha word 0x32"
        );
        let radius = 640.0 * 0.3688 - 16.0;
        let centre_y = 240.0 + radius * (480.0 * 0.4196 / (640.0 * 0.3688));
        let x = (quad.corners[0][0] + quad.corners[3][0]) / 2.0;
        let y = (quad.corners[0][1] + quad.corners[3][1]) / 2.0;
        assert!(
            (x - 320.0).abs() < 1e-3 && (y - centre_y).abs() < 1e-3,
            "quad centred on the ring: {x} {y}"
        );
        assert!(
            (hits.timer[1] - (1.0 - 2.0 / 60.0)).abs() < 1e-6,
            "decayed by 2 / 60 per call"
        );
        guest.0.insert(DOUBLE_STEP, 1);
        hits.step(&guest, 1);
        assert!(
            (hits.timer[1] - (1.0 - 6.0 / 60.0)).abs() < 1e-5,
            "double step decays twice"
        );
        for _ in 0..60 {
            hits.step(&guest, 1);
        }
        assert_eq!(hits.timer[1], 0.0, "clamped at 0");
        assert_eq!(hits.step(&guest, 1), None);
        hits.hit(2, [0.0, 0.0, 1.0], 0.0);
        hits.reset();
        assert_eq!(hits, HitSectors::default(), "level reset");
        hits.hit(7, [0.0, 0.0, 1.0], 0.0);
        assert_eq!(hits, HitSectors::default(), "out-of-range entry ignored");
        // Layout: one additive rotated quad per vitals request with a sector.
        let frame = HudFrame::read(&guest).unwrap();
        let space = HudSpace::for_window(1280.0, 960.0, HudAnchor::Picture, 0.05);
        let req = DrawRequest {
            hit_sector: Some(quad),
            ..request(HudElement::Vitals)
        };
        let draw = layout(
            &frame,
            &[req],
            HudOwnership::of(&[HudElement::Vitals]),
            &HudLayout::original(),
            &space,
        )
        .unwrap();
        assert_eq!(draw.polys.len(), 1);
        assert_eq!(draw.polys[0].corners_px[0].x, quad.corners[0][0] * 2.0);
        assert_eq!(draw.polys[0].texture, HIT_SECTOR_TEXTURE);
    }
}

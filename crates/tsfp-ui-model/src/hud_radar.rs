//! T2312 increment 5: the radar as native Rust (`0x133AB0`, called by the HUD frame `0x14F0B0`).
//!
//! The original (MEASURED from the XBE disassembly, docs/t2312-native-hud.md, "Radar"):
//!
//! 1. A pulse alpha from the frame clock (`0x7DE318`) through the guest sine table.
//! 2. Blip collection into the private array `0x7A5DC0` (count `0x7490F8`, entries of 16 bytes: angle, distance,
//!    colour word, texture) through `0x1339E0`: the other connected players (records `*0x7356D8 + i * 0xEA0`),
//!    the object list `0x7A20C0`, and per game mode the objects of `0x7A2BE4`, `0x2744C0`/`0x2744E0`,
//!    `0x7A2C64` and tag searches `0x240070`.
//! 3. The radar: the ring `hudradar`, the rotating sweep `hudradarcentre` (a square turned towards the locked
//!    target, smoothed by the per-slot angle `0x748000`), the own dot `radarspot`, every blip (`radarspot` or
//!    `hudradarsquare` inside the ring, `hudradararrow` on the rim when the blip is out of range) and the
//!    centre pulse `hudradarpulse` (timer `+0x2C` of the player's HUD record, reset at 1.4 to -0.5).
//!
//! [`RadarState::read`] replays all of it on the live state and returns the pieces in painter order. The
//! guest is never written: the blip array, the sweep angle and the pulse reset are Rust-owned
//! ([`RadarState`]). The read-only predicates the collection uses are called through the guest call.
//! Modes the native radar does not decode yet (the tag searches of the 0x40 modes, the recolour modes)
//! return [`RadarCall::Legacy`] and the original draws that frame.
use crate::hud_native::{GuestCall, GuestRead};

/// Guest addresses of the radar (MEASURED from the disassembly of `0x133AB0` and `0x1339E0`).
pub mod radar_addr {
    /// The radar drawer (cdecl, no arguments).
    pub const FN_RADAR: u32 = 0x0013_3ab0;
    /// Viewer record: `+4` slot, `+0x14` object, `+0xA8` / `+0xB0` x / z, `+0xAC` y, `+0x100` yaw (degrees).
    pub const VIEWER: u32 = 0x007b_0c7c;
    /// Player HUD records `0x7A6160 + slot * 0x177C`: `+0` state, `+0x2C` pulse timer (f32).
    pub const HUD_RECORDS: u32 = 0x007a_6160;
    pub const HUD_RECORD_SIZE: u32 = 0x177c;
    pub const HUD_PULSE: u32 = 0x2c;
    /// Frame rate (int), frame clock (int), time scale (f32).
    pub const FRAME_RATE: u32 = 0x004e_8cec;
    pub const CLOCK: u32 = 0x007d_e318;
    pub const TIME_SCALE: u32 = 0x004e_794c;
    pub const SIN_TABLE: u32 = 0x007a_38a0;
    /// Players: count `0x790950` + `0x7A2958`, records `*0x7356D8 + i * 0xEA0`, team slots `0x777244 + i * 0x124`.
    pub const PLAYER_COUNT: u32 = 0x0079_0950;
    pub const EXTRA_PLAYER_COUNT: u32 = 0x007a_2958;
    pub const PLAYERS_PTR: u32 = 0x0073_56d8;
    pub const PLAYER_SIZE: u32 = 0xea0;
    pub const TEAM_SLOTS: u32 = 0x0077_7244;
    pub const TEAM_SLOT_SIZE: u32 = 0x124;
    pub const TEAM_SLOTS_END: u32 = 0x0077_ad94;
    pub const TEAM_FLAGS: u32 = 0x0077_41b4;
    /// Game mode bytes and words.
    pub const MODE_A: u32 = 0x007d_e455;
    pub const MODE_B: u32 = 0x007d_e456;
    pub const MODE_FLAGS: u32 = 0x007d_e458;
    pub const MODE_C: u32 = 0x007d_e470;
    pub const MODE_SCORE_GATE: u32 = 0x007d_e474;
    pub const MODE_HOLDER: u32 = 0x007d_e320;
    pub const MODE_LEVEL: u32 = 0x007d_e403;
    pub const FLASH: u32 = 0x0074_97b0;
    /// Object lists: `0x7A20C0` (linked, object at `+0x4C`), `0x7A2BE4` (6 entries of 8 bytes), `0x7A2C64`
    /// (count `0x74C394`, entries of 8 bytes).
    pub const LIST_OBJECTS: u32 = 0x007a_20c0;
    pub const LIST_MODE1: u32 = 0x007a_2be4;
    pub const LIST_MODE1_END: u32 = 0x007a_2c14;
    pub const LIST_MODE6: u32 = 0x007a_2c64;
    pub const LIST_MODE6_COUNT: u32 = 0x0074_c394;
    /// Colours of the team modes (words read from the XBE data).
    pub const COLOUR_OWN: u32 = 0x004d_1e50;
    pub const COLOUR_OTHER: u32 = 0x004d_1e3c;
    /// Viewport `+4` x1, `+0x14` y0, `+0x30` small radar flag; HUD rectangle margins `0x7A6144` (top),
    /// `0x7A6148` (right).
    pub const VIEWPORT_PTR: u32 = 0x007e_71c8;
    pub const HUD_TOP: u32 = 0x007a_6144;
    pub const HUD_RIGHT: u32 = 0x007a_6148;
    /// HUD layout scales: x, y, and the rim marker scale.
    pub const SCALE_X: u32 = 0x004e_68f4;
    pub const SCALE_Y: u32 = 0x004e_68f8;
    pub const SCALE_RIM: u32 = 0x004e_68f0;
    /// Locked target handle at `+0x90` of the viewer's object extension; handle pool `0x72F5E8` (0x2E8).
    pub const TARGET_HANDLE: u32 = 0x90;
    pub const HANDLE_POOL: u32 = 0x0072_f5e8;
    pub const HANDLE_ENTRY_SIZE: u32 = 0x2e8;
    /// Sweep angle per slot (f32 degrees), smoothed towards the target bearing; Rust-owned natively.
    pub const SWEEP_ANGLES: u32 = 0x0074_8000;
    /// Texture rows (`0x80F20`: handle at `0x4D2D64 + id * 24`) and records (`*0x4B85C8 + handle * 0x34`,
    /// size `+0x26` / `+0x2A`, u16).
    pub const TEXTURE_ROWS: u32 = 0x004d_2d64;
    pub const TEXTURE_RECORDS_PTR: u32 = 0x004b_85c8;
    /// Predicates called through the guest (read-only): slot connected, object flag pairs, mode table,
    /// session active, contact list, mode-2 objects, recolour mode.
    pub const FN_CONNECTED: u32 = 0x0035_68c0;
    pub const FN_FLAG_PAIR: u32 = 0x0001_74e0;
    pub const FN_MASKED_PAIR: u32 = 0x0001_7540;
    pub const FN_MODE_TABLE: u32 = 0x0022_df30;
    pub const FN_SESSION: u32 = 0x0035_33f0;
    pub const FN_CONTACT: u32 = 0x0017_9f70;
    pub const FN_MODE2_ID: u32 = 0x0027_44c0;
    pub const FN_MODE2_LOOKUP: u32 = 0x0009_a420;
    pub const FN_MODE2_OBJECT: u32 = 0x0027_44e0;
    pub const FN_RECOLOUR_MODE: u32 = 0x0012_cb40;
}

/// Constants of the radar code (f32 values from the XBE, addresses in the comments).
mod k {
    pub const PULSE_RATE: f32 = 9.424_778; // 0x47DAC8
    pub const TO_INDEX: f32 = 325.949_3; // 0x475CE0
    pub const ROUND: f32 = 0.5; // 0x475CD4
    pub const BYTE: f32 = 127.0; // 0x475CC0
    pub const RADIUS_SMALL: f32 = 30.0; // 0x47CFF4
    pub const RADIUS: f32 = 35.0; // 0x47DAC4
    pub const DEG_TO_RAD: f32 = 0.017_453_292; // 0x475D0C
    pub const RAD_TO_DEG: f32 = 57.295_776; // 0x478638
    pub const RANGE: f32 = 0.02; // 0x4785D4
    pub const RANGE_WIDE: f32 = 0.05; // 0x475CE8
    pub const TARGET_RANGE_SQ: f32 = 2500.0; // 0x47D744
    pub const INV_360: f32 = 0.002_777_777_8; // 0x475CB4
    pub const TURN: f32 = 360.0; // 0x475C88
    pub const SWEEP_OFFSET: f32 = 45.0; // 0x478900
    pub const SWEEP_RATE: f32 = 8.0; // 0x47D1E4
    pub const SWEEP_SIZE: f32 = 1.3; // 0x47D690
    pub const DOT: f32 = 2.0; // 0x475D10
    pub const BLIP: f32 = 2.5; // 0x475D00
    pub const RIM_SIZE: f32 = 8.0; // 0x47D1E4
    pub const RIM_HALF_WIDTH: f32 = 4.0; // 0x475DB4 / 0x47DAC0
    pub const TAU: f32 = 6.283_185_5; // 0x475C94
    pub const QUARTER: f32 = 1.570_796_4; // 0x475CE4
    pub const PI: f32 = 3.141_592_7; // 0x475C90
    pub const PULSE_MIN: f32 = 0.01; // 0x475D1C
    pub const PULSE_SPEED: f32 = 0.714_285_7; // 0x47DABC
    pub const PULSE_END: f32 = 1.4; // 0x47D6D4
    pub const PULSE_RESTART: f32 = -0.5; // 0x4760C4
    pub const ALT_HEIGHT: f32 = 1.5; // 0x475D20
    pub const HEAT: f32 = 0.666_666_7; // 0x47D9F4
    /// Texel units of the rect draw `0x1B7B0` (16 per texel, 0x475DC0).
    pub const TEXEL_UNITS: f32 = 16.0;
}

/// Radar texture ids (the argument of `0x80F20`) and their names (name-table row = id - 1, the convention
/// of the reticle table, MEASURED in increment 2).
pub const RADAR_TEXTURES: [(u32, &str); 6] = [
    (0xfa, "hudradar"),
    (0xfb, "hudradarpulse"),
    (0xfc, "hudradarcentre"),
    (0xfd, "hudradararrow"),
    (0xfe, "radarspot"),
    (0xff, "hudradarsquare"),
];

pub fn radar_texture_name(id: u32) -> Option<&'static str> {
    RADAR_TEXTURES
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, n)| *n)
}

/// One radar draw in guest pixels (640 x 480 design space), painter order, all with the radar's
/// blend mode 1 (alpha). Colour words are R, G, B, A high to low, 0x80 = 1.0.
#[derive(Clone, Debug, PartialEq)]
pub enum RadarPiece {
    /// `0x1B7B0` / `0x1F090`: rectangle left, top, right, bottom with texture coordinates (0..1).
    Rect {
        texture: &'static str,
        rect: [f32; 4],
        uv: [f32; 4],
        color: u32,
    },
    /// `0x167B0`: quad corners in strip order (texture top-left, top-right, bottom-left, bottom-right).
    Quad {
        texture: &'static str,
        corners: [[f32; 2]; 4],
        color: u32,
    },
}

/// One blip of the radar (the entry `0x1339E0` writes): angle (radians, relative to the view), distance
/// (1 = the rim), colour word and texture id.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blip {
    pub angle: f32,
    pub distance: f32,
    pub color: u32,
    pub texture: u32,
}

/// Owned presentation state. The caller advances once for its presentation
/// interval, independently of simulation ticks and without guest timer writes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RadarAnimation {
    pub sweep_degrees: f32,
    pub pulse_seconds: f32,
}
impl RadarAnimation {
    pub fn advance(&mut self, bearing_degrees: f32, sweep_seconds: f64, pulse_seconds: f32) {
        let a = wrap_degrees(self.sweep_degrees);
        let b = wrap_degrees(bearing_degrees - k::SWEEP_OFFSET);
        let difference = period_difference(a, b, 0.0, k::TURN);
        self.sweep_degrees = wrap_degrees(
            (difference as f64 * (sweep_seconds * k::SWEEP_RATE as f64) + a as f64) as f32,
        );
        self.pulse_seconds += pulse_seconds;
    }

    /// The original restarts the pulse after presenting the endpoint. Keep this
    /// separate so the endpoint remains renderable for the current frame.
    pub fn finish_frame(&mut self) {
        if self.pulse_seconds >= k::PULSE_END {
            self.pulse_seconds = k::PULSE_RESTART;
        }
    }
}

/// What the native radar does with one drawer call.
#[derive(Clone, Debug, PartialEq)]
pub enum RadarCall {
    /// No HUD slot: the original returns before drawing.
    Nothing,
    /// The radar pieces, painter order.
    Draw(Vec<RadarPiece>),
    /// A mode the native radar does not decode: the original draws (reason for the report).
    Legacy(&'static str),
}

/// Rust-owned radar state (replaces the guest writes of the original): the sweep angle per slot
/// (`0x748000`, seeded from the guest value, re-seeded after the level reset `0x148400`) and the pulse
/// restart (the original writes -0.5 into the HUD record's timer; natively the guest timer keeps counting
/// and the restart is an offset).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RadarState {
    pub sweep: [Option<f32>; 4],
    pub pulse_base: [f32; 4],
    /// The blips of the last call (evidence: compared with the original's array by the shadow check).
    pub blips: Vec<Blip>,
}

/// Result of [`RadarState::shadow_check`]: the previous call's Rust blips and sweep against the original's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShadowCheck {
    pub blips_equal: bool,
    pub rust_count: usize,
    pub guest_count: u32,
    pub max_angle_error: f32,
    pub max_distance_error: f32,
    pub sweep_error: f32,
}

/// `cvttss2si`: truncation, out of range and NaN give `0x8000_0000`.
fn cvtt(x: f32) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
        i32::MIN
    } else {
        x as i32
    }
}

/// Guest sine table entry (`0x7A38A0`, 2048 entries), computed if the table cannot be read.
fn table<G: GuestRead + ?Sized>(g: &G, index: i32) -> f32 {
    let entry = (index & 0x7ff) as u32;
    g.read_f32(radar_addr::SIN_TABLE + 4 * entry)
        .unwrap_or_else(|| (entry as f32 * std::f32::consts::TAU / 2048.0).sin())
}

/// `0x3C8501` floor and the original's modulo 360 (x87, stored as f32).
fn wrap_degrees(a: f32) -> f32 {
    let turns = ((a as f64) * (k::INV_360 as f64)).floor();
    (a as f64 - turns * k::TURN as f64) as f32
}

/// `0x12570(a, b, 0, 360)`: the difference `b - a` with the smallest magnitude over the period.
pub fn period_difference(a: f32, b: f32, low: f32, high: f32) -> f32 {
    let period = high - low;
    let d1 = b - a;
    let d2 = b - (period + a);
    let d3 = b - (a - period);
    if d2.abs() > d1.abs() {
        if d3.abs() > d1.abs() { d1 } else { d3 }
    } else if d3.abs() > d2.abs() {
        d2
    } else {
        d3
    }
}

impl RadarState {
    /// `0x148400` (level start): the sweep angles are re-seeded from the guest on the next draw.
    pub fn reset(&mut self) {
        self.sweep = [None; 4];
    }

    /// One `0x133AB0` call on the live state.
    pub fn read<G: GuestCall + ?Sized>(&mut self, g: &G) -> RadarCall {
        match self.read_inner(g) {
            Some(call) => call,
            None => RadarCall::Legacy("unreadable state"),
        }
    }

    fn read_inner<G: GuestCall + ?Sized>(&mut self, g: &G) -> Option<RadarCall> {
        use radar_addr::*;
        let viewer = g.read32(VIEWER)?;
        let slot = g.read32(viewer + 4)? as i32;
        if slot < 0 {
            return Some(RadarCall::Nothing);
        }
        let slot_index = slot as usize;
        if slot_index >= self.sweep.len() {
            return Some(RadarCall::Legacy("slot out of range"));
        }
        let flags = g.read32(MODE_FLAGS)? as u8;
        let mode_c = g.read32(MODE_C)?;
        if flags & 0x40 != 0 && matches!(mode_c, 0xa | 0xb | 0x14) {
            return Some(RadarCall::Legacy("tag-search mode"));
        }
        let hud = HUD_RECORDS + slot as u32 * HUD_RECORD_SIZE;
        let state = g.read32(hud)? as i32;
        if g.call(FN_RECOLOUR_MODE, &[])? != 0 && !(5..=6).contains(&state) {
            return Some(RadarCall::Legacy("recolour mode"));
        }
        let rate = g.read32(FRAME_RATE)? as i32 as f32;
        let clock = g.read32(CLOCK)? as i32 as f32;
        let phase = 1.0f32 / rate * clock * k::PULSE_RATE * k::TO_INDEX + k::ROUND;
        let wave = (table(g, cvtt(phase)) + 1.0) * k::ROUND;
        let wave = if wave > 1.0 {
            1.0
        } else if 0.0 > wave {
            0.0
        } else {
            wave
        };
        let pulse = cvtt(wave * k::BYTE) as u32;
        let blips = self.collect(g, viewer, pulse)?;
        let drawn = self
            .draw(g, viewer, slot_index, hud, &blips)
            .map(RadarCall::Draw);
        self.blips = blips;
        drawn
    }

    /// Diagnostic (the original keeps drawing): before this frame's original call, its blip array
    /// (`0x7A5DC0`, count `0x7490F8`) and sweep angle (`0x748000`) still hold what it computed one call
    /// earlier from the same state the Rust replay read; compare them with the Rust results of that call.
    pub fn shadow_check<G: GuestRead + ?Sized>(&self, g: &G, slot: usize) -> Option<ShadowCheck> {
        use radar_addr::*;
        let guest_count = g.read32(0x0074_90f8)?;
        let mut check = ShadowCheck {
            rust_count: self.blips.len(),
            guest_count,
            blips_equal: true,
            ..ShadowCheck::default()
        };
        if guest_count as usize != self.blips.len() {
            check.blips_equal = false;
        }
        for (i, blip) in self.blips.iter().enumerate().take(guest_count as usize) {
            let entry = 0x007a_5dc0 + 16 * i as u32;
            let angle = g.read_f32(entry)?;
            let distance = g.read_f32(entry + 4)?;
            let color = g.read32(entry + 8)?;
            let handle = g.read32(entry + 12)?;
            let expected_handle = g.read32(TEXTURE_ROWS + blip.texture * 24)?;
            check.max_angle_error = check.max_angle_error.max((angle - blip.angle).abs());
            check.max_distance_error = check
                .max_distance_error
                .max((distance - blip.distance).abs());
            if color != blip.color || handle != expected_handle {
                check.blips_equal = false;
            }
        }
        if let Some(sweep) = self.sweep.get(slot).copied().flatten() {
            check.sweep_error = (g.read_f32(SWEEP_ANGLES + 4 * slot as u32)? - sweep).abs();
        }
        Some(check)
    }

    /// The blip collection of `0x133AB0` (calls of `0x1339E0`), in the original's order.
    fn collect<G: GuestCall + ?Sized>(&self, g: &G, viewer: u32, pulse: u32) -> Option<Vec<Blip>> {
        use radar_addr::*;
        let mut blips = Vec::new();
        let flags = g.read32(MODE_FLAGS)? as u8;
        let mode_a = g.read32(MODE_A)? as u8;
        let mode_b = g.read32(MODE_B)? as u8;
        let mode_c = g.read32(MODE_C)?;
        let wide = flags & 0x40 != 0 && mode_c == 0xf;
        let push = |blips: &mut Vec<Blip>, position: u32, color: u32, texture: u32| -> Option<()> {
            blips.push(blip(g, viewer, position, color, texture, wide)?);
            Some(())
        };
        // Other players.
        let count =
            (g.read32(PLAYER_COUNT)? as i32).wrapping_add(g.read32(EXTRA_PLAYER_COUNT)? as i32);
        let mut index: i32 = 0;
        let mut team_slot = TEAM_SLOTS;
        while index < count && team_slot < TEAM_SLOTS_END {
            let record = g
                .read32(PLAYERS_PTR)?
                .wrapping_add(index as u32 * PLAYER_SIZE);
            if let Some(color) = self.player_blip_color(
                g, viewer, index, record, team_slot, mode_a, mode_b, flags, mode_c,
            )? {
                let object = g.read32(record)?;
                push(&mut blips, object + 0x64, color, 0xfe)?;
            }
            index += 1;
            team_slot += TEAM_SLOT_SIZE;
        }
        // The object list (contacts pulse).
        let mut node = g.read32(LIST_OBJECTS)?;
        let mut guard = 0;
        while node != 0 && guard < 4096 {
            let object = g.read32(node + 0x4c)?;
            let color = if g.call(FN_CONTACT, &[object, 0])? != 0 {
                (pulse & 0xff) | 0x6030_6000
            } else {
                0x6030_607f
            };
            push(&mut blips, object + 0x64, color, 0xfe)?;
            node = g.read32(node)?;
            guard += 1;
        }
        let flash = g.read32(FLASH)? & 0x10 != 0;
        let live = |object: u32| -> Option<bool> {
            Some(object != 0 && g.read32(object + 0x28)? & 1 != 0)
        };
        if mode_a == 1 {
            let color = if flash { 0x7f7f_7f7f } else { 0 };
            let mut entry = LIST_MODE1;
            while index < 0x34 && entry < LIST_MODE1_END {
                let object = g.read32(entry)?;
                if live(object)? {
                    push(&mut blips, object + 0x64, color, 0xff)?;
                }
                entry += 8;
            }
        } else if mode_a == 2 && 0x34 - index >= 4 && g.read32(MODE_LEVEL)? as u8 >= 2 {
            let own = (g.read32(COLOUR_OWN)? & 0xffff_ff00) | pulse;
            let other = (g.read32(COLOUR_OTHER)? & 0xffff_ff00) | pulse;
            let first = g.call(FN_MODE2_LOOKUP, &[g.call(FN_MODE2_ID, &[1])?])?;
            let second = g.call(FN_MODE2_LOOKUP, &[g.call(FN_MODE2_ID, &[2])?])?;
            let object_two = g.call(FN_MODE2_OBJECT, &[2])?;
            let object_one = g.call(FN_MODE2_OBJECT, &[1])?;
            if first != 0 {
                push(&mut blips, first + 0xc, 0x7f, 0xff)?;
            }
            if second != 0 {
                push(&mut blips, second + 0xc, 0x7f, 0xff)?;
            }
            if live(object_two)? {
                push(&mut blips, object_two + 0x64, own, 0xff)?;
            }
            if live(object_one)? {
                push(&mut blips, object_one + 0x64, other, 0xff)?;
            }
        }
        if mode_a == 6 {
            let entries = g.read32(LIST_MODE6_COUNT)? as i32;
            let mut n = 0;
            // The original bounds this loop by the player loop's final index (< 0x34), as the other lists.
            while n < entries && index < 0x34 {
                let object = g.read32(LIST_MODE6 + 8 * n as u32)?;
                if object != 0 {
                    let kind = g.read32(g.read32(object + 0x7c)? + 0x1c)?;
                    let color = match (flash, kind) {
                        (false, _) => 0,
                        (true, 1) => 0x7f1e_1e5a,
                        (true, 2) => 0x1e1e_7f5a,
                        (true, _) => 0x1e7f_1e5a,
                    };
                    push(&mut blips, object + 0x64, color, 0xff)?;
                }
                n += 1;
            }
        }
        Some(blips)
    }

    /// Colour of another player's blip, `None` when the original skips the player.
    #[allow(clippy::too_many_arguments)]
    fn player_blip_color<G: GuestCall + ?Sized>(
        &self,
        g: &G,
        viewer: u32,
        index: i32,
        record: u32,
        team_slot: u32,
        mode_a: u8,
        mode_b: u8,
        flags: u8,
        mode_c: u32,
    ) -> Option<Option<u32>> {
        use radar_addr::*;
        let object = g.read32(record)?;
        if object == 0
            || g.call(FN_CONNECTED, &[index as u32])? == 0
            || index as u32 == g.read32(viewer)?
        {
            return Some(None);
        }
        let pair = if mode_a == 0xa {
            [0x10, 0x14]
        } else {
            [0x18, 0x1c]
        };
        if g.read32(record + pair[0])? | g.read32(record + pair[1])? == 0 {
            return Some(None);
        }
        if g.call(FN_FLAG_PAIR, &[record, 1, 0])? != 0
            || g.call(FN_MASKED_PAIR, &[record, 0x10000, 0])? != 0
        {
            return Some(None);
        }
        if g.call(FN_MODE_TABLE, &[record])? == 0 {
            return Some(None);
        }
        if g.read32(MODE_SCORE_GATE)? != 0 && g.read16(record + 0xa60)? == 0 {
            return Some(None);
        }
        if g.call(FN_SESSION, &[])? != 0 {
            let team = g.read32(team_slot)?;
            if team != 0 {
                let id = g.read32(team)?;
                let byte = g.read32((id.wrapping_mul(0x30c).wrapping_add(TEAM_FLAGS)) & !3)?
                    >> (8 * ((id.wrapping_mul(0x30c).wrapping_add(TEAM_FLAGS)) & 3));
                if byte & 0x10 == 0 {
                    return Some(None);
                }
            }
        }
        let color = if mode_b == 0x12 {
            if object == g.read32(MODE_HOLDER)? {
                g.read32(COLOUR_OWN)?
            } else {
                g.read32(COLOUR_OTHER)?
            }
        } else if flags & 0x40 != 0 && mode_c == 0xf {
            let heat = g.read_f32(record + 0x9e0)?;
            let v = ((heat as f64) * (k::HEAT as f64) * (k::BYTE as f64)) as i32 as u32;
            ((v & 0xff) << 16) | (0x7fu32.wrapping_sub(v) << 24) | 0x64
        } else if flags & 1 != 0 {
            g.read32(record + 0xa58)?
        } else {
            g.read32(COLOUR_OTHER)?
        };
        Some(Some(color))
    }

    /// The drawing half of `0x133AB0`: ring, sweep, own dot, blips, centre pulse.
    fn draw<G: GuestCall + ?Sized>(
        &mut self,
        g: &G,
        viewer: u32,
        slot: usize,
        hud: u32,
        blips: &[Blip],
    ) -> Option<Vec<RadarPiece>> {
        self.draw_input(g, viewer, slot, hud, blips, None)
    }

    /// Render owned blips and animation. Reads only viewport, original texture
    /// metadata and original trigonometry tables; never the guest HUD lifecycle.
    pub fn draw_owned<G: GuestCall + ?Sized>(
        &mut self,
        g: &G,
        animation: &RadarAnimation,
        blips: &[Blip],
    ) -> Option<Vec<RadarPiece>> {
        self.draw_input(g, 0, 0, 0, blips, Some(animation))
    }

    fn draw_input<G: GuestCall + ?Sized>(
        &mut self,
        g: &G,
        viewer: u32,
        slot: usize,
        hud: u32,
        blips: &[Blip],
        native: Option<&RadarAnimation>,
    ) -> Option<Vec<RadarPiece>> {
        use radar_addr::*;
        let mut out = Vec::new();
        let viewport = g.read32(VIEWPORT_PTR)?;
        let r = if g.read32(viewport + 0x30)? != 0 {
            k::RADIUS_SMALL
        } else {
            k::RADIUS
        };
        let sx = g.read_f32(SCALE_X)?;
        let sy = g.read_f32(SCALE_Y)?;
        let x1 = (g.read32(viewport + 4)? as i32).wrapping_sub(g.read32(HUD_RIGHT)? as i32);
        let y0 = (g.read32(viewport + 0x14)? as i32).wrapping_add(g.read32(HUD_TOP)? as i32);
        let cx = x1 as f32 - r * sx;
        let cy = y0 as f32 + r * sy;
        let frame = 0x7f7f_7f3f;
        out.push(RadarPiece::Rect {
            texture: "hudradar",
            rect: [cx - r * sx, cy - r * sy, cx + r * sx, cy + r * sy],
            uv: [0.0, 0.0, 1.0, 1.0],
            color: frame,
        });
        // The sweep: turned towards the bearing of the locked target (when close), smoothed.
        let sweep = if let Some(animation) = native {
            animation.sweep_degrees
        } else {
            let object = g.read32(viewer + 0x14)?;
            let ext = g.read32(object + 0x7c)?;
            let yaw = g.read_f32(viewer + 0x100)?;
            let bearing = target_bearing(g, ext, yaw)?;
            let seeded = match self.sweep[slot] {
                Some(a) => a,
                None => g.read_f32(SWEEP_ANGLES + 4 * slot as u32)?,
            };
            let a = wrap_degrees(seeded);
            let b = wrap_degrees(bearing - k::SWEEP_OFFSET);
            let diff = period_difference(a, b, 0.0, k::TURN);
            let rate = g.read32(FRAME_RATE)? as i32 as f64;
            let step = (1.0 / rate) * g.read_f32(TIME_SCALE)? as f64 * k::SWEEP_RATE as f64;
            let moved = (diff as f64 * step + a as f64) as f32;
            let sweep = wrap_degrees(moved);
            self.sweep[slot] = Some(sweep);
            sweep
        };
        let index =
            ((sweep as f64 * k::DEG_TO_RAD as f64) * k::TO_INDEX as f64 + k::ROUND as f64) as i32;
        let c = table(g, index + 0x200) * r * k::SWEEP_SIZE;
        let s = table(g, index) * r * k::SWEEP_SIZE;
        out.push(RadarPiece::Quad {
            texture: "hudradarcentre",
            corners: [
                [cx + c * sx, cy + s * sy],
                [cx + s * sx, cy - c * sy],
                [cx - s * sx, cy + c * sy],
                [cx - c * sx, cy - s * sy],
            ],
            color: frame,
        });
        // The own dot (`0x1F090`: integer rectangle).
        let dot_uv = texel_uv(g, 0xfe, 0x50)?;
        out.push(RadarPiece::Rect {
            texture: "radarspot",
            rect: [
                cvtt(cx - k::DOT) as f32,
                cvtt(cy - k::DOT) as f32,
                cvtt(cx + k::DOT) as f32,
                cvtt(cy + k::DOT) as f32,
            ],
            uv: dot_uv,
            color: 0x7f7f_7f80,
        });
        let rim = g.read_f32(SCALE_RIM)? * k::RIM_SIZE;
        for blip in blips.iter().filter(|b| b.color != 0) {
            let Some(texture) = radar_texture_name(blip.texture) else {
                continue;
            };
            if blip.distance >= 1.0 {
                let angle = k::TAU - blip.angle + k::QUARTER;
                let (sin, cos) = ((angle as f64).sin(), (angle as f64).cos());
                let turn = |x: f32, y: f32| {
                    let (x, y) = (x as f64, y as f64);
                    [
                        ((cos * x - sin * y) as f32) * sx + cx,
                        ((sin * x + cos * y) as f32) * sy + cy,
                    ]
                };
                out.push(RadarPiece::Quad {
                    texture: "hudradararrow",
                    corners: [
                        turn(-k::RIM_HALF_WIDTH, rim + r),
                        turn(k::RIM_HALF_WIDTH, rim + r),
                        turn(-k::RIM_HALF_WIDTH, r),
                        turn(k::RIM_HALF_WIDTH, r),
                    ],
                    color: blip.color,
                });
            } else {
                let index = cvtt(blip.angle * k::TO_INDEX + k::ROUND);
                let bx = cx - table(g, index + 0x200) * blip.distance * r * sx;
                let by = table(g, index) * blip.distance * r * sy + cy;
                let (hx, hy) = (sx * k::BLIP, sy * k::BLIP);
                out.push(RadarPiece::Rect {
                    texture,
                    rect: [bx - hx, by - hy, bx + hx, by + hy],
                    uv: texel_uv(g, blip.texture, 0x50)?,
                    color: blip.color,
                });
            }
        }
        // The centre pulse (timer of the HUD record, restart Rust-owned).
        let (timer, guest_timer) = if let Some(animation) = native {
            (animation.pulse_seconds, 0.0)
        } else {
            let guest_timer = g.read_f32(hud + HUD_PULSE)?;
            let base = &mut self.pulse_base[slot];
            if guest_timer - *base < k::PULSE_RESTART - 0.001 {
                *base = 0.0;
            }
            (guest_timer - *base, guest_timer)
        };
        if timer > k::PULSE_MIN {
            let x = timer * k::PULSE_SPEED;
            let x = if x > 1.0 { 1.0 } else { x };
            let ring = table(g, cvtt(x * k::QUARTER * k::TO_INDEX + k::ROUND)) * r;
            let amp = table(g, cvtt(x * k::PI * k::TO_INDEX + k::ROUND));
            let colour = 0x7f7f_7f50u32;
            let alpha = ((colour & 0xff) as f64 * amp as f64) as i32 as u32;
            out.push(RadarPiece::Rect {
                texture: "hudradarpulse",
                rect: [
                    cx - ring * sx,
                    cy - ring * sy,
                    cx + ring * sx,
                    cy + ring * sy,
                ],
                uv: [0.0, 0.0, 1.0, 1.0],
                color: (colour & 0xffff_ff00) | (alpha & 0xff),
            });
            if native.is_none() && timer >= k::PULSE_END {
                self.pulse_base[slot] = guest_timer - k::PULSE_RESTART;
            }
        }
        Some(out)
    }
}

/// `0x1339E0`: angle and distance of `position` relative to the viewer.
fn blip<G: GuestRead + ?Sized>(
    g: &G,
    viewer: u32,
    position: u32,
    color: u32,
    texture: u32,
    wide: bool,
) -> Option<Blip> {
    Some(blip_from_positions(
        [g.read_f32(viewer + 0xa8)?, g.read_f32(viewer + 0xb0)?],
        g.read_f32(viewer + 0x100)?,
        [g.read_f32(position)?, g.read_f32(position + 8)?],
        color,
        texture,
        wide,
    ))
}

/// Native `0x1339E0` spatial producer. Coordinates are x/z; yaw is degrees.
pub fn blip_from_positions(
    viewer: [f32; 2],
    yaw_degrees: f32,
    position: [f32; 2],
    color: u32,
    texture: u32,
    wide: bool,
) -> Blip {
    let yaw = yaw_degrees * k::DEG_TO_RAD;
    let dx = position[0] - viewer[0];
    let dz = 0.0 - (position[1] - viewer[1]);
    let (dx64, dz64) = (dx as f64, dz as f64);
    let length = (dz64 * dz64 + dx64 * dx64).sqrt();
    let distance = (length * if wide { k::RANGE_WIDE } else { k::RANGE } as f64) as f32;
    let angle = (dz64.atan2(dx64) - yaw as f64) as f32;
    Blip {
        angle,
        distance,
        color,
        texture,
    }
}

/// Bearing (degrees) the sweep turns to: the locked target (`+0x90` handle of the viewer's object extension)
/// when it is a character closer than 50 units, else 0.
fn target_bearing<G: GuestRead + ?Sized>(g: &G, ext: u32, yaw: f32) -> Option<f32> {
    use radar_addr::*;
    let handle = g.read32(ext + TARGET_HANDLE)?;
    if handle == 0 {
        return Some(0.0);
    }
    let entry = g
        .read32(HANDLE_POOL)?
        .wrapping_add((handle >> 22) * HANDLE_ENTRY_SIZE);
    // A stale handle: the original clears `+0x90` (the only guest write it makes here); natively it stays.
    if g.read32(entry + 0x2c)? != handle {
        return Some(0.0);
    }
    if g.read32(entry + 0x20)? != 8 || entry == g.read32(ext)? {
        return Some(0.0);
    }
    let other = g.read32(entry + 0x7c)?;
    if g.read32(other + 0x24)? & 8 != 0 {
        return Some(0.0);
    }
    let dz = g.read_f32(other + 0xbbc)? - g.read_f32(ext + 0xbbc)?;
    let dx = g.read_f32(other + 0xbb4)? - g.read_f32(ext + 0xbb4)?;
    if !(k::TARGET_RANGE_SQ > dz * dz + dx * dx) {
        return Some(0.0);
    }
    Some((yaw as f64 - (dx as f64).atan2(dz as f64) * k::RAD_TO_DEG as f64) as f32)
}

/// Texture coordinates of `0x1B7B0` for `units` (1/16 texel) on texture `id`: units / (16 * record size).
fn texel_uv<G: GuestRead + ?Sized>(g: &G, id: u32, units: u32) -> Option<[f32; 4]> {
    use radar_addr::*;
    let handle = g.read32(TEXTURE_ROWS + id * 24)?;
    let record = g
        .read32(g.read32(TEXTURE_RECORDS_PTR)?)?
        .wrapping_add(handle.wrapping_mul(0x34));
    let width = g.read16(record + 0x26)? as f32;
    let height = g.read16(record + 0x2a)? as f32;
    if width <= 0.0 || height <= 0.0 {
        return Some([0.0, 0.0, 1.0, 1.0]);
    }
    let u = units as f32 * (1.0 / (width * k::TEXEL_UNITS));
    let v = units as f32 * (1.0 / (height * k::TEXEL_UNITS));
    Some([0.0, 0.0, u, v])
}

#[cfg(test)]
mod tests {
    //! Tier 0, FABRICATED guest values on the real offsets.
    use super::*;
    use std::collections::HashMap;

    struct Guest(HashMap<u32, u32>);
    impl GuestRead for Guest {
        fn read32(&self, va: u32) -> Option<u32> {
            Some(self.0.get(&va).copied().unwrap_or(0))
        }
    }
    impl GuestCall for Guest {
        fn call(&self, va: u32, _args: &[u32]) -> Option<u32> {
            Some(self.0.get(&(0xf000_0000 + va)).copied().unwrap_or(0))
        }
    }

    const VIEW: u32 = 0x0100_0000;
    const OBJECT: u32 = 0x0200_0000;
    const EXT: u32 = 0x0300_0000;
    const PLAYERS: u32 = 0x0400_0000;
    const OTHER: u32 = 0x0500_0000;
    const VIEWPORT: u32 = 0x0600_0000;

    fn f(v: f32) -> u32 {
        v.to_bits()
    }

    /// Viewer at the origin facing yaw 0, one other player 25 units along +x (distance 0.5 of the rim).
    fn guest() -> Guest {
        use radar_addr::*;
        let mut m = HashMap::new();
        m.insert(radar_addr::VIEWER, VIEW);
        m.insert(VIEW, 0); // own player index
        m.insert(VIEW + 4, 0); // slot
        m.insert(VIEW + 0x14, OBJECT);
        m.insert(OBJECT + 0x7c, EXT);
        m.insert(FRAME_RATE, 60);
        m.insert(CLOCK, 0);
        m.insert(TIME_SCALE, f(1.0));
        for i in 0..2048u32 {
            m.insert(
                SIN_TABLE + 4 * i,
                f((i as f32 * std::f32::consts::TAU / 2048.0).sin()),
            );
        }
        m.insert(PLAYER_COUNT, 2);
        m.insert(PLAYERS_PTR, PLAYERS);
        m.insert(PLAYERS + PLAYER_SIZE, OTHER);
        m.insert(PLAYERS + PLAYER_SIZE + 0x18, 1);
        m.insert(PLAYERS + PLAYER_SIZE + 0xa58, 0x7f00_0060);
        m.insert(MODE_FLAGS, 1);
        m.insert(OTHER + 0x64, f(25.0));
        m.insert(0xf000_0000 + FN_CONNECTED, 1);
        m.insert(0xf000_0000 + FN_MODE_TABLE, 1);
        m.insert(VIEWPORT_PTR, VIEWPORT);
        m.insert(VIEWPORT + 4, 640);
        m.insert(VIEWPORT + 0x14, 0);
        m.insert(HUD_RIGHT, 32);
        m.insert(HUD_TOP, 24);
        m.insert(SCALE_X, f(1.0));
        m.insert(SCALE_Y, f(1.0));
        m.insert(SCALE_RIM, f(1.0));
        Guest(m)
    }

    fn pieces(call: RadarCall) -> Vec<RadarPiece> {
        match call {
            RadarCall::Draw(p) => p,
            other => panic!("expected a draw, got {other:?}"),
        }
    }

    #[test]
    fn radar_replays_ring_sweep_dot_and_blips() {
        let mut g = guest();
        let mut state = RadarState::default();
        let p = pieces(state.read(&g));
        // Centre (640 - 32 - 35, 0 + 24 + 35) = (573, 59), radius 35.
        assert_eq!(
            p[0],
            RadarPiece::Rect {
                texture: "hudradar",
                rect: [538.0, 24.0, 608.0, 94.0],
                uv: [0.0, 0.0, 1.0, 1.0],
                color: 0x7f7f_7f3f
            }
        );
        assert!(matches!(
            p[1],
            RadarPiece::Quad {
                texture: "hudradarcentre",
                ..
            }
        ));
        assert!(matches!(
            p[2],
            RadarPiece::Rect {
                texture: "radarspot",
                rect: [571.0, 57.0, 575.0, 61.0],
                ..
            }
        ));
        // The other player: angle atan2(-0, 25) = 0, distance 0.5: x = cx - cos(0) * 0.5 * 35, y = cy.
        let RadarPiece::Rect {
            texture,
            rect,
            color,
            ..
        } = &p[3]
        else {
            panic!("blip {:?}", p[3])
        };
        assert_eq!((*texture, *color), ("radarspot", 0x7f00_0060));
        assert!(
            (rect[0] + 2.5 - (573.0 - 17.5)).abs() < 1e-3 && (rect[1] + 2.5 - 59.0).abs() < 1e-3,
            "{rect:?}"
        );
        assert_eq!(p.len(), 4, "no pulse while the timer is 0");
        // Out of range (100 units = distance 2): a rim marker, a rotated quad outside the inner ring.
        g.0.insert(OTHER + 0x64, f(100.0));
        let p = pieces(state.read(&g));
        let RadarPiece::Quad {
            texture, corners, ..
        } = &p[3]
        else {
            panic!("rim {:?}", p[3])
        };
        assert_eq!(*texture, "hudradararrow");
        let mid_x = (corners[2][0] + corners[3][0]) / 2.0;
        assert!(
            (mid_x - 573.0).abs() > 30.0,
            "rim marker sits on the ring: {corners:?}"
        );
    }

    #[test]
    fn sweep_smooths_towards_the_bearing_and_is_rust_owned() {
        let g = guest();
        let mut state = RadarState::default();
        state.read(&g);
        // Seeded from the guest (0), moved towards -45 (= 315): 0 + (-45) * (1 / 60) * 8 = -6, wrapped 354.
        let first = state.sweep[0].expect("seeded");
        assert!((first - 354.0).abs() < 1e-3, "{first}");
        state.read(&g);
        assert!(state.sweep[0].unwrap() < first);
        state.reset();
        assert_eq!(state.sweep[0], None);
    }

    #[test]
    fn pulse_restart_is_an_offset_not_a_guest_write() {
        let mut g = guest();
        let hud = radar_addr::HUD_RECORDS;
        let mut state = RadarState::default();
        g.0.insert(hud + radar_addr::HUD_PULSE, f(0.7));
        let p = pieces(state.read(&g));
        let RadarPiece::Rect { texture, color, .. } = p.last().unwrap() else {
            panic!()
        };
        assert_eq!(*texture, "hudradarpulse");
        // x = 0.5: amplitude sin(pi / 2) = 1, alpha 0x50.
        assert_eq!(*color & 0xff, 0x50);
        g.0.insert(hud + radar_addr::HUD_PULSE, f(1.5));
        state.read(&g);
        assert!(
            (state.pulse_base[0] - 2.0).abs() < 1e-6,
            "restart at -0.5: {}",
            state.pulse_base[0]
        );
        // The guest timer keeps counting: 1.6 is 0.1 after the restart (-0.4), no pulse drawn.
        g.0.insert(hud + radar_addr::HUD_PULSE, f(1.6));
        assert!(!pieces(state.read(&g)).iter().any(|p| matches!(
            p,
            RadarPiece::Rect {
                texture: "hudradarpulse",
                ..
            }
        )));
    }

    #[test]
    fn undecoded_modes_stay_with_the_original() {
        let mut g = guest();
        g.0.insert(radar_addr::MODE_FLAGS, 0x40);
        g.0.insert(radar_addr::MODE_C, 0xb);
        assert_eq!(
            RadarState::default().read(&g),
            RadarCall::Legacy("tag-search mode")
        );
        let mut g = guest();
        g.0.insert(0xf000_0000 + radar_addr::FN_RECOLOUR_MODE, 1);
        assert_eq!(
            RadarState::default().read(&g),
            RadarCall::Legacy("recolour mode")
        );
        let mut g = guest();
        g.0.insert(VIEW + 4, u32::MAX);
        assert_eq!(RadarState::default().read(&g), RadarCall::Nothing);
    }

    #[test]
    fn owned_radar_does_not_read_guest_animation_or_world() {
        struct Inputs(Guest);
        impl GuestRead for Inputs {
            fn read32(&self, va: u32) -> Option<u32> {
                assert!(
                    !(radar_addr::HUD_RECORDS
                        ..radar_addr::HUD_RECORDS + radar_addr::HUD_RECORD_SIZE * 4)
                        .contains(&va)
                );
                assert!(!(radar_addr::SWEEP_ANGLES..radar_addr::SWEEP_ANGLES + 16).contains(&va));
                assert!(va != VIEW + 0x14 && va != VIEW + 0x100);
                self.0.read32(va)
            }
        }
        impl GuestCall for Inputs {
            fn call(&self, va: u32, args: &[u32]) -> Option<u32> {
                self.0.call(va, args)
            }
        }
        let g = guest();
        let mut legacy = RadarState::default();
        let expected = pieces(legacy.read(&g));
        let mut animation = RadarAnimation::default();
        animation.advance(0.0, 1.0 / 60.0, 0.0);
        let blips = [blip_from_positions(
            [0.0, 0.0],
            0.0,
            [25.0, 0.0],
            0x7f00_0060,
            0xfe,
            false,
        )];
        let actual = RadarState::default()
            .draw_owned(&Inputs(g), &animation, &blips)
            .unwrap();
        assert_eq!(actual, expected);
        let before = animation;
        RadarState::default()
            .draw_owned(&Inputs(guest()), &animation, &blips)
            .unwrap();
        assert_eq!(animation, before, "rendering is read-only");
        animation.pulse_seconds = 1.5;
        animation.finish_frame();
        assert_eq!(animation.pulse_seconds, -0.5);
    }

    #[test]
    fn period_difference_takes_the_short_way() {
        assert_eq!(period_difference(10.0, 350.0, 0.0, 360.0), -20.0);
        assert_eq!(period_difference(350.0, 10.0, 0.0, 360.0), 20.0);
        assert_eq!(period_difference(10.0, 40.0, 0.0, 360.0), 30.0);
    }
}

//! T2315 increment 4: typed views of the guest data the in-game BACK pages draw (the Arcade scoreboard `0x2E72E0` and the
//! Story objectives `0x2F5B30` / `0x2F45F0`), read at the frame boundary into `tsfp_ui_model::ingame_rows`.
//!
//! Addresses MEASURED by static reading of the owner's XBE (docs/t2315-ui-page-completion.md, increment 4). Game modes
//! whose branches lack live admission return `None`, so the page keeps the original draw. Offline timed subtype6
//! team subtype4, type3/subtype16, type5/subtype12, type9/subtype19 and five-lives
//! type9/subtype9 have measured readback admission; other variants remain gated (docs/t2316-ui-leftovers.md).

use crate::live_page::{LivePage, style_height, widget_id};
use crate::native_page::GuestUi;
use tsfp_ui_model::ingame_rows::{
    LineTint, OBJECTIVES_ROW_DRAW, Objective, ObjectiveGroup, ObjectiveStatus, Objectives,
    RankedPlayer, SCOREBOARD_ROW_DRAW, ScoreLine, ScoreMode, ScoreName, ScoreText, Scoreboard,
    select_score_rows, time_parts,
};
use tsfp_ui_model::list_rows::{Edges, IngameRow};
use tsfp_ui_model::skin::PackedColor;

/// Guest addresses (MEASURED from code).
pub mod addr {
    /// `[0x7BA038]` text unit, `[0x7BA188]` line unit.
    pub const TEXT_UNIT: u32 = 0x7B_A038;
    pub const LINE_UNIT: u32 = 0x7B_A188;
    /// `0x3533F0`: a network session is active.
    pub const SESSION: u32 = 0x76_B108;
    /// `0x1CDAE0` -> `0x3567C0` without a session: `[0x790950]`, or `[0x5236C4]` above 4.
    pub const PLAYERS: u32 = 0x79_0950;
    pub const PLAYERS_CAP: u32 = 0x52_36C4;
    /// Extra (bot) entries of the rank list: the scoreboard loops `[0x790950] + [0x7A2958]` times.
    pub const BOTS: u32 = 0x7A_2958;
    /// Rank order (i32 player indices, negative = none) and the player records `[0x7356D8] + index * 0xEA0`.
    pub const RANK_ORDER: u32 = 0x7D_E4C0;
    pub const PLAYER_TABLE: u32 = 0x73_56D8;
    pub const PLAYER_STRIDE: u32 = 0xEA0;
    pub const PLAYER_ID: u32 = 0x08;
    /// Pointer to the name (`%s` of the word).
    pub const PLAYER_NAME: u32 = 0x70;
    pub const PLAYER_RANK: u32 = 0xA68;
    pub const PLAYER_SCORE: u32 = 0xA6C;
    /// The local player: `[[[0x7B0C7C] + 0x14] + 0x7C]`.
    pub const GAME: u32 = 0x7B_0C7C;
    /// Game type byte, sub type byte, flags word, lives word.
    pub const GAME_TYPE: u32 = 0x7D_E455;
    pub const GAME_SUBTYPE: u32 = 0x7D_E456;
    pub const GAME_FLAGS: u32 = 0x7D_E458;
    pub const GAME_LIVES: u32 = 0x7D_E474;
    /// `[0x79094C]` 0x64: Map Maker test play (labels from `0x33EDA0`).
    pub const FRONT_MODE: u32 = 0x79_094C;
    /// `0x746C0`: UI draw mode; `0x2F5E10` suppresses its row in modes 3, 4 and 6.
    pub const UI_DRAW_MODE: u32 = 0x6F_73E8;
    /// Objective records `[0x78CECC] + id * 0x4C`, the listed ids `[[0x75C14C] + n * 4]` for n below `[0x78CEC8]`,
    /// active counters `[0x75C134 + kind * 4]`, the "count listed" switch `[0x6D6154]`.
    pub const OBJECTIVE_TABLE: u32 = 0x78_CECC;
    pub const OBJECTIVE_STRIDE: u32 = 0x4C;
    pub const OBJECTIVE_COUNT: u32 = 0x78_CEC8;
    pub const OBJECTIVE_ORDER: u32 = 0x75_C14C;
    pub const OBJECTIVE_COUNTERS: u32 = 0x75_C134;
    pub const OBJECTIVE_LISTED: u32 = 0x6D_6154;
    /// Row colour table pointer (`0x75F60`): 8 bytes per entry.
    pub const ROW_COLORS: u32 = 0x90;
}

/// Score value meaning "no score" (`0x2E7080` prints `-`).
pub const NO_SCORE: i32 = 0x8000_0001u32 as i32;

fn byte(g: &dyn GuestUi, va: u32) -> u8 {
    g.read32(va & !3).map_or(0, |w| (w >> ((va & 3) * 8)) as u8)
}

fn f32_at(g: &dyn GuestUi, va: u32) -> f32 {
    f32::from_bits(g.read32(va).unwrap_or(0))
}

fn cstring(g: &dyn GuestUi, va: u32, max: u32) -> Vec<u8> {
    (0..max)
        .map(|i| byte(g, va + i))
        .take_while(|b| *b != 0)
        .collect()
}

/// `0x1CDAE0` without a session.
pub fn player_count(g: &dyn GuestUi) -> i32 {
    let count = g.read32(addr::PLAYERS).unwrap_or(0) as i32;
    if count > 4 {
        g.read32(addr::PLAYERS_CAP).unwrap_or(0) as i32
    } else {
        count
    }
}

/// The scoreboard text style of `0x2E72E0`: 8 with one player, 3 with two, else 15.
pub fn scoreboard_style(players: i32) -> u32 {
    match players {
        1 => 8,
        2 => 3,
        _ => 15,
    }
}

/// Admission covers measured offline scoreboard slices only. Other settings retain the original draw.
fn verified_scoreboard_variant(game_type: u8, subtype: u8, flags: u32, lives: u32) -> bool {
    match (game_type, subtype, lives) {
        (1, 6, 0) | (5, 12, 0) | (9, 19, 0) | (9, 9, 5) => flags & 1 == 0,
        (9, 4, 0) | (3, 16, 0) => flags & 1 != 0,
        _ => false,
    }
}

/// The scoreboard lines (`0x2E72E0` with pass0); unverified variants retain legacy drawing.
pub fn read_scoreboard(
    g: &dyn GuestUi,
    area: Edges,
    widget: tsfp_ui_model::WidgetId,
) -> Option<Scoreboard> {
    let game_type = byte(g, addr::GAME_TYPE);
    let subtype = byte(g, addr::GAME_SUBTYPE);
    let flags = g.read32(addr::GAME_FLAGS).unwrap_or(0);
    let lives = g.read32(addr::GAME_LIVES).unwrap_or(0);
    let gated = matches!(game_type, 1 | 3 | 5)
        || matches!(subtype, 0xC | 0x13)
        || flags & 1 != 0
        || lives != 0;
    if g.read32(addr::SESSION).unwrap_or(0) != 0
        || (gated
            && !verified_scoreboard_variant(game_type, subtype, flags, lives)
            && std::env::var("TSFP_UI_SCOREBOARD_VARIANTS").as_deref() != Ok("1"))
    {
        return None;
    }
    read_scoreboard_variants(g, area, widget)
}

/// Runtime string id boundary of `0x3D7B0`. Missing banks decline instead of fabricating text.
fn scoreboard_string(g: &dyn GuestUi, id: u32) -> Option<Vec<u8>> {
    use crate::live_page::draw_fields as f;
    let story = g.read32(f::STORY_BANK).filter(|p| *p != 0)?;
    let pointer = if id >= f::STORY_FIRST_ID {
        g.read32(story.checked_add((id - f::STORY_FIRST_ID).checked_mul(4)?)?)?
    } else if id == 0 {
        0x4766D8
    } else {
        let common = g.read32(f::COMMON_BANK).filter(|p| *p != 0)?;
        g.read32(common.checked_add(id.checked_mul(4)?)?)?
    };
    checked_string(g, pointer, 1024)
}

fn checked_string(g: &dyn GuestUi, pointer: u32, max: u32) -> Option<Vec<u8>> {
    if pointer == 0 {
        return None;
    }
    let mut bytes = Vec::new();
    for n in 0..max {
        let va = pointer.checked_add(n)?;
        let byte = (g.read32(va & !3)? >> ((va & 3) * 8)) as u8;
        if byte == 0 {
            return Some(bytes);
        }
        bytes.push(byte);
    }
    None
}

/// Runtime decimal printf subset: width/zero padding plus %% and %d.
/// Unknown specifiers decline the original preset; formats are never embedded game strings.
fn scoreboard_format(format: &[u8], numbers: &[i32]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut args = numbers.iter();
    let mut i = 0;
    while i < format.len() {
        let code = format[i];
        i += 1;
        if code != b'%' {
            out.push(code);
            continue;
        }
        if format.get(i) == Some(&b'%') {
            out.push(b'%');
            i += 1;
            continue;
        }
        let zero = format.get(i) == Some(&b'0');
        let mut width = 0usize;
        while let Some(c @ b'0'..=b'9') = format.get(i) {
            width = width.checked_mul(10)?.checked_add((c - b'0') as usize)?;
            if width > 32 {
                return None;
            }
            i += 1;
        }
        if format.get(i) != Some(&b'd') {
            return None;
        }
        i += 1;
        let number = args.next()?.to_string();
        let pad = width.saturating_sub(number.len());
        if zero && number.starts_with('-') {
            out.push(b'-');
            out.extend(std::iter::repeat_n(b'0', pad));
            out.extend_from_slice(&number.as_bytes()[1..]);
        } else {
            out.extend(std::iter::repeat_n(if zero { b'0' } else { b' ' }, pad));
            out.extend_from_slice(number.as_bytes());
        }
    }
    if args.next().is_some() {
        return None;
    }
    Some(out)
}

fn scoreboard_score(g: &dyn GuestUi, score: ScoreText) -> Option<Vec<u8>> {
    match score {
        ScoreText::Number(n) => Some(n.to_string().into_bytes()),
        ScoreText::Dash => Some(b"-".to_vec()),
        ScoreText::Text(id) => scoreboard_string(g, id),
        ScoreText::Lives(n) => {
            let mut bytes = scoreboard_format(&checked_string(g, 0x476AC8, 64)?, &[n])?;
            bytes.extend(scoreboard_string(g, if n == 1 { 0x6C6 } else { 0x6C5 })?);
            Some(bytes)
        }
        ScoreText::Time(0) => scoreboard_string(g, 0xE93),
        ScoreText::Time(frames) => {
            let fps = g.read32(0x4E8CEC)? as i32;
            let (minutes, seconds, tenths) = time_parts(frames, fps)?;
            scoreboard_format(
                &checked_string(g, 0x478834, 64)?,
                &[minutes, seconds, tenths],
            )
        }
    }
}

/// `0xD6C90`'s character category table is read from runtime data, never copied.
fn scoreboard_hidden_bot(g: &dyn GuestUi, character: i32) -> Option<bool> {
    let index = character.wrapping_sub(0x5D) as u32;
    if index > 0x3E {
        return Some(false);
    }
    let va = 0xD6CBC + index;
    let class = (g.read32(va & !3)? >> ((va & 3) * 8)) as u8;
    match g.read32(0xD6CB4 + u32::from(class) * 4)? {
        0xD6CAA => Some(true),
        0xD6CB0 => Some(false),
        _ => None,
    }
}

/// Complete offline pass0 snapshot adapter. Explicitly callable for validation; the live
/// reader keeps its historical admission gate until each variant has frame/trace evidence.
/// Session player eligibility/count is a separate provider contract, so sessions still decline.
pub fn read_scoreboard_variants(
    g: &dyn GuestUi,
    area: Edges,
    widget: tsfp_ui_model::WidgetId,
) -> Option<Scoreboard> {
    if g.read32(addr::SESSION).unwrap_or(0) != 0 {
        return None;
    }
    let mode = ScoreMode {
        game_type: byte(g, addr::GAME_TYPE),
        sub_type: byte(g, addr::GAME_SUBTYPE),
        lives_mode: g.read32(addr::GAME_LIVES).unwrap_or(0),
        timed_mask: g.read32(0x7A2BC8).unwrap_or(0),
    };
    let style = scoreboard_style(player_count(g));
    let height = style_height(g, style)?;
    let local = g
        .read32(addr::GAME)
        .filter(|p| *p != 0)
        .and_then(|game| g.read32(game + 0x14))
        .filter(|p| *p != 0)
        .and_then(|p| g.read32(p + 0x7C))
        .filter(|p| *p != 0)
        .and_then(|p| g.read32(p + addr::PLAYER_ID));
    let table = g.read32(addr::PLAYER_TABLE).filter(|t| *t != 0)?;
    let humans = g.read32(addr::PLAYERS).unwrap_or(0);
    let bots = g.read32(addr::BOTS).unwrap_or(0);
    let total = (humans as i32).checked_add(bots as i32)?;
    if !(0..=32).contains(&total) {
        return None;
    }
    let mut players = Vec::new();
    for n in 0..total as u32 {
        let index = g.read32(addr::RANK_ORDER + 4 * n)? as i32;
        if index < 0 {
            continue;
        }
        if index >= 32 {
            return None;
        }
        let record = table.checked_add(index as u32 * addr::PLAYER_STRIDE)?;
        let id = g.read32(record + addr::PLAYER_ID)?;
        let hidden_bot = if mode.sub_type == 0x13 && id >= humans {
            scoreboard_hidden_bot(g, g.read32(record + 0x48)? as u16 as i16 as i32)?
        } else {
            false
        };
        players.push(RankedPlayer {
            id,
            rank: g.read32(record + addr::PLAYER_RANK)? as i32,
            score: g.read32(record + addr::PLAYER_SCORE)? as i32,
            alternate_score: if mode.game_type == 1 && g.read32(record + 0x80)? != 0 {
                Some(g.read32(record + 0xA64)? as i32)
            } else {
                None
            },
            lives: g.read32(record + 0xA70)? as i32,
            team: g.read32(record + 0x84)?,
            name: g
                .read32(record + addr::PLAYER_NAME)
                .filter(|p| *p != 0)
                .map_or_else(Vec::new, |p| cstring(g, p, 32)),
            eligible: true,
            hidden_bot,
        });
    }
    let palette = |team: u32| {
        if !(1..=4).contains(&team) {
            return None;
        }
        g.read32(0x4D1E3C + 20 * (team - 1)).map(PackedColor)
    };
    let selected = select_score_rows(
        &players,
        mode,
        g.read32(addr::GAME_FLAGS).unwrap_or(0) & 1 != 0,
        local,
        humans,
        0,
        &palette,
    )?;
    let mut lines = Vec::new();
    for row in selected {
        let name = match row.name {
            ScoreName::Player(bytes) => bytes,
            ScoreName::Team { team, members } => {
                let mut bytes = scoreboard_string(g, 0xB2B + team)?;
                bytes.extend(scoreboard_format(
                    &checked_string(g, 0x49F6E0, 64)?,
                    &[members as i32],
                )?);
                bytes
            }
        };
        if row.digit_mode > 0 {
            use crate::live_page::draw_fields as f;
            let record = g.read32(f::STYLES + style * f::STYLE_STRIDE)?;
            let advance = f32_at(g, 0x4C8C28 + record.checked_mul(0x38)? + 0x1C);
            if !advance.is_finite() || advance <= 0.0 {
                return None;
            }
        }
        lines.push(ScoreLine {
            rank: row.rank,
            score: scoreboard_score(g, row.score)?,
            fixed_spacing: row.digit_mode > 0,
            name,
            tint: row.tint,
        });
    }
    Some(Scoreboard {
        widget,
        area,
        text_unit: f32_at(g, addr::TEXT_UNIT),
        line_unit: f32_at(g, addr::LINE_UNIT),
        line_height: (height + 1.0).trunc(),
        font: style,
        lines,
    })
}

/// The header text of objective kind `kind` (`0x227FB0`): singular when its counter is 1.
pub fn objective_header(g: &dyn GuestUi, kind: u32) -> u32 {
    let single = g.read32(addr::OBJECTIVE_COUNTERS + 4 * kind) == Some(1);
    match (kind, single) {
        (0, true) => 0x15F3,
        (0, false) => 0x15F4,
        (_, true) => 0x15F5,
        (_, false) => 0x15F6,
    }
}

/// Snapshot of callback-returned counts, owned by Rust for one guest frame.
/// No values are fabricated when the call boundary refuses a callback.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProgressSamples {
    pub values: std::collections::BTreeMap<u32, (i32, i32)>,
}

impl ProgressSamples {
    /// Include dynamic counts in the page-cache key, not just the guest record fields.
    pub fn checksum(&self) -> u32 {
        self.values
            .iter()
            .fold(0, |hash, (record, (current, total))| {
                hash.rotate_left(5)
                    ^ record
                    ^ (*current as u32).rotate_left(11)
                    ^ (*total as u32).rotate_left(19)
            })
    }
}

fn objectives_supported(g: &dyn GuestUi) -> bool {
    g.read32(addr::SESSION).unwrap_or(0) == 0
        && byte(g, addr::GAME_TYPE) != 3
        && g.read32(addr::FRONT_MODE) != Some(0x64)
        && !matches!(g.read32(addr::UI_DRAW_MODE), Some(3 | 4 | 6))
}

fn shown_objectives(g: &dyn GuestUi, kind: u32) -> Vec<u32> {
    let listed = g.read32(addr::OBJECTIVE_LISTED).unwrap_or(0) != 0;
    let table = g.read32(addr::OBJECTIVE_TABLE).unwrap_or(0);
    let order = g.read32(addr::OBJECTIVE_ORDER).unwrap_or(0);
    let records: Vec<u32> = (0..g.read32(addr::OBJECTIVE_COUNT).unwrap_or(0).min(256))
        .filter_map(|n| g.read32(order + 4 * n))
        .map(|id| table.wrapping_add(id.wrapping_mul(addr::OBJECTIVE_STRIDE)))
        .filter(|r| byte(g, r + 8) as u32 == kind && (byte(g, r + 0xB) != 1 || listed))
        .collect();
    let shown = if listed {
        records.len()
    } else {
        (g.read32(addr::OBJECTIVE_COUNTERS + 4 * kind).unwrap_or(0) as i32).max(0) as usize
    };
    records.into_iter().take(shown).collect()
}

/// `0x227360`: sample both no-argument callbacks in current/total order.
/// Called only on the guest frame thread, once per shown active objective per sample.
/// The callbacks are a handler boundary, not pad input or guest state writes.
pub fn sample_progress(g: &mut dyn GuestUi, live: &LivePage) -> Option<ProgressSamples> {
    let mut samples = ProgressSamples::default();
    if !objectives_supported(g)
        || !live
            .rows
            .iter()
            .any(|r| r.draw == OBJECTIVES_ROW_DRAW && r.rect.is_some())
    {
        return Some(samples);
    }
    for kind in 0..2 {
        for record in shown_objectives(g, kind) {
            if !matches!(byte(g, record + 0xB), 2 | 3) || byte(g, record + 9) & 1 == 0 {
                continue;
            }
            let current = g.read32(record + 0x40).unwrap_or(0);
            let total = g.read32(record + 0x44).unwrap_or(0);
            // The original uses stored counters unless BOTH callbacks are present.
            if current == 0 || total == 0 {
                continue;
            }
            let current = g.call(current, &[])? as i32;
            let total = g.call(total, &[])? as i32;
            samples.values.insert(record, (current, total));
        }
    }
    Some(samples)
}

/// The objectives of `0x2F5B30` (kinds 0 and 1, `0x227A70` / `0x227AD0`), None for the modes not ported.
pub fn read_objectives(
    g: &dyn GuestUi,
    area: Edges,
    widget: tsfp_ui_model::WidgetId,
    header_color: PackedColor,
) -> Option<Objectives> {
    read_objectives_with_progress(g, area, widget, header_color, None)
}

pub fn read_objectives_with_progress(
    g: &dyn GuestUi,
    area: Edges,
    widget: tsfp_ui_model::WidgetId,
    header_color: PackedColor,
    samples: Option<&ProgressSamples>,
) -> Option<Objectives> {
    if !objectives_supported(g) {
        return None;
    }
    let split = player_count(g) > 2;
    let (header_font, item_font) = if split { (3, 18) } else { (9, 3) };
    let listed = g.read32(addr::OBJECTIVE_LISTED).unwrap_or(0) != 0;
    let table = g.read32(addr::OBJECTIVE_TABLE).unwrap_or(0);
    let order = g.read32(addr::OBJECTIVE_ORDER).unwrap_or(0);
    let count = g.read32(addr::OBJECTIVE_COUNT).unwrap_or(0).min(256);
    let mut groups = Vec::new();
    for kind in 0..2u32 {
        let records: Vec<u32> = (0..count)
            .filter_map(|n| g.read32(order + 4 * n))
            .map(|id| table.wrapping_add(id.wrapping_mul(addr::OBJECTIVE_STRIDE)))
            .filter(|r| byte(g, r + 8) as u32 == kind && (byte(g, r + 0xB) != 1 || listed))
            .collect();
        let shown = if listed {
            records.len() as i32
        } else {
            g.read32(addr::OBJECTIVE_COUNTERS + 4 * kind).unwrap_or(0) as i32
        };
        let mut items = Vec::new();
        for record in records.into_iter().take(shown.max(0) as usize) {
            let status = match byte(g, record + 0xB) {
                2 | 3 => {
                    let progress = if byte(g, record + 9) & 1 != 0 {
                        // Callbacks are sampled on the guest thread; this reader only consumes the owned values.
                        if g.read32(record + 0x40).unwrap_or(0) != 0
                            && g.read32(record + 0x44).unwrap_or(0) != 0
                        {
                            Some(*samples?.values.get(&record)?)
                        } else {
                            Some((
                                g.read32(record + 0x2C).unwrap_or(0) as i32,
                                g.read32(record + 0x30).unwrap_or(0) as i32,
                            ))
                        }
                    } else {
                        None
                    };
                    ObjectiveStatus::Active { progress }
                }
                4 => ObjectiveStatus::Failed,
                5 => ObjectiveStatus::Done,
                _ => ObjectiveStatus::Hidden,
            };
            let label = g.read32(record).map(|w| w >> 16)?;
            items.push(Objective { label, status });
        }
        if shown > 0 {
            groups.push(ObjectiveGroup {
                header: objective_header(g, kind),
                items,
            });
        }
    }
    Some(Objectives {
        widget,
        area,
        text_unit: f32_at(g, addr::TEXT_UNIT),
        header_font,
        header_height: style_height(g, header_font)?,
        item_font,
        item_height: style_height(g, item_font)?,
        header_color,
        empty_header: objective_header(g, 0),
        groups,
    })
}

/// The in-game custom row of `live`, None when it has none or its mode is not ported.
pub fn read(g: &dyn GuestUi, live: &LivePage) -> Option<IngameRow> {
    read_with_progress(g, live, None)
}

pub fn read_with_progress(
    g: &dyn GuestUi,
    live: &LivePage,
    samples: Option<&ProgressSamples>,
) -> Option<IngameRow> {
    let (index, row) = live
        .rows
        .iter()
        .enumerate()
        .find(|(_, r)| r.draw == SCOREBOARD_ROW_DRAW || r.draw == OBJECTIVES_ROW_DRAW)?;
    let [x0, y0, x1, y1] = row.rect?;
    let area = Edges::new(x0, y0, x1, y1);
    let widget = widget_id(index);
    if row.draw == SCOREBOARD_ROW_DRAW {
        return read_scoreboard(g, area, widget).map(IngameRow::Scoreboard);
    }
    // `0x75F60` first colour, entry 0 of the row colour table. INFERRED from the legacy picture (white header): the page's
    // only row is its cursor row, whose entry 1 pulses in the skin's focus colours (looked at), so the cursor-row rule of `0x75F60`
    // does not apply here (`0x75F40` / the row flags not decoded for this page).
    let table = g.read32(row.node + addr::ROW_COLORS).filter(|t| *t != 0);
    let color = PackedColor(table.and_then(|t| g.read32(t)).unwrap_or(0x7F7F_7F7F));
    if std::env::var_os("TSFP_UI_ROW_GEOM").is_some() {
        let table: Vec<String> = (0..12)
            .map(|n| {
                format!(
                    "{:08x}",
                    table.and_then(|t| g.read32(t + 4 * n)).unwrap_or(0)
                )
            })
            .collect();
        eprintln!(
            "tsfp-ui: objectives row colours {} page flags {:#x} row flags {:#x}",
            table.join(" "),
            g.read32(live_record(live) + 0x40).unwrap_or(0),
            row.flags
        );
    }
    read_objectives_with_progress(g, area, widget, color, samples).map(IngameRow::Objectives)
}

fn live_record(live: &LivePage) -> u32 {
    crate::menu_state::layout::PAGE_TABLE + live.slot * crate::menu_state::layout::PAGE_STRIDE
}

/// A word that changes when the scoreboard or objectives change (scores, ranks, statuses), for the page cache.
pub fn checksum(g: &dyn GuestUi) -> u32 {
    let mut sum = 2_166_136_261u32;
    for va in [
        addr::SESSION,
        addr::PLAYERS,
        addr::PLAYERS_CAP,
        addr::BOTS,
        addr::GAME_TYPE & !3,
        addr::GAME_FLAGS,
        addr::GAME_LIVES,
        addr::UI_DRAW_MODE,
        0x7A2BC8,
        0x4E8CEC,
    ] {
        sum = sum.wrapping_mul(16_777_619) ^ g.read32(va).unwrap_or(0);
    }
    for team in 0..4 {
        sum = sum.wrapping_mul(16_777_619) ^ g.read32(0x4D1E3C + 20 * team).unwrap_or(0);
    }
    if let Some(table) = g.read32(addr::PLAYER_TABLE).filter(|t| *t != 0) {
        for n in 0..32u32 {
            let index = g.read32(addr::RANK_ORDER + 4 * n).unwrap_or(u32::MAX) as i32;
            sum = sum.wrapping_mul(16_777_619) ^ index as u32;
            if index >= 0 && (index as u32) < 32 {
                let record = table.wrapping_add(index as u32 * addr::PLAYER_STRIDE);
                sum = sum.wrapping_mul(16_777_619)
                    ^ g.read32(record + addr::PLAYER_SCORE).unwrap_or(0);
                for offset in [
                    addr::PLAYER_RANK,
                    addr::PLAYER_ID,
                    0x48,
                    0x80,
                    0x84,
                    0xA64,
                    0xA70,
                    addr::PLAYER_NAME,
                ] {
                    sum = sum.wrapping_mul(16_777_619) ^ g.read32(record + offset).unwrap_or(0);
                }
                if let Some(name) = g.read32(record + addr::PLAYER_NAME).filter(|p| *p != 0) {
                    for byte in cstring(g, name, 32) {
                        sum = sum.wrapping_mul(16_777_619) ^ u32::from(byte);
                    }
                }
            }
        }
    }
    let table = g.read32(addr::OBJECTIVE_TABLE).unwrap_or(0);
    let order = g.read32(addr::OBJECTIVE_ORDER).unwrap_or(0);
    for n in 0..g.read32(addr::OBJECTIVE_COUNT).unwrap_or(0).min(64) {
        let record = table.wrapping_add(
            g.read32(order + 4 * n)
                .unwrap_or(0)
                .wrapping_mul(addr::OBJECTIVE_STRIDE),
        );
        sum = sum.wrapping_mul(16_777_619)
            ^ g.read32(record + 8).unwrap_or(0)
            ^ g.read32(record + 0x2C).unwrap_or(0)
            ^ g.read32(record + 0x30).unwrap_or(0).rotate_left(11)
            ^ g.read32(record + 0x40).unwrap_or(0).rotate_left(17)
            ^ g.read32(record + 0x44).unwrap_or(0).rotate_left(23);
    }
    sum
}

#[cfg(test)]
mod tests {
    //! Tier 0 against a FABRICATED guest memory (layout MEASURED from code).
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Mem(HashMap<u32, u32>);
    impl GuestUi for Mem {
        fn read32(&self, va: u32) -> Option<u32> {
            Some(*self.0.get(&va).unwrap_or(&0))
        }
        fn write32(&mut self, _: u32, _: u32) -> bool {
            false
        }
        fn call(&mut self, _: u32, _: &[u32]) -> Option<u32> {
            None
        }
    }

    fn styles(m: &mut Mem) {
        // style_height reads; any positive value will do: fill the style tables with 1.0 / record 0.
        use crate::live_page::draw_fields as f;
        for style in 0..32 {
            m.0.insert(f::STYLES + f::STYLE_STRIDE * style + 0xC, 1.0f32.to_bits());
        }
        m.0.insert(f::HEIGHT_LINES, 1);
        m.0.insert(0x4C8C44, 10.0f32.to_bits());
        for va in [
            f::RECORD_HEIGHTS,
            f::HEIGHT_SCALE_A,
            f::HEIGHT_SCALE_B,
            f::HEIGHT_SCALE_C,
        ] {
            m.0.insert(va, 18.0f32.sqrt().sqrt().to_bits());
        }
    }

    #[test]
    fn live_admission_is_scoped_to_measured_offline_modes() {
        assert!(verified_scoreboard_variant(1, 6, 0x020F8402, 0));
        assert!(verified_scoreboard_variant(9, 4, 0x000F842B, 0));
        assert!(verified_scoreboard_variant(3, 16, 0x000F840B, 0));
        assert!(verified_scoreboard_variant(5, 12, 0x02078402, 0));
        assert!(verified_scoreboard_variant(9, 19, 0x020F8422, 0));
        assert!(verified_scoreboard_variant(9, 9, 0x020F8422, 5));
        for (kind, sub, flags, lives) in [
            (1, 0, 0, 0),
            (3, 6, 0, 0),
            (5, 6, 0, 0),
            (9, 19, 1, 0),
            (9, 19, 0, 5),
            (9, 9, 0, 3),
            (9, 9, 1, 5),
            (5, 12, 1, 0),
            (5, 12, 0, 5),
            (3, 16, 0, 0),
            (3, 16, 1, 3),
            (1, 6, 1, 0),
            (1, 6, 0, 3),
            (9, 4, 0, 0),
            (9, 4, 1, 3),
            (9, 5, 1, 0),
            (1, 0xC, 0, 0),
            (1, 0x13, 0, 0),
        ] {
            assert!(!verified_scoreboard_variant(kind, sub, flags, lives));
        }
    }

    #[test]
    fn scoreboard_reads_ranked_players_and_marks_the_local_one() {
        let mut m = Mem::default();
        styles(&mut m);
        m.0.insert(addr::PLAYERS, 1);
        m.0.insert(addr::BOTS, 2);
        m.0.insert(addr::PLAYER_TABLE, 0x100_0000);
        m.0.insert(addr::RANK_ORDER, 2);
        m.0.insert(addr::RANK_ORDER + 4, u32::MAX);
        m.0.insert(addr::RANK_ORDER + 8, 0);
        let rec = |i: u32| 0x100_0000 + i * addr::PLAYER_STRIDE;
        m.0.insert(rec(2) + addr::PLAYER_RANK, 1);
        m.0.insert(rec(2) + addr::PLAYER_SCORE, 6);
        m.0.insert(rec(2) + addr::PLAYER_NAME, 0x400_0000);
        m.0.insert(0x400_0000, u32::from_le_bytes(*b"Ren\0"));
        m.0.insert(rec(2) + addr::PLAYER_ID, 7);
        m.0.insert(rec(0) + addr::PLAYER_RANK, 5);
        m.0.insert(rec(0) + addr::PLAYER_SCORE, NO_SCORE as u32);
        m.0.insert(rec(0) + addr::PLAYER_ID, 9);
        m.0.insert(addr::GAME, 0x200_0000);
        m.0.insert(0x200_0014, 0x200_0100);
        m.0.insert(0x200_017C, rec(0));
        let board = read_scoreboard(
            &m,
            Edges::new(0.0, 0.0, 100.0, 100.0),
            tsfp_ui_model::WidgetId(1),
        )
        .unwrap();
        assert_eq!(board.font, 8);
        assert_eq!(board.lines.len(), 2, "negative order entries are skipped");
        assert_eq!(
            (
                board.lines[0].rank,
                board.lines[0].name.as_slice(),
                board.lines[0].tint
            ),
            (1, &b"Ren"[..], LineTint::Plain)
        );
        assert_eq!(
            (board.lines[1].score.as_slice(), board.lines[1].tint),
            (&b"-"[..], LineTint::Local)
        );
        m.0.insert(addr::GAME_FLAGS, 1);
        assert!(
            read_scoreboard(
                &m,
                Edges::new(0.0, 0.0, 1.0, 1.0),
                tsfp_ui_model::WidgetId(1)
            )
            .is_none(),
            "team mode is not ported"
        );
    }

    fn put_string(m: &mut Mem, at: u32, bytes: &[u8]) {
        let mut bytes = bytes.to_vec();
        bytes.push(0);
        for (n, chunk) in bytes.chunks(4).enumerate() {
            let mut word = [0; 4];
            word[..chunk.len()].copy_from_slice(chunk);
            m.0.insert(at + n as u32 * 4, u32::from_le_bytes(word));
        }
    }

    #[test]
    fn variant_reader_formats_timed_lives_and_team_rows_from_runtime_data() {
        let mut m = Mem::default();
        styles(&mut m);
        let table = 0x1000000;
        m.0.insert(addr::PLAYER_TABLE, table);
        m.0.insert(addr::PLAYERS, 1);
        m.0.insert(addr::RANK_ORDER, 0);
        m.0.insert(table + addr::PLAYER_RANK, 1);
        m.0.insert(table + addr::PLAYER_SCORE, 120);
        m.0.insert(addr::GAME_TYPE & !3, 1 << 8);
        m.0.insert(0x4E8CEC, 60);
        put_string(&mut m, 0x478834, b"%02d:%02d.%d");
        let read = |m: &Mem| {
            read_scoreboard_variants(
                m,
                Edges::new(0.0, 0.0, 100.0, 100.0),
                tsfp_ui_model::WidgetId(1),
            )
        };
        let board = read(&m).unwrap();
        assert_eq!(board.lines[0].score, b"00:02.0");
        assert!(board.lines[0].fixed_spacing);
        assert!(
            read_scoreboard(&m, board.area, board.widget).is_none(),
            "unvalidated live variant stays gated"
        );
        m.0.insert(table + 0x80, 1);
        m.0.insert(table + 0xA64, 90);
        assert_eq!(read(&m).unwrap().lines[0].score, b"00:01.5");
        m.0.insert(addr::GAME_TYPE & !3, 5 << 8);
        m.0.insert(table + addr::PLAYER_SCORE, (-90i32) as u32);
        assert_eq!(read(&m).unwrap().lines[0].score, b"00:01.5");
        m.0.insert(addr::GAME_LIVES, 1);
        m.0.insert(table + 0xA70, 2);
        use crate::live_page::draw_fields as f;
        m.0.insert(f::STORY_BANK, 0x2000000);
        m.0.insert(f::COMMON_BANK, 0x2100000);
        m.0.insert(0x2100000 + 4 * 0x6C5, 0x2200000);
        put_string(&mut m, 0x2200000, b" remaining");
        put_string(&mut m, 0x476AC8, b"%d");
        let line = &read(&m).unwrap().lines[0];
        assert_eq!(line.score, b"2 remaining");
        assert!(!line.fixed_spacing);
        m.0.insert(addr::GAME_TYPE & !3, 0xC << 16);
        m.0.insert(addr::GAME_LIVES, 0);
        m.0.insert(0x2000000 + 4 * (0xE93 - f::STORY_FIRST_ID), 0x2400000);
        put_string(&mut m, 0x2400000, b"unset");
        assert_eq!(
            read(&m).unwrap().lines[0].score,
            b"unset",
            "missing timed mask bit uses zero-time string"
        );
        m.0.insert(0x7A2BC8, 1);
        m.0.insert(table + addr::PLAYER_SCORE, (-1000i32) as u32);
        m.0.insert(0x2000000 + 4 * (0x16D3 - f::STORY_FIRST_ID), 0x2500000);
        put_string(&mut m, 0x2500000, b"pending");
        let line = &read(&m).unwrap().lines[0];
        assert_eq!(line.score, b"pending");
        assert!(!line.fixed_spacing);
        m.0.insert(table + addr::PLAYER_SCORE, (-880i32) as u32);
        assert_eq!(read(&m).unwrap().lines[0].score, b"00:02.0");
        let old = checksum(&m);
        m.0.insert(table + 0xA70, 9);
        assert_ne!(checksum(&m), old);
        let old = checksum(&m);
        m.0.insert(0x7A2BC8, 2);
        assert_ne!(checksum(&m), old);
        m.0.insert(addr::GAME_TYPE & !3, 0);
        m.0.insert(addr::GAME_FLAGS, 1);
        m.0.insert(table + 0x84, 2);
        m.0.insert(0x4D1E3C + 20, 0x1234567F);
        put_string(&mut m, 0x49F6E0, b" # %d");
        m.0.insert(0x2100000 + 4 * (0xB2B + 2), 0x2300000);
        put_string(&mut m, 0x2300000, b"Second");
        let line = &read(&m).unwrap().lines[0];
        assert_eq!(line.name, b"Second # 1");
        assert_eq!(line.tint, LineTint::Team(PackedColor(0x1234567F)));
        m.0.insert(addr::SESSION, 1);
        assert!(read(&m).is_none());
    }

    #[test]
    fn subtype13_reads_character_category_and_refuses_unknown_jump_targets() {
        let mut m = Mem::default();
        assert_eq!(scoreboard_hidden_bot(&m, -1), Some(false));
        m.0.insert(0xD6CBC, 0);
        m.0.insert(0xD6CB4, 0xD6CAA);
        assert_eq!(scoreboard_hidden_bot(&m, 0x5D), Some(true));
        m.0.insert(0xD6CB4, 0xD6CB0);
        assert_eq!(scoreboard_hidden_bot(&m, 0x5D), Some(false));
        m.0.insert(0xD6CB4, 0xDEAD);
        assert_eq!(scoreboard_hidden_bot(&m, 0x5D), None);
        assert_eq!(
            scoreboard_format(b"%03d %% %2d", &[-1, 4]),
            Some(b"-01 %  4".to_vec())
        );
        assert!(scoreboard_format(b"%s", &[2]).is_none());
        assert!(scoreboard_format(b"%d", &[]).is_none());
        assert!(scoreboard_format(b"%999d", &[1]).is_none());
    }

    struct Calls {
        memory: Mem,
        returned: HashMap<u32, u32>,
        calls: Vec<(u32, Vec<u32>)>,
    }
    impl GuestUi for Calls {
        fn read32(&self, va: u32) -> Option<u32> {
            self.memory.read32(va)
        }
        fn write32(&mut self, _: u32, _: u32) -> bool {
            panic!("progress must not write guest state")
        }
        fn call(&mut self, va: u32, args: &[u32]) -> Option<u32> {
            self.calls.push((va, args.to_vec()));
            self.returned.get(&va).copied()
        }
    }
    fn progress_fixture() -> (Calls, LivePage, u32) {
        use crate::live_page::{LiveRow, LiveText};
        let mut memory = Mem::default();
        styles(&mut memory);
        memory.0.insert(addr::OBJECTIVE_TABLE, 0x300_0000);
        memory.0.insert(addr::OBJECTIVE_ORDER, 0x310_0000);
        memory.0.insert(addr::OBJECTIVE_COUNT, 2);
        memory.0.insert(addr::OBJECTIVE_COUNTERS, 1);
        memory.0.insert(0x310_0000, 0);
        memory.0.insert(0x310_0004, 1);
        let record = 0x300_0000;
        for r in [record, record + addr::OBJECTIVE_STRIDE] {
            memory.0.insert(r, 0x1111 << 16);
            memory.0.insert(r + 8, (2 << 24) | (1 << 8));
            memory.0.insert(r + 0x2C, 3);
            memory.0.insert(r + 0x30, 12);
            memory.0.insert(r + 0x40, 0x111);
            memory.0.insert(r + 0x44, 0x222);
        }
        let row = LiveRow {
            node: 0x4000000,
            node_type: 5,
            id: 0,
            flags: 0,
            text: LiveText::None,
            font: 3,
            value_font: 3,
            align: 0,
            rect: Some([0.0, 0.0, 100.0, 100.0]),
            clip: None,
            value: None,
            activate: 0,
            draw: OBJECTIVES_ROW_DRAW,
            kind: 0,
            selected: 0,
            options: vec![],
            brightness: 1.0,
            height: 1.0,
            text_align: 0,
            text_x: 0.0,
            text_y: 0.0,
        };
        let live = LivePage {
            slot: 1,
            builder: 0x2F5E60,
            state: 3,
            cursor_node: row.node,
            rows: vec![row],
            units: (1.0, 1.0),
        };
        (
            Calls {
                memory,
                returned: [(0x111, 7), (0x222, 20)].into(),
                calls: vec![],
            },
            live,
            record,
        )
    }
    #[test]
    fn callback_progress_samples_shown_items_in_original_order_and_updates_the_model() {
        let (mut g, live, record) = progress_fixture();
        let samples = sample_progress(&mut g, &live).unwrap();
        assert_eq!(
            g.calls,
            [(0x111, vec![]), (0x222, vec![])],
            "unshown second record is not sampled"
        );
        assert_eq!(samples.values[&record], (7, 20));
        let Some(IngameRow::Objectives(page)) = read_with_progress(&g, &live, Some(&samples))
        else {
            panic!("callbacks should admit the objectives row")
        };
        assert_eq!(
            page.groups[0].items[0].status,
            ObjectiveStatus::Active {
                progress: Some((7, 20))
            }
        );
        assert!(
            read(&g, &live).is_none(),
            "without samples the original draw stays selected"
        );
        g.returned.insert(0x222, 21);
        let next = sample_progress(&mut g, &live).unwrap();
        assert_ne!(
            samples.checksum(),
            next.checksum(),
            "total changes invalidate the cached UI"
        );
    }
    #[test]
    fn incomplete_callbacks_use_stored_counts_and_refused_calls_never_fabricate_a_value() {
        let (mut g, live, record) = progress_fixture();
        g.memory.0.insert(record + 0x44, 0);
        let samples = sample_progress(&mut g, &live).unwrap();
        assert!(g.calls.is_empty());
        let Some(IngameRow::Objectives(page)) = read_with_progress(&g, &live, Some(&samples))
        else {
            panic!("stored progress")
        };
        assert_eq!(
            page.groups[0].items[0].status,
            ObjectiveStatus::Active {
                progress: Some((3, 12))
            }
        );
        g.memory.0.insert(record + 0x44, 0x222);
        g.returned.remove(&0x222);
        assert!(sample_progress(&mut g, &live).is_none());
        assert!(read(&g, &live).is_none());
    }
    #[test]
    fn disabled_progress_hidden_status_and_unsupported_modes_do_not_call() {
        let (mut g, live, record) = progress_fixture();
        for word in [2 << 24, (5 << 24) | (1 << 8)] {
            g.memory.0.insert(record + 8, word);
            assert!(sample_progress(&mut g, &live).unwrap().values.is_empty());
        }
        g.memory.0.insert(record + 8, (2 << 24) | (1 << 8));
        for mode in [3, 4, 6] {
            g.memory.0.insert(addr::UI_DRAW_MODE, mode);
            assert!(sample_progress(&mut g, &live).unwrap().values.is_empty());
            assert!(
                read(&g, &live).is_none(),
                "suppressed rows keep the original draw"
            );
        }
        g.memory.0.insert(addr::UI_DRAW_MODE, 0);
        g.memory
            .0
            .insert(addr::GAME_TYPE & !3, 3 << (8 * (addr::GAME_TYPE & 3)));
        assert!(sample_progress(&mut g, &live).unwrap().values.is_empty());
        assert!(g.calls.is_empty());
    }

    #[test]
    fn objectives_group_by_kind_and_map_statuses() {
        let mut m = Mem::default();
        styles(&mut m);
        m.0.insert(addr::OBJECTIVE_TABLE, 0x300_0000);
        m.0.insert(addr::OBJECTIVE_ORDER, 0x310_0000);
        m.0.insert(addr::OBJECTIVE_COUNT, 3);
        m.0.insert(addr::OBJECTIVE_COUNTERS, 1);
        for (n, (id, kind, status, label)) in [
            (4u32, 0u32, 2u32, 0x1111u32),
            (5, 1, 1, 0x2222),
            (6, 0, 5, 0x3333),
        ]
        .into_iter()
        .enumerate()
        {
            m.0.insert(0x310_0000 + 4 * n as u32, id);
            let r = 0x300_0000 + id * addr::OBJECTIVE_STRIDE;
            m.0.insert(r, label << 16);
            m.0.insert(r + 8, kind | (status << 24));
        }
        // Counter of kind 0 is 1: only the first listed kind-0 objective is shown; kind 1 has a pending one only.
        let page = read_objectives(
            &m,
            Edges::new(0.0, 0.0, 1.0, 1.0),
            tsfp_ui_model::WidgetId(1),
            PackedColor(0x7F7F_7F7F),
        )
        .unwrap();
        assert_eq!(page.groups.len(), 1);
        assert_eq!(page.groups[0].header, 0x15F3);
        assert_eq!(
            page.groups[0].items,
            vec![Objective {
                label: 0x1111,
                status: ObjectiveStatus::Active { progress: None }
            }]
        );
        m.0.insert(addr::OBJECTIVE_COUNTERS, 2);
        let page = read_objectives(
            &m,
            Edges::new(0.0, 0.0, 1.0, 1.0),
            tsfp_ui_model::WidgetId(1),
            PackedColor(0),
        )
        .unwrap();
        assert_eq!(page.groups[0].header, 0x15F4);
        assert_eq!(page.groups[0].items[1].status, ObjectiveStatus::Done);
        assert_eq!((page.header_font, page.item_font), (9, 3));
    }
}

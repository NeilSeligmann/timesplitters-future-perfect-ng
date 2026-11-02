//! T2315 increment 4: the row draws of the two in-game pages the BACK action opens (`0x1CE7EB` -> `0x2E6980`), as
//! readable Rust:
//!
//! * the Arcade **scoreboard** page `0x2F5920` (descriptor `0x524750`): one row whose draw `0x2EC900` calls
//!   `0x2E72E0(page, row, rect, 1, 0)`, one line per shown player (rank, name, score);
//! * the Story **objectives** page `0x2F5E60` (descriptor `0x524740`): one row whose draw `0x2F5E10` calls
//!   `0x2F5B30(page, row, rect, 1)`, a header per objective kind and one `0x2F45F0` line per objective (label, status).
//!
//! MEASURED by static reading of the owner's XBE (capstone listing of the lift, docs/t2315-ui-page-completion.md,
//! increment 4). The host reads the players and objectives through typed views (`tsfp_ui_input::list_rows_live`); this
//! module only lays them out. Units: 640x480 design units; text widths come from the host's font measure (`0x405C0`).

use crate::bridge::TextPlacement;
use crate::list_rows::{Edges, ListDraw, alpha_byte};
use crate::model::{Align, TextId, TextRef, WidgetId};
use crate::skin::{OUTLINE_OFFSETS, PackedColor};
use tsfp_ui_renderer::{Color, Viewport};

/// Row draw of the scoreboard page (`0x2EC900`, body `0x2E72E0`).
pub const SCOREBOARD_ROW_DRAW: u32 = 0x2E_C900;
/// Row draw of the objectives page in Story (`0x2F5E10`, body `0x2F5B30`; the other variant `0x2ED710` is not ported).
pub const OBJECTIVES_ROW_DRAW: u32 = 0x2F_5E10;
/// The name column starts this far right of the rank (`0x2E72E0`: `[0x4C0460]` 1.0 times `[0x47CFF4]` 30).
pub const NAME_COLUMN: f32 = 30.0;
/// The score ends this far left of the row's right edge (`[0x4C0460]` times `[0x4788FC]` 15).
pub const SCORE_INSET: f32 = 15.0;
/// Header advance of the objectives page, in header line heights (`[0x478398]` 1.25).
pub const HEADER_ADVANCE: f32 = 1.25;
/// Gap after each objective, in item line heights (`[0x475CD4]` 0.5).
pub const ITEM_GAP: f32 = 0.5;
/// Objective label colour (`0x2F45F0`: `0x6E6E6E00 | alpha`).
pub const LABEL_RGB: u32 = 0x6E6E_6E00;

/// One scoreboard line (`0x2E72E0` loop over `[0x7DE4C0]`, the players in rank order).
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreLine {
    /// `+0xA68` of the player record, drawn with `%d`.
    pub rank: i32,
    /// The score text of `0x2E7080` (the default `%d` of `+0xA6C`, `-` for 0x80000001, negated in game type 5).
    pub score: Vec<u8>,
    pub fixed_spacing: bool,
    /// The player name `+0x70` (`%s`).
    pub name: Vec<u8>,
    pub tint: LineTint,
}

/// T2316: the score column of `0x2E7080(id, rank, score, lives, buffer, pass)` in every game mode (MEASURED by static
/// reading). The caller passes `+0xA64` instead of `+0xA6C` as the score in game type 1 when the player word `+0x80` is
/// set; the result decides the digit mode (`0x3FA70(1)`, fixed width, when it is positive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScoreMode {
    /// Game type byte `[0x7DE455]`, sub type byte `[0x7DE456]`, lives word `[0x7DE474]`.
    pub game_type: u8,
    pub sub_type: u8,
    pub lives_mode: u32,
    /// Sub type 0xC: the player mask `[0x7A2BC8]` (bit `id`: the player has a time).
    pub timed_mask: u32,
}

/// What `0x2E7080` prints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScoreText {
    /// `%d` (`[0x4760FC]`).
    Number(i32),
    /// `-` (`[0x47D0F8]`): score 0x80000001.
    Dash,
    /// `0x64FC0`: a time in frames (minutes, seconds, tenths; text 0xE93 for 0).
    Time(i32),
    /// Sub type 0xC with score -1000: text 0x16D3.
    Text(u32),
    /// Lives mode with lives left: `%d` (`[0x476AC8]`) of the lives, then text 0x6C6 (one) or 0x6C5 (more).
    Lives(i32),
}

/// `0x2E7080`: the score text and the digit mode (1 fixed width, 0 proportional, -1 text).
pub fn score_text(mode: ScoreMode, id: u32, score: i32, lives: i32, pass: i32) -> (ScoreText, i32) {
    let timed_type = mode.game_type == 1 && pass != 1 && pass != 2;
    if timed_type || mode.sub_type == 0xC {
        if mode.sub_type != 0xC {
            return (ScoreText::Time(score), 1);
        }
        if mode.timed_mask & 1u32.wrapping_shl(id) == 0 {
            return (ScoreText::Time(0), 1);
        }
        if score == -1000 {
            return (ScoreText::Text(0x16D3), -1);
        }
        return (ScoreText::Time(score.wrapping_add(1000)), 1);
    }
    if mode.lives_mode != 0 {
        if lives > 0 {
            return (ScoreText::Lives(lives), -1);
        }
        if score == NO_SCORE_VALUE {
            return (ScoreText::Dash, 1);
        }
        return (ScoreText::Time(score), 1);
    }
    if mode.game_type == 5 {
        return (ScoreText::Time(score.wrapping_neg()), 1);
    }
    if score == NO_SCORE_VALUE {
        return (ScoreText::Dash, 1);
    }
    (ScoreText::Number(score), 0)
}

/// The "no score" value of `0x2E7080`.
pub const NO_SCORE_VALUE: i32 = 0x8000_0001u32 as i32;

/// `0x64FC0(buffer, frames)`: minutes (at most 99), seconds and tenths at `fps` frames per second (`[0x4E8CEC]`), for the
/// format `[0x478834]`; None for 0 (the original prints text 0xE93 through `[0x475D88]`).
pub fn time_parts(frames: i32, fps: i32) -> Option<(i32, i32, i32)> {
    if frames == 0 || fps <= 0 {
        return None;
    }
    let (total, rest) = (frames / fps, frames % fps);
    let minutes = (total / 60).min(99);
    let seconds = total % 60;
    let tenths = rest.wrapping_mul(100).checked_div(fps.wrapping_mul(10))?;
    Some((minutes, seconds, tenths))
}

/// The line colours of `0x2E72E0` (fill, outline), alpha from the page fade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineTint {
    /// Other players: `0x7F7F7F` at `fade * 127`, black outline at `fade * 64`.
    Plain,
    /// The local player: `0x407878` with a `0x000050` outline, both at `fade * 127`.
    Local,
    /// The local player with a negative rank: `0x7F1E1E`, black outline at `fade * 64`.
    Out,
    /// Runtime team palette RGB and alpha; original fade scales alpha only.
    Team(PackedColor),
}

impl LineTint {
    fn colors(self, fade: f32) -> (Color, Color) {
        let full = alpha_byte(fade * 127.0);
        let half = alpha_byte(fade * 64.0);
        let (fill, edge) = match self {
            LineTint::Plain => (0x7F7F_7F00 | full, half),
            LineTint::Local => (0x4078_7800 | full, 0x0000_5000 | full),
            LineTint::Out => (0x7F1E_1E00 | full, half),
            LineTint::Team(color) => (
                (color.0 & 0xFFFF_FF00) | alpha_byte((color.0 & 255) as f32 * fade.clamp(0.0, 1.0)),
                full,
            ),
        };
        (PackedColor(fill).to_color(), PackedColor(edge).to_color())
    }
}

/// Snapshot of one nonnegative rank-order entry. The adapter owns session eligibility and
/// the subtype-0x13 character predicate; the model never reads guest memory or calls handlers.
#[derive(Clone, Debug, PartialEq)]
pub struct RankedPlayer {
    pub id: u32,
    pub rank: i32,
    pub score: i32,
    pub alternate_score: Option<i32>,
    pub lives: i32,
    pub team: u32,
    pub name: Vec<u8>,
    pub eligible: bool,
    pub hidden_bot: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScoreName {
    Player(Vec<u8>),
    /// `0x2E7270` counts all eligible ranked team members, including subtype-filtered bots.
    Team {
        team: u32,
        members: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectedScoreRow {
    pub rank: i32,
    pub score: ScoreText,
    pub digit_mode: i32,
    pub name: ScoreName,
    pub tint: LineTint,
}

/// Readable selection policy of `0x2E746E`, in rank order. Type3 always shows individuals
/// with team colours; other team games deduplicate teams and retain the first member's
/// rank/score. Team colours and names are supplied from runtime data by the boundary.
/// Missing palette entries refuse the native view instead of inventing colours.
pub fn select_score_rows(
    players: &[RankedPlayer],
    mode: ScoreMode,
    teams: bool,
    local: Option<u32>,
    humans: u32,
    pass: i32,
    palette: &dyn Fn(u32) -> Option<PackedColor>,
) -> Option<Vec<SelectedScoreRow>> {
    let mut seen = 0u32;
    let mut rows = Vec::new();
    for player in players.iter().filter(|p| p.eligible) {
        if mode.sub_type == 0x13 && player.id >= humans && player.hidden_bot {
            continue;
        }
        let team_row = mode.game_type != 3 && teams;
        let tint = if mode.game_type == 3 || team_row {
            if team_row {
                let bit = 1u32.wrapping_shl(player.team);
                if seen & bit != 0 {
                    continue;
                }
                seen |= bit;
            }
            LineTint::Team(palette(player.team)?)
        } else if local == Some(player.id) {
            if player.rank < 0 || matches!(pass, 1 | 2) {
                LineTint::Out
            } else {
                LineTint::Local
            }
        } else {
            LineTint::Plain
        };
        let value = if mode.game_type == 1 {
            player.alternate_score.unwrap_or(player.score)
        } else {
            player.score
        };
        let (score, digit_mode) = score_text(mode, player.id, value, player.lives, pass);
        let name = if team_row && pass == 0 {
            ScoreName::Team {
                team: player.team,
                members: players
                    .iter()
                    .filter(|p| p.eligible && p.team == player.team)
                    .count(),
            }
        } else {
            ScoreName::Player(player.name.clone())
        };
        rows.push(SelectedScoreRow {
            rank: player.rank,
            score,
            digit_mode,
            name,
            tint,
        });
    }
    Some(rows)
}

/// The scoreboard row and the values its draw reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Scoreboard {
    pub widget: WidgetId,
    /// The row's draw rect (`0x71E90`).
    pub area: Edges,
    /// `[0x7BA038]`.
    pub text_unit: f32,
    /// `[0x7BA188]`: the first line sits half a unit down.
    pub line_unit: f32,
    /// `int([0x7E7264] + 1)` in the scoreboard style.
    pub line_height: f32,
    /// Text style: 8 with one player, 3 with two, else 15 (`0x1CDAE0`).
    pub font: u32,
    pub lines: Vec<ScoreLine>,
}

/// Text width in design units: `(text, runtime bytes, font)`.
pub type TextWidth<'a> = dyn Fn(TextRef, &[u8], u32) -> f32 + 'a;
/// The bytes of a text id (the string bank), for wrapping.
pub type TextBytes<'a> = dyn Fn(TextRef) -> Option<Vec<u8>> + 'a;

struct Writer<'a> {
    draw: ListDraw,
    widget: WidgetId,
    viewport: Viewport,
    next: usize,
    base: usize,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl Writer<'_> {
    fn new(widget: WidgetId, viewport: Viewport, base: usize) -> Self {
        Writer {
            draw: ListDraw::default(),
            widget,
            viewport,
            next: 0,
            base,
            _marker: std::marker::PhantomData,
        }
    }

    /// A point anchor with a real column width; the shared widget supplies its ink height.
    fn put_in_width(
        &mut self,
        text: TextRef,
        bytes: Option<&[u8]>,
        x: f32,
        y: f32,
        width: f32,
        font: u32,
        color: Color,
    ) {
        let widget = match bytes {
            Some(bytes) => {
                let id = WidgetId::anon(self.base + self.next);
                self.next += 1;
                self.draw.dynamic.push((id, bytes.to_vec()));
                id
            }
            None => self.widget,
        };
        if let Some(rect_px) = Edges::new(x, y, x + width.max(0.0), y + 1.0).to_px(self.viewport) {
            self.draw.texts.push(TextPlacement {
                widget,
                text,
                font_index: font,
                value_font_index: font,
                align: Align::Start,
                rect_px,
                color,
                value: None,
            });
        }
    }

    /// `0x42010` / `0x41E30`: four outline passes in `edge` ([`OUTLINE_OFFSETS`]), then the text in `fill`.
    fn outlined(
        &mut self,
        text: TextRef,
        bytes: Option<&[u8]>,
        x: f32,
        y: f32,
        width: f32,
        font: u32,
        fill: Color,
        edge: Color,
    ) {
        for (dx, dy) in OUTLINE_OFFSETS {
            self.put_in_width(text, bytes, x + dx, y + dy, width, font, edge);
        }
        self.put_in_width(text, bytes, x, y, width, font, fill);
    }
}

/// `0x40A40(text, available, 0, 1)`: shrink X only, never grow or change Y.
/// Nonpositive widths/limits leave the original scale unchanged (MEASURED static contract).
pub fn name_fit_x(width: f32, available: f32) -> f32 {
    if width > 0.0 && available > 0.0 {
        (available / width).min(1.0)
    } else {
        1.0
    }
}

/// Draw the scoreboard (`0x2E72E0` with draw 1, pass 0): line `n` at `y = y0 + line unit / 2 + n * line height`; the
/// rank at `x0`, the score ending at `x1 - 15`, the name at `x0 + 30`, every text outlined (`0x42010`). The name fit of
/// `0x40A40` shrinks names horizontally to `score x - (x0 + 30 + 2 text units)`.
pub fn draw_scoreboard(
    board: &Scoreboard,
    width: &TextWidth<'_>,
    viewport: Viewport,
    fade: f32,
) -> ListDraw {
    draw_scoreboard_with_fixed_width(board, width, width, viewport, fade)
}

/// The fixed-width provider measures the same metrics the shaper uses.
pub fn draw_scoreboard_with_fixed_width(
    board: &Scoreboard,
    width: &TextWidth<'_>,
    fixed_width: &TextWidth<'_>,
    viewport: Viewport,
    fade: f32,
) -> ListDraw {
    let mut out = Writer::new(board.widget, viewport, 0x5400);
    let font = board.font;
    let x0 = board.area.x0;
    let mut y = board.area.y0 + board.line_unit * 0.5;
    for line in &board.lines {
        let (fill, edge) = line.tint.colors(fade);
        let rank = line.rank.to_string().into_bytes();
        out.outlined(
            TextRef::Dynamic,
            Some(&rank),
            x0,
            y,
            NAME_COLUMN,
            font,
            fill,
            edge,
        );
        let score_width = if line.fixed_spacing {
            fixed_width
        } else {
            width
        };
        let score_w = score_width(TextRef::Dynamic, &line.score, font);
        let score_x = board.area.x1 - SCORE_INSET - score_w;
        let first_score = out.draw.dynamic.len();
        out.outlined(
            TextRef::Dynamic,
            Some(&line.score),
            score_x,
            y,
            score_w,
            font,
            fill,
            edge,
        );
        if line.fixed_spacing {
            out.draw
                .fixed_spacing
                .extend(out.draw.dynamic[first_score..].iter().map(|(id, _)| *id));
        }
        let name_x = x0 + NAME_COLUMN;
        let fit = name_fit_x(
            width(TextRef::Dynamic, &line.name, font),
            score_x - name_x - 2.0 * board.text_unit,
        );
        let first = out.draw.dynamic.len();
        out.outlined(
            TextRef::Dynamic,
            Some(&line.name),
            name_x,
            y,
            score_x - name_x - 2.0 * board.text_unit,
            font,
            fill,
            edge,
        );
        out.draw
            .text_scale_x
            .extend(out.draw.dynamic[first..].iter().map(|(id, _)| (*id, fit)));
        y += board.line_height;
    }
    out.draw
}

/// Status of one objective (`+0xB`, the jump table of `0x2F45F0`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjectiveStatus {
    /// Status 2 or 3: text `0x1820` in `0x3C3C3C`, black outline; with flag `+0x9` bit 0 the progress `%d/%d`
    /// (`0x227360`: `+0x2C` / `+0x30`) instead.
    Active { progress: Option<(i32, i32)> },
    /// Status 4: text `0x1824` in `0x640000`, black outline.
    Failed,
    /// Status 5: text `0x1825` in `0x787878`, outline `0x000050`.
    Done,
    /// Any other status: the jump table's default keeps the text " " in colour 0 (nothing visible).
    Hidden,
}

pub const STATUS_ACTIVE_TEXT: u32 = 0x1820;
pub const STATUS_FAILED_TEXT: u32 = 0x1824;
pub const STATUS_DONE_TEXT: u32 = 0x1825;
/// "No objectives" text of `0x2F5B30` (drawn right-aligned when no kind has an objective).
pub const NO_OBJECTIVES_TEXT: u32 = 0x1798;

/// One objective line: its label text id (`0x227310`: word `+0x2`) and status.
#[derive(Clone, Debug, PartialEq)]
pub struct Objective {
    pub label: u32,
    pub status: ObjectiveStatus,
}

/// One objective kind (`0x227A70` / `0x227AD0` with kind 0 and 1): the header text id (`0x227FB0`) and its objectives.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectiveGroup {
    pub header: u32,
    pub items: Vec<Objective>,
}

/// The objectives row and the values its draw reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Objectives {
    pub widget: WidgetId,
    pub area: Edges,
    pub text_unit: f32,
    /// Header style (9, or 3 with more than two players) and its `[0x7E7264]`.
    pub header_font: u32,
    pub header_height: f32,
    /// Objective style (3, or 18 with more than two players) and its `[0x7E7264]`.
    pub item_font: u32,
    pub item_height: f32,
    /// Header colour: the row's first colour of `0x75F60`.
    pub header_color: PackedColor,
    /// The kind 0 header for the empty page (`0x227FB0(0)`).
    pub empty_header: u32,
    pub groups: Vec<ObjectiveGroup>,
}

/// Greedy word wrap at spaces to `max` design units (`0x40F00` -> `0x40C10` inserts line breaks; the rule is INFERRED,
/// `0x40C10` is not decoded). A word longer than the line keeps its own line.
pub fn wrap(bytes: &[u8], max: f32, measure: &dyn Fn(&[u8]) -> f32) -> Vec<Vec<u8>> {
    crate::text_widget::lines(bytes, max, max <= 0.0, measure)
}

/// Draw the objectives (`0x2F5B30` with draw 1, `0x2F45F0` per objective, single screen): `x0, y, x1` truncated; per
/// kind with objectives the header at `(x0, y)`, `y = int(y + 1.25 Hh)`; per objective the label wrapped to `x1 - status
/// width - 2 units - x0` at `(x0, y)` in `0x6E6E6E` (plain), the status outlined ending at `x1`, then
/// `y = int(0.5 Hi + y + label height)`; after the kind `y = int(y + Hi)`. With no objective: the kind 0 header and the
/// "no objectives" text ending at `x1`. Label height = lines * Hi (INFERRED for `0x40620`).
pub fn draw_objectives(
    page: &Objectives,
    width: &TextWidth<'_>,
    bytes: &TextBytes<'_>,
    viewport: Viewport,
    fade: f32,
) -> ListDraw {
    let mut out = Writer::new(page.widget, viewport, 0x5500);
    let x0 = page.area.x0.trunc();
    let x1 = page.area.x1.trunc();
    let mut y = page.area.y0.trunc();
    let header_color = page.header_color.with_alpha_scaled(fade.clamp(0.0, 1.0));
    let full = alpha_byte(fade * 127.0);
    let id = |n: u32| TextRef::Id(TextId(n));
    let (hh, hi) = (page.header_height, page.item_height);
    if page.groups.iter().all(|g| g.items.is_empty()) {
        out.put_in_width(
            id(page.empty_header),
            None,
            x0,
            y,
            x1 - x0,
            page.header_font,
            header_color,
        );
        y = (y + hh * HEADER_ADVANCE).trunc();
        let w = width(id(NO_OBJECTIVES_TEXT), &[], page.item_font);
        out.put_in_width(
            id(NO_OBJECTIVES_TEXT),
            None,
            x1 - w,
            y,
            w,
            page.item_font,
            header_color,
        );
        return out.draw;
    }
    for group in page.groups.iter().filter(|g| !g.items.is_empty()) {
        out.put_in_width(
            id(group.header),
            None,
            x0,
            y,
            x1 - x0,
            page.header_font,
            header_color,
        );
        y = (y + hh * HEADER_ADVANCE).trunc();
        for item in &group.items {
            let font = page.item_font;
            let (status, fill, edge) = match &item.status {
                ObjectiveStatus::Active {
                    progress: Some((done, total)),
                } => (
                    Err(format!("{done}/{total}").into_bytes()),
                    0x3C3C_3C00 | full,
                    full,
                ),
                ObjectiveStatus::Active { progress: None } => {
                    (Ok(STATUS_ACTIVE_TEXT), 0x3C3C_3C00 | full, full)
                }
                ObjectiveStatus::Failed => (Ok(STATUS_FAILED_TEXT), 0x6400_0000 | full, full),
                ObjectiveStatus::Done => {
                    (Ok(STATUS_DONE_TEXT), 0x7878_7800 | full, 0x0000_5000 | full)
                }
                ObjectiveStatus::Hidden => (Err(b" ".to_vec()), 0, 0),
            };
            let visible = item.status != ObjectiveStatus::Hidden;
            let status_w = match &status {
                Ok(text) => width(id(*text), &[], font),
                Err(b) => width(TextRef::Dynamic, b, font),
            };
            let max = x1 - status_w - page.text_unit * 2.0 - x0;
            let label = bytes(id(item.label)).unwrap_or_default();
            let lines = wrap(&label, max, &|b| width(TextRef::Dynamic, b, font));
            let label_color = PackedColor(LABEL_RGB | full).to_color();
            if lines.len() <= 1 {
                out.put_in_width(id(item.label), None, x0, y, max, font, label_color);
            } else {
                for (n, line) in lines.iter().enumerate() {
                    out.put_in_width(
                        TextRef::Dynamic,
                        Some(line),
                        x0,
                        y + n as f32 * hi,
                        max,
                        font,
                        label_color,
                    );
                }
            }
            let (fill, edge) = (PackedColor(fill).to_color(), PackedColor(edge).to_color());
            match &status {
                Ok(text) => out.outlined(
                    id(*text),
                    None,
                    x1 - status_w,
                    y,
                    status_w,
                    font,
                    fill,
                    edge,
                ),
                Err(b) if visible => out.outlined(
                    TextRef::Dynamic,
                    Some(b),
                    x1 - status_w,
                    y,
                    status_w,
                    font,
                    fill,
                    edge,
                ),
                Err(_) => {}
            }
            let height = lines.len().max(1) as f32 * hi;
            y = (hi * ITEM_GAP + y + height).trunc();
        }
        y = (y + hi).trunc();
    }
    out.draw
}

#[cfg(test)]
mod score_tests {
    //! Tier 0, FABRICATED values; the branch order is MEASURED from `0x2E7080` and `0x64FC0`.
    use super::*;

    const DEATHMATCH: ScoreMode = ScoreMode {
        game_type: 0,
        sub_type: 0,
        lives_mode: 0,
        timed_mask: 0,
    };

    #[test]
    fn the_default_branch_prints_the_number_or_a_dash() {
        assert_eq!(
            score_text(DEATHMATCH, 0, 7, 0, 0),
            (ScoreText::Number(7), 0)
        );
        assert_eq!(
            score_text(DEATHMATCH, 0, NO_SCORE_VALUE, 0, 0),
            (ScoreText::Dash, 1)
        );
    }

    #[test]
    fn timed_types_lives_and_sub_type_0xc_follow_the_original_order() {
        let type1 = ScoreMode {
            game_type: 1,
            ..DEATHMATCH
        };
        assert_eq!(score_text(type1, 0, 600, 0, 0), (ScoreText::Time(600), 1));
        // Passes 1 and 2 of game type 1 fall through to the default branch.
        assert_eq!(score_text(type1, 0, 600, 0, 1), (ScoreText::Number(600), 0));
        let type5 = ScoreMode {
            game_type: 5,
            ..DEATHMATCH
        };
        assert_eq!(score_text(type5, 0, -90, 0, 0), (ScoreText::Time(90), 1));
        let lives = ScoreMode {
            lives_mode: 3,
            ..type5
        };
        assert_eq!(score_text(lives, 0, 5, 2, 0), (ScoreText::Lives(2), -1));
        assert_eq!(
            score_text(lives, 0, 5, 0, 0),
            (ScoreText::Time(5), 1),
            "no lives left: the score as a time"
        );
        let sub = ScoreMode {
            sub_type: 0xC,
            timed_mask: 0b10,
            ..DEATHMATCH
        };
        assert_eq!(
            score_text(sub, 0, 77, 0, 0),
            (ScoreText::Time(0), 1),
            "player 0 has no time"
        );
        assert_eq!(
            score_text(sub, 1, -1000, 0, 0),
            (ScoreText::Text(0x16D3), -1)
        );
        assert_eq!(score_text(sub, 1, 200, 0, 0), (ScoreText::Time(1200), 1));
    }

    #[test]
    fn teams_keep_first_ranked_member_and_type3_keeps_individuals() {
        let player = |id, team, eligible, hidden_bot| RankedPlayer {
            id,
            team,
            eligible,
            hidden_bot,
            rank: id as i32 + 1,
            score: 7,
            alternate_score: None,
            lives: 0,
            name: vec![b'A' + id as u8],
        };
        let players = vec![
            player(0, 2, true, false),
            player(1, 1, false, false),
            player(2, 2, true, true),
            player(3, 1, true, false),
        ];
        let palette = |team| Some(PackedColor(team << 24 | 127u32));
        let rows = select_score_rows(&players, DEATHMATCH, true, Some(0), 2, 0, &palette).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].name,
            ScoreName::Team {
                team: 2,
                members: 2
            }
        );
        assert_eq!(rows[1].rank, 4);
        let type3 = ScoreMode {
            game_type: 3,
            sub_type: 0x13,
            ..DEATHMATCH
        };
        let rows = select_score_rows(&players, type3, true, Some(0), 2, 0, &palette).unwrap();
        assert_eq!(rows.len(), 2, "hidden bot excluded, type3 not deduplicated");
        assert_eq!(rows[0].name, ScoreName::Player(vec![b'A']));
        assert!(
            matches!(rows[0].tint, LineTint::Team(_)),
            "team colour overrides local tint"
        );
        let filtered = ScoreMode {
            sub_type: 0x13,
            ..DEATHMATCH
        };
        let rows = select_score_rows(&players, filtered, true, None, 2, 0, &palette).unwrap();
        assert_eq!(
            rows[0].name,
            ScoreName::Team {
                team: 2,
                members: 2
            },
            "member count ignores draw-only bot filter"
        );
        assert!(select_score_rows(&players, DEATHMATCH, true, None, 2, 0, &|_| None).is_none());
    }

    #[test]
    fn type1_uses_alternate_score_and_team_fade_preserves_rgb() {
        let p = RankedPlayer {
            id: 0,
            rank: 1,
            score: 90,
            alternate_score: Some(120),
            lives: 0,
            team: 1,
            name: vec![],
            eligible: true,
            hidden_bot: false,
        };
        let rows = select_score_rows(
            &[p],
            ScoreMode {
                game_type: 1,
                ..DEATHMATCH
            },
            false,
            Some(0),
            1,
            0,
            &|_| None,
        )
        .unwrap();
        assert_eq!(
            (rows[0].score, rows[0].digit_mode),
            (ScoreText::Time(120), 1)
        );
        let (fill, edge) = LineTint::Team(PackedColor(0x1234567F)).colors(0.5);
        assert_eq!(fill, PackedColor(0x1234563F).to_color());
        assert_eq!(edge, PackedColor(0x0000003F).to_color());
    }

    #[test]
    fn times_split_into_minutes_seconds_and_tenths() {
        assert_eq!(time_parts(0, 60), None);
        assert_eq!(time_parts(1, 0), None);
        assert_eq!(time_parts(i32::MAX, i32::MAX), Some((0, 1, 0)));
        assert_eq!(time_parts(60 * 125 + 30, 60), Some((2, 5, 5)));
        assert_eq!(
            time_parts(60 * 60 * 120, 60),
            Some((99, 0, 0)),
            "minutes stop at 99"
        );
    }
}

#[cfg(test)]
mod tests {
    //! Tier 0 with FABRICATED rows and a fixed-width measure (10 units per byte, 50 per id); constants MEASURED.
    use super::*;

    fn viewport() -> Viewport {
        crate::anchor::window_viewport(640, 480).unwrap()
    }

    fn measure(text: TextRef, bytes: &[u8], _font: u32) -> f32 {
        match text {
            TextRef::Id(_) => 50.0,
            _ => 10.0 * bytes.len() as f32,
        }
    }

    fn fills(draw: &ListDraw) -> Vec<&TextPlacement> {
        // Every fifth placement of an outlined run is the fill.
        draw.texts.iter().skip(4).step_by(5).collect()
    }

    #[test]
    fn scoreboard_lines_put_rank_score_and_name_in_their_columns() {
        let board = Scoreboard {
            widget: WidgetId(3),
            area: Edges::new(100.0, 150.0, 540.0, 360.0),
            text_unit: 6.4,
            line_unit: 4.8,
            line_height: 26.0,
            font: 8,
            lines: vec![
                ScoreLine {
                    rank: 1,
                    score: b"6".to_vec(),
                    fixed_spacing: false,
                    name: b"AB".to_vec(),
                    tint: LineTint::Plain,
                },
                ScoreLine {
                    rank: 5,
                    score: b"0".to_vec(),
                    fixed_spacing: false,
                    name: b"ME".to_vec(),
                    tint: LineTint::Local,
                },
            ],
        };
        let draw = draw_scoreboard(&board, &measure, viewport(), 1.0);
        assert_eq!(draw.texts.len(), 2 * 3 * 5, "three outlined runs per line");
        let fill = fills(&draw);
        // Line 0 at y = 150 + 2.4; rank at x0, score ends at 540 - 15 (width 10), name at x0 + 30.
        assert_eq!(fill[0].rect_px.origin.x, 100.0);
        assert!((fill[0].rect_px.origin.y - 152.4).abs() < 1e-3);
        assert_eq!(fill[1].rect_px.origin.x, 515.0);
        assert_eq!(fill[0].rect_px.size.x, NAME_COLUMN);
        assert_eq!(fill[1].rect_px.size.x, 10.0);
        assert!((fill[2].rect_px.size.x - 372.2).abs() < 1e-3);
        assert_eq!(fill[2].rect_px.origin.x, 130.0);
        assert!(
            (fill[3].rect_px.origin.y - 178.4).abs() < 1e-3,
            "second line one line height down"
        );
        // Outline passes first, at the OUTLINE_OFFSETS.
        assert_eq!(draw.texts[0].rect_px.origin.x, 98.0);
        // The local player is cyan with a blue outline.
        assert_eq!(fill[3].color, PackedColor(0x4078_787F).to_color());
        assert_eq!(draw.texts[15].color, PackedColor(0x0000_507F).to_color());
        assert_eq!(
            draw.texts[0].color,
            PackedColor(0x0000_0040).to_color(),
            "plain outline at fade * 64"
        );
        let names: Vec<&[u8]> = draw.dynamic.iter().map(|(_, b)| b.as_slice()).collect();
        assert!(names.contains(&&b"ME"[..]) && names.contains(&&b"5"[..]));
    }

    #[test]
    fn long_scoreboard_names_fit_only_the_gap_before_the_score() {
        assert_eq!(name_fit_x(100.0, 40.0), 0.4);
        assert_eq!(name_fit_x(10.0, 40.0), 1.0);
        assert_eq!(name_fit_x(0.0, 40.0), 1.0);
        assert_eq!(name_fit_x(100.0, -5.0), 1.0);
        let board = Scoreboard {
            widget: WidgetId(1),
            area: Edges::new(0.0, 0.0, 100.0, 100.0),
            text_unit: 5.0,
            line_unit: 10.0,
            line_height: 20.0,
            font: 8,
            lines: vec![ScoreLine {
                rank: 1,
                score: b"5".to_vec(),
                fixed_spacing: false,
                name: vec![b'A'; 10],
                tint: LineTint::Plain,
            }],
        };
        let draw = draw_scoreboard(&board, &measure, viewport(), 1.0);
        // Score starts at 75; name at 30; two units of padding leave 35 for 100 units of name.
        assert_eq!(
            draw.text_scale_x.len(),
            5,
            "four outline passes and the fill all shrink"
        );
        assert!(draw.text_scale_x.iter().all(|(_, fit)| *fit == 0.35));
        for (id, _) in &draw.text_scale_x {
            assert_eq!(
                draw.dynamic
                    .iter()
                    .find(|(widget, _)| widget == id)
                    .unwrap()
                    .1,
                vec![b'A'; 10]
            );
        }
        assert_eq!(draw.texts.last().unwrap().rect_px.origin.y, 5.0);
    }

    #[test]
    fn objectives_stack_header_label_and_status() {
        let page = Objectives {
            widget: WidgetId(2),
            area: Edges::new(100.5, 200.7, 540.2, 300.0),
            text_unit: 6.4,
            header_font: 9,
            header_height: 20.0,
            item_font: 3,
            item_height: 16.0,
            header_color: PackedColor(0x7F7F_7F7F),
            empty_header: 0x15F3,
            groups: vec![ObjectiveGroup {
                header: 0x15F3,
                items: vec![
                    Objective {
                        label: 0x111,
                        status: ObjectiveStatus::Active { progress: None },
                    },
                    Objective {
                        label: 0x222,
                        status: ObjectiveStatus::Done,
                    },
                ],
            }],
        };
        let bytes = |t: TextRef| match t {
            TextRef::Id(TextId(0x222)) => Some(
                b"aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp"
                    .to_vec(),
            ),
            _ => Some(b"short".to_vec()),
        };
        let draw = draw_objectives(&page, &measure, &bytes, viewport(), 1.0);
        // Header at (100, 200); first label at y = int(200 + 25) = 225, status ends at 540.
        assert_eq!(draw.texts[0].rect_px.origin.x, 100.0);
        assert_eq!(draw.texts[0].rect_px.origin.y, 200.0);
        assert_eq!(draw.texts[1].text, TextRef::Id(TextId(0x111)));
        assert_eq!(draw.texts[1].rect_px.origin.y, 225.0);
        assert_eq!(draw.texts[1].color, PackedColor(0x6E6E_6E7F).to_color());
        let status = &draw.texts[6];
        assert_eq!(status.text, TextRef::Id(TextId(STATUS_ACTIVE_TEXT)));
        assert_eq!(
            (status.rect_px.origin.x, status.rect_px.origin.y),
            (490.0, 225.0)
        );
        assert!(
            (status.rect_px.size.x - 50.0).abs() < 1e-3,
            "status clip ends at the authored column edge"
        );
        // Second objective at y = int(8 + 225 + 16) = 249; its label wraps (max 540 - 50 - 12.8 - 100 = 377.2: 7 words).
        let wrapped: Vec<&TextPlacement> = draw
            .texts
            .iter()
            .filter(|t| t.text == TextRef::Dynamic)
            .collect();
        assert_eq!(wrapped.len(), 3);
        assert_eq!(wrapped[0].rect_px.origin.y, 249.0);
        for line in &wrapped {
            assert!(
                (line.rect_px.size.x - 377.2).abs() < 1e-3,
                "label clip reserves the status column"
            );
        }
        assert_eq!(wrapped[1].rect_px.origin.y, 265.0);
        let done = draw
            .texts
            .iter()
            .rev()
            .find(|t| t.text == TextRef::Id(TextId(STATUS_DONE_TEXT)))
            .unwrap();
        assert_eq!(done.color, PackedColor(0x7878_787F).to_color());
    }

    #[test]
    fn empty_objectives_show_the_kind0_header_and_the_none_text() {
        let page = Objectives {
            widget: WidgetId(2),
            area: Edges::new(100.0, 200.0, 540.0, 300.0),
            text_unit: 6.4,
            header_font: 9,
            header_height: 20.0,
            item_font: 3,
            item_height: 16.0,
            header_color: PackedColor(0x7F7F_7F7F),
            empty_header: 0x15F4,
            groups: vec![],
        };
        let draw = draw_objectives(&page, &measure, &|_| None, viewport(), 1.0);
        assert_eq!(draw.texts.len(), 2);
        assert_eq!(draw.texts[1].text, TextRef::Id(TextId(NO_OBJECTIVES_TEXT)));
        assert_eq!(
            (
                draw.texts[1].rect_px.origin.x,
                draw.texts[1].rect_px.origin.y
            ),
            (490.0, 225.0)
        );
    }

    #[test]
    fn wrap_breaks_at_spaces_before_the_limit() {
        let m = |b: &[u8]| b.len() as f32;
        assert_eq!(
            wrap(b"ab cd ef", 5.0, &m),
            vec![b"ab cd".to_vec(), b"ef".to_vec()]
        );
        assert_eq!(wrap(b"abcdefgh", 3.0, &m), vec![b"abcdefgh".to_vec()]);
        assert_eq!(
            wrap(b"ab cd\nef", 5.0, &m),
            vec![b"ab cd".to_vec(), b"ef".to_vec()]
        );
        assert_eq!(wrap(b"ab cd", 0.0, &m), vec![b"ab cd".to_vec()]);
    }
}

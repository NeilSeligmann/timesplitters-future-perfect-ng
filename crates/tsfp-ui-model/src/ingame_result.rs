//! Typed challenge/result-summary layout (`0x2ED710` / `0x2ED1B0`, T2316).
//! Runtime providers resolve timer, challenge thresholds and medal assets; no game strings
//! or assets are embedded here. This is separate from the Story objective list.
use crate::ingame_rows::TextWidth;
use crate::list_rows::{Edges, alpha_byte};
use crate::model::TextRef;
use crate::skin::PackedColor;

pub const RESULT_ROW_DRAW: u32 = 0x2E_D710;

/// Wrapper admission predicate. The rect is page x/y plus width/height, not a row rect.
pub fn visible(game_flags: u32) -> bool {
    game_flags & (0x40 | 0x0800_0000) != 0
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultThreshold {
    pub medal: u32,
    /// Already localised/formatted at the Rust source boundary.
    pub label: Vec<u8>,
    pub value: Vec<u8>,
    /// Runtime medal palettes (`0x664E0` fill, `0x66520` outline).
    pub fill: PackedColor,
    pub edge: PackedColor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultSummary {
    pub area: Edges,
    /// None for challenge flags bit0x10. Value uses `0x65040`, not score `0x64FC0`.
    pub timer: Option<(Vec<u8>, Vec<u8>)>,
    /// `0x22D110(0)`: optional common and player-specific result text.
    pub common_result: Option<Vec<u8>>,
    pub player_result: Option<Vec<u8>>,
    /// Text0x3D8 label and resolved best result (`0x231FB0`, or text0x3D9).
    pub best_label: Vec<u8>,
    pub best_value: Vec<u8>,
    pub earned_medal: u32,
    /// Threshold order is3,2,1. The source must provide all three.
    pub thresholds: [ResultThreshold; 3],
}

/// Provider attachment point: the result subsystem owns formatting and reads of challenge
/// records; the UI consumes an owned snapshot. Returning None preserves the original drawer.
pub trait ResultSummarySource {
    fn snapshot(&self, area: Edges) -> Option<ResultSummary>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultText {
    pub bytes: Vec<u8>,
    pub x: f32,
    pub y: f32,
    pub font: u32,
    pub fill: PackedColor,
    pub edge: PackedColor,
}

/// A request to the medal renderer (`0x2BF650` contract), not a guessed single sprite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResultMedal {
    pub rank: u32,
    pub x: f32,
    pub y: f32,
    pub size: [u32; 2],
    pub fade: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultLayout {
    pub texts: Vec<ResultText>,
    pub medals: Vec<ResultMedal>,
    /// Original body returns final Y, including three threshold advances.
    pub end_y: f32,
    pub threshold_font: u32,
}

fn fade(color: PackedColor, factor: f32) -> PackedColor {
    PackedColor((color.0 & !255) | alpha_byte((color.0 & 255) as f32 * factor))
}

/// Reconstruction of the draw1 layout. Typed runs and medal requests let a renderer draw
/// each original outline pass and resolve assets without handlers or guest state writes.
pub fn layout(summary: &ResultSummary, width: &TextWidth<'_>, fade_value: f32) -> ResultLayout {
    let page_fade = fade_value.clamp(0.0, 1.0);
    let x = (summary.area.x0 + 20.0).trunc();
    let right = (summary.area.x1 - 20.0).trunc();
    let mut y = (summary.area.y0 + 15.0).trunc();
    let fill = PackedColor(0x6464_6400 | alpha_byte(page_fade * 63.5));
    let edge = PackedColor(0x5A14_4600 | alpha_byte(page_fade * 17.5));
    let mut out = ResultLayout {
        texts: Vec::new(),
        medals: Vec::new(),
        end_y: y,
        threshold_font: 21,
    };
    let measure = |bytes: &[u8], font| width(TextRef::Dynamic, bytes, font);
    let mut put = |bytes: &[u8], x, y, font, fill, edge| {
        out.texts.push(ResultText {
            bytes: bytes.to_vec(),
            x,
            y,
            font,
            fill,
            edge,
        })
    };
    if let Some((label, value)) = &summary.timer {
        put(label, x, y, 22, fill, edge);
        put(value, right - measure(value, 22), y, 22, fill, edge);
        y = (y + 33.0).trunc();
    }
    if let Some(common) = &summary.common_result {
        put(common, x, y, 22, fill, edge);
    }
    if let Some(player) = &summary.player_result {
        put(player, right - measure(player, 22), y, 22, fill, edge);
        y += 50.0;
    }
    // The max label width adds10 before truncation; max value width truncates separately.
    let labels = summary
        .thresholds
        .iter()
        .map(|r| (measure(&r.label, 21) + 10.0).trunc())
        .fold(measure(&summary.best_label, 21).trunc(), f32::max);
    let values = summary
        .thresholds
        .iter()
        .map(|r| measure(&r.value, 21).trunc())
        .fold(measure(&summary.best_value, 21).trunc(), f32::max);
    let font = if right - x - labels - values < 0.0 {
        20
    } else {
        21
    };
    out.threshold_font = font;
    put(&summary.best_label, x + 32.0, y, 21, fill, edge);
    put(
        &summary.best_value,
        right - measure(&summary.best_value, font),
        y,
        font,
        fill,
        edge,
    );
    out.medals.push(ResultMedal {
        rank: summary.earned_medal,
        x,
        y,
        size: [22, 22],
        fade: page_fade * 0.5,
    });
    y += 29.0;
    for row in &summary.thresholds {
        put(&row.label, x + 32.0, y, font, fill, edge);
        put(
            &row.value,
            right - measure(&row.value, font),
            y,
            font,
            fade(row.fill, page_fade),
            fade(row.edge, page_fade * 0.5),
        );
        out.medals.push(ResultMedal {
            rank: row.medal,
            x,
            y,
            size: [22, 22],
            fade: page_fade * 0.5,
        });
        y += 22.0;
    }
    out.end_y = y;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn summary() -> ResultSummary {
        ResultSummary {
            area: Edges::new(10.8, 20.8, 400.8, 300.0),
            timer: Some((b"clock".to_vec(), b"value".to_vec())),
            common_result: Some(b"common".to_vec()),
            player_result: Some(b"player".to_vec()),
            best_label: b"best".to_vec(),
            best_value: b"record".to_vec(),
            earned_medal: 2,
            thresholds: [3, 2, 1].map(|medal| ResultThreshold {
                medal,
                label: vec![b'A'; medal as usize],
                value: b"limit".to_vec(),
                fill: PackedColor(0x1234567F),
                edge: PackedColor(0x6543217F),
            }),
        }
    }
    #[test]
    fn timer_result_and_threshold_rows_have_distinct_advances_and_medals() {
        assert!(!visible(1));
        assert!(visible(0x40));
        assert!(visible(0x08000000));
        let out = layout(&summary(), &|_, b, _| b.len() as f32 * 10.0, 0.5);
        assert_eq!(
            (out.texts[0].x, out.texts[0].y, out.texts[0].font),
            (30.0, 35.0, 22)
        );
        assert_eq!((out.texts[2].y, out.texts[4].y), (68.0, 118.0));
        assert_eq!(
            out.medals.iter().map(|m| m.y).collect::<Vec<_>>(),
            [118.0, 147.0, 169.0, 191.0]
        );
        assert_eq!(
            out.medals.iter().map(|m| m.rank).collect::<Vec<_>>(),
            [2, 3, 2, 1]
        );
        assert_eq!(out.end_y, 213.0);
        assert_eq!(out.texts[7].fill, PackedColor(0x1234563F));
        assert_eq!(out.texts[7].edge, PackedColor(0x6543211F));
    }
    #[test]
    fn omitted_timer_and_player_result_do_not_reserve_their_vertical_space() {
        let mut summary = summary();
        summary.timer = None;
        summary.player_result = None;
        summary.area.x1 = 150.0;
        let out = layout(&summary, &|_, b, _| b.len() as f32 * 20.0, 1.0);
        assert_eq!(out.texts[0].y, 35.0);
        assert_eq!(out.medals[0].y, 35.0);
        assert_eq!(
            out.threshold_font, 20,
            "narrow columns choose smaller original font"
        );
        assert_eq!(out.texts[0].font, 22);
        assert_eq!(
            out.texts[1].font, 21,
            "best label draws before smaller-font decision"
        );
        assert_eq!(out.texts[2].font, 20);
        assert_eq!(out.end_y, 130.0);
    }
}

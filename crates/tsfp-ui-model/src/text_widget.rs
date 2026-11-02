//! Shared native box text policy. Original horizontal name fitting wins; other text
//! wraps at word boundaries. Overlong individual words are clipped, never split.

pub fn lines(
    bytes: &[u8],
    width: f32,
    fitted: bool,
    measure: &dyn Fn(&[u8]) -> f32,
) -> Vec<Vec<u8>> {
    bytes
        .split(|b| *b == b'\n')
        .flat_map(|paragraph| {
            if fitted || measure(paragraph) <= width {
                vec![paragraph.to_vec()]
            } else {
                word_lines(paragraph, width.max(0.0), measure)
            }
        })
        .collect()
}

fn word_lines(text: &[u8], width: f32, measure: &dyn Fn(&[u8]) -> f32) -> Vec<Vec<u8>> {
    let mut lines: Vec<Vec<u8>> = Vec::new();
    let mut current: Vec<u8> = Vec::new();
    for word in text.split(|b| *b == b' ').filter(|w| !w.is_empty()) {
        let mut candidate = current.clone();
        if !candidate.is_empty() {
            candidate.push(b' ');
        }
        candidate.extend_from_slice(word);
        if current.is_empty() || measure(&candidate) <= width {
            current = candidate;
        } else {
            lines.push(std::mem::take(&mut current));
            current.extend_from_slice(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn words_newlines_and_long_tokens() {
        let measure = |b: &[u8]| b.len() as f32;
        assert_eq!(
            lines(b"one two three\nfour", 7.0, false, &measure),
            [b"one two".to_vec(), b"three".to_vec(), b"four".to_vec()]
        );
        assert_eq!(
            lines(b"unbreakable", 3.0, false, &measure),
            [b"unbreakable".to_vec()]
        );
        assert_eq!(
            lines(b"one two three", 3.0, true, &measure),
            [b"one two three".to_vec()]
        );
    }
    #[test]
    fn widget_and_page_clip_intersect_without_restoring_empty_scissors() {
        use tsfp_ui_renderer::{Rect, Vec2};
        let rect = |x| Rect {
            origin: Vec2::new(x, 0.0),
            size: Vec2::new(10.0, 20.0),
        };
        assert_eq!(
            intersect_clip(Some(rect(0.0)), Some(rect(5.0)))
                .unwrap()
                .size
                .x,
            5.0
        );
        assert_eq!(
            intersect_clip(Some(rect(0.0)), Some(rect(15.0)))
                .unwrap()
                .size
                .x,
            0.0
        );
    }
}

/// Intersect widget and page/list scissors. An empty intersection remains an empty
/// scissor, rather than accidentally restoring the full viewport.
pub fn intersect_clip(
    a: Option<tsfp_ui_renderer::Rect>,
    b: Option<tsfp_ui_renderer::Rect>,
) -> Option<tsfp_ui_renderer::Rect> {
    use tsfp_ui_renderer::{Rect, Vec2};
    match (a, b) {
        (Some(a), Some(b)) => {
            let x = a.origin.x.max(b.origin.x);
            let y = a.origin.y.max(b.origin.y);
            Some(Rect {
                origin: Vec2::new(x, y),
                size: Vec2::new(
                    (a.origin.x + a.size.x).min(b.origin.x + b.size.x).max(x) - x,
                    (a.origin.y + a.size.y).min(b.origin.y + b.size.y).max(y) - y,
                ),
            })
        }
        (a, b) => a.or(b),
    }
}

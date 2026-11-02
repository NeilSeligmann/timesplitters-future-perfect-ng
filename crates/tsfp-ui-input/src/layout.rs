//! Layout seam: what UiInput needs to know about a widget tree.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WidgetId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

/// Rectangle in the 640x480 design space (x may leave 0..640 under widescreen anchoring).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    /// Half-open: left/top edges inside, right/bottom edges outside, so abutting rects never overlap.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w * 0.5, self.y + self.h * 0.5)
    }
}

/// One widget as seen by input. Implemented by the model's widgets or by test fixtures.
pub trait HitTarget {
    fn id(&self) -> WidgetId;
    fn rect(&self) -> Rect;
    /// Higher layers win hit tests; equal layers: the later visited wins (drawn on top).
    fn layer(&self) -> i32 {
        0
    }
    fn enabled(&self) -> bool {
        true
    }
    /// Can receive focus and be activated.
    fn focusable(&self) -> bool {
        true
    }
    /// Can be chosen as the destination of directional movement. This is separate from
    /// `focusable`: the current selection may remain on a row skipped by directional input.
    fn directionally_navigable(&self) -> bool {
        self.enabled() && self.focusable()
    }
    /// Scrollable list this widget belongs to, if any (wheel and page intents scroll it).
    fn scroll_list(&self) -> Option<crate::bindings::ListId> {
        None
    }
}

/// A page's widgets in focus (tab) order.
pub trait UiLayout {
    fn visit(&self, f: &mut dyn FnMut(&dyn HitTarget));
    /// Authored neighbour link (original pages have explicit ones); `None` falls back to geometry.
    fn neighbour(&self, _from: WidgetId, _dir: Dir) -> Option<WidgetId> {
        None
    }
}

fn collect(layout: &dyn UiLayout) -> Vec<Snap> {
    let mut out = Vec::new();
    layout.visit(&mut |t| {
        out.push(Snap {
            id: t.id(),
            rect: t.rect(),
            layer: t.layer(),
            usable: t.enabled() && t.focusable(),
            navigable: t.directionally_navigable(),
            enabled: t.enabled(),
            list: t.scroll_list(),
        })
    });
    out
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Snap {
    pub id: WidgetId,
    pub rect: Rect,
    pub layer: i32,
    pub usable: bool,
    pub navigable: bool,
    pub enabled: bool,
    pub list: Option<crate::bindings::ListId>,
}

pub(crate) fn snapshot(layout: &dyn UiLayout) -> Vec<Snap> {
    collect(layout)
}

/// Topmost enabled widget under the point.
pub fn hit_test(layout: &dyn UiLayout, px: f32, py: f32) -> Option<WidgetId> {
    hit_snap(&collect(layout), px, py).map(|s| s.id)
}

pub(crate) fn hit_snap(snaps: &[Snap], px: f32, py: f32) -> Option<Snap> {
    let mut best: Option<Snap> = None;
    for s in snaps {
        if s.enabled && s.rect.contains(px, py) && best.is_none_or(|b| s.layer >= b.layer) {
            best = Some(*s);
        }
    }
    best
}

/// Next focus in `dir`: authored link, else nearest usable widget in that half-plane
/// (axis distance plus twice the cross-axis offset), else wrap to the far side when `wrap`.
pub(crate) fn navigate(
    layout: &dyn UiLayout,
    snaps: &[Snap],
    from: WidgetId,
    dir: Dir,
    wrap: bool,
) -> Option<WidgetId> {
    if let Some(n) = layout.neighbour(from, dir) {
        if snaps.iter().any(|s| s.id == n && s.navigable) {
            return Some(n);
        }
    }
    let cur = snaps.iter().find(|s| s.id == from)?;
    let (cx, cy) = cur.rect.center();
    let score = |s: &Snap, sign: f32| -> Option<f32> {
        let (x, y) = s.rect.center();
        let (axis, cross) = match dir {
            Dir::Up => ((cy - y) * sign, (x - cx).abs()),
            Dir::Down => ((y - cy) * sign, (x - cx).abs()),
            Dir::Left => ((cx - x) * sign, (y - cy).abs()),
            Dir::Right => ((x - cx) * sign, (y - cy).abs()),
        };
        (axis > 0.0).then_some(axis + 2.0 * cross)
    };
    let pick = |sign: f32, far: bool| {
        let mut best: Option<(f32, WidgetId)> = None;
        for s in snaps.iter().filter(|s| s.navigable && s.id != from) {
            if let Some(v) = score(s, sign) {
                let v = if far { -v } else { v };
                if best.is_none_or(|(b, _)| v < b) {
                    best = Some((v, s.id));
                }
            }
        }
        best.map(|b| b.1)
    };
    pick(1.0, false).or_else(|| if wrap { pick(-1.0, true) } else { None })
}

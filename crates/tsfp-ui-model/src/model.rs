//! Data types of the UI model: pages, widgets, options, theme, transitions.
//! Text is referenced by TEXT ID only. Guest addresses appear only as opaque
//! `HandlerRef::Legacy` tokens that name an original handler to call through bindings.

use crate::focus::{Dir, FocusGraph, NavLayout, Wrap};
use std::collections::BTreeMap;
use tsfp_ui_renderer::{AnchorSpace, AnchoredRect, Color, Edge, Vec2};

/// Logical design space shared with `tsfp-ui-renderer`.
pub const DESIGN_WIDTH: f32 = 640.0;
pub const DESIGN_HEIGHT: f32 = 480.0;

/// Stable page identity (the descriptor id of the dump, never a slot).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageId(pub u64);
/// Widget identity, unique inside one page. Loaded widgets use the row id;
/// patch-added widgets use ids at or above `WidgetId::PATCH_BASE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WidgetId(pub u64);
impl WidgetId {
    pub const PATCH_BASE: u64 = 1 << 32;
    /// Rows the dump gives no usable id (negative spacer ids such as -1, or an id already used on
    /// the page) get `ANON_BASE + row index`, so ids stay unique and stable for one capture.
    pub const ANON_BASE: u64 = 1 << 33;
    pub const fn anon(index: usize) -> Self {
        Self(Self::ANON_BASE + index as u64)
    }
    pub const fn patch(n: u32) -> Self {
        Self(Self::PATCH_BASE + n as u64)
    }
}
/// Index into a string bank. Strings themselves never enter the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextRef {
    None,
    Id(TextId),
    /// Runtime formatted text, supplied by a binding.
    Dynamic,
}

/// Name of a handler implemented elsewhere (original code, or a native handler).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandlerRef {
    /// Opaque token of an original handler.
    Legacy(u32),
    Named(String),
}

/// Name of a value binding resolved through `UiBindings`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BindingId(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WidgetKind {
    Label,
    Button,
    Cycler,
    Slider,
    List,
    TextField,
    Panel,
    Image,
    Custom(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WidgetRole {
    Plain,
    Title,
    Back,
    Accept,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiOption {
    pub id: u32,
    pub text: TextRef,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Widget {
    pub id: WidgetId,
    pub kind: WidgetKind,
    pub role: WidgetRole,
    pub rect: AnchoredRect,
    pub text: TextRef,
    pub font_index: u32,
    pub value_font_index: u32,
    pub align: Align,
    pub enabled: bool,
    /// Whether the cursor may land on this widget; disabled rows need the explicit override below.
    pub navigable: bool,
    /// Allow focus on a disabled widget when the native menu contract requires it.
    /// Activation remains subject to the bound Rust handler's state checks.
    pub focus_when_disabled: bool,
    /// Keep this widget available as stored focus, but skip it as a directional destination.
    pub directional_skip: bool,
    pub options: Vec<UiOption>,
    /// Index into `options`.
    pub selected: usize,
    pub bound: Option<BindingId>,
    pub activate: Option<HandlerRef>,
    pub draw: Option<HandlerRef>,
    /// Text drawn upper case (original row flag 0x2000, case conversion `0x3FB90`, T2307).
    pub uppercase: bool,
    /// T2315: where the original default row draw `0x76150` anchors the label, read from the live row record; `None`
    /// keeps the skin's page-wide rule (centred rows, `text_drop`).
    pub text_anchor: Option<TextAnchor>,
    /// T2315: the part of `rect` inside the page's list view (a scrolled row's cut layout box, `0x722B0`); the page
    /// frame and hit testing use it. `None`: the whole `rect`.
    pub bounds: Option<AnchoredRect>,
    /// T2315: where the option value draw `0x7B7B0` places the selected option's text, relative to the row rect like
    /// [`Widget::text_anchor`] (`None`: the value goes with the label placement as before).
    pub value_anchor: Option<TextAnchor>,
}

/// T2315: the label anchor of `0x76150(page, row, rect)` (MEASURED from code): `x = rect.x0 + row[+0x3C] * [0x7BA038]`,
/// `y = rect.y0 + row[+0x40] * [0x7BA188]`, alignment from row `+0x30` (0 start at the anchor, 2 centred on it, the row
/// layout writes `+0x3C` = half the row width for those; 1 sets the end mode, INFERRED). Design units from the row's
/// top-left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextAnchor {
    pub x: f32,
    pub y: f32,
    pub align: Align,
}

impl Widget {
    pub fn new(id: WidgetId, kind: WidgetKind) -> Self {
        Self {
            id,
            kind,
            role: WidgetRole::Plain,
            rect: menu_rect(Vec2::new(0.0, 0.0), Vec2::new(0.0, 0.0)),
            text: TextRef::None,
            font_index: 0,
            value_font_index: 0,
            align: Align::Start,
            enabled: true,
            navigable: true,
            focus_when_disabled: false,
            directional_skip: false,
            options: Vec::new(),
            selected: 0,
            bound: None,
            activate: None,
            draw: None,
            uppercase: false,
            text_anchor: None,
            bounds: None,
            value_anchor: None,
        }
    }
    /// Whether this row can remain the current focus target, including while temporarily disabled.
    pub fn can_retain_focus(&self) -> bool {
        self.navigable && !matches!(self.kind, WidgetKind::Panel | WidgetKind::Image)
    }
    pub fn is_focusable(&self) -> bool {
        (self.enabled || self.focus_when_disabled) && self.can_retain_focus()
    }
}

/// Menu geometry: top-left anchored in the 4:3 safe area, logical units.
pub fn menu_rect(offset: Vec2, size: Vec2) -> AnchoredRect {
    AnchoredRect {
        space: AnchorSpace::SafeArea4x3,
        horizontal: Edge::Start,
        vertical: Edge::Start,
        offset,
        size,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandlerSlot {
    Select,
    Back,
    AfterSelect,
    Gate,
    Draw,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageHandlers {
    pub select: Option<HandlerRef>,
    pub back: Option<HandlerRef>,
    pub after_select: Option<HandlerRef>,
    pub gate: Option<HandlerRef>,
    pub draw: Option<HandlerRef>,
}
impl PageHandlers {
    pub fn slot(&self, slot: HandlerSlot) -> &Option<HandlerRef> {
        match slot {
            HandlerSlot::Select => &self.select,
            HandlerSlot::Back => &self.back,
            HandlerSlot::AfterSelect => &self.after_select,
            HandlerSlot::Gate => &self.gate,
            HandlerSlot::Draw => &self.draw,
        }
    }
    pub fn slot_mut(&mut self, slot: HandlerSlot) -> &mut Option<HandlerRef> {
        match slot {
            HandlerSlot::Select => &mut self.select,
            HandlerSlot::Back => &mut self.back,
            HandlerSlot::AfterSelect => &mut self.after_select,
            HandlerSlot::Gate => &mut self.gate,
            HandlerSlot::Draw => &mut self.draw,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ease {
    Linear,
    In,
    Out,
    InOut,
}
impl Ease {
    /// Quadratic easing of `t` in 0..=1.
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Ease::Linear => t,
            Ease::In => t * t,
            Ease::Out => t * (2.0 - t),
            Ease::InOut if t < 0.5 => 2.0 * t * t,
            Ease::InOut => -1.0 + (4.0 - 2.0 * t) * t,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionKind {
    None,
    Fade,
    Slide(Dir),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition {
    pub kind: TransitionKind,
    pub duration_ms: u32,
    pub ease: Ease,
}
impl Transition {
    pub const NONE: Transition = Transition {
        kind: TransitionKind::None,
        duration_ms: 0,
        ease: Ease::Linear,
    };
}

/// Running animation as plain data; `advance` is deterministic in integer milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimState {
    pub transition: Transition,
    pub elapsed_ms: u32,
}
impl AnimState {
    pub fn start(transition: Transition) -> Self {
        Self {
            transition,
            elapsed_ms: 0,
        }
    }
    pub fn advance(&mut self, dt_ms: u32) {
        self.elapsed_ms = self
            .elapsed_ms
            .saturating_add(dt_ms)
            .min(self.transition.duration_ms);
    }
    pub fn finished(&self) -> bool {
        self.elapsed_ms >= self.transition.duration_ms
    }
    /// Eased progress 0..=1. A zero duration is finished at once.
    pub fn progress(&self) -> f32 {
        if self.transition.duration_ms == 0 {
            return 1.0;
        }
        self.transition
            .ease
            .apply(self.elapsed_ms as f32 / self.transition.duration_ms as f32)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub id: PageId,
    /// Widget family of the original page record (1 list, 2 cycler page, 3 panel).
    pub widget_type: u32,
    pub widgets: Vec<Widget>,
    /// Original page box in design units; absent for dump-only pages without page geometry.
    pub panel_bounds: Option<AnchoredRect>,
    /// Focus callback edge thresholds in design units (original front-end style).
    pub highlight_edges: Option<HighlightEdges>,
    pub handlers: PageHandlers,
    pub nav: NavLayout,
    pub wrap: Wrap,
    /// Explicit neighbour overrides applied after the generated graph.
    pub links: Vec<FocusLink>,
    pub initial_focus: Option<WidgetId>,
    pub enter: Transition,
    pub exit: Transition,
}

/// The front-end focus callback extends the first or last row towards the page rim.
/// Raw page edges are retained independently of the integer panel draw bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HighlightEdges {
    pub top: f32,
    pub bottom: f32,
    pub top_margin: f32,
    pub bottom_margin: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusLink {
    pub from: WidgetId,
    pub dir: Dir,
    pub to: WidgetId,
}

impl Page {
    pub fn new(id: PageId, widget_type: u32) -> Self {
        Self {
            id,
            widget_type,
            widgets: Vec::new(),
            panel_bounds: None,
            highlight_edges: None,
            handlers: PageHandlers::default(),
            nav: NavLayout::Grid(1),
            wrap: Wrap::Wrap,
            links: Vec::new(),
            initial_focus: None,
            enter: Transition::NONE,
            exit: Transition::NONE,
        }
    }
    pub fn widget(&self, id: WidgetId) -> Option<&Widget> {
        self.widgets.iter().find(|w| w.id == id)
    }
    pub fn widget_mut(&mut self, id: WidgetId) -> Option<&mut Widget> {
        self.widgets.iter_mut().find(|w| w.id == id)
    }
    pub fn focus_graph(&self) -> FocusGraph {
        FocusGraph::build(self)
    }
    /// First focus target: the declared initial focus if focusable, else the first focusable widget.
    pub fn start_focus(&self) -> Option<WidgetId> {
        self.initial_focus
            .and_then(|id| self.widget(id))
            .filter(|w| w.is_focusable())
            .or_else(|| self.widgets.iter().find(|w| w.is_focusable()))
            .map(|w| w.id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    Open,
    Close,
}

/// A page-to-page navigation observed in the dump (`None` is the root, no page).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageEdge {
    pub from: Option<PageId>,
    pub to: Option<PageId>,
    pub kind: EdgeKind,
    /// Button and stick names recorded for the edge.
    pub input: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub name: String,
    pub background: Color,
    pub panel: Color,
    pub focus: Color,
    pub text: Color,
    pub text_disabled: Color,
    pub row_height: f32,
    pub row_gap: f32,
    pub column_width: f32,
    pub top: f32,
}
impl Theme {
    /// Neutral synthetic defaults. They are layout placeholders, not recovered original colours.
    pub fn original() -> Self {
        Self {
            name: "original".to_string(),
            background: Color([0.0, 0.0, 0.0, 0.0]),
            panel: Color([0.1, 0.1, 0.1, 0.8]),
            focus: Color([0.9, 0.7, 0.1, 0.9]),
            text: Color([1.0, 1.0, 1.0, 1.0]),
            text_disabled: Color([0.5, 0.5, 0.5, 1.0]),
            row_height: 24.0,
            row_gap: 4.0,
            column_width: 320.0,
            top: 96.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiModel {
    pub pages: BTreeMap<PageId, Page>,
    pub root: Option<PageId>,
    pub edges: Vec<PageEdge>,
    pub theme: Theme,
    /// Name of the preset applied to this model (`original` is the empty patch list).
    pub preset: String,
}

impl Default for UiModel {
    fn default() -> Self {
        Self::new()
    }
}

impl UiModel {
    pub fn new() -> Self {
        Self {
            pages: BTreeMap::new(),
            root: None,
            edges: Vec::new(),
            theme: Theme::original(),
            preset: "original".to_string(),
        }
    }
    pub fn page(&self, id: PageId) -> Option<&Page> {
        self.pages.get(&id)
    }
    pub fn page_mut(&mut self, id: PageId) -> Option<&mut Page> {
        self.pages.get_mut(&id)
    }
}

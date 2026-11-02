//! Backend-independent UI drawing contract (T2187).
//! Layout is logical 640×480; submitted geometry is in window pixels.
//! No original-preset parity is claimed by this contract alone.

/// A position or extent in the coordinate space documented by its containing type.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}
impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Top-left origin, nonnegative extent. Pixel rectangles use continuous coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub origin: Vec2,
    pub size: Vec2,
}
impl Rect {
    pub fn new(origin: Vec2, size: Vec2) -> Result<Self, LayoutError> {
        if ![origin.x, origin.y, size.x, size.y]
            .iter()
            .all(|v| v.is_finite())
            || size.x < 0.0
            || size.y < 0.0
        {
            return Err(LayoutError);
        }
        Ok(Self { origin, size })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutError;

/// Caller-supplied safe area in window pixels; usually a centred 4:3 region.
/// Scale is pixels per logical unit, independent of OS DPI.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    size_px: Vec2,
    scale: f32,
    safe_area: Rect,
}
impl Viewport {
    pub fn new(size_px: Vec2, scale: f32, safe_area: Rect) -> Result<Self, LayoutError> {
        Rect::new(Vec2::default(), size_px)?;
        Rect::new(safe_area.origin, safe_area.size)?;
        if size_px.x <= 0.0
            || size_px.y <= 0.0
            || !scale.is_finite()
            || scale <= 0.0
            || safe_area.origin.x < 0.0
            || safe_area.origin.y < 0.0
            || safe_area.origin.x + safe_area.size.x > size_px.x
            || safe_area.origin.y + safe_area.size.y > size_px.y
        {
            return Err(LayoutError);
        }
        Ok(Self {
            size_px,
            scale,
            safe_area,
        })
    }
    pub fn size_px(self) -> Vec2 {
        self.size_px
    }
    pub fn scale(self) -> f32 {
        self.scale
    }
    pub fn safe_area(self) -> Rect {
        self.safe_area
    }

    /// Align both the child and reference box at the selected edge; offsets are
    /// logical units, positive right/down. No implicit stretch or aspect change.
    ///
    /// ```
    /// use tsfp_ui_renderer::{AnchorSpace, AnchoredRect, Edge, Rect, Vec2, Viewport};
    /// let safe = Rect::new(Vec2::new(160.0, 0.0), Vec2::new(960.0, 720.0))?;
    /// let view = Viewport::new(Vec2::new(1280.0, 720.0), 1.5, safe)?;
    /// let menu = AnchoredRect {
    ///     space: AnchorSpace::SafeArea4x3,
    ///     horizontal: Edge::Center,
    ///     vertical: Edge::Center,
    ///     offset: Vec2::new(0.0, 0.0),
    ///     size: Vec2::new(100.0, 40.0),
    /// };
    /// assert_eq!(view.resolve(menu)?.origin, Vec2::new(565.0, 330.0));
    /// # Ok::<(), tsfp_ui_renderer::LayoutError>(())
    /// ```
    pub fn resolve(self, rect: AnchoredRect) -> Result<Rect, LayoutError> {
        Rect::new(rect.offset, rect.size)?;
        let area = match rect.space {
            AnchorSpace::SafeArea4x3 => self.safe_area,
            AnchorSpace::Window => Rect::new(Vec2::default(), self.size_px)?,
        };
        let size = Vec2::new(rect.size.x * self.scale, rect.size.y * self.scale);
        let origin = Vec2::new(
            area.origin.x
                + (area.size.x - size.x) * rect.horizontal.fraction()
                + rect.offset.x * self.scale,
            area.origin.y
                + (area.size.y - size.y) * rect.vertical.fraction()
                + rect.offset.y * self.scale,
        );
        Rect::new(origin, size)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchorSpace {
    SafeArea4x3,
    Window,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Start,
    Center,
    End,
}
impl Edge {
    fn fraction(self) -> f32 {
        match self {
            Self::Start => 0.0,
            Self::Center => 0.5,
            Self::End => 1.0,
        }
    }
}
/// Logical-unit geometry; use Window for HUD edge anchoring, SafeArea4x3 for menus.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnchoredRect {
    pub space: AnchorSpace,
    pub horizontal: Edge,
    pub vertical: Edge,
    pub offset: Vec2,
    pub size: Vec2,
}

/// Linear RGBA, straight alpha. Backends use source-over blending.
/// Premultiplied-looking values are still interpreted as straight: representation
/// cannot be inferred from RGBA alone. Loaders must convert premultiplied assets.
/// All channels must be finite; alpha is in [0, 1].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color(pub [f32; 4]);
/// Opaque backend resource key, never a guest address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureId(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FontId(pub u64);

impl FontId {
    /// Runtime glyph scale in thousandths, shared by shaping and atlas lookup.
    /// Low 32 bits hold the source font; a zero scale field means identity.
    pub fn scaled(font: u32, scale: [f32; 2]) -> Self {
        let part = |v: f32| -> u64 {
            if (v - 1.0).abs() < 1e-4 { 0 } else { (v * 1000.0).round().clamp(1.0, 65535.0) as u64 }
        };
        Self(u64::from(font) | part(scale[0]) << 32 | part(scale[1]) << 48)
    }

    pub fn source_and_scale(self) -> (u32, [f32; 2]) {
        let part = |bits: u64| if bits == 0 { 1.0 } else { bits as f32 / 1000.0 };
        (self.0 as u32, [part(self.0 >> 32 & 0xffff), part(self.0 >> 48 & 0xffff)])
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlyphId(pub u32);
/// Normalised texture coordinates, top-left origin. Flipping is allowed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvRect {
    pub min: Vec2,
    pub max: Vec2,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub rect_px: Rect,
    pub color: Color,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sprite {
    pub rect_px: Rect,
    pub uv: UvRect,
    pub color: Color,
    pub texture: TextureId,
}
/// A textured quad with free corners (rotated HUD sprites, T2312), in pixels. Corners and
/// texture coordinates are in strip order: the texture's top-left, top-right, bottom-left and
/// bottom-right (two triangles 0-1-2 and 2-1-3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Poly {
    pub corners_px: [Vec2; 4],
    pub uv: [Vec2; 4],
    pub color: Color,
    pub texture: TextureId,
}
/// Glyph position is a baseline offset in pixels relative to TextRun::baseline_px.
/// The layout provider, not the renderer, performs shaping/kerning/line breaking.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PositionedGlyph {
    pub id: GlyphId,
    pub offset_px: Vec2,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextRun<'a> {
    pub font: FontId,
    /// Positive raster height used for GlyphKey lookup.
    pub raster_height_px: u32,
    pub glyphs: &'a [PositionedGlyph],
    pub baseline_px: Vec2,
    pub color: Color,
}
/// Metrics and atlas region at the requested raster size, in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtlasGlyph {
    pub texture: TextureId,
    pub uv: UvRect,
    pub size_px: Vec2,
    pub bearing_px: Vec2,
    pub advance_px: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub font: FontId,
    pub glyph: GlyphId,
    pub raster_height_px: u32,
}
/// Providers may load original bitmap fonts or rasterise opt-in scalable fonts.
/// Returned texture/UV handles remain valid until the current frame ends.
/// None means missing glyph; the caller chooses replacement or omission explicitly.
pub trait GlyphAtlas {
    type Error;
    fn prepare(&mut self, keys: &[GlyphKey]) -> Result<(), Self::Error>;
    fn glyph(&self, key: GlyphKey) -> Option<AtlasGlyph>;
}

/// How a draw combines with the pixels under it (T2309). `Alpha` is straight alpha blending
/// (src * a + dst * (1 - a)), the default of every frame. `Additive` adds src * a to the
/// destination (the original's blend mode 2 of `0x19D70`, used by the focused-row fill).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlendMode {
    #[default]
    Alpha,
    Additive,
}

/// Batches preserve painter order. Clip replaces the current pixel scissor;
/// None resets to the viewport, Some(empty) suppresses drawing. No implicit stack.
/// Blend replaces the current blend mode for the following batches; every frame
/// begins with [`BlendMode::Alpha`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Batch<'a> {
    Quads(&'a [Quad]),
    Sprites(&'a [Sprite]),
    /// Textured quads with free corners (T2312), drawn like sprites.
    Polys(&'a [Poly]),
    Text(TextRun<'a>),
    Clip(Option<Rect>),
    Blend(BlendMode),
}
/// begin -> submit* -> end. Implementations reject invalid lifecycle/geometry
/// through Error. Empty batches are valid. Data is borrowed only for each call.
/// No sorting across batches, retained guest pointers, or implicit presentation.
/// On any error, abort_frame releases transient state before another begin.
pub trait UiRenderer {
    type Error;
    fn begin_frame(&mut self, viewport: Viewport) -> Result<(), Self::Error>;
    fn submit(&mut self, batches: &[Batch<'_>]) -> Result<(), Self::Error>;
    fn end_frame(&mut self) -> Result<(), Self::Error>;
    fn abort_frame(&mut self);
}
/// Legacy backend seam: preserves batch/clip order while emitting original draw
/// calls. Implementers own resource mapping and pixel-to-legacy conversion.
/// This signature does not implement or claim fidelity of that conversion.
pub trait LegacyDrawSink {
    type Error;
    fn begin_ui(&mut self, viewport: Viewport) -> Result<(), Self::Error>;
    fn emit_ui(&mut self, batch: Batch<'_>) -> Result<(), Self::Error>;
    fn end_ui(&mut self) -> Result<(), Self::Error>;
    fn abort_ui(&mut self);
}

/// Compileable forwarding seam for a future legacy converter. The sink owns
/// conversion and validation; this adapter only preserves calls and their order.
/// It performs no original rendering and does not establish original parity.
pub struct LegacyAdapter<S>(pub S);
impl<S: LegacyDrawSink> UiRenderer for LegacyAdapter<S> {
    type Error = S::Error;
    fn begin_frame(&mut self, viewport: Viewport) -> Result<(), Self::Error> {
        self.0.begin_ui(viewport)
    }
    fn submit(&mut self, batches: &[Batch<'_>]) -> Result<(), Self::Error> {
        for batch in batches {
            self.0.emit_ui(*batch)?;
        }
        Ok(())
    }
    fn end_frame(&mut self) -> Result<(), Self::Error> {
        self.0.end_ui()
    }
    fn abort_frame(&mut self) {
        self.0.abort_ui();
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
//! T1916 increment 3: the row and widget draws of a default-style page read from the live rows
//! (typed view; `tsfp_ui_model::default_widgets` draws them). Addresses MEASURED from the lift
//! (docs/t-system-mapmaker-ui.md, "Increment 3").
use crate::list_rows_live::texture_name;
use crate::live_page::{LivePage, LiveRow, LiveText, draw_fields as f, style_height, widget_id};
use crate::native_page::GuestUi;
use tsfp_ui_model::default_widgets::{
    GROUP_TITLE_STYLE, PlacedText, STORAGE_HARD_DISK, STORAGE_MEMORY_UNIT_A, STORAGE_MEMORY_UNIT_B,
    ScrollBar, TexturePart, Units, WidgetDraw, WidgetText, Widgets, Wrap,
};
use tsfp_ui_model::list_rows::Edges;
use tsfp_ui_model::model::TextRef;
use tsfp_ui_model::skin::PackedColor;

pub mod addr {
    /// Row draw callbacks with a Rust implementation.
    pub const INSET_FRAME_DRAW: u32 = 0x31_2610;
    pub const DIVIDER_DRAW: u32 = 0x7_3110;
    pub const STORAGE_ICON_DRAW: u32 = 0x19_1470;
    /// Colour block: divider line colours (`0x73110`).
    pub const DIVIDER_UPPER: u32 = 0x3C;
    pub const DIVIDER_LOWER: u32 = 0x40;
    /// Row `+0x14`: the storage row's device slot (`0x191470`).
    pub const ROW_DEVICE: u32 = 0x14;
    /// `0x191470`: when either flag is set the slot indexes the device table (stride 36, first word the device).
    pub const DEVICE_REMAP_A: u32 = 0x74_F9E4;
    pub const DEVICE_REMAP_B: u32 = 0x74_F9E8;
    pub const DEVICE_TABLE: u32 = 0x79_EC60;
    pub const DEVICE_TABLE_STRIDE: u32 = 36;
    /// `0x24E60(device)`: device records (stride 48, devices 0..10), "memory unit A" when bytes 0 and 3 are set.
    pub const DEVICE_RECORDS: u32 = 0x56_55B0;
    pub const DEVICE_RECORD_STRIDE: u32 = 48;
    pub const DEVICE_COUNT: u32 = 11;
    /// Devices 8 and up are the hard disk.
    pub const FIRST_HARD_DISK: u32 = 8;
    /// `0x7BCD0`: the radio option row draw (`0x7BB90` with `checked = [[row +0x8C] +0xC8] == row +0xD0`).
    pub const RADIO_DRAW: u32 = 0x7_BCD0;
    pub const RADIO_GROUP: u32 = 0x8C;
    pub const RADIO_VALUE: u32 = 0xD0;
    /// Row `+0xC8`: the widget value (check box on when 1, slider percent, radio group selection).
    pub const VALUE: u32 = 0xC8;
    /// Row `+0x44`: the text wrap width in x units (`0x76150`, 0 = one line).
    pub const WRAP_WIDTH: u32 = 0x44;
    /// Row word `+0x9A`: slider tick count.
    pub const TICKS: u32 = 0x9A;
    /// Page record `+0x24`: the style margins (four f32 in units: left, top, right, bottom; `0x76810` insets the group
    /// box by them).
    pub const MARGINS: u32 = 0x24;
    /// Page record `+0x38`: the UI context index; the context records `0x7BA260 + 0x1E0 * index` hold the pointer at
    /// `+0x20` / `+0x24` (design units, `0x711F0`).
    pub const CONTEXT: u32 = 0x38;
    pub const CONTEXTS: u32 = 0x7B_A260;
    pub const CONTEXT_STRIDE: u32 = 0x1E0;
    pub const POINTER_X: u32 = 0x20;
    pub const POINTER_Y: u32 = 0x24;
    /// `[0x7BA030]`: the check box size scale (`0x7BB90`: size = `[0x7BA030] * style height - 1`).
    pub const CHECK_SCALE: u32 = 0x7B_A030;
    /// Colour block: box gradient top `+0x84` and bottom `+0x88` (`0x72560`).
    pub const BOX_TOP: u32 = 0x84;
    pub const BOX_BOTTOM: u32 = 0x88;
    /// Row kinds (`+0xB4`) drawn by `0x7F8B0`.
    pub const KIND_RADIO_GROUP: u32 = 3;
    pub const KIND_CHECK_BOX: u32 = 4;
    pub const KIND_SLIDER: u32 = 7;
    pub const KIND_DROP_DOWN: u32 = 2;
    pub const KIND_BUTTON: u32 = 5;
    /// Map Settings row draws: the label `0x319E60` and the tile light cell `0x31AD90`.
    pub const LABEL_DRAW: u32 = 0x31_9E60;
    pub const LIGHT_DRAW: u32 = 0x31_AD90;
    /// `0x319E60`: the label string is row `+0x14` on the page whose record `+4` is 0x52EA48, else string 0xBC2.
    pub const LABEL_PAGE_VTABLE: u32 = 0x52_EA48;
    pub const LABEL_STRING: u32 = 0x14;
    pub const LABEL_DEFAULT_STRING: u32 = 0xBC2;
    /// `0x70EF0(page, 0)`: the page style's font `+0x48`; `0x319E60` falls back to font 2.
    pub const STYLE_FONT: u32 = 0x48;
    pub const LABEL_FALLBACK_FONT: u32 = 2;
    /// Colour block `+0x8C`: the drop-down glow on the cursor row (`0x7BAA0`).
    pub const GLOW: u32 = 0x8C;
    /// Row `+0xB8` bit 0x200000: no selected option (`0x757A0`).
    pub const NO_SELECTION: u32 = 0x20_0000;
    /// Row `+0x48`: the button text (a C string, `0x7F3B0`).
    pub const BUTTON_TEXT: u32 = 0x48;
    /// Page style `+0x20`: the right inset (x units) of a right-aligned button text.
    pub const STYLE_RIGHT_INSET: u32 = 0x20;
    /// Arrow sprite texture handle (`0x81BC0` and the side arrows of `0x7B7B0`) and its down-arrow part (1/16 texels).
    pub const ARROW_TEXTURE: u32 = 0x4D_6C7C;
    pub const DOWN_ARROW_PART: (u32, u32, u32, u32) = (0x600, 0x3F0, 0x7F0, 0x200);
    /// `0x81EC0` frame texture handle, `0x81C10(0x27)`: UI texture slots `[0x7BA040 + 4 * index]`, the bulb is 0x27.
    pub const FRAME_TEXTURE: u32 = 0x4D_4564;
    pub const UI_TEXTURES: u32 = 0x7B_A040;
    pub const BULB_SLOT: u32 = 0x27;
    /// `0x31AD90`: row `+0x38` handler; 0x320030 uses the zero light descriptor, the others not modelled read the
    /// current descriptor `0x783A60` (five words); the palette `0x783A80` (u32 colours, by byte index).
    pub const LIGHT_HANDLER: u32 = 0x38;
    pub const LIGHT_HANDLER_ZERO: u32 = 0x32_0030;
    pub const LIGHT_DESCRIPTOR: u32 = 0x78_3A60;
    pub const LIGHT_PALETTE: u32 = 0x78_3A80;
    /// Page record scroll fields (`0x71400`): total `+0x5C`, view top `+0x6C` and bottom `+0x70`, offset `+0x84` (units),
    /// y origin `+0xAC` (design units), hover state `+0x18C` / `+0x194`; style `+0x70` side, `+0x74` / `+0x78` x offsets.
    pub const SCROLL_TOTAL: u32 = 0x5C;
    pub const VIEW_TOP: u32 = 0x6C;
    pub const VIEW_BOTTOM: u32 = 0x70;
    pub const SCROLL: u32 = 0x84;
    pub const PAGE_Y: u32 = 0xAC;
    pub const HOVER_A: u32 = 0x18C;
    pub const HOVER_B: u32 = 0x194;
    pub const STYLE_BAR_RIGHT: u32 = 0x70;
    pub const STYLE_BAR_X0: u32 = 0x74;
    pub const STYLE_BAR_X1: u32 = 0x78;
    /// Style word bit 0x40: the page draws the scroll bar `0x727F0`.
    pub const STYLE_SCROLL_BAR: u32 = 0x40;
    /// Scroll bar textures (table entries) and the arrow height share `[0x47839C]` (1.15).
    pub const BAR_TEXTURES: [u32; 4] = [0x3F0, 0x3F1, 0x3F2, 0x3F3];
    pub const ARROW_SHARE: f32 = 1.15;
    /// Colour block: scroll bar colours (`+0x60` / `+0x68` left, `+0x64` / `+0x6C` right) and the hover factor `+0x80`.
    pub const BAR_TOP: u32 = 0x60;
    pub const BAR_BOTTOM: u32 = 0x68;
    pub const BAR_HOVER: u32 = 0x80;
}

/// Row draw callbacks [`read`] turns into widget draws.
pub const ROW_DRAWS: &[u32] = &[
    addr::INSET_FRAME_DRAW,
    addr::DIVIDER_DRAW,
    addr::STORAGE_ICON_DRAW,
    addr::RADIO_DRAW,
    addr::LABEL_DRAW,
    addr::LIGHT_DRAW,
];

/// Value widget kinds the Rust default style draws (kind 3, the radio group header, draws nothing).
pub const KINDS: &[u32] = &[
    addr::KIND_DROP_DOWN,
    addr::KIND_RADIO_GROUP,
    addr::KIND_CHECK_BOX,
    addr::KIND_BUTTON,
    addr::KIND_SLIDER,
];

fn c_string(g: &dyn GuestUi, mut va: u32) -> Vec<u8> {
    let mut out = Vec::new();
    while va != 0 && out.len() < 256 {
        let Some(b) = byte(g, va) else { break };
        if b == 0 {
            break;
        }
        out.push(b);
        va += 1;
    }
    out
}

fn named_part(
    g: &dyn GuestUi,
    handle_va: u32,
    part: Option<(u32, u32, u32, u32)>,
) -> Option<TexturePart> {
    let handle = g.read32(handle_va)?;
    let named = crate::list_rows_live::handle_named_texture(g, handle)?;
    let size = (named.width, named.height);
    let texels = part.unwrap_or((0, 0, (size.0 * 16.0) as u32, (size.1 * 16.0) as u32));
    Some(TexturePart::sixteenths(named.name, texels, size))
}

/// `0x30A2D0(descriptor)` for a static light (mode `+4` 0): the palette colour of byte 0, alpha 0x80. Animated modes
/// (1..3: pulse, flash, strobe on the frame counter) are drawn with their first colour (INFERRED, not ported).
pub fn light_colour(g: &dyn GuestUi, descriptor: [u32; 5]) -> PackedColor {
    let first = descriptor[0] & 0xFF;
    let colour = g.read32(addr::LIGHT_PALETTE + 4 * first).unwrap_or(0);
    PackedColor((colour & !0xFF) | 0x80)
}

/// The page scroll bar of `0x727F0` when the page style asks for it and the page scrolls; also the scroll view.
pub fn scroll_bar(
    g: &dyn GuestUi,
    record: u32,
    colours: u32,
    units: Units,
) -> (Option<(f32, f32)>, Option<ScrollBar>) {
    let style = g
        .read32(record + crate::menu_state::layout::page::STYLE)
        .unwrap_or(0);
    let word = g.read32(style).unwrap_or(0);
    let total = float(g, record + addr::SCROLL_TOTAL);
    let (top, bottom, offset) = (
        float(g, record + addr::VIEW_TOP),
        float(g, record + addr::VIEW_BOTTOM),
        float(g, record + addr::SCROLL),
    );
    if !(total > bottom - top) || total <= 0.0 {
        return (None, None);
    }
    let page_y = float(g, record + addr::PAGE_Y);
    let (y0, y1) = (top * units.y + page_y, bottom * units.y + page_y);
    if word & addr::STYLE_SCROLL_BAR == 0 {
        return (Some((y0, y1)), None);
    }
    let right = g.read32(style + addr::STYLE_BAR_RIGHT).unwrap_or(0) != 0;
    let side = if right {
        float(g, record + 0xEC)
    } else {
        float(g, record + 0xE4)
    };
    let x0 = float(g, style + addr::STYLE_BAR_X0) * units.x + side;
    let x1 = float(g, style + addr::STYLE_BAR_X1) * units.x + side;
    let w = x1 - x0;
    let arrow = w * addr::ARROW_SHARE;
    let (track0, track1) = (y0 + w, y1 - w);
    let (a, b) = (offset / total, (bottom - top + offset) / total);
    let length = track1 - track0;
    let colour = |o: u32| PackedColor(g.read32(colours + o).unwrap_or(0));
    let shift = if right { 4 } else { 0 };
    let (mut c_top, mut c_bottom) = (
        colour(addr::BAR_TOP + shift),
        colour(addr::BAR_BOTTOM + shift),
    );
    let hover = float(g, colours + addr::BAR_HOVER);
    let state = (
        g.read32(record + addr::HOVER_A).unwrap_or(0) as i32,
        g.read32(record + addr::HOVER_B).unwrap_or(0) as i32,
    );
    let mix = tsfp_ui_model::default_widgets::mix;
    // INFERRED: the thumb takes the bar gradient at its own top and bottom (`0x154D50(top, bottom, 1 - a)`).
    let (mut t_top, mut t_bottom) = (mix(c_top, c_bottom, a), mix(c_top, c_bottom, b));
    if state.0 == -2 || state.1 == -2 {
        t_top = tsfp_ui_model::default_widgets::brighten(t_top, hover);
        t_bottom = tsfp_ui_model::default_widgets::brighten(t_bottom, hover);
    }
    if state.0 == -3 || state.0 == -4 {
        c_top = tsfp_ui_model::default_widgets::brighten(c_top, hover);
        c_bottom = tsfp_ui_model::default_widgets::brighten(c_bottom, hover);
    }
    let bar = ScrollBar {
        outer: Edges::new(x0, y0, x1, y1),
        thumb: Edges::new(x0, track0 + length * a, x1, track0 + length * b),
        up: Edges::new(x0, y0, x1, y0 + arrow),
        down: Edges::new(x0, y1 - arrow, x1, y1),
        textures: addr::BAR_TEXTURES.map(|i| texture_name(g, i as i32)),
        top: c_top,
        bottom: c_bottom,
        thumb_top: t_top,
        thumb_bottom: t_bottom,
    };
    (Some((y0, y1)), Some(bar))
}

fn float(g: &dyn GuestUi, va: u32) -> f32 {
    g.read32(va)
        .map(f32::from_bits)
        .filter(|v| v.is_finite())
        .unwrap_or(0.0)
}

/// The front end's unit globals.
pub fn units(g: &dyn GuestUi) -> Units {
    Units {
        x: float(g, f::UNIT_X),
        y: float(g, f::UNIT_Y),
        origin_x: float(g, f::ORIGIN_X),
        origin_y: float(g, f::ORIGIN_Y),
    }
}

/// `0x7B520(row)`: the row's value box in units (`+0xE4..+0xF0` offset by the parent's page x and top, and the scroll
/// when the row scrolls). The kinds 6 and 7 variants for parent flags 0x800 / 0x400 are not used by the Map Maker
/// pages (MEASURED live flags) and are not modelled.
pub fn value_box(g: &dyn GuestUi, node: u32) -> Option<Edges> {
    let parent = g.read32(node + f::PARENT).filter(|p| *p != 0)?;
    let x0 = (float(g, parent + f::PAGE_X) - float(g, f::ORIGIN_X)) * float(g, f::VALUE_X_SCALE)
        + float(g, node + f::VALUE_BOX);
    let mut y0 = float(g, node + f::VALUE_BOX + 4) + float(g, parent + f::PARENT_TOP);
    if g.read32(node + 0x28).unwrap_or(0) & f::FLAG_SCROLLED != 0 {
        y0 += float(g, parent + f::PARENT_VIEW_TOP) - float(g, parent + f::PARENT_SCROLL);
    }
    Some(Edges::new(
        x0,
        y0,
        x0 + float(g, node + f::VALUE_BOX + 8),
        y0 + float(g, node + f::VALUE_BOX + 12),
    ))
}

fn widget_text(index: usize, text: &LiveText) -> Option<WidgetText> {
    let widget = widget_id(index);
    match text {
        LiveText::None => None,
        LiveText::Id(id) => Some(WidgetText {
            widget,
            text: TextRef::Id(tsfp_ui_model::model::TextId(*id)),
            bytes: Vec::new(),
        }),
        LiveText::Bytes(bytes) => Some(WidgetText {
            widget,
            text: TextRef::Dynamic,
            bytes: bytes.clone(),
        }),
    }
}

/// The value widget of a row in the second row loop of `0x76810` (types 4, 7, 8 with a value count or kind 5 / 6).
fn has_value_widget(g: &dyn GuestUi, row: &LiveRow) -> bool {
    matches!(row.node_type, 4 | 7 | 8)
        && (g.read32(row.node + f::VALUE_COUNT).unwrap_or(0) as i32 > 0
            || matches!(row.kind, 5 | 6))
        && row.flags & 0x20 == 0
}

fn byte(g: &dyn GuestUi, va: u32) -> Option<u8> {
    g.read32(va & !3).map(|word| (word >> ((va & 3) * 8)) as u8)
}

/// `0x24E60`: the memory unit in `device` is present (record bytes 0 and 3 both set).
pub fn memory_unit_ready(g: &dyn GuestUi, device: u32) -> bool {
    if device >= addr::DEVICE_COUNT {
        return false;
    }
    let record = addr::DEVICE_RECORDS + addr::DEVICE_RECORD_STRIDE * device;
    byte(g, record).unwrap_or(0) != 0 && byte(g, record + 3).unwrap_or(0) != 0
}

/// The texture table entry `0x191470` picks for the storage row whose `+0x14` is `slot`.
pub fn storage_texture(g: &dyn GuestUi, slot: u32) -> u32 {
    let remapped = g.read32(addr::DEVICE_REMAP_A).unwrap_or(0) != 0
        || g.read32(addr::DEVICE_REMAP_B).unwrap_or(0) != 0;
    let device = if remapped {
        g.read32(addr::DEVICE_TABLE + addr::DEVICE_TABLE_STRIDE * slot)
            .unwrap_or(u32::MAX)
    } else {
        slot
    };
    if device >= addr::FIRST_HARD_DISK {
        STORAGE_HARD_DISK
    } else if memory_unit_ready(g, device) {
        STORAGE_MEMORY_UNIT_A
    } else {
        STORAGE_MEMORY_UNIT_B
    }
}

/// Rows of `live` the default style cannot draw yet, as `(row type, kind)`: group boxes (type 5) and the value widgets
/// of the second row loop of `0x76810` (types 4, 7, 8 with a value count `+0xC4 > 0` or kind 5 / 6, drawn by `0x7F8B0`).
pub fn unsupported(live: &LivePage) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    for row in &live.rows {
        let widget = match row.node_type {
            5 => false,
            4 | 7 | 8 => (row.kind != 0 || !row.options.is_empty()) && !KINDS.contains(&row.kind),
            _ => false,
        };
        if widget && !out.contains(&(row.node_type, row.kind)) {
            out.push((row.node_type, row.kind));
        }
    }
    out
}

/// The widget draws of `live` whose row draw callback has a Rust implementation; `colours` is the page colour block.
pub fn read(g: &dyn GuestUi, live: &LivePage, slot: u32, colours: u32) -> Widgets {
    let units = units(g);
    let mut out = Widgets {
        units,
        ..Widgets::default()
    };
    let colour = |offset: u32| PackedColor(g.read32(colours + offset).unwrap_or(0));
    let record =
        crate::menu_state::layout::PAGE_TABLE + slot * crate::menu_state::layout::PAGE_STRIDE;
    let margins = g.read32(record + addr::MARGINS).unwrap_or(0);
    let context = g.read32(record + addr::CONTEXT).unwrap_or(0);
    let pointer = (context < 8).then(|| {
        let base = addr::CONTEXTS + addr::CONTEXT_STRIDE * context;
        (
            float(g, base + addr::POINTER_X),
            float(g, base + addr::POINTER_Y),
        )
    });
    let check_size =
        |font: u32| float(g, addr::CHECK_SCALE) * style_height(g, font).unwrap_or(0.0) - 1.0;
    let value = |node: u32| g.read32(node + addr::VALUE).unwrap_or(0) as i32;
    // Rows drawn in the original's two loops: the row draws first (group boxes, row callbacks), then the value widgets.
    for (index, row) in live.rows.iter().enumerate() {
        let Some([x0, y0, x1, y1]) = row.rect else {
            continue;
        };
        let edges = Edges::new(x0, y0, x1, y1);
        if row.node_type == 5 && row.draw == 0 {
            let m = |i: u32| float(g, margins + 4 * i);
            let inner = Edges::new(
                x0 + units.x * m(0),
                y0 + units.y * m(1),
                x1 - units.x * m(2),
                y1 - units.y * m(3),
            );
            out.draws.push(WidgetDraw::GroupBox {
                outer: edges,
                inner,
                title: widget_text(index, &row.text),
                title_right: row.text_align == 1,
                title_height: style_height(g, GROUP_TITLE_STYLE).unwrap_or(0.0),
                light: colour(addr::DIVIDER_UPPER),
                dark: colour(addr::DIVIDER_LOWER),
            });
            continue;
        }
        let focused = row.node == live.cursor_node;
        match row.draw {
            addr::INSET_FRAME_DRAW => out.draws.push(WidgetDraw::InsetFrame {
                area: edges,
                frame: named_part(g, addr::FRAME_TEXTURE, Some((0, 0, 0x180, 0x180))),
            }),
            addr::LABEL_DRAW => {
                let id = if g.read32(record + 4) == Some(addr::LABEL_PAGE_VTABLE) {
                    g.read32(row.node + addr::LABEL_STRING)
                        .unwrap_or(addr::LABEL_DEFAULT_STRING)
                } else {
                    addr::LABEL_DEFAULT_STRING
                };
                let style = g
                    .read32(record + crate::menu_state::layout::page::STYLE)
                    .unwrap_or(0);
                let font = g.read32(style + addr::STYLE_FONT).unwrap_or(0);
                let text = WidgetText {
                    widget: widget_id(index),
                    text: TextRef::Id(tsfp_ui_model::model::TextId(id)),
                    bytes: Vec::new(),
                };
                out.draws.push(WidgetDraw::Label {
                    text: PlacedText {
                        text,
                        font,
                        at: (x0, y0),
                        align_end: false,
                        focused,
                        line_height: crate::list_rows_live::line_advance(g, font),
                    },
                    width: x1 - x0 - units.x,
                    max_height: y1 - y0,
                    fallback: Some((
                        addr::LABEL_FALLBACK_FONT,
                        crate::list_rows_live::line_advance(g, addr::LABEL_FALLBACK_FONT),
                    )),
                });
            }
            addr::LIGHT_DRAW => {
                let handler = g.read32(row.node + addr::LIGHT_HANDLER).unwrap_or(0);
                let descriptor = if handler == addr::LIGHT_HANDLER_ZERO {
                    [0; 5]
                } else {
                    std::array::from_fn(|i| {
                        g.read32(addr::LIGHT_DESCRIPTOR + 4 * i as u32).unwrap_or(0)
                    })
                };
                out.draws.push(WidgetDraw::LightCell {
                    area: edges,
                    state: u32::from(focused),
                    light: light_colour(g, descriptor),
                    frame: named_part(g, addr::FRAME_TEXTURE, Some((0, 0, 0x180, 0x180))),
                    // The bulb is a slot texture (`mm_cursor40`, not in the named table): the host adds its pixels.
                    bulb: named_part(g, addr::UI_TEXTURES + 4 * addr::BULB_SLOT, None).or_else(
                        || {
                            crate::list_rows_live::slot_texture(g, addr::BULB_SLOT).map(|t| {
                                TexturePart {
                                    texture: t.key(),
                                    uv: tsfp_ui_renderer::UvRect {
                                        min: tsfp_ui_renderer::Vec2 { x: 0.0, y: 0.0 },
                                        max: tsfp_ui_renderer::Vec2 { x: 1.0, y: 1.0 },
                                    },
                                }
                            })
                        },
                    ),
                });
            }
            addr::RADIO_DRAW => {
                let group = g.read32(row.node + addr::RADIO_GROUP).unwrap_or(0);
                let checked = group != 0
                    && g.read32(group + addr::VALUE) == g.read32(row.node + addr::RADIO_VALUE);
                out.draws.push(WidgetDraw::CheckBox {
                    area: edges,
                    size: check_size(row.font),
                    checked,
                    top: colour(addr::BOX_TOP),
                    bottom: colour(addr::BOX_BOTTOM),
                });
            }
            addr::DIVIDER_DRAW => out.draws.push(WidgetDraw::Divider {
                row: edges,
                upper: colour(addr::DIVIDER_UPPER),
                lower: colour(addr::DIVIDER_LOWER),
            }),
            addr::STORAGE_ICON_DRAW => {
                let slot = g.read32(row.node + addr::ROW_DEVICE).unwrap_or(0);
                if let Some(texture) = texture_name(g, storage_texture(g, slot) as i32) {
                    out.draws.push(WidgetDraw::StorageIcon {
                        row: edges,
                        texture,
                    });
                }
            }
            _ => {}
        }
    }
    // `0x76150`: rows of types 4, 7 and 8 with a wrap width (`+0x44`, units) are broken into lines.
    for (index, row) in live.rows.iter().enumerate() {
        let wrap = float(g, row.node + addr::WRAP_WIDTH);
        if matches!(row.node_type, 4 | 7 | 8)
            && row.rect.is_some()
            && wrap > 0.0
            && row.text != LiveText::None
        {
            out.wraps.push(Wrap {
                widget: widget_id(index),
                width: wrap * units.x,
                line_height: crate::list_rows_live::line_advance(g, row.font),
            });
        }
    }
    for row in &live.rows {
        if row.rect.is_none() || !has_value_widget(g, row) {
            continue;
        }
        let Some(band) = value_box(g, row.node).map(|b| units.to_design(b)) else {
            continue;
        };
        match row.kind {
            addr::KIND_CHECK_BOX => out.draws.push(WidgetDraw::CheckBox {
                area: band,
                size: check_size(row.font),
                checked: value(row.node) == 1,
                top: colour(addr::BOX_TOP),
                bottom: colour(addr::BOX_BOTTOM),
            }),
            addr::KIND_DROP_DOWN => {
                let font = g.read32(row.node + f::VALUE_FONT).unwrap_or(0);
                let side = (float(g, addr::CHECK_SCALE) * style_height(g, font).unwrap_or(0.0)
                    + 1.0)
                    * units.y;
                let arrow = Edges::new(band.x1 - side, band.y0, band.x1, band.y0 + side);
                let selected = (g.read32(row.node + 0xB8).unwrap_or(0) & addr::NO_SELECTION == 0)
                    .then(|| row.options.get(row.selected as usize))
                    .flatten();
                let focused = row.node == live.cursor_node;
                let text = selected.and_then(|option| {
                    let index = live.rows.iter().position(|r| r.node == row.node)?;
                    let text = widget_text(index, &option.text)?;
                    let align_end = g.read32(row.node + f::ALIGN).unwrap_or(0) == 1;
                    let x = if align_end {
                        arrow.x0 - units.x * 2.0
                    } else {
                        float(g, row.node + f::VALUE_DX) * units.x + band.x0
                    };
                    Some(PlacedText {
                        text,
                        font,
                        at: (
                            x,
                            float(g, row.node + f::VALUE_DY) * units.y + band.y0 + 0.5,
                        ),
                        align_end,
                        focused,
                        line_height: crate::list_rows_live::line_advance(g, font),
                    })
                });
                out.draws.push(WidgetDraw::DropDown {
                    area: band,
                    arrow,
                    glow: if focused {
                        float(g, colours + addr::GLOW)
                    } else {
                        1.0
                    },
                    top: colour(addr::BOX_TOP),
                    bottom: colour(addr::BOX_BOTTOM),
                    arrow_sprite: named_part(g, addr::ARROW_TEXTURE, Some(addr::DOWN_ARROW_PART)),
                    text,
                });
            }
            addr::KIND_BUTTON => {
                let bytes = c_string(g, g.read32(row.node + addr::BUTTON_TEXT).unwrap_or(0));
                let focused = row.node == live.cursor_node;
                let index = live
                    .rows
                    .iter()
                    .position(|r| r.node == row.node)
                    .unwrap_or(0);
                let align_end = g.read32(row.node + f::ALIGN).unwrap_or(0) == 1;
                let style = g
                    .read32(record + crate::menu_state::layout::page::STYLE)
                    .unwrap_or(0);
                let x = if align_end {
                    band.x1 - float(g, style + addr::STYLE_RIGHT_INSET) * units.x
                } else {
                    float(g, row.node + f::VALUE_DX) * units.x + band.x0
                };
                let text = (!bytes.is_empty()).then(|| PlacedText {
                    text: WidgetText {
                        widget: widget_id(index),
                        text: TextRef::Dynamic,
                        bytes,
                    },
                    font: row.font,
                    at: (
                        x,
                        float(g, row.node + f::VALUE_DY) * units.y + band.y0 + 0.5,
                    ),
                    align_end,
                    focused,
                    line_height: crate::list_rows_live::line_advance(g, row.font),
                });
                out.draws.push(WidgetDraw::Button {
                    area: band,
                    top: colour(addr::BOX_TOP),
                    bottom: colour(addr::BOX_BOTTOM),
                    text,
                });
            }
            addr::KIND_SLIDER => {
                let percent = value(row.node) as f32;
                let (_, thumb) = tsfp_ui_model::default_widgets::slider_edges(band, percent, units);
                // `0x711F0`: the pointer strictly inside the thumb on both axes.
                let hovered = pointer.is_some_and(|(x, y)| {
                    thumb.x0 <= x && x < thumb.x1 && thumb.y0 <= y && y < thumb.y1
                });
                let ticks = g
                    .read32((row.node + addr::TICKS) & !3)
                    .map(|w| (w >> 16) as i16)
                    .unwrap_or(0)
                    .max(0) as u32;
                out.draws.push(WidgetDraw::Slider {
                    band,
                    value: percent,
                    ticks,
                    hovered,
                    light: colour(addr::DIVIDER_UPPER),
                    dark: colour(addr::DIVIDER_LOWER),
                });
            }
            _ => {}
        }
    }
    let (clip, bar) = scroll_bar(g, record, colours, units);
    out.clip = clip;
    out.scroll_bar = bar;
    out
}

#[cfg(test)]
mod tests {
    //! Tier 0, FABRICATED memory.
    use super::*;
    use std::collections::HashMap;

    struct Mem(HashMap<u32, u32>);
    impl GuestUi for Mem {
        fn read32(&self, va: u32) -> Option<u32> {
            self.0.get(&va).copied()
        }
        fn write32(&mut self, _va: u32, _value: u32) -> bool {
            false
        }
        fn call(&mut self, _va: u32, _args: &[u32]) -> Option<u32> {
            None
        }
    }

    #[test]
    fn storage_texture_follows_the_device() {
        let mut m = HashMap::new();
        assert_eq!(storage_texture(&Mem(m.clone()), 9), STORAGE_HARD_DISK);
        assert_eq!(storage_texture(&Mem(m.clone()), 2), STORAGE_MEMORY_UNIT_B);
        let record = addr::DEVICE_RECORDS + addr::DEVICE_RECORD_STRIDE * 2;
        m.insert(record, 0x0100_0001);
        assert_eq!(storage_texture(&Mem(m.clone()), 2), STORAGE_MEMORY_UNIT_A);
        // Remapped: slot 0 holds device 9.
        m.insert(addr::DEVICE_REMAP_B, 1);
        m.insert(addr::DEVICE_TABLE, 9);
        assert_eq!(storage_texture(&Mem(m), 0), STORAGE_HARD_DISK);
    }
}

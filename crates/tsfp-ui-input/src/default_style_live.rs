// SPDX-License-Identifier: GPL-3.0-or-later
//! T1916 increment 2: the default page style's inputs read from the live page record (typed view,
//! `tsfp_ui_model::default_style` draws them). Addresses MEASURED from the lift of `0x76810`,
//! `0x72B70` and `0x80430` (docs/t-system-mapmaker-ui.md, "Native Map Maker UI", increment 2).
use crate::menu_state::layout as guest_layout;
use crate::native_page::GuestUi;
use tsfp_ui_model::list_rows::Edges;
use tsfp_ui_model::skin::PackedColor;

pub mod addr {
    /// Page record: colour block pointer.
    pub const COLOURS: u32 = 0x20;
    /// Page record: flags (bit 0x400 full-width highlight, MEASURED from `0x72B70`).
    pub const FLAGS: u32 = 0x40;
    pub const FLAG_FULL_WIDTH: u32 = 0x400;
    /// Page record: pointer texture handle (`0x76810` stores `[0x6FEBAC]` here in pointer state 1).
    pub const POINTER_TEXTURE: u32 = 0x4C;
    /// Page record: page box x0, y0, x1, y1 (f32, design units) and the fade.
    pub const BOX: u32 = 0xE4;
    pub const FADE: u32 = 0xF4;
    /// Page record: the full-width bar's left edge in percent units (`* [0x7BA038] + [0x7BA180]`).
    pub const BAR_LEFT: u32 = 0x214;
    /// Colour block: frame corners `+0x50..+0x5C`, highlight bar `+0x74`, edges `+0x78`, `+0x7C`.
    pub const FRAME_CORNERS: u32 = 0x50;
    pub const HIGHLIGHT_BAR: u32 = 0x74;
    pub const HIGHLIGHT_TOP: u32 = 0x78;
    pub const HIGHLIGHT_BOTTOM: u32 = 0x7C;
    /// Style word 0 (`0x80430`): bit 0x200 skips the frame.
    pub const STYLE_NO_FRAME: u32 = 0x200;
    /// Pulse: frame counter, its period (int), the sine table (2048 entries).
    pub const FRAME_COUNTER: u32 = 0x74_97A8;
    pub const PULSE_PERIOD: u32 = 0x4E_8CEC;
    /// UI context `+0xF8`: pointer drawn when 1 or 3.
    pub const CONTEXT_POINTER_SHOWN: u32 = 0xF8;
    /// Pointer texture of pointer state 1.
    pub const POINTER_TEXTURE_GLOBAL: u32 = 0x6F_EBAC;
}

/// The raw default-style words of one page, for the model and for the diagnostics log.
#[derive(Clone, Debug, PartialEq)]
pub struct DefaultStyleWords {
    pub style: u32,
    pub style_word: u32,
    pub flags: u32,
    pub page_box: Edges,
    pub fade: f32,
    pub colours: u32,
    pub frame: [u32; 4],
    pub bar: u32,
    pub edge_top: u32,
    pub edge_bottom: u32,
    /// The full-width bar's left edge in design units, `None` without bit 0x400.
    pub bar_left: Option<f32>,
}

impl DefaultStyleWords {
    /// The style has no frame or highlight callback: the framework default draws it.
    pub fn frame_shown(&self) -> bool {
        self.style_word & addr::STYLE_NO_FRAME == 0
    }
    pub fn frame_colours(&self) -> [PackedColor; 4] {
        self.frame.map(PackedColor)
    }
}

fn float(g: &dyn GuestUi, va: u32) -> Option<f32> {
    g.read32(va).map(f32::from_bits).filter(|v| v.is_finite())
}

/// Read the default-style inputs of the page in `slot`.
pub fn read(g: &dyn GuestUi, slot: u32) -> Option<DefaultStyleWords> {
    let record = guest_layout::PAGE_TABLE + slot * guest_layout::PAGE_STRIDE;
    let style = g.read32(record + guest_layout::page::STYLE)?;
    let colours = g.read32(record + addr::COLOURS)?;
    let flags = g.read32(record + addr::FLAGS)?;
    let page_box = Edges::new(
        float(g, record + addr::BOX)?,
        float(g, record + addr::BOX + 4)?,
        float(g, record + addr::BOX + 8)?,
        float(g, record + addr::BOX + 12)?,
    );
    let word = |o: u32| g.read32(colours + o).unwrap_or(0);
    let bar_left = (flags & addr::FLAG_FULL_WIDTH != 0)
        .then(|| {
            let percent = float(g, record + addr::BAR_LEFT)?;
            let scale = float(g, 0x7B_A038)?;
            let origin = float(g, 0x7B_A180)?;
            Some(percent * scale + origin)
        })
        .flatten();
    Some(DefaultStyleWords {
        style,
        style_word: g.read32(style).unwrap_or(0),
        flags,
        page_box,
        fade: float(g, record + addr::FADE).unwrap_or(1.0),
        colours,
        frame: [0, 4, 8, 12].map(|o| word(addr::FRAME_CORNERS + o)),
        bar: word(addr::HIGHLIGHT_BAR),
        edge_top: word(addr::HIGHLIGHT_TOP),
        edge_bottom: word(addr::HIGHLIGHT_BOTTOM),
        bar_left,
    })
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
    fn reads_the_box_colours_and_the_full_width_bar() {
        let record = guest_layout::PAGE_TABLE + 3 * guest_layout::PAGE_STRIDE;
        let mut m = HashMap::new();
        m.insert(record + guest_layout::page::STYLE, 0x1000);
        m.insert(0x1000, 0x200);
        m.insert(record + addr::COLOURS, 0x2000);
        m.insert(record + addr::FLAGS, 0x400);
        for (i, v) in [352.0f32, 52.0, 640.0, 425.0].iter().enumerate() {
            m.insert(record + addr::BOX + 4 * i as u32, v.to_bits());
        }
        m.insert(record + addr::BAR_LEFT, 55.0f32.to_bits());
        m.insert(0x7B_A038, 6.4f32.to_bits());
        m.insert(0x7B_A180, 0.0f32.to_bits());
        m.insert(0x2000 + addr::HIGHLIGHT_BAR, 0x6050_7F7F);
        m.insert(0x2000 + addr::FRAME_CORNERS + 12, 0x0102_0304);
        let words = read(&Mem(m), 3).unwrap();
        assert!(!words.frame_shown());
        assert_eq!(words.page_box, Edges::new(352.0, 52.0, 640.0, 425.0));
        assert_eq!(words.bar, 0x6050_7F7F);
        assert_eq!(words.frame[3], 0x0102_0304);
        assert_eq!(words.fade, 1.0);
        assert!((words.bar_left.unwrap() - 352.0).abs() < 1e-3);
    }
}

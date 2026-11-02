// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-only runtime inputs for the native editor Controls callback 0x314A50.
//! Typed snapshots feed Rust-owned animation; unsupported layouts keep original drawing.
use crate::{
    list_rows_live::texture_name,
    native_page::{GuestUi, NativeCommand},
    widgets::Step,
};
use tsfp_mapmaker::controls_ui::{
    ControlsFocusStep, ControlsPageCommand, ControlsPageState, ControlsPageUpdate, ControlsRowId,
};
use tsfp_ui_model::mapmaker_controls::{ControllerPictures, PICTURE_TEXTURES};
use tsfp_ui_renderer::Vec2;

fn float(g: &dyn GuestUi, address: u32) -> Option<f32> {
    g.read32(address)
        .map(f32::from_bits)
        .filter(|v| v.is_finite())
}

/// `page` is the original page record, not the slot index. All constants and
/// names are read from the owner's runtime input; no asset bytes are embedded.
pub fn read_pictures(g: &dyn GuestUi, page: u32) -> Option<ControllerPictures> {
    if page == 0 {
        return None;
    }
    let centre = Vec2::new(
        float(g, page + 0xA8)? + float(g, page + 0xB0)? * float(g, 0x49_DE58)?,
        float(g, page + 0xAC)? + float(g, page + 0xB4)? * float(g, 0x47_88E8)?,
    );
    ControllerPictures::new(
        centre,
        float(g, page + 0xF4)?,
        float(g, 0x4C_0460)? * float(g, 0x49_F670)?,
        float(g, 0x47_8620)?,
        [float(g, 0x49_EF78)?, float(g, 0x47_DA48)?],
        [
            texture_name(g, PICTURE_TEXTURES[0] as i32)?,
            texture_name(g, PICTURE_TEXTURES[1] as i32)?,
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    struct Mem(HashMap<u32, u32>);
    impl GuestUi for Mem {
        fn read32(&self, va: u32) -> Option<u32> {
            self.0.get(&va).copied()
        }
        fn write32(&mut self, _: u32, _: u32) -> bool {
            panic!("read-only adapter wrote memory")
        }
        fn call(&mut self, _: u32, _: &[u32]) -> Option<u32> {
            panic!("read-only adapter called original")
        }
    }
    #[test]
    fn acquires_runtime_geometry_and_names_without_guest_calls() {
        let mut m = HashMap::new();
        for (va, value) in [
            (0x10A8, 10.0f32),
            (0x10AC, 20.0),
            (0x10B0, 100.0),
            (0x10B4, 200.0),
            (0x10F4, 0.5),
            (0x49_DE58, 0.5),
            (0x47_88E8, 0.25),
            (0x4C_0460, 1.0),
            (0x49_F670, 101.9),
            (0x47_8620, 1.25),
            (0x49_EF78, 80.0),
            (0x47_DA48, 70.0),
        ] {
            m.insert(va, value.to_bits());
        }
        for (i, name) in [b"shade\0".as_slice(), b"pad\0".as_slice()]
            .iter()
            .enumerate()
        {
            let at = 0x2000 + i as u32 * 32;
            m.insert(
                crate::list_rows_live::addr::TEXTURES
                    + crate::list_rows_live::addr::TEXTURE_STRIDE * PICTURE_TEXTURES[i]
                    - 8,
                at,
            );
            for (n, chunk) in name.chunks(4).enumerate() {
                let mut word = [0u8; 4];
                word[..chunk.len()].copy_from_slice(chunk);
                m.insert(at + n as u32 * 4, u32::from_le_bytes(word));
            }
        }
        let p = read_pictures(&Mem(m.clone()), 0x1000).unwrap();
        assert_eq!(p.centre, Vec2::new(60.0, 70.0));
        assert_eq!(p.pictures[0].texture, "shade");
        assert_eq!(p.pictures[1].texture, "pad");
        assert_eq!(p.pictures[0].area.y0, -10.0);
        m.insert(0x10B0, f32::NAN.to_bits());
        assert!(read_pictures(&Mem(m), 0x1000).is_none());
    }

    #[test]
    fn editor_snapshot_uses_integer_indices_and_never_calls_or_writes() {
        let mut m = HashMap::new();
        for (va, value) in [
            (0x10A8, 10.0f32),
            (0x10AC, 20.0),
            (0x10B0, 100.0),
            (0x10B4, 200.0),
            (0x10F4, 0.5),
            (0x49_DE58, 0.5),
            (0x47_88E8, 0.25),
            (0x4C_0460, 1.0),
            (0x49_F670, 101.9),
            (0x47_8620, 1.25),
            (0x49_EF78, 80.0),
            (0x47_DA48, 70.0),
        ] {
            m.insert(va, value.to_bits());
        }
        for (i, name) in [b"shade\0".as_slice(), b"pad\0".as_slice()]
            .iter()
            .enumerate()
        {
            let at = 0x2000 + i as u32 * 32;
            m.insert(
                crate::list_rows_live::addr::TEXTURES
                    + crate::list_rows_live::addr::TEXTURE_STRIDE * PICTURE_TEXTURES[i]
                    - 8,
                at,
            );
            for (n, chunk) in name.chunks(4).enumerate() {
                let mut word = [0u8; 4];
                word[..chunk.len()].copy_from_slice(chunk);
                m.insert(at + n as u32 * 4, u32::from_le_bytes(word));
            }
        }

        m.insert(CONTROLS_RECORD + 0x10C, 0);
        for i in 0..18u32 {
            m.insert(CONTROLS_RECORD + i * 12, 0.25f32.to_bits());
            m.insert(CONTROLS_RECORD + i * 12 + 4, 3);
            m.insert(CONTROLS_RECORD + i * 12 + 8, 2);
        }
        for i in 0..6u32 {
            m.insert(CONTROLS_RECORD + 0xE4 + i * 4, 0.0f32.to_bits());
            m.insert(0x78_9800 + i * 4, 2);
        }
        for i in 0..51u32 {
            m.insert(0x78_9658 + i * 8, (i as f32).to_bits());
            m.insert(0x78_965C + i * 8, (i as f32 * 18.0).to_bits());
        }
        for va in [0x78_9818, 0x78_9650, 0x78_981C, 0x78_964C, 0x78_9644] {
            m.insert(va, 0);
        }
        m.insert(0x7B_A038, 6.4f32.to_bits());
        let f = crate::live_page::draw_fields::STYLES;
        m.insert(f + crate::live_page::draw_fields::STYLE_STRIDE * 0x12, 0);
        m.insert(crate::live_page::draw_fields::HEIGHT_LINES, 480);
        for va in [
            crate::live_page::draw_fields::RECORD_HEIGHTS,
            crate::live_page::draw_fields::HEIGHT_SCALE_A,
            crate::live_page::draw_fields::HEIGHT_SCALE_B,
            crate::live_page::draw_fields::HEIGHT_SCALE_C,
            f + crate::live_page::draw_fields::STYLE_STRIDE * 0x12 + 0xC,
        ] {
            m.insert(va, 1.0f32.to_bits());
        }
        let g = Mem(m.clone());
        let texts = crate::live_page::TextIndex::default();
        let mut rows = Vec::new();
        for i in 0..18 {
            let mut row = crate::live_page::read_row(&g, &texts, 0x9000 + i * 0x200);
            row.node_type = 7;
            row.kind = 1;
            row.selected = 0;
            row.text = LiveText::Bytes(b"label".to_vec());
            row.options = vec![crate::live_page::LiveOption {
                id: 0,
                text: LiveText::Bytes(b"value".to_vec()),
                enabled: true,
            }];
            rows.push(row);
        }
        let live = LivePage {
            slot: 0,
            builder: 0,
            state: 0,
            cursor_node: 0,
            rows,
            units: (6.4, 4.8),
        };
        let d = read_diagram(&g, 0x1000, &live).unwrap();
        assert_eq!(
            d.animation.cells[0],
            CurveCell {
                fraction: 0.25,
                target: 3,
                segment: 2
            }
        );
        assert_eq!(
            d.geometry(d.animation.cells[0]).unwrap()[0],
            Vec2::new(3.25, 58.5)
        );
        assert_eq!(d.labels.len(), 18);
        let viewport = tsfp_ui_model::anchor::window_viewport(640, 480).unwrap();
        let drawn = d.draw(
            tsfp_ui_renderer::TextureId(1),
            &|_| Some(tsfp_ui_renderer::TextureId(2)),
            &|_, _, _| 20.0,
            viewport,
        );
        assert_eq!(drawn.texts.len(), 36);
        assert_eq!(drawn.dynamic.len(), 36);
        assert_eq!(drawn.sprites[0].texture, tsfp_ui_renderer::TextureId(2));
        assert_eq!(drawn.texts[0].rect_px.origin.x, 8.25);
        assert!(drawn.texts[0].widget != drawn.texts[1].widget);

        m.insert(crate::default_style_live::addr::FRAME_COUNTER, 1);
        m.insert(0x4E_7948, 1.0f32.to_bits());
        m.insert(0x78_9648, 0.25f32.to_bits());
        let mut session = ControlsSession::default();
        let a = session.sample(&Mem(m.clone()), 0x1000, &live).unwrap();
        let b = session.sample(&Mem(m.clone()), 0x1000, &live).unwrap();
        assert_eq!(
            a.animation, b.animation,
            "same frame returns same draw state"
        );
        m.insert(crate::default_style_live::addr::FRAME_COUNTER, 2);
        let c = session.sample(&Mem(m.clone()), 0x1000, &live).unwrap();
        assert_eq!(
            c.animation.cells[0].fraction, 0.5,
            "native state persists without guest writes"
        );
        m.insert(CONTROLS_RECORD, 0.1f32.to_bits());
        let reset = session.sample(&Mem(m.clone()), 0x1000, &live).unwrap();
        assert_eq!(
            reset.animation.cells[0].fraction, 0.1,
            "changed original seed resynchronises"
        );
        m.insert(CONTROLS_RECORD + 0x10C, 1);
        assert!(read_diagram(&Mem(m.clone()), 0x1000, &live).is_none());
        m.insert(CONTROLS_RECORD + 0x10C, 0);
        m.insert(CONTROLS_RECORD + 8, 48);
        assert!(read_diagram(&Mem(m), 0x1000, &live).is_none());
    }

    #[test]
    fn missing_runtime_inputs_never_produce_a_placeholder_picture() {
        let g = Mem(HashMap::new());
        assert!(read_pictures(&g, 0).is_none());
        assert!(read_pictures(&g, 0x1000).is_none());
    }
}

use crate::live_page::{LivePage, LiveText, style_height, widget_id};
use tsfp_ui_model::default_widgets::WidgetText;
use tsfp_ui_model::mapmaker_controls::{
    ControlsDiagram, CurveAnimation, CurveCell, CurveIcon, CurveLabel,
};
use tsfp_ui_model::model::TextRef;

pub const CONTROLS_RECORD: u32 = 0x78_3BA0;

/// Read the actual editor-layout contract. Alternate horizontal layouts are
/// retained originals; missing selected options cannot admit a partial page.
pub fn read_diagram(g: &dyn GuestUi, page: u32, live: &LivePage) -> Option<ControlsDiagram> {
    let record = CONTROLS_RECORD;
    if g.read32(record + 0x10C)? != 0 || live.rows.len() != 18 {
        return None;
    }
    let pictures = read_pictures(g, page)?;
    let mut cells = Vec::new();
    for i in 0..18 {
        let address = record + i * 12;
        let target = g.read32(address + 4)? as i32;
        let segment = g.read32(address + 8)? as i32;
        if !(0..=47).contains(&target) || !(0..=47).contains(&segment) {
            return None;
        }
        cells.push(CurveCell {
            fraction: float(g, address)?,
            target,
            segment,
        });
    }
    let mut outlines = [0.0; 6];
    for (i, v) in outlines.iter_mut().enumerate() {
        *v = float(g, record + 0xE4 + i as u32 * 4)?;
    }
    let points = (0..51u32)
        .map(|i| {
            Some(Vec2::new(
                float(g, 0x78_9658 + i * 8)?,
                float(g, 0x78_965C + i * 8)?,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    let mut connector_segments = [0; 6];
    for (i, s) in connector_segments.iter_mut().enumerate() {
        let v = g.read32(0x78_9800 + i as u32 * 4)?;
        if v > 49 {
            return None;
        }
        *s = v as usize;
    }
    let text = |t: &LiveText, index: usize| WidgetText {
        widget: widget_id(index),
        text: match t {
            LiveText::None => TextRef::None,
            LiveText::Id(id) => TextRef::Id(tsfp_ui_model::model::TextId(*id)),
            LiveText::Bytes(_) => TextRef::Dynamic,
        },
        bytes: if let LiveText::Bytes(b) = t {
            b.clone()
        } else {
            Vec::new()
        },
    };
    let mut labels = Vec::new();
    for (i, r) in live.rows.iter().take(18).enumerate() {
        if r.node_type != 7 || r.kind != 1 || r.draw != 0 {
            return None;
        }
        let selected = r.options.get(r.selected as usize)?;
        labels.push(CurveLabel {
            label_icon: read_icon(g, &r.text)?,
            value_icon: read_icon(g, &selected.text)?,
            label: text(&r.text, i),
            value: text(&selected.text, i),
        });
    }
    Some(ControlsDiagram {
        pictures,
        animation: CurveAnimation { cells, outlines },
        points,
        connector_segments,
        labels,
        unit_x: float(g, 0x7B_A038)?,
        scale_x: float(g, 0x4C_0460)?,
        line_height: style_height(g, 0x12)?,
        cell_colours: [g.read32(0x78_9818)?, g.read32(0x78_9650)?],
        outline_rgb: [
            g.read32(0x78_981C)? as u8,
            g.read32(0x78_964C)? as u8,
            g.read32(0x78_9644)? as u8,
        ],
    })
}

/// Guest-thread lifetime boundary. Native draw-time animation stays in typed
/// Rust state. A changed guest seed (builder reinitialisation or F9 original
/// drawing) resynchronises it; duplicate frame observations never tick twice.
#[derive(Default)]
pub struct ControlsSession {
    page: u32,
    first_node: u32,
    seed: Option<CurveAnimation>,
    animation: Option<CurveAnimation>,
    last_frame: Option<u32>,
    drawn: Option<CurveAnimation>,
}
impl ControlsSession {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn sample(
        &mut self,
        g: &dyn GuestUi,
        page: u32,
        live: &LivePage,
    ) -> Option<ControlsDiagram> {
        let mut diagram = read_diagram(g, page, live)?;
        let first_node = live.rows.first()?.node;
        if self.page != page
            || self.first_node != first_node
            || self.seed.as_ref() != Some(&diagram.animation)
        {
            self.page = page;
            self.first_node = first_node;
            self.seed = Some(diagram.animation.clone());
            self.animation = Some(diagram.animation.clone());
            self.last_frame = None;
            self.drawn = None;
        }
        let frame = g.read32(crate::default_style_live::addr::FRAME_COUNTER)?;
        let step = float(g, 0x4E_7948)? * float(g, 0x78_9648)?;
        if step < 0.0 || !step.is_finite() {
            return None;
        }
        if self.last_frame != Some(frame) {
            self.drawn = self.animation.clone();
            self.animation.as_mut()?.advance(step);
            self.last_frame = Some(frame);
        }
        diagram.animation = self.drawn.clone()?;
        Some(diagram)
    }
}

/// Reduce native input for the measured 18-row Map Maker Controls page into
/// generation-qualified Rust state. The original page callback remains the
/// temporary runtime focus/Back effect until the subsystem adoption gate.
pub fn dispatch_controls_commands(
    state: &mut ControlsPageState,
    commands: &[NativeCommand],
) -> Vec<ControlsPageUpdate> {
    let mut updates = Vec::new();
    for command in commands {
        let typed = match *command {
            NativeCommand::Focus(index) | NativeCommand::Select(index) => {
                let Ok(index) = u8::try_from(index) else {
                    continue;
                };
                let Some(row) = ControlsRowId::new(index) else {
                    continue;
                };
                ControlsPageCommand::Focus(state.target(row))
            }
            NativeCommand::Step(Step::Up) => ControlsPageCommand::Step(ControlsFocusStep::Up),
            NativeCommand::Step(Step::Down) => ControlsPageCommand::Step(ControlsFocusStep::Down),
            NativeCommand::Back => ControlsPageCommand::Back,
            NativeCommand::Accept
            | NativeCommand::Delete
            | NativeCommand::Shortcut
            | NativeCommand::Join(_)
            | NativeCommand::ResumeGame
            | NativeCommand::Step(Step::Left | Step::Right)
            | NativeCommand::Adjust { .. }
            | NativeCommand::Scroll(_) => continue,
        };
        match state.dispatch(typed) {
            ControlsPageUpdate::Ignored => {}
            update => updates.push(update),
        }
    }
    updates
}

struct IconGuest<'a>(&'a dyn GuestUi);
impl tsfp_ui_model::hud_native::GuestRead for IconGuest<'_> {
    fn read32(&self, va: u32) -> Option<u32> {
        self.0.read32(va)
    }
}
/// `Some(None)` is ordinary text; `None` refuses an undecoded icon-bearing
/// string. Mixed glyph/icon strings remain original until a shared layout owns
/// them; the editor's measured S/A/D labels are pure two-byte escapes.
fn read_icon(g: &dyn GuestUi, text: &LiveText) -> Option<Option<CurveIcon>> {
    use tsfp_ui_model::hud_prompts::{self, FontMetrics};
    let reader = IconGuest(g);
    let bytes = match text {
        LiveText::None => return Some(None),
        LiveText::Bytes(bytes) => bytes.clone(),
        LiveText::Id(id) => hud_prompts::string_by_id(&reader, *id)?,
    };
    if !bytes.contains(&0x0B) {
        return Some(None);
    }
    if bytes.len() != 2 || bytes[0] != 0x0B {
        return None;
    }
    let metrics = FontMetrics::read(&reader, 0x12)?;
    let icon = hud_prompts::icon(&reader, &metrics, bytes[1])?;
    if ![icon.size, icon.advance]
        .iter()
        .all(|v| v.is_finite() && *v >= 0.0 && *v <= 1024.0)
        || !icon.uv.iter().all(|v| v.is_finite())
    {
        return None;
    }
    Some(Some(CurveIcon {
        texture: icon.texture.map(str::to_owned),
        uv: tsfp_ui_renderer::UvRect {
            min: Vec2::new(icon.uv[0], icon.uv[1]),
            max: Vec2::new(icon.uv[2], icon.uv[3]),
        },
        size: icon.size,
        advance: icon.advance,
    }))
}

#[cfg(test)]
mod input_tests {
    use super::*;
    use tsfp_mapmaker::controls_ui::{
        ControlsPageGeneration, ControlsPageState, ControlsPageUpdate, ControlsRowId,
        MEASURED_CONTROLS_ROW_COUNT,
    };

    #[test]
    fn keyboard_steps_and_back_dispatch_into_owned_controls_state() {
        let mut state = ControlsPageState::new(
            ControlsPageGeneration::new(1),
            MEASURED_CONTROLS_ROW_COUNT,
            None,
        )
        .unwrap();
        let updates = dispatch_controls_commands(
            &mut state,
            &[
                NativeCommand::Step(Step::Down),
                NativeCommand::Step(Step::Down),
                NativeCommand::Back,
            ],
        );
        assert_eq!(
            updates,
            [
                ControlsPageUpdate::Focused(ControlsRowId::new(0).unwrap()),
                ControlsPageUpdate::Focused(ControlsRowId::new(1).unwrap()),
                ControlsPageUpdate::BackRequested,
            ]
        );
        assert_eq!(state.focused(), Some(ControlsRowId::new(1).unwrap()));
    }

    #[test]
    fn pointer_selection_updates_only_the_generation_scoped_focus() {
        let mut state = ControlsPageState::new(
            ControlsPageGeneration::new(4),
            MEASURED_CONTROLS_ROW_COUNT,
            None,
        )
        .unwrap();
        assert_eq!(
            dispatch_controls_commands(&mut state, &[NativeCommand::Select(17)]),
            [ControlsPageUpdate::Focused(ControlsRowId::new(17).unwrap())]
        );
        assert!(dispatch_controls_commands(&mut state, &[NativeCommand::Select(18)]).is_empty());
        assert_eq!(state.focused(), Some(ControlsRowId::new(17).unwrap()));
    }
}

//! T2189 UI-6 slice 13: `PagePresenter<WgpuUiRenderer>` with the frame-hash scaffold.
//! FABRICATED layouts, MEASURED software-GPU (llvmpipe) output, cross-checked against the
//! CPU reference `CpuRasterRenderer`. No game parity evidence. Fails without an adapter.
use tsfp_ui_model::frame::{ShapedRun, TextPart};
use tsfp_ui_model::framehash::{self, Golden, HashCheck};
use tsfp_ui_model::present::PagePresenter;
use tsfp_ui_model::raster::{BoxAtlas, CpuRasterRenderer};
use tsfp_ui_model::switch::UiSwitch;
use tsfp_ui_model::{PageId, RenderLayout, TextPlacement, TextShaper};
use tsfp_ui_renderer::{Color, Quad, Vec2, Viewport};
use tsfp_ui_wgpu::present::{WGPU_ORIGINAL_PRESET_GOLDENS, present_and_check};
use tsfp_ui_wgpu::{UiBackend, WgpuUiRenderer};
use tsfp_wgpu::context::{GpuContext, Preference};

const PAGE: PageId = PageId(0x20);
const CLEAR: [u8; 4] = [0, 0, 0, 255];
const SIZE: (u32, u32) = (64, 48);

/// Quad-only pages: glyph coverage needs the original font pages (private image).
struct NoText;
impl TextShaper for NoText {
    type Error = ();
    fn shape(&mut self, _: &TextPlacement, _: TextPart) -> Result<Option<ShapedRun>, ()> {
        Ok(None)
    }
}

mod support;
use support::rect;

fn viewport() -> Viewport {
    let size = Vec2::new(SIZE.0 as f32, SIZE.1 as f32);
    Viewport::new(size, 1.0, rect(0.0, 0.0, size.x, size.y)).unwrap()
}
fn layout(shift: f32) -> RenderLayout {
    RenderLayout {
        quads: vec![
            Quad {
                rect_px: rect(4.0 + shift, 4.0, 40.0, 20.0),
                color: Color([0.0, 0.0, 1.0, 1.0]),
            },
            Quad {
                rect_px: rect(10.0, 12.0, 30.0, 24.0),
                color: Color([1.0, 0.0, 0.0, 0.5]),
            },
        ],
        texts: vec![],
    }
}
fn rust() -> UiSwitch {
    UiSwitch::parse(None, ["--ui=rust"]).unwrap().0
}
fn target(gpu: &GpuContext) -> wgpu::Texture {
    // The overlay loads the scene; clear it to the CPU reference clear colour.
    support::target(gpu, "ui-presenter-test", SIZE, CLEAR)
}

/// Present once on wgpu; returns the check and the readback.
fn wgpu_run(switch: UiSwitch, goldens: &[Golden], shift: f32) -> (HashCheck, Vec<u8>) {
    let gpu = GpuContext::new(&Preference::Software).expect("software GPU required");
    let texture = target(&gpu);
    let renderer = WgpuUiRenderer::new(&gpu.device, &gpu.queue, &texture, UiBackend::Wgpu).unwrap();
    let mut presenter = PagePresenter::new(switch, renderer);
    let checked = present_and_check(
        &mut presenter,
        goldens,
        PAGE,
        true,
        &layout(shift),
        &mut NoText,
        viewport(),
    )
    .unwrap();
    let rgba = presenter.renderer.read_target_rgba().unwrap();
    (checked, rgba)
}

fn cpu_reference(shift: f32) -> Vec<u8> {
    let renderer = CpuRasterRenderer::new(BoxAtlas { width_ratio: 0.5 }, CLEAR);
    let mut presenter = PagePresenter::new(rust(), renderer);
    framehash::present_and_check(
        &mut presenter,
        &[],
        PAGE,
        true,
        &layout(shift),
        &mut NoText,
        viewport(),
    )
    .unwrap();
    presenter.renderer.finished().unwrap().rgba.clone()
}

#[test]
fn wgpu_presenter_matches_cpu_reference_in_colour() {
    let (checked, gpu) = wgpu_run(rust(), WGPU_ORIGINAL_PRESET_GOLDENS, 0.0);
    let actual = framehash::frame_hash_rgba(SIZE.0, SIZE.1, &gpu);
    assert_eq!(checked, HashCheck::Pending { actual });
    let cpu = cpu_reference(0.0);
    assert_eq!(gpu.len(), cpu.len());
    // Spot pixels: opaque blue panel, half red over blue, half red over the clear colour.
    let at = |x: u32, y: u32| &gpu[((y * SIZE.0 + x) * 4) as usize..][..4];
    assert_eq!(at(5, 5), [0, 0, 255, 255]);
    assert!(at(20, 20)[0].abs_diff(128) <= 1 && at(20, 20)[2].abs_diff(128) <= 1);
    assert!(at(20, 30)[0].abs_diff(128) <= 1 && at(20, 30)[2] == 0);
    // MEASURED: every channel within 1 of the CPU reference, alpha included, with the
    // off-by-one bytes only where the half-alpha quad blends (8-bit rounding differs).
    // So the hashes differ and each renderer keeps its own golden table.
    let mut rounded = 0;
    for (index, (g, c)) in gpu.iter().zip(&cpu).enumerate() {
        assert!(g.abs_diff(*c) <= 1, "byte {index}: {g} vs {c}");
        rounded += usize::from(g != c);
    }
    println!("off-by-one bytes: {rounded}");
    assert!(rounded > 0 && rounded <= 30 * 24 * 4);
    assert_ne!(actual, framehash::frame_hash_rgba(SIZE.0, SIZE.1, &cpu));
}

#[test]
fn wgpu_presenter_golden_match_and_shift_mismatch() {
    let (_, rgba) = wgpu_run(rust(), &[], 0.0);
    let golden = Golden {
        page: PAGE,
        width: SIZE.0,
        height: SIZE.1,
        hash: framehash::frame_hash_rgba(SIZE.0, SIZE.1, &rgba),
    };
    assert_eq!(wgpu_run(rust(), &[golden], 0.0).0, HashCheck::Match);
    assert!(matches!(
        wgpu_run(rust(), &[golden], 1.0).0,
        HashCheck::Mismatch { .. }
    ));
}

#[test]
fn legacy_default_draws_nothing_on_wgpu() {
    let (checked, rgba) = wgpu_run(UiSwitch::default(), &[], 0.0);
    assert_eq!(checked, HashCheck::NotRust);
    assert_eq!(rgba, CLEAR.repeat((SIZE.0 * SIZE.1) as usize));
}

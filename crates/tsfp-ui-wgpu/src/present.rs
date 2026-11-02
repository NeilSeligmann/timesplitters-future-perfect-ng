//! `PagePresenter<WgpuUiRenderer>` for the `original` preset with a frame-hash check
//! (T2189 UI-6 slice 13). The presenter routes the page; only `Route::Rust` pages draw into
//! the target, which is then read back and hashed with `tsfp_ui_model::framehash`. Legacy
//! stays the default route, so with a default switch nothing is drawn or read back.
//!
//! Goldens are per renderer: on llvmpipe a quad-only frame reads back within 1 per channel of
//! the CPU reference, but half-alpha blends round differently (MEASURED in tests/presenter.rs),
//! so the hashes differ and a CPU golden is never reused here.

use crate::{Error, WgpuUiRenderer};
use tsfp_ui_model::framehash::{Golden, HashCheck, check_rgba};
use tsfp_ui_model::present::{PagePresenter, PresentError, Presented};
use tsfp_ui_model::{PageId, RenderLayout, TextShaper};
use tsfp_ui_renderer::{Sprite, Viewport};

/// Original-preset goldens of the wgpu path. Empty until a legacy capture exists, so every
/// Rust page reports `Pending` (unverified debt, not a pass).
pub const WGPU_ORIGINAL_PRESET_GOLDENS: &[Golden] = &[];

/// Present `page` through wgpu, read the target back and check its frame hash.
pub fn present_and_check<S: TextShaper>(
    presenter: &mut PagePresenter<WgpuUiRenderer<'_>>,
    goldens: &[Golden],
    page: PageId,
    layout_complete: bool,
    layout: &RenderLayout,
    shaper: &mut S,
    viewport: Viewport,
) -> Result<HashCheck, PresentError<S::Error, Error>> {
    present_and_check_skinned(
        presenter,
        goldens,
        page,
        layout_complete,
        layout,
        &[],
        shaper,
        viewport,
    )
}

/// [`present_and_check`] with the skin sprites of `tsfp_ui_model::skin` under the text (T2307).
#[allow(clippy::too_many_arguments)]
pub fn present_and_check_skinned<S: TextShaper>(
    presenter: &mut PagePresenter<WgpuUiRenderer<'_>>,
    goldens: &[Golden],
    page: PageId,
    layout_complete: bool,
    layout: &RenderLayout,
    sprites: &[Sprite],
    shaper: &mut S,
    viewport: Viewport,
) -> Result<HashCheck, PresentError<S::Error, Error>> {
    match presenter.present_skinned(page, layout_complete, layout, sprites, shaper, viewport)? {
        Presented::Legacy(_) => Ok(HashCheck::NotRust),
        Presented::Rust { .. } => {
            let rgba = presenter
                .renderer
                .read_target_rgba()
                .map_err(PresentError::Render)?;
            let (width, height) = presenter.renderer.target_size();
            Ok(check_rgba(goldens, page, width, height, &rgba))
        }
    }
}

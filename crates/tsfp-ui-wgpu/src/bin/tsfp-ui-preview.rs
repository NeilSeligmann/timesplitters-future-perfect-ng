//! `tsfp-ui-preview`: the owner-facing `--ui=rust` check (T2189 UI-6 slice 14). Loads a captured
//! menu tree (default `tmp/ui-tree/tree.json`, private, never tracked), finds the main menu page
//! by its builder, routes it through `PagePresenter<WgpuUiRenderer>` and writes the frame as a
//! PPM. A layout-incomplete page (or `--ui=legacy`) falls back to legacy: nothing is drawn and
//! the exit code is 4. Slice 15: with `--disc` the rows carry text (original font tables and
//! string banks, stand-in glyph pixels, see `overlay`); without a readable disc, panels only.
//!
//! usage: tsfp-ui-preview [--ui=rust|legacy] [--tree PATH] [--builder 0xADDR | --page 0xID]
//!                        [--size WxH] [--out PATH] [--disc ISO] [--lang e|f|g|i|s]
//!                        [--focus N] [--backdrop PPM] [--export-skin DIR] [--export-font-pages DIR]
//! T2307 fonts: `TSFP_UI_FONTS=outline` (feature `outline-fonts`) draws the rows from outline fonts in
//! `TSFP_UI_FONT_DIR` (default tmp/ui-fonts, see tools/dev/fetch_fonts.sh); `--export-font-pages` writes the original A8 pages.
//! T2307: with the disc the page uses the original skin (frame, fill, row highlight from the
//! disc textures; `TSFP_UI_SKIN_DIR` = owner override images). `--focus N` puts the cursor on the
//! N-th focusable row (0 = first), `--backdrop` draws over a P6 PPM of `--size` (e.g. the game
//! area of a legacy screenshot) for a side-by-side check. `--export-skin DIR` writes the decoded
//! skin textures as `<name>.png` plus the override pair `<name>.rgba` / `<name>.size` (owner-local,
//! for upscaled or redrawn replacements read back through `TSFP_UI_SKIN_DIR`).
use std::process::ExitCode;

use std::path::Path;

use tsfp_ui_model::frontend::{self, MAIN_MENU_BUILDER};
use tsfp_ui_model::switch::UiSwitch;
use tsfp_ui_model::{PageId, UiModel};
use tsfp_ui_wgpu::overlay::{self, TextAssets};
use tsfp_wgpu::context::{GpuContext, Preference};

struct Options {
    switch: UiSwitch,
    tree: String,
    builder: u64,
    page: Option<PageId>,
    size: (u32, u32),
    out: String,
    disc: Option<String>,
    language: char,
    focus: usize,
    backdrop: Option<String>,
    export_skin: Option<String>,
    export_font_pages: Option<String>,
}

fn hex(text: &str) -> Result<u64, String> {
    u64::from_str_radix(text.trim_start_matches("0x"), 16)
        .map_err(|e| format!("bad hex {text}: {e}"))
}

fn parse() -> Result<Options, String> {
    let env = std::env::var("TSFP_UI").ok();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut switch, rest) =
        UiSwitch::parse(env.as_deref(), args.iter().map(String::as_str)).map_err(|e| e.0)?;
    // The preview exists to show the Rust page, so an unflagged run means rust.
    if env.is_none() && !args.iter().any(|a| a.starts_with("--ui=") || a == "--ui") {
        switch = UiSwitch::parse(None, ["--ui=rust"]).map_err(|e| e.0)?.0;
    }
    let mut options = Options {
        switch,
        tree: "tmp/ui-tree/tree.json".into(),
        builder: MAIN_MENU_BUILDER,
        page: None,
        size: (640, 480),
        out: "tmp/ui-preview/main-menu.ppm".into(),
        disc: None,
        language: 'e',
        focus: 0,
        backdrop: None,
        export_skin: None,
        export_font_pages: None,
    };
    let mut it = rest.into_iter();
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or(format!("{arg} needs a value"));
        match arg {
            "--tree" => options.tree = value()?.to_owned(),
            "--builder" => options.builder = hex(value()?)?,
            "--page" => options.page = Some(PageId(hex(value()?)?)),
            "--out" => options.out = value()?.to_owned(),
            "--disc" => options.disc = Some(value()?.to_owned()),
            "--focus" => {
                let text = value()?;
                options.focus = text.parse().map_err(|_| format!("bad --focus {text}"))?;
            }
            "--backdrop" => options.backdrop = Some(value()?.to_owned()),
            "--export-skin" => options.export_skin = Some(value()?.to_owned()),
            "--export-font-pages" => options.export_font_pages = Some(value()?.to_owned()),
            "--lang" => {
                let text = value()?;
                options.language = match text {
                    "e" | "f" | "g" | "i" | "s" => text.chars().next().unwrap_or('e'),
                    _ => return Err(format!("bad --lang {text}: e, f, g, i or s")),
                };
            }
            "--size" => {
                let text = value()?;
                let (w, h) = text.split_once('x').ok_or(format!("bad --size {text}"))?;
                options.size = (
                    w.parse().map_err(|_| format!("bad --size {text}"))?,
                    h.parse().map_err(|_| format!("bad --size {text}"))?,
                );
            }
            "--help" | "-h" => return Err(String::new()),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(options)
}

fn write_ppm(path: &str, (width, height): (u32, u32), rgba: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
    bytes.extend(rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]));
    std::fs::write(path, bytes)
}

/// A binary PPM (P6, maxval 255) as RGBA8, opaque.
fn read_ppm(path: &str) -> Result<((u32, u32), Vec<u8>), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let mut fields = Vec::new();
    let mut at = 0;
    while fields.len() < 4 {
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        if at < bytes.len() && bytes[at] == b'#' {
            while at < bytes.len() && bytes[at] != b'\n' {
                at += 1;
            }
            continue;
        }
        let start = at;
        while at < bytes.len() && !bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        if start == at {
            return Err(format!("{path}: short PPM header"));
        }
        fields.push(String::from_utf8_lossy(&bytes[start..at]).into_owned());
    }
    let number = |i: usize| {
        fields[i]
            .parse::<u32>()
            .map_err(|_| format!("{path}: bad PPM header"))
    };
    if fields[0] != "P6" || number(3)? != 255 {
        return Err(format!("{path}: only binary P6 PPM with maxval 255"));
    }
    let size = (number(1)?, number(2)?);
    let pixels = bytes
        .get(at + 1..at + 1 + (size.0 * size.1 * 3) as usize)
        .ok_or(format!("{path}: PPM shorter than its header"))?;
    Ok((
        size,
        pixels
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
    ))
}

fn run(options: Options) -> Result<ExitCode, String> {
    let text = std::fs::read_to_string(&options.tree).map_err(|e| {
        format!(
            "cannot read menu tree {}: {e} (capture one, see docs/owner-run-guide.md)",
            options.tree
        )
    })?;
    let model = UiModel::from_tree_json(&text, &frontend::dump_load_options())
        .map_err(|e| format!("menu tree {}: {e:?}", options.tree))?;
    let page = match options.page {
        Some(page) => page,
        None => frontend::page_for_builder(&text, options.builder).ok_or(format!(
            "no page built by 0x{:x} in {}",
            options.builder, options.tree
        ))?,
    };
    // The cursor rests on the first focusable row, as when the menu opens (theme focus colour).
    let focused = model
        .page(page)
        .and_then(|p| {
            p.widgets
                .iter()
                .filter(|w| w.is_focusable())
                .nth(options.focus)
        })
        .map(|w| w.id);
    let text_assets = match &options.disc {
        Some(disc) => match TextAssets::from_disc(Path::new(disc), options.language) {
            Ok(assets) => Some(assets),
            Err(e) => {
                println!("ui-preview: no text ({e}), panels only");
                None
            }
        },
        None => None,
    };
    let gpu = GpuContext::new(&Preference::Software)?;
    println!("ui-preview: adapter {}", gpu.describe());
    if let Some(assets) = &text_assets {
        match (&assets.skin, &assets.skin_note) {
            (Some(skin), _) => println!(
                "ui-preview: original skin: {}",
                skin.images
                    .iter()
                    .map(|(role, image, source)| format!(
                        "{} {}x{} ({source})",
                        role.name(),
                        image.width,
                        image.height
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            (None, note) => println!(
                "ui-preview: no skin ({}), placeholder panels",
                note.as_deref().unwrap_or("?")
            ),
        }
    }
    if let (Some(dir), Some(skin)) = (
        &options.export_skin,
        text_assets.as_ref().and_then(|t| t.skin.as_ref()),
    ) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{dir}: {e}"))?;
        for (role, image, _) in &skin.images {
            let base = Path::new(dir).join(role.name());
            let png = tsfp_import::png::encode(
                image.width as usize,
                image.height as usize,
                tsfp_import::png::Color::Rgba,
                &image.rgba,
            );
            std::fs::write(base.with_extension("png"), png).map_err(|e| format!("{dir}: {e}"))?;
            std::fs::write(base.with_extension("rgba"), &image.rgba)
                .map_err(|e| format!("{dir}: {e}"))?;
            std::fs::write(
                base.with_extension("size"),
                format!("{} {}\n", image.width, image.height),
            )
            .map_err(|e| format!("{dir}: {e}"))?;
        }
        println!("ui-preview: skin textures written to {dir} (owner-local, never commit)");
    }
    #[cfg(not(feature = "outline-fonts"))]
    for font in tsfp_ui_wgpu::skin_assets::font_overrides_from_env() {
        println!(
            "ui-preview: improved preset: {} font for the '{}' atlas at {} found; this build lacks the outline-fonts feature (sh tools/dev/build.sh enables it), the original pages still draw",
            font.kind,
            font.atlas,
            font.path.display()
        );
    }
    #[cfg(feature = "outline-fonts")]
    if tsfp_ui_wgpu::outline_font::selected_from_env() {
        let dir = tsfp_ui_wgpu::outline_font::font_dir_from_env();
        let faces = tsfp_ui_wgpu::outline_font::find_faces(&dir);
        if faces.is_empty() {
            println!(
                "ui-preview: TSFP_UI_FONTS=outline but no font in {} (run sh tools/dev/fetch_fonts.sh), the original pages draw",
                dir.display()
            );
        }
        for face in faces {
            println!(
                "ui-preview: improved preset: '{}' atlas drawn from {} ({} face)",
                face.atlas,
                face.path.display(),
                face.kind
            );
        }
    }
    if let (Some(dir), Some(assets)) = (&options.export_font_pages, &text_assets) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{dir}: {e}"))?;
        for (i, page) in assets.font_pages.iter().enumerate() {
            let mut pgm = format!("P5\n{} {}\n255\n", page.width, page.height).into_bytes();
            pgm.extend_from_slice(&page.coverage);
            std::fs::write(format!("{dir}/font-page-{i}.pgm"), pgm)
                .map_err(|e| format!("{dir}: {e}"))?;
        }
        println!(
            "ui-preview: {} font pages written to {dir} (owner-local, never commit)",
            assets.font_pages.len()
        );
    }
    let frame = match &options.backdrop {
        Some(path) => {
            let (size, rgba) = read_ppm(path)?;
            if size != options.size {
                return Err(format!(
                    "{path} is {}x{}, --size is {}x{}",
                    size.0, size.1, options.size.0, options.size.1
                ));
            }
            overlay::render_page_over(
                &gpu,
                options.switch,
                &model,
                page,
                focused,
                text_assets.as_ref(),
                options.size,
                &rgba,
            )?
        }
        // Opaque black backdrop: the host scene behind the menu is not part of this preview.
        None => overlay::render_page(
            &gpu,
            options.switch,
            &model,
            page,
            focused,
            text_assets.as_ref(),
            options.size,
            [0, 0, 0, 255],
        )?,
    };
    if let Some(note) = &frame.layout_note {
        println!("ui-preview: page 0x{:x} layout incomplete: {note}", page.0);
    }
    println!("ui-preview: route 0x{:x} {}", page.0, frame.route.name());
    println!("ui-preview: frame check {:?}", frame.check);
    let Some(rgba) = frame.rgba else {
        println!("ui-preview: legacy keeps the page, nothing drawn");
        return Ok(ExitCode::from(4));
    };
    write_ppm(&options.out, options.size, &rgba).map_err(|e| format!("{}: {e}", options.out))?;
    println!(
        "ui-preview: page drawn by the Rust UI: {} panels, {} skin sprites, {} text rows, {} text runs drawn, {} declined, glyphs from the {}, {} glyphs without a cell -> {}",
        frame.quads,
        frame.sprites,
        frame.text_rows,
        frame.text_runs,
        frame.declined_text,
        frame.glyphs,
        frame.dropped_glyphs,
        options.out
    );
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    match parse().and_then(run) {
        Ok(code) => code,
        Err(message) => {
            if !message.is_empty() {
                eprintln!("tsfp-ui-preview: {message}");
            }
            eprintln!(
                "usage: tsfp-ui-preview [--ui=rust|legacy] [--tree PATH] [--builder 0xADDR | --page 0xID] [--size WxH] [--out PATH] [--disc ISO] [--lang e|f|g|i|s] [--focus N] [--backdrop PPM] [--export-skin DIR] [--export-font-pages DIR] (env TSFP_UI_FONTS=outline: improved outline fonts)"
            );
            ExitCode::from(2)
        }
    }
}

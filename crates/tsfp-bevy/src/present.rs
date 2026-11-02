// SPDX-License-Identifier: GPL-3.0-or-later
//! T2036 slice 1: presentation maths and shell options for aspect and filtering (docs/t-widescreen-design.md).
//! Pure functions, no Bevy types, so the scaling is unit tested. Shell presentation only: the game still renders 4:3 or
//! (with the host's `--eeprom-widescreen`) its own anamorphic 16:9 into a 640x480 target.
use std::sync::OnceLock;

/// What `--aspect` asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AspectRequest {
    /// Follow what the game renders (4:3 now).
    Auto,
    /// Width over height.
    Ratio(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    Nearest,
    Linear,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShellOptions {
    pub aspect: AspectRequest,
    pub filter: Filter,
}

impl Default for ShellOptions {
    fn default() -> Self {
        Self {
            aspect: AspectRequest::Auto,
            filter: Filter::Linear,
        }
    }
}

static OPTIONS: OnceLock<ShellOptions> = OnceLock::new();

pub fn set_options(options: ShellOptions) {
    let _ = OPTIONS.set(options);
}

pub fn options() -> ShellOptions {
    OPTIONS.get().copied().unwrap_or_default()
}

pub fn parse_aspect(text: &str) -> Result<AspectRequest, String> {
    if text == "auto" {
        return Ok(AspectRequest::Auto);
    }
    let bad = || format!("bad --aspect {text:?}: expected auto, 4:3, 16:9, 21:9 or <w>:<h>");
    let (w, h) = text.split_once(':').ok_or_else(bad)?;
    let (w, h): (f32, f32) = (w.parse().map_err(|_| bad())?, h.parse().map_err(|_| bad())?);
    if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) || w / h < 1.0 || w / h > 8.0 {
        return Err(bad());
    }
    Ok(AspectRequest::Ratio(w / h))
}

pub fn parse_filter(text: &str) -> Result<Filter, String> {
    match text {
        "nearest" => Ok(Filter::Nearest),
        "linear" => Ok(Filter::Linear),
        _ => Err(format!("bad --filter {text:?}: expected nearest or linear")),
    }
}

/// Take `--aspect X` and `--filter X` out of the arguments (they are the shell's, not the C host's).
pub fn extract(arguments: &mut Vec<String>) -> Result<ShellOptions, String> {
    let mut options = ShellOptions::default();
    let mut at = 0;
    while at < arguments.len() {
        let name = arguments[at].as_str();
        if name == "--aspect" || name == "--filter" {
            let is_aspect = name == "--aspect";
            let value = arguments
                .get(at + 1)
                .ok_or_else(|| format!("{name} needs a value"))?
                .clone();
            if is_aspect {
                options.aspect = parse_aspect(&value)?;
            } else {
                options.filter = parse_filter(&value)?;
            }
            arguments.drain(at..at + 2);
        } else {
            at += 1;
        }
    }
    Ok(options)
}

pub const ASPECT_4_3: f32 = 4.0 / 3.0;
pub const ASPECT_16_9: f32 = 16.0 / 9.0;

/// What the game is asked to render and how its picture is shown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    /// Pass `--eeprom-widescreen` to the host: the game takes its own 16:9 path (wider projection, 0.75 UI scale).
    pub game_widescreen: bool,
    /// Aspect the finished picture is displayed at (the game's own, 4:3 or 16:9). The 640x480 buffer is stretched to it.
    pub picture_aspect: f32,
    /// Aspect used to size the initial window (the requested one).
    pub window_aspect: f32,
    /// T2094: `--aspect-override W:H` for the host, so the game itself renders this aspect (Hor+ projection). None = the game's own 4:3 or 16:9.
    pub aspect_override: Option<(u32, u32)>,
}

/// Slice 1: the game only knows 4:3 and exactly 16:9, so a request of 16:9 or wider uses the game's 16:9 mode and shows it
/// at 16:9 inside the window (pillarbox on 21:9 and 32:9 until the game patch and the wgpu target exist, slices 2+).
pub fn plan(request: AspectRequest) -> Plan {
    match request {
        AspectRequest::Auto => Plan {
            game_widescreen: false,
            picture_aspect: ASPECT_4_3,
            window_aspect: ASPECT_4_3,
            aspect_override: None,
        },
        AspectRequest::Ratio(ratio) if (ratio - ASPECT_16_9).abs() < 0.01 => Plan {
            game_widescreen: true,
            picture_aspect: ASPECT_16_9,
            window_aspect: ratio,
            aspect_override: None,
        },
        AspectRequest::Ratio(ratio) if ratio > ASPECT_4_3 + 0.01 => {
            // The game renders this aspect itself (T2094): the shown picture is that aspect, no pillarbox.
            let (num, den) = ratio_pair(ratio);
            Plan {
                game_widescreen: ratio > ASPECT_16_9,
                picture_aspect: num as f32 / den as f32,
                window_aspect: ratio,
                aspect_override: Some((num, den)),
            }
        }
        AspectRequest::Ratio(ratio) => Plan {
            game_widescreen: false,
            picture_aspect: ASPECT_4_3,
            window_aspect: ratio,
            aspect_override: None,
        },
    }
}

/// Shortest `num:den` (den <= 30) within 0.1 percent of `ratio`, else a hundredths pair (same rule as the host's aspect_override.c).
pub fn ratio_pair(ratio: f32) -> (u32, u32) {
    for den in 1..=30u32 {
        let num = (ratio * den as f32 + 0.5) as u32;
        if num >= den && ((num as f32 / den as f32) - ratio).abs() <= ratio * 0.001 {
            let g = gcd(num, den);
            return (num / g, den / g);
        }
    }
    ((ratio * 100.0).round() as u32, 100)
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Default window: 960 rows high at the requested aspect, at most 2560 wide.
pub fn default_window_size(window_aspect: f32) -> (f32, f32) {
    let height = window_height(
        std::env::var("TSFP_WINDOW_HEIGHT").ok().as_deref(),
        crate::input_recording::fast_forward_switch(
            std::env::var("TSFP_FAST_FORWARD").ok().as_deref(),
        ),
    );
    ((height * window_aspect).min(2560.0).round(), height)
}

/// T3388: window height in pixels. `TSFP_WINDOW_HEIGHT` (64 to 4320) wins; a fast-forward replay
/// defaults to 120 so lavapipe does not spend most of the replay's CPU drawing a 1280x960 window
/// nobody watches (MEASURED on r5: llvmpipe threads used 301 CPU s of a 50 s replay); else 960.
pub fn window_height(requested: Option<&str>, fast_forward: bool) -> f32 {
    if let Some(height) = requested.and_then(|value| value.trim().parse::<u32>().ok()) {
        return height.clamp(64, 4320) as f32;
    }
    if fast_forward { 120.0 } else { 960.0 }
}

/// Largest rectangle of aspect `aspect` that fits in a window of `window` size (width, height), centred: (width, height).
/// The remainder is the letterbox or pillarbox bars. Degenerate sizes give zero.
pub fn fit(window: (f32, f32), aspect: f32) -> (f32, f32) {
    if !(window.0 > 0.0 && window.1 > 0.0 && aspect > 0.0) {
        return (0.0, 0.0);
    }
    if window.0 / window.1 >= aspect {
        (window.1 * aspect, window.1)
    } else {
        (window.0, window.0 / aspect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn fit_pillarboxes_a_wider_window() {
        let (w, h) = fit((1920.0, 1080.0), ASPECT_4_3);
        assert!(close(w, 1440.0) && close(h, 1080.0));
    }

    #[test]
    fn fit_letterboxes_a_taller_window() {
        let (w, h) = fit((1000.0, 1000.0), ASPECT_16_9);
        assert!(close(w, 1000.0) && close(h, 562.5));
    }

    #[test]
    fn fit_exact_aspect_fills() {
        assert_eq!(fit((1280.0, 960.0), ASPECT_4_3), (1280.0, 960.0));
        let (w, h) = fit((3440.0, 1440.0), 3440.0 / 1440.0);
        assert!(close(w, 3440.0) && close(h, 1440.0));
    }

    #[test]
    fn fit_ultrawide_pillarboxes_sixteen_nine() {
        let (w, h) = fit((3440.0, 1440.0), ASPECT_16_9);
        assert!(close(w, 2560.0) && close(h, 1440.0));
        let (w, h) = fit((5120.0, 1440.0), ASPECT_16_9); // 32:9 monitor
        assert!(close(w, 2560.0) && close(h, 1440.0));
    }

    #[test]
    fn fit_never_exceeds_the_window_and_degenerate_is_zero() {
        for (ww, wh) in [(1.0, 1.0), (7.0, 900.0), (900.0, 7.0), (2560.0, 1080.0)] {
            for a in [ASPECT_4_3, ASPECT_16_9, 21.0 / 9.0, 32.0 / 9.0] {
                let (w, h) = fit((ww, wh), a);
                assert!(w <= ww + 1e-3 && h <= wh + 1e-3 && close(w / h, a));
            }
        }
        assert_eq!(fit((0.0, 100.0), ASPECT_4_3), (0.0, 0.0));
        assert_eq!(fit((100.0, 100.0), 0.0), (0.0, 0.0));
    }

    #[test]
    fn parse_aspect_forms() {
        assert_eq!(parse_aspect("auto"), Ok(AspectRequest::Auto));
        assert!(
            matches!(parse_aspect("21:9"), Ok(AspectRequest::Ratio(r)) if close(r, 21.0 / 9.0))
        );
        assert!(matches!(parse_aspect("16:10"), Ok(AspectRequest::Ratio(r)) if close(r, 1.6)));
        for bad in [
            "", "16", "0:9", "16:0", "a:b", "9:16", "100:1", "-4:3", "nan:3",
        ] {
            assert!(parse_aspect(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn plan_selects_game_mode() {
        assert!(!plan(AspectRequest::Auto).game_widescreen);
        let four_three = plan(parse_aspect("4:3").unwrap());
        assert!(!four_three.game_widescreen && close(four_three.picture_aspect, ASPECT_4_3));
        let sixteen_ten = plan(parse_aspect("16:10").unwrap());
        assert!(!sixteen_ten.game_widescreen && close(sixteen_ten.window_aspect, 1.6));
        assert_eq!(sixteen_ten.aspect_override, Some((8, 5)));
        let sixteen_nine = plan(parse_aspect("16:9").unwrap());
        assert!(
            sixteen_nine.game_widescreen
                && close(sixteen_nine.picture_aspect, ASPECT_16_9)
                && sixteen_nine.aspect_override.is_none()
        );
        for (text, pair) in [("21:9", (7, 3)), ("32:9", (32, 9))] {
            let plan = plan(parse_aspect(text).unwrap());
            assert!(
                plan.game_widescreen && close(plan.picture_aspect, pair.0 as f32 / pair.1 as f32),
                "{text}"
            );
            assert_eq!(plan.aspect_override, Some(pair), "{text}");
        }
        assert_eq!(plan(parse_aspect("auto").unwrap()).aspect_override, None);
        assert_eq!(ratio_pair(3440.0 / 1440.0), (43, 18));
    }

    #[test]
    fn window_height_env_wins_and_fast_forward_is_small() {
        assert_eq!(window_height(None, false), 960.0);
        assert_eq!(window_height(None, true), 120.0);
        assert_eq!(window_height(Some("480"), true), 480.0);
        assert_eq!(window_height(Some("8"), false), 64.0);
        assert_eq!(window_height(Some("junk"), false), 960.0);
    }

    #[test]
    fn default_window_size_is_large_and_capped() {
        assert_eq!(default_window_size(ASPECT_4_3), (1280.0, 960.0));
        assert_eq!(default_window_size(32.0 / 9.0).0, 2560.0);
    }

    #[test]
    fn extract_removes_shell_options_only() {
        let mut arguments: Vec<String> = [
            "a.xbe",
            "--aspect",
            "21:9",
            "--filter",
            "nearest",
            "--interactive",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let options = extract(&mut arguments).unwrap();
        assert_eq!(arguments, ["a.xbe", "--interactive"]);
        assert_eq!(options.filter, Filter::Nearest);
        assert!(matches!(options.aspect, AspectRequest::Ratio(_)));
        assert!(extract(&mut vec!["--aspect".to_string()]).is_err());
        assert!(extract(&mut vec!["--filter".to_string(), "cubic".to_string()]).is_err());
    }
}

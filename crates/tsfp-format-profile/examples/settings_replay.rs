//! T2196 replay of original Accept writes through the Rust settings model.
//!
//!     cargo run -p tsfp-format-profile --example settings_replay -- tmp/t2196/accepts
//!
//! Input: the files of `python3 -m tools.t2196_settings_trace extract` (owner profile data, tmp/ only).
//! For every Accept the original wrote (hybrid host, lifted or Rust-port code), two model paths start
//! from the record before the Accept and the page's working state, and must reproduce the whole
//! 0x1F54-byte record after it: the typed path (`SettingsEdit`) and the runtime table path
//! (`GuestTables` + `SettingsPages::apply`). Exit 1 on any mismatch.
use std::{env, fs, path::Path, process::ExitCode};
use tsfp_format_profile::{
    Profile,
    runtime::{GuestTables, PAGE_AUDIO_VIDEO, PAGE_CONTROLS, PROFILE_SLOT0, Snapshot},
    settings::{AudioVideo, Binding, HostSettings, Settings, SettingsEdit, SettingsPages},
};

struct NoHost;
impl HostSettings for NoHost {
    fn set(&mut self, _key: u32, _value: u32) -> tsfp_format_core::Result<()> {
        Err("no host rows in the original preset")
    }
}

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn profile(record: &[u8]) -> Profile {
    Profile {
        payload: record.to_vec(),
        signature: [0; 20],
    }
}

/// Apply every row of one page from a working-state view (payload-relative word reader).
fn apply_rows(
    pages: &SettingsPages,
    page: u32,
    target: &mut Profile,
    value_at: impl Fn(usize) -> Option<u32>,
) -> Result<(), String> {
    for row in pages.rows.iter().filter(|r| r.page == page) {
        let (offset, mask) = match row.binding {
            Binding::ProfileWord { offset } => (offset, u32::MAX),
            Binding::ProfileBits { offset, mask } => (offset, mask),
            Binding::Host { .. } => return Err("host row".into()),
        };
        let Some(value) = value_at(offset) else {
            continue;
        };
        pages
            .apply(target, page, row.row, value & mask, &mut NoHost)
            .map_err(|e| format!("row {page}/{}: {e}", row.row))?;
    }
    Ok(())
}

fn compare(label: &str, got: &[u8], want: &[u8]) -> bool {
    let diff: Vec<usize> = (0..want.len()).filter(|&i| got[i] != want[i]).collect();
    if diff.is_empty() {
        println!("  {label}: EQUAL (0x{:x} bytes)", want.len());
        true
    } else {
        println!(
            "  {label}: DIFF at {} bytes, first R+0x{:x}",
            diff.len(),
            diff[0]
        );
        false
    }
}

fn replay(stem: &str, code: Option<&[u8]>) -> Result<bool, String> {
    let read = |suffix: &str| fs::read(format!("{stem}.{suffix}.bin")).map_err(|e| e.to_string());
    let (before, after, ui, audio) = (read("before")?, read("after")?, read("ui")?, read("audio")?);
    let changed: Vec<usize> = (0..after.len())
        .filter(|&i| before[i] != after[i])
        .collect();
    let controls = changed.iter().all(|&i| (4..0x80).contains(&i));
    let audio_video = changed.iter().all(|&i| (0x198..0x1b4).contains(&i));
    println!("{stem}: {} record bytes changed", changed.len());
    let snapshot = Snapshot {
        base: PROFILE_SLOT0,
        bytes: &before,
    };
    let mut pages = SettingsPages::load(&mut GuestTables {
        guest: &snapshot,
        slot: 0,
    })
    .map_err(|e| format!("runtime tables: {e}"))?;
    if let Some(code) = code {
        let code = Snapshot {
            base: 0,
            bytes: code,
        };
        // Full guest code image produced at runtime from the owner's executable.
        GuestTables {
            guest: &code,
            slot: 0,
        }
        .resolve_labels(&mut pages)
        .map_err(|e| format!("runtime labels: {e}"))?;
        GuestTables {
            guest: &code,
            slot: 0,
        }
        .label(PAGE_CONTROLS, 0)
        .map_err(|e| format!("layout label: {e}"))?;
        GuestTables {
            guest: &code,
            slot: 0,
        }
        .label(PAGE_CONTROLS, 9)
        .map_err(|e| format!("turn-speed label: {e}"))?;
        println!("  original row labels: {} resolved", pages.rows.len());
    }
    let mut ok = true;
    if controls {
        let working = Settings::from_head(&ui[..0x7c]).map_err(String::from)?;
        let mut typed = profile(&before);
        let mut edit = SettingsEdit::open(&typed).map_err(String::from)?;
        edit.controls.set_layout(working.layout());
        if working.mapping() != edit.controls.mapping() {
            edit.controls.set_custom_mapping(working.mapping());
            edit.controls.set_layout(working.layout());
        }
        edit.controls
            .set_controls(working.controls().map_err(String::from)?);
        edit.controls
            .set_turn_speed(working.turn_speed())
            .map_err(String::from)?;
        edit.accept_controls(&mut typed).map_err(String::from)?;
        ok &= compare("controls typed path", &typed.payload, &after);
        let mut table = profile(&before);
        apply_rows(&pages, PAGE_CONTROLS, &mut table, |offset| {
            Some(word(&ui, offset - 4))
        })?;
        let mut head = table.settings().map_err(String::from)?;
        head.set_turn_speed(working.turn_speed())
            .map_err(String::from)?;
        table.accept_settings(&head).map_err(String::from)?;
        ok &= compare("controls runtime-table path", &table.payload, &after);
    } else if audio_video {
        let value = AudioVideo {
            music_enabled: word(&audio, 0),
            sound_enabled: word(&audio, 4),
            music_volume: word(&audio, 8),
            sound_volume: word(&audio, 0xc),
            slider_4: word(&audio, 0x10),
            option_5: word(&audio, 0x14),
        };
        let mut typed = profile(&before);
        let mut edit = SettingsEdit::open(&typed).map_err(String::from)?;
        edit.audio_video = value;
        edit.accept_audio_video(&mut typed).map_err(String::from)?;
        ok &= compare("audio/video typed path", &typed.payload, &after);
        let source = |offset: usize| match offset {
            0x198 => Some(value.music_enabled),
            0x19c => Some(value.sound_enabled),
            0x1a0 => Some(value.music_volume),
            0x1a4 => Some(value.sound_volume),
            0x1b0 => Some(value.option_5),
            _ => None,
        };
        let mut table = profile(&before);
        apply_rows(&pages, PAGE_AUDIO_VIDEO, &mut table, source)?;
        ok &= compare("audio/video runtime-table path", &table.payload, &after);
    } else {
        println!(
            "  not a settings Accept (bytes outside both write sets, e.g. profile load), skipped"
        );
    }
    Ok(ok)
}

fn main() -> ExitCode {
    let Some(dir) = env::args().nth(1) else {
        eprintln!("usage: settings_replay DIR");
        return ExitCode::from(2);
    };
    let code = env::args().nth(2).map(fs::read).transpose();
    let code = match code {
        Ok(code) => code,
        Err(error) => {
            eprintln!("code image: {error}");
            return ExitCode::from(2);
        }
    };
    let mut stems: Vec<String> = fs::read_dir(Path::new(&dir))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.path().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    stems.retain(|p| p.ends_with(".before.bin"));
    stems
        .iter_mut()
        .for_each(|p| p.truncate(p.len() - ".before.bin".len()));
    stems.sort();
    if stems.is_empty() {
        eprintln!("no accepts in {dir}");
        return ExitCode::from(2);
    }
    let mut ok = true;
    for stem in &stems {
        match replay(stem, code.as_deref()) {
            Ok(equal) => ok &= equal,
            Err(error) => {
                println!("{stem}: ERROR {error}");
                ok = false;
            }
        }
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

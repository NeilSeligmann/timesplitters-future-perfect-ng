//! Replay owner-data Accepts through native settings persistence, then reload from disk.
//! Usage: settings_save ACCEPT_DIR OUTPUT_DIR. Output contains private profile data.
//! Signatures are fabricated zeros; this validates native storage, not Xbox authentication.
use std::{env, fs, path::PathBuf};
use tsfp_engine_fileio::{save::{SettingsPage, load_profile, save_settings}, vfs::Vfs};
use tsfp_format_profile::{Profile, settings::{AudioVideo, Settings, SettingsEdit}};
fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn main() -> Result<(), String> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 { return Err("usage: settings_save ACCEPT_DIR OUTPUT_DIR".into()); }
    let source = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let mut vfs = Vfs::new();
    vfs.mount_host_dir("C:", &output).map_err(|e| format!("mount {e:x}"))?;
    let mut count = 0;
    for entry in fs::read_dir(&source).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path.file_name().unwrap().to_string_lossy();
        let Some(stem) = name.strip_suffix(".before.bin") else { continue };
        let read = |suffix| fs::read(source.join(format!("{stem}.{suffix}.bin"))).map_err(|e| e.to_string());
        let before = read("before")?;
        let mut after = read("after")?;
        let changed: Vec<_> = (0..before.len()).filter(|&i| before[i] != after[i]).collect();
        let mut profile = Profile { payload: before, signature: [0; 20] };
        let mut edit = SettingsEdit::open(&profile).map_err(String::from)?;
        let page = if changed.iter().all(|i| (4..0x80).contains(i)) {
            let ui = read("ui")?;
            let working = Settings::from_head(&ui[..0x7c]).map_err(String::from)?;
            edit.controls.set_layout(working.layout());
            edit.controls.set_controls(working.controls().map_err(String::from)?);
            edit.controls.set_turn_speed(working.turn_speed()).map_err(String::from)?;
            SettingsPage::Controls
        } else if changed.iter().all(|i| (0x198..0x1b4).contains(i)) {
            let audio = read("audio")?;
            edit.audio_video = AudioVideo {
                music_enabled: word(&audio, 0), sound_enabled: word(&audio, 4),
                music_volume: word(&audio, 8), sound_volume: word(&audio, 12),
                slider_4: word(&audio, 16), option_5: word(&audio, 20),
            };
            SettingsPage::AudioVideo
        } else { continue };
        // The runtime pointer is never serialized (original save boundary contract).
        profile.payload[..4].fill(0);
        after[..4].fill(0);
        let path = format!("C:\\accept-{stem}");
        save_settings(&mut vfs, &path, &mut profile, edit, page).map_err(|e| format!("save {e:x}"))?;
        let reloaded = load_profile(&mut vfs, &path).map_err(|e| format!("load {e:x}"))?;
        if reloaded.payload != after { return Err(format!("{stem}: reloaded record differs")); }
        count += 1;
        println!("{stem}: native save/reload EQUAL (runtime pointer normalized, unsigned)");
    }
    if count == 0 { return Err("no settings Accepts".into()); }
    Ok(())
}

//! Replay original Cancel/B pending edit decisions on owned settings.
use std::{env, fs};
use tsfp_format_profile::{
    Profile,
    settings::{Settings, SettingsEdit},
};
fn main() -> Result<(), String> {
    let dir = env::args()
        .nth(1)
        .ok_or("usage: settings_back_replay PRIVATE_CASE_DIR")?;
    for name in ["cancel", "back"] {
        let bytes = fs::read(format!("{dir}/{name}.profile.bin")).map_err(|e| e.to_string())?;
        let working = fs::read(format!("{dir}/{name}.working.bin")).map_err(|e| e.to_string())?;
        let profile = Profile {
            payload: bytes.clone(),
            signature: [0; 20],
        };
        let mut edit = SettingsEdit::open(&profile).map_err(String::from)?;
        edit.controls = Settings::from_head(&working).map_err(String::from)?;
        if !edit.controls_changed(&profile).map_err(String::from)? {
            return Err(format!("{name}: missing pending edit"));
        }
        drop(edit);
        if profile.payload != bytes {
            return Err(format!("{name}: profile changed"));
        }
        println!("{name}: pending edit decision EQUAL, profile unchanged");
    }
    Ok(())
}

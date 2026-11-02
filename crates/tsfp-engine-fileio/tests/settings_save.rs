//! Native settings persistence contract; synthetic data, no signature authentication claim.
use tsfp_engine_fileio::{save::{SettingsPage, load_profile, save_settings}, vfs::Vfs};
use tsfp_format_profile::{Profile, settings::SettingsEdit};

#[test]
fn settings_accept_reload_and_failed_save_are_transactional() {
    let dir = std::env::temp_dir().join(format!("tsfp-settings-save-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut vfs = Vfs::new();
    vfs.mount_host_dir("C:", &dir).unwrap();
    let mut profile = Profile { payload: vec![0; 0x1f54], signature: [19; 20] };
    profile.payload[..4].copy_from_slice(&0x12345678u32.to_le_bytes());
    profile.payload[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
    let mut edit = SettingsEdit::open(&profile).unwrap();
    edit.controls.set_turn_speed(1.0).unwrap();
    save_settings(&mut vfs, "C:\\profile", &mut profile, edit, SettingsPage::Controls).unwrap();
    let loaded = load_profile(&mut vfs, "C:\\profile").unwrap();
    assert_eq!(loaded.settings().unwrap().turn_speed(), 1.0);
    assert_eq!(&loaded.payload[..4], &[0; 4]);
    assert_eq!(&profile.payload[..4], &0x12345678u32.to_le_bytes());
    assert_eq!(&loaded.payload[4..], &profile.payload[4..]);
    assert_eq!(loaded.signature, profile.signature);
    let mut edit = SettingsEdit::open(&profile).unwrap();
    edit.audio_video.slider_4 = 64;
    save_settings(&mut vfs, "C:\\profile", &mut profile, edit, SettingsPage::AudioVideo).unwrap();
    assert_eq!(load_profile(&mut vfs, "C:\\profile").unwrap().audio_video().unwrap().slider_4, 64);
    let before = profile.payload.clone();
    let disk = std::fs::read(dir.join("profile")).unwrap();
    let mut edit = SettingsEdit::open(&profile).unwrap();
    edit.controls.set_layout(3);
    assert!(save_settings(&mut vfs, "missing:\\profile", &mut profile, edit, SettingsPage::Controls).is_err());
    assert_eq!(profile.payload, before);
    assert_eq!(std::fs::read(dir.join("profile")).unwrap(), disk);
    let mut edit = SettingsEdit::open(&profile).unwrap();
    edit.audio_video.slider_4 = 65;
    assert!(save_settings(&mut vfs, "C:\\profile", &mut profile, edit, SettingsPage::AudioVideo).is_err());
    assert_eq!(profile.payload, before);
    assert_eq!(std::fs::read(dir.join("profile")).unwrap(), disk);
    std::fs::remove_file(dir.join("profile")).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

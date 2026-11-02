//! Synthetic fixtures only: no game bytes. The fake `default.xbe` is plain text.
use std::path::{Path, PathBuf};
use tsfp_launcher_core::{config, identify, install, mods, packs};

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tsfp-launcher-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(p: &Path, b: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, b).unwrap();
}

fn fake_disc(root: &Path) -> PathBuf {
    let disc = root.join("disc");
    write(&disc.join("default.xbe"), b"not a real xbe");
    write(&disc.join("data/readme.txt"), b"hello");
    disc
}

#[test]
fn identify_reports_unknown_dump_with_hash() {
    let d = scratch("ident");
    let id = identify::identify(&fake_disc(&d)).unwrap();
    assert_eq!(id.layout, "directory");
    assert_eq!(id.file_count, 2);
    let h = id.xbe_sha256.clone().unwrap();
    assert_eq!(h, tsfp_import::sha256::hex(b"not a real xbe"));
    assert!(id.known.is_none());
    assert_eq!(id.version_id(), "xbox-unknown");
    assert!(id.verdict().contains("unknown dump"));
}

#[test]
fn identify_rejects_folder_without_xbe() {
    let d = scratch("noxbe");
    write(&d.join("disc/a.txt"), b"x");
    let id = identify::identify(&d.join("disc")).unwrap();
    assert_eq!(id.xbe_sha256, None);
    assert!(install::install(&d.join("disc"), &d.join("game"), true, &mut |_| {}).is_err());
}

#[test]
fn known_dump_lookup_is_case_insensitive() {
    let k = identify::KNOWN_DUMPS[0];
    assert_eq!(identify::lookup(&k.xbe_sha256.to_uppercase()), Some(k));
    assert_eq!(identify::lookup("00"), None);
}

#[test]
fn install_refuses_unknown_dump_unless_allowed() {
    let d = scratch("refuse");
    let err = install::install(&fake_disc(&d), &d.join("game"), false, &mut |_| {}).unwrap_err();
    assert!(err.contains("unknown dump"));
    assert_eq!(
        install::status(&d.join("game")),
        vec![install::Status::NotInstalled]
    );
}

#[test]
fn install_then_verify_then_detect_damage() {
    let d = scratch("install");
    let game = d.join("game");
    let mut steps = Vec::new();
    install::install(&fake_disc(&d), &game, true, &mut |s| steps.push(s.clone())).unwrap();
    assert_eq!(steps.len(), 4);
    assert_eq!(steps[1], install::Step::Extracted(2));
    assert!(matches!(steps[3], install::Step::Verified(n) if n > 0));
    let st = install::status(&game);
    assert!(matches!(&st[..], [install::Status::Installed { id, .. }] if id == "xbox-unknown"));
    let manifest = install::version_path(&game, "xbox-unknown").join("manifest.json");
    let mut text = std::fs::read(&manifest).unwrap();
    text.push(b' ');
    std::fs::write(&manifest, text).unwrap();
    assert!(matches!(
        &install::status(&game)[..],
        [install::Status::Damaged { .. }]
    ));
}

#[test]
fn config_round_trips_and_defaults() {
    let d = scratch("config");
    let path = d.join("cfg/launcher.json");
    let missing = config::Config::load(&path, d.join("g")).unwrap();
    assert_eq!(missing, config::Config::with_game_dir(d.join("g")));
    let mut c = missing.clone();
    c.channel = config::Channel::Nightly;
    c.enabled_mods = vec!["a".into(), "b".into()];
    c.save(&path).unwrap();
    assert_eq!(config::Config::load(&path, PathBuf::new()).unwrap(), c);
    assert!(config::Config::from_json(r#"{"channel":"beta"}"#, PathBuf::new()).is_err());
}

fn make_mod(mods: &Path, id: &str, files: &[&str], extra: &str) {
    write(
        &mods.join(id).join("mod.json"),
        format!(r#"{{"schema":1,"id":"{id}","version":"1.0"{extra}}}"#).as_bytes(),
    );
    for f in files {
        write(&mods.join(id).join("files").join(f), b"x");
    }
}

#[test]
fn mods_scan_validate_and_resolve_conflicts() {
    let d = scratch("mods");
    let m = d.join("mods");
    make_mod(&m, "alpha", &["textures/a.ktx2", "audio/b.flac"], "");
    make_mod(
        &m,
        "beta",
        &["textures/a.ktx2"],
        r#","affects_gameplay":true"#,
    );
    write(
        &m.join("Bad Id/mod.json"),
        br#"{"schema":1,"id":"Bad Id","version":"1"}"#,
    );
    write(
        &m.join("mismatch/mod.json"),
        br#"{"schema":1,"id":"other","version":"1"}"#,
    );
    let (ok, bad) = mods::scan(&m);
    assert_eq!(
        ok.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
        ["alpha", "beta"]
    );
    assert_eq!(ok[0].files, ["audio/b.flac", "textures/a.ktx2"]);
    assert!(ok[1].affects_gameplay && !ok[0].affects_gameplay);
    assert_eq!(bad.len(), 2);
    let order = vec!["beta".to_string(), "alpha".to_string()];
    let c = mods::resolve(&ok, &order).unwrap();
    assert_eq!(c.len(), 1);
    assert_eq!(
        (c[0].winner.as_str(), c[0].shadowed.as_slice()),
        ("alpha", &["beta".to_string()][..])
    );
    assert!(mods::resolve(&ok, &["gone".to_string()]).is_err());
}

#[test]
fn packs_are_placeholders_until_installed() {
    let d = scratch("packs");
    assert!(
        packs::CATALOG
            .iter()
            .all(|p| packs::state(&d, p) == packs::PackState::Unavailable)
    );
    std::fs::create_dir_all(packs::pack_dir(&d, "hq-music")).unwrap();
    assert_eq!(
        packs::state(&d, &packs::CATALOG[0]),
        packs::PackState::Installed
    );
}

/// Linux splits config (`~/.config`) from data (`~/.local/share`); macOS keeps both in Application Support.
#[cfg(target_os = "linux")]
#[test]
fn default_game_dir_is_not_under_config_dir() {
    let game = config::default_game_dir();
    assert!(game.ends_with(Path::new("tsfp").join("game")));
    if let Some(cfg) = config::default_config_dir() {
        assert!(
            !game.starts_with(&cfg),
            "{} under {}",
            game.display(),
            cfg.display()
        );
    }
}

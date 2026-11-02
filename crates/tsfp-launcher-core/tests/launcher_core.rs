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
    // Final events in order; progress events in between. A folder source is not copied.
    let finals: Vec<_> = steps
        .iter()
        .filter(|s| {
            !matches!(
                s,
                install::Step::Extracting { .. }
                    | install::Step::Converting { .. }
                    | install::Step::Verifying { .. }
            )
        })
        .cloned()
        .collect();
    assert_eq!(finals.len(), 5);
    assert_eq!(
        finals[1],
        install::Step::Checked {
            existing: false,
            ok: false,
            files: 0
        }
    );
    assert_eq!(finals[2], install::Step::Extracted(2));
    assert!(matches!(finals[4], install::Step::Verified(n) if n > 0));
    assert!(steps.contains(&install::Step::Extracting { done: 2, total: 2 }));
    assert!(
        !steps
            .iter()
            .any(|s| matches!(s, install::Step::Copied { .. }))
    );
    // Installing the same disc again verifies the existing install and skips extraction.
    let mut again = Vec::new();
    install::install(&fake_disc(&d), &game, true, &mut |s| again.push(s.clone())).unwrap();
    assert!(matches!(
        again.last(),
        Some(install::Step::Checked { existing: true, ok: true, files }) if *files > 0
    ));
    assert!(
        !again
            .iter()
            .any(|s| matches!(s, install::Step::Extracted(_)))
    );
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
    c.last_disc = Some("/discs/tsfp.iso".into());
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
    // A folder without a verified manifest is not an install.
    std::fs::create_dir_all(packs::pack_dir(&d, "hq-music")).unwrap();
    assert_eq!(
        packs::state(&d, &packs::CATALOG[0]),
        packs::PackState::Unavailable
    );
    write(&packs::pack_dir(&d, "hq-music").join("pack.json"), b"{}");
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

mod update_check {
    use tsfp_launcher_core::config::Channel;
    use tsfp_launcher_core::update::{Version, newest};

    fn rel(tag: &str, pre: bool, draft: bool, signed: bool) -> String {
        let sig = if signed {
            format!(r#",{{"name":"a.minisig","browser_download_url":"https://x/{tag}/a.minisig"}}"#)
        } else {
            String::new()
        };
        format!(
            r#"{{"tag_name":"{tag}","prerelease":{pre},"draft":{draft},"body":"n{tag}","assets":[{{"name":"a","browser_download_url":"https://x/{tag}/a"}}{sig}]}}"#
        )
    }

    #[test]
    fn versions_order_prereleases_first() {
        let v = |s| Version::parse(s).unwrap();
        assert!(v("v1.2.0-nightly.3") < v("1.2.0"));
        assert!(v("1.2.0") < v("1.10.0"));
        assert!(Version::parse("1.2").is_none() && Version::parse("1.2.3.4").is_none());
    }

    #[test]
    fn channel_signature_and_draft_rules() {
        let json = format!(
            "[{},{},{},{},{}]",
            rel("v0.3.0", false, true, true),
            rel("v0.2.1-nightly.1", true, false, true),
            rel("v0.2.0", false, false, true),
            rel("v0.2.5", false, false, false),
            rel("v0.0.9", false, false, true),
        );
        let stable = newest(&json, Channel::Stable, "0.1.0", "a")
            .unwrap()
            .unwrap();
        assert_eq!(
            (stable.version.as_str(), stable.notes.as_str()),
            ("v0.2.0", "nv0.2.0")
        );
        assert_eq!(stable.signature_url, "https://x/v0.2.0/a.minisig");
        let nightly = newest(&json, Channel::Nightly, "0.1.0", "a")
            .unwrap()
            .unwrap();
        assert_eq!(nightly.version, "v0.2.1-nightly.1");
        assert_eq!(newest(&json, Channel::Stable, "0.2.0", "a").unwrap(), None);
        assert!(newest("{}", Channel::Stable, "0.1.0", "a").is_err());
    }
}

#[test]
fn mod_install_from_wrapped_folder_replace_and_uninstall() {
    let d = scratch("modinstall");
    let src = d.join("download");
    make_mod(&src, "gamma", &["textures/x.ktx2"], r#","name":"Gamma""#);
    let mods_dir = d.join("game/mods");
    // Archive layout with one wrapping folder: download/gamma/mod.json.
    let m = mods::install_dir(&src, &mods_dir).unwrap();
    assert_eq!((m.id.as_str(), m.files.len()), ("gamma", 1));
    make_mod(&src, "gamma", &["textures/x.ktx2", "audio/y.flac"], "");
    assert_eq!(
        mods::install_dir(&src.join("gamma"), &mods_dir)
            .unwrap()
            .files
            .len(),
        2
    );
    let (ok, bad) = mods::scan(&mods_dir);
    assert_eq!((ok.len(), bad.len()), (1, 0));
    mods::uninstall(&mods_dir, "gamma").unwrap();
    assert!(mods::scan(&mods_dir).0.is_empty());
    assert!(mods::uninstall(&mods_dir, "../x").is_err());
}

#[test]
fn mod_install_rejects_bad_id_and_keeps_existing() {
    let d = scratch("modbad");
    let mods_dir = d.join("mods");
    make_mod(&d.join("src"), "keep", &["a/b"], "");
    mods::install_dir(&d.join("src/keep"), &mods_dir).unwrap();
    write(
        &d.join("bad/mod.json"),
        br#"{"schema":1,"id":"keep","version":"2"}"#,
    );
    write(
        &d.join("bad2/mod.json"),
        br#"{"schema":2,"id":"keep","version":"2"}"#,
    );
    // Missing files/ is allowed, wrong schema is not: the old install must survive.
    assert!(mods::install_dir(&d.join("bad2"), &mods_dir).is_err());
    assert_eq!(mods::scan(&mods_dir).0[0].version, "1.0");
    write(
        &d.join("evil/mod.json"),
        br#"{"schema":1,"id":"../evil","version":"1"}"#,
    );
    assert!(mods::install_dir(&d.join("evil"), &mods_dir).is_err());
}

#[test]
fn load_order_moves_and_active_json_lists_winners() {
    let mut order: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
    mods::move_in_order(&mut order, "c", -1);
    mods::move_in_order(&mut order, "a", -1);
    mods::move_in_order(&mut order, "b", 1);
    // c moved up; a at the top and b at the bottom cannot move further.
    assert_eq!(order, ["a", "c", "b"].map(String::from).to_vec());
    mods::move_in_order(&mut order, "a", 1);
    assert_eq!(order, ["c", "a", "b"].map(String::from).to_vec());

    let d = scratch("active");
    let m = d.join("mods");
    make_mod(&m, "base", &["t/a", "t/b"], "");
    make_mod(&m, "over", &["t/a"], r#","affects_gameplay":true"#);
    let (mods, _) = mods::scan(&m);
    let order = vec!["base".to_string(), "over".to_string()];
    mods::write_active(&m, &mods, &order).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(m.join("active.json")).unwrap()).unwrap();
    assert_eq!(v["files"]["t/a"], "over");
    assert_eq!(v["files"]["t/b"], "base");
    assert_eq!(v["affects_gameplay"], true);
    assert!(mods::write_active(&m, &mods, &["nope".to_string()]).is_err());
}

fn pack_manifest(files: &[(&str, &[u8])]) -> String {
    let rows: Vec<String> = files
        .iter()
        .map(|(p, b)| {
            format!(
                r#"{{"path":"{p}","url":"https://cdn.example/{p}","size":{},"sha256":"{}"}}"#,
                b.len(),
                tsfp_import::sha256::hex(b)
            )
        })
        .collect();
    format!(
        r#"{{"schema":1,"id":"hq-music","version":"1","files":[{}]}}"#,
        rows.join(",")
    )
}

#[test]
fn pack_manifest_rejects_unsafe_entries() {
    let ok = pack_manifest(&[("music/a.flac", b"aa")]);
    assert_eq!(packs::parse_manifest(&ok).unwrap().files.len(), 1);
    for (from, to) in [
        ("\"path\":\"music/a.flac", "\"path\":\"../a.flac"),
        ("\"path\":\"music/a.flac", "\"path\":\"/etc/a"),
        ("\"path\":\"music/a.flac", "\"path\":\"C:/a"),
        ("https://", "http://"),
        ("hq-music", "hq-other"),
    ] {
        assert!(
            packs::parse_manifest(&ok.replacen(from, to, 1)).is_err(),
            "{to}"
        );
    }
    let bad_hash = ok.replace(&tsfp_import::sha256::hex(b"aa"), "zz");
    assert!(packs::parse_manifest(&bad_hash).is_err());
}

#[test]
fn pack_install_verify_and_repair_only_changed_files() {
    let d = scratch("packinstall");
    let text = pack_manifest(&[("music/a.flac", b"aa"), ("music/b.flac", b"bbb")]);
    let m = packs::parse_manifest(&text).unwrap();
    assert_eq!(packs::needed(&d, &m).len(), 2);
    assert!(packs::store(&d, &m, &m.files[0], b"tampered").is_err());
    packs::store(&d, &m, &m.files[0], b"aa").unwrap();
    packs::store(&d, &m, &m.files[1], b"bbb").unwrap();
    packs::commit(&d, &m, &text).unwrap();
    assert!(packs::needed(&d, &m).is_empty());
    assert_eq!(packs::installed(&d, "hq-music"), Some(m.clone()));
    std::fs::write(packs::pack_dir(&d, "hq-music").join("music/b.flac"), b"bbX").unwrap();
    let repair: Vec<_> = packs::needed(&d, &m)
        .iter()
        .map(|f| f.path.clone())
        .collect();
    assert_eq!(repair, ["music/b.flac"]);
    packs::uninstall(&d, "hq-music").unwrap();
    assert_eq!(packs::installed(&d, "hq-music"), None);
    assert!(packs::uninstall(&d, "../x").is_err());
}

#[test]
fn disc_copy_is_verified_recorded_reused_and_repaired() {
    let d = scratch("disccopy");
    let image = d.join("My Game.ISO");
    let data: Vec<u8> = (0..20_000u32).map(|i| (i % 251) as u8).collect();
    write(&image, &data);
    let game = d.join("game");
    let mut last = (0, 0);
    let (copy, reused) =
        install::ensure_disc_copy(&image, &game, "xbox-usa", "abc", &mut |a, b| last = (a, b))
            .unwrap();
    assert!(!reused);
    assert_eq!(copy, install::disc_dir(&game).join("xbox-usa.iso"));
    assert_eq!(std::fs::read(&copy).unwrap(), data);
    assert_eq!(last, (2 * data.len() as u64, 2 * data.len() as u64));
    let record: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(install::disc_dir(&game).join("xbox-usa.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(record["sha256"], tsfp_import::sha256::hex(&data));
    assert_eq!(record["original_name"], "My Game.ISO");
    assert!(!install::disc_dir(&game).join("xbox-usa.part").exists());
    assert_eq!(install::disc_copy(&game, "xbox-usa"), Some(copy.clone()));
    assert_eq!(install::disc_copy(&game, "other"), None);

    let (_, reused) =
        install::ensure_disc_copy(&image, &game, "xbox-usa", "abc", &mut |_, _| {}).unwrap();
    assert!(reused);

    let mut bad = data.clone();
    bad[100] ^= 1;
    std::fs::write(&copy, &bad).unwrap();
    let (_, reused) =
        install::ensure_disc_copy(&image, &game, "xbox-usa", "abc", &mut |_, _| {}).unwrap();
    assert!(!reused);
    assert_eq!(std::fs::read(&copy).unwrap(), data);
}

#[test]
fn hash_file_streams_the_same_hash() {
    let d = scratch("hashfile");
    let data = vec![7u8; 9 << 20];
    write(&d.join("f"), &data);
    let mut calls = 0;
    let h = install::hash_file(&d.join("f"), &mut |_, _| calls += 1).unwrap();
    assert_eq!(h, tsfp_import::sha256::hex(&data));
    assert!(calls >= 2);
}

#[test]
fn damaged_install_is_re_extracted_not_skipped() {
    let d = scratch("repair");
    let game = d.join("game");
    install::install(&fake_disc(&d), &game, true, &mut |_| {}).unwrap();
    let extracted = install::version_path(&game, "xbox-unknown").join(".source/data/readme.txt");
    std::fs::write(&extracted, b"tampered").unwrap();
    let mut steps = Vec::new();
    install::install(&fake_disc(&d), &game, true, &mut |s| steps.push(s.clone())).unwrap();
    assert!(steps.contains(&install::Step::Checked {
        existing: true,
        ok: false,
        files: 0
    }));
    assert!(steps.contains(&install::Step::Extracted(2)));
    assert_eq!(std::fs::read(&extracted).unwrap(), b"hello");
}

#[test]
fn logo_is_read_from_the_installed_game_only() {
    use tsfp_launcher_core::branding;
    let d = scratch("logo");
    assert_eq!(branding::logo(&d), None);
    let version = install::version_path(&d, "xbox-usa");
    write(&version.join("manifest.json"), b"{}");
    // 256x128 BC1, every block opaque red.
    let block = [0x00u8, 0xF8, 0x00, 0xF8, 0, 0, 0, 0];
    let level: Vec<u8> = block.iter().copied().cycle().take(64 * 32 * 8).collect();
    let ktx = tsfp_import::ktx2::encode_bc(256, 128, 131, 128, 8, &[&level]).unwrap();
    write(
        &version.join("textures/pak/arcade/l_103/3edb3999.ktx2"),
        &ktx,
    );
    let logo = branding::logo(&d).unwrap();
    assert_eq!(
        (logo.width, logo.height, &logo.rgba[..4]),
        (256, 128, &[255u8, 0, 0, 255][..])
    );
    // A texture with the wrong shape is not taken for the logo.
    let square = tsfp_import::ktx2::encode_bc(4, 4, 131, 128, 8, &[&block]).unwrap();
    write(
        &version.join("textures/pak/arcade/l_103/3edb3999.ktx2"),
        &square,
    );
    assert_eq!(branding::logo(&d), None);
}

#[cfg(unix)]
mod game_runtime {
    use super::*;
    use tsfp_launcher_core::game::{self, GameState};

    fn fake_repo(name: &str, play_rc: i32) -> PathBuf {
        let d = scratch(name);
        write(&d.join("tools/dev/play.sh"), format!("echo \"$@ disc=$TSFP_DISC\" > args.txt\necho 'play.sh: last words' >&2\nexit {play_rc}\n").as_bytes());
        write(
            &d.join("tools/dev/build.sh"),
            b"echo building \"$@\"\necho done\n",
        );
        d
    }

    #[test]
    fn repo_is_found_from_a_nested_path_only() {
        let d = fake_repo("gamefind", 0);
        let nested = d.join("crates/tsfp-launcher/target/release/tsfp-launcher");
        assert_eq!(game::find_repo(&[nested]).map(|r| r.root), Some(d.clone()));
        assert_eq!(
            game::find_repo(&[PathBuf::from("/nonexistent-tsfp-launcher/x")]),
            None
        );
    }

    #[test]
    fn probe_maps_every_play_sh_exit_code() {
        for (rc, want) in [(0, GameState::Ready), (4, GameState::NoDisc)] {
            let repo = game::Repo {
                root: fake_repo(&format!("probe{rc}"), rc),
            };
            assert_eq!(game::probe(&repo, Some(Path::new("/d.iso"))), want);
        }
        let repo = game::Repo {
            root: fake_repo("probe10", 10),
        };
        assert!(
            matches!(game::probe(&repo, None), GameState::NeedsBuild { args, .. } if args == ["--relift"])
        );
        let repo = game::Repo {
            root: fake_repo("probe99", 99),
        };
        assert_eq!(
            game::probe(&repo, None),
            GameState::Error("play.sh: last words".into())
        );
        assert!(
            matches!(game::state_from_exit(Some(3), ""), GameState::NeedsBuild { args, .. } if args.is_empty())
        );
        assert!(
            matches!(game::state_from_exit(Some(7), ""), GameState::NeedsBuild { args, .. } if args == ["--reconfigure"])
        );
    }

    #[test]
    fn play_and_build_pass_disc_and_args_and_log_output() {
        let root = fake_repo("gamerun", 0);
        let repo = game::Repo { root: root.clone() };
        let log = root.join("logs/play.log");
        assert!(
            game::play(&repo, Some(Path::new("/x/disc.iso")), &log)
                .unwrap()
                .wait()
                .unwrap()
                .success()
        );
        assert_eq!(
            std::fs::read_to_string(root.join("args.txt"))
                .unwrap()
                .trim(),
            "default disc=/x/disc.iso"
        );
        assert_eq!(game::last_line(&log), "play.sh: last words");
        let blog = root.join("logs/build.log");
        let status = game::build(&repo, &["--relift".into()], &blog)
            .unwrap()
            .wait()
            .unwrap();
        assert!(status.success());
        assert!(
            std::fs::read_to_string(&blog)
                .unwrap()
                .contains("building --relift")
        );
        assert_eq!(game::last_line(&blog), "done");
    }

    #[test]
    fn stopping_a_build_ends_it_and_its_children() {
        let root = scratch("gamestop");
        write(&root.join("tools/dev/play.sh"), b"exit 0\n");
        write(&root.join("tools/dev/build.sh"), b"sleep 30 &\nsleep 30\n");
        let repo = game::Repo { root: root.clone() };
        let mut child = game::build(&repo, &[], &root.join("b.log")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        let started = std::time::Instant::now();
        game::stop(child.id());
        let status = child.wait().unwrap();
        assert!(!status.success());
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }
}

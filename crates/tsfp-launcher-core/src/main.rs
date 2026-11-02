//! Headless launcher: the same steps as the GUI setup wizard, for scripts and for testing.
use std::path::PathBuf;
use tsfp_launcher_core::{Result, config, identify, install, mods, packs};

const USAGE: &str = "usage:
  tsfp-launcher-cli identify --disc PATH
  tsfp-launcher-cli install  --disc PATH [--game-dir DIR] [--allow-unknown]
  tsfp-launcher-cli status   [--game-dir DIR]
  tsfp-launcher-cli mods     [--game-dir DIR]
  tsfp-launcher-cli packs    [--game-dir DIR]
--game-dir defaults to the launcher config's game_dir, else the per-user data folder.";

fn arg(a: &[String], k: &str) -> Option<String> {
    a.iter()
        .position(|x| x == k)
        .and_then(|i| a.get(i + 1))
        .cloned()
}

fn game_dir(a: &[String]) -> Result<PathBuf> {
    if let Some(d) = arg(a, "--game-dir") {
        return Ok(PathBuf::from(d));
    }
    let fallback = config::default_game_dir();
    match config::default_config_dir() {
        Some(dir) => Ok(config::Config::load(&dir.join("launcher.json"), fallback)?.game_dir),
        None => Ok(fallback),
    }
}

fn run() -> Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let disc = || arg(&a, "--disc").map(PathBuf::from).ok_or(USAGE);
    match a.first().map(String::as_str).ok_or(USAGE)? {
        "identify" => {
            let id = identify::identify(&disc()?)?;
            println!(
                "{} ({} files, layout {})",
                id.verdict(),
                id.file_count,
                id.layout
            );
        }
        "install" => {
            let dir = game_dir(&a)?;
            install::install(
                &disc()?,
                &dir,
                a.iter().any(|x| x == "--allow-unknown"),
                &mut |s| println!("{s:?}"),
            )?;
            println!("installed into {}", dir.display());
        }
        "status" => {
            for s in install::status(&game_dir(&a)?) {
                println!("{s:?}");
            }
        }
        "mods" => {
            let (ok, bad) = mods::scan(&game_dir(&a)?.join("mods"));
            for m in ok {
                let tag = if m.affects_gameplay {
                    " [non-faithful]"
                } else {
                    ""
                };
                println!("{} {} ({} files){tag}", m.id, m.version, m.files.len());
            }
            for e in bad {
                println!("broken: {e}");
            }
        }
        "packs" => {
            let dir = game_dir(&a)?;
            for p in packs::CATALOG {
                println!("{} {:?}: {}", p.id, packs::state(&dir, p), p.title);
            }
        }
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

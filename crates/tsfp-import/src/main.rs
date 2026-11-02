//! `tsfp-import list|extract|convert|verify`. Use only on your own copy of the game.
use std::path::PathBuf;
use tsfp_import::{Result, convert, source};

const USAGE: &str = "usage:
  tsfp-import list    --source PATH --platform xbox|ps2|gamecube [--id ID]
  tsfp-import extract --source PATH --platform P --id VERSION --out GAME_DIR [--only SUBSTR]      (stage A)
  tsfp-import convert --source PATH --platform P --id VERSION --out GAME_DIR [--only SUBSTR] [--names FILE]
  tsfp-import verify  --out GAME_DIR --id VERSION";

fn arg(a: &[String], k: &str) -> Option<String> {
    a.iter()
        .position(|x| x == k)
        .and_then(|i| a.get(i + 1))
        .cloned()
}

fn run() -> Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let cmd = a.first().ok_or(USAGE)?.as_str();
    let id = arg(&a, "--id").unwrap_or_else(|| "default".into());
    let out = arg(&a, "--out").map(PathBuf::from);
    let only = arg(&a, "--only").unwrap_or_default();
    if cmd == "verify" {
        let n = convert::verify(&out.ok_or(USAGE)?, &id)?;
        println!("verified {n} files");
        return Ok(());
    }
    let platform = source::Platform::parse(&arg(&a, "--platform").ok_or(USAGE)?)?;
    let src = source::open(
        &PathBuf::from(arg(&a, "--source").ok_or(USAGE)?),
        &id,
        platform,
    )?;
    match cmd {
        "list" => {
            let files = src.list()?;
            for f in &files {
                println!("{:>12}  {}", f.size, f.path);
            }
            println!("{} files, layout {}", files.len(), src.descriptor().layout);
        }
        "extract" => {
            let out = out.ok_or(USAGE)?;
            let files = convert::stage_a(src.as_ref(), &out, &only)?;
            println!("extracted {} files", files.len());
        }
        "convert" => {
            let out = out.ok_or(USAGE)?;
            let names = match arg(&a, "--names") {
                Some(p) => std::fs::read_to_string(&p)
                    .map_err(|e| format!("{p}: {e}"))?
                    .lines()
                    .map(str::to_string)
                    .filter(|l| !l.is_empty())
                    .collect(),
                None => Vec::new(),
            };
            convert::stage_a(src.as_ref(), &out, &only)?;
            let (outs, rep) = convert::stage_b_report(src.as_ref(), &out, &only, &names)?;
            convert::write_override_contract(&out)?;
            println!(
                "converted {} outputs ({} lossless); {} files converted, {} reused, {} members named",
                outs.len(),
                outs.iter().filter(|o| o.lossless).count(),
                rep.converted_files,
                rep.reused_files,
                rep.named_members
            );
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

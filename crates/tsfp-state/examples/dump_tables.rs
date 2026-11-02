// SPDX-License-Identifier: GPL-3.0-or-later
//! Writes every immutable table body as `<out>/<name>.bin` plus a one line summary, for the trace
//! oracle (tools.s3_tables compare-trace) and diagnostics. Usage: dump_tables <default.xbe> <out-dir>
use std::{env, fs, process::ExitCode};
use tsfp_state::tables::OwnedTables;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: dump_tables <default.xbe> <out-dir>");
        return ExitCode::from(2);
    }
    let tables = match load(&args[1]) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("load failed: {e:?}");
            return ExitCode::from(1);
        }
    };
    fs::create_dir_all(&args[2]).expect("create out dir");
    for spec in tables.as_tables().specs() {
        let mut body = tables.body(spec.name).expect("body").to_vec();
        // Mutation check for the oracle: TSFP_TABLES_MUTATE=<name>:<offset> flips one byte.
        if let Ok(m) = env::var("TSFP_TABLES_MUTATE") {
            if let Some((name, at)) = m.split_once(':') {
                if name == spec.name {
                    body[at.parse::<usize>().expect("offset")] ^= 0xff;
                }
            }
        }
        fs::write(format!("{}/{}.bin", args[2], spec.name), &body).expect("write");
        println!("{} {:#x} {:#x} ok", spec.name, spec.va, spec.size);
    }
    ExitCode::SUCCESS
}

fn load(path: &str) -> Result<OwnedTables, String> {
    if path.ends_with(".iso") {
        let disc = tsfp_engine_fileio::xiso::Xiso::open(std::path::Path::new(path))
            .map_err(|e| format!("{e:?}"))?;
        let entry = disc.lookup("default.xbe").map_err(|e| format!("{e:?}"))?;
        let mut xbe = vec![0; entry.size as usize];
        disc.read(&entry, 0, &mut xbe).map_err(|e| format!("{e:?}"))?;
        return OwnedTables::from_xbe(&xbe).map_err(|e| format!("{e:?}"));
    }
    let xbe = fs::read(path).map_err(|e| e.to_string())?;
    OwnedTables::from_xbe(&xbe).map_err(|e| format!("{e:?}"))
}

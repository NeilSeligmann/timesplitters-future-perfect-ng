// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline canonical files only; never launches a replay or GPU.
use std::{fs::File, io::BufReader, process::ExitCode};
use tsfp_scene_compare::{Failure, Limits, Side, compare, wire::Reader};

fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: scene-compare ORACLE.drawlist NATIVE.drawlist".into());
    }
    let mut oracle = Reader::new(BufReader::new(File::open(&args[0])?))?;
    let mut native = Reader::new(BufReader::new(File::open(&args[1])?))?;
    let a = oracle.header().clone();
    let b = native.header().clone();
    if a.side != Side::Oracle || b.side != Side::Native {
        return Err("capture side mismatch".into());
    }
    match compare(
        &a.capture,
        &b.capture,
        b.producer_revision,
        &mut oracle,
        &mut native,
        Limits::default(),
    ) {
        Ok(result) => {
            println!("EQUAL {result:?}");
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            let difference = matches!(error.failure, Failure::Different { .. });
            println!(
                "{} {error:?}",
                if difference {
                    "DIFFERENT"
                } else {
                    "INCOMPLETE"
                }
            );
            Ok(ExitCode::from(if difference { 1 } else { 2 }))
        }
    }
}
fn main() -> ExitCode {
    match run() {
        Ok(status) => status,
        Err(error) => {
            eprintln!("INCOMPLETE {error}");
            ExitCode::from(2)
        }
    }
}

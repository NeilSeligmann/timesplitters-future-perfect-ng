// SPDX-License-Identifier: GPL-3.0-or-later
use std::{env, fs::File, process::ExitCode};
fn main() -> ExitCode {
    let mut args = env::args_os();
    let _ = args.next();
    let Some(first) = args.next() else {
        eprintln!("usage: tsfp-wsola-capture [--audit-prefix|--native-abi] CAPTURE");
        return ExitCode::FAILURE;
    };
    let audit = first == "--audit-prefix";
    let abi = first == "--native-abi";
    let path = if audit || abi {
        match args.next() {
            Some(p) => p,
            None => {
                eprintln!("missing capture");
                return ExitCode::FAILURE;
            }
        }
    } else {
        first
    };
    if args.next().is_some() {
        eprintln!("expected one capture");
        return ExitCode::FAILURE;
    }
    match File::open(path)
        .map_err(|_| "cannot open capture".to_string())
        .and_then(|f| {
            if audit {
                tsfp_wsola_capture::audit_prefix(f)
            } else if abi {
                tsfp_wsola_capture::compare_native_abi(f)
            } else {
                tsfp_wsola_capture::compare(f)
            }
        }) {
        Ok(report) => {
            if audit {
                println!(
                    "INCOMPLETE diagnostic prefix: {} calls compared, {} primes; no conformance verdict",
                    report.calls, report.primes
                );
                ExitCode::FAILURE
            } else {
                println!(
                    "EQUAL: {} calls, {} primes (captured engine only)",
                    report.calls, report.primes
                );
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("INCOMPLETE or DIVERGED: {e}");
            ExitCode::FAILURE
        }
    }
}

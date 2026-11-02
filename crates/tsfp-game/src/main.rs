// SPDX-License-Identifier: GPL-3.0-or-later
//! T2001 phase 1 of the Rust-hosted executable: the process `main` is Rust.
//!
//! The window, input, audio, renderer, kernel layer and the lifted code are the existing C host,
//! linked in by build.rs. Every argument is passed through to the host unchanged (`--rust-ports=...`
//! included), so behaviour is the C-main hybrid host's. Rust owns only the entry and a small CLI.
use std::ffi::{CString, c_char, c_int};
use std::process::ExitCode;

// T2003: SDL-free window/input logic (unit tested) and, with -DTSFP_RUST_WINDOW=ON, its SDL3 C-callable face.
#[cfg_attr(not(tsfp_rust_window), allow(dead_code))]
mod window_input;
#[cfg(tsfp_rust_window)]
mod sdl_window;
mod ui_native;
mod ui_switch;

#[cfg(not(tsfp_no_host))]
unsafe extern "C" {
    /// src/host/rust_main.c: the C host's former `main`, renamed.
    fn tsfp_host_entry(argc: c_int, argv: *mut *mut c_char) -> c_int;
}

/// Built without the C host (no TSFP_GAME_HOST_BUILD_DIR / TSFP_LIFTED_DIR): `cargo check` and tests only.
#[cfg(tsfp_no_host)]
unsafe fn tsfp_host_entry(_argc: c_int, _argv: *mut *mut c_char) -> c_int {
    eprintln!("tsfp-game was built without the C host, see docs/t-rust-main.md");
    2
}

const INFO: &str = concat!(
    "tsfp-game ", env!("CARGO_PKG_VERSION"), " (T2001, Rust main over the C host)\n",
    "host build dir: ", env!("TSFP_GAME_HOST_BUILD_DIR"), "\n",
    "build stamp: git ", env!("TSFP_BUILD_GIT_SHA"), " ports.a sha256 ", env!("TSFP_BUILD_PORTS_ARCHIVE"),
    " ports-src sha256 ", env!("TSFP_BUILD_PORTS_SRC"), "\n",
);

fn main() -> ExitCode {
    #[cfg(tsfp_rust_window)]
    sdl_window::keep_exports();
    let mut arguments: Vec<String> = std::env::args().collect();
    let program = arguments.remove(0);
    let ui = std::env::var("TSFP_UI").ok();
    let (ui, arguments) = match ui_switch::take(ui.as_deref(), arguments) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{}", error.0);
            return ExitCode::from(2);
        }
    };
    let ui_line = ui_switch::describe(&ui);
    ui_switch::install(ui);
    match arguments.first().map(String::as_str) {
        Some("--rust-main-info") => {
            print!("{INFO}");
            println!("{ui_line}");
            println!("window/input: {}", if cfg!(tsfp_rust_window) { "Rust (T2003, sdl_window.rs)" } else { "C (present_sink.c)" });
            return ExitCode::SUCCESS;
        }
        Some("--rust-main-help") => {
            println!(
                "usage: {program} [XBE] [host options...]\n\
                 Every option is the C host's (see its --help), including --rust-ports=off|all|HEX,...\n\
                 Extra here: --rust-main-info, --rust-main-help (first argument only),\n\
                 --ui=rust|legacy and --ui-legacy-page=ADDR (T2189, also TSFP_UI, default legacy)."
            );
            return ExitCode::SUCCESS;
        }
        _ => {}
    }
    let mut owned: Vec<CString> = Vec::with_capacity(arguments.len() + 1);
    owned.push(CString::new(program).expect("program name has no NUL"));
    for argument in arguments {
        match CString::new(argument) {
            Ok(value) => owned.push(value),
            Err(_) => {
                eprintln!("argument contains a NUL byte");
                return ExitCode::from(2);
            }
        }
    }
    let mut pointers: Vec<*mut c_char> = owned.iter().map(|value| value.as_ptr().cast_mut()).collect();
    pointers.push(std::ptr::null_mut());
    // SAFETY: argv is NUL-terminated, `owned` outlives the call, the host does not retain it after return.
    let status = unsafe { tsfp_host_entry(owned.len() as c_int, pointers.as_mut_ptr()) };
    ExitCode::from(status as u8)
}

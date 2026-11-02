// SPDX-License-Identifier: GPL-3.0-or-later
//! T3796: constructed-call Tier 2 reference runs (AGENTS.md rules 9(b) and 43, docs/t3796-constructed-call-tier2.md).
//!
//! `tsfp-bevy --t2-construct <inputs.bin> --va 0x<VA> --out <results.jsonl> [--input-ms 2000]`
//!
//! `<inputs.bin>` is a T3741 spool (`tools/t2corpus/spool.py`) written by `python3 -m tools.t2harness`: each record
//! carries only FABRICATED inputs (entry registers, implicit state, the memory runs of the constructed state). For
//! every record the host prepares the `tsfp-t2-offline` fault-tracked arena with those runs and calls the ORIGINAL
//! lifted function with no call hook, so its callees run for real. In a capture build (`TSFP_BUILD_CAPTURE=1`) with
//! `TSFP_T2_CAPTURE=<spool>` set, the capture runtime (`src/host/t2_capture.c`) records the call exactly as it records
//! a call in a replay: read-set, write-set, callees, exit state. That record is the reference entry the unchanged
//! `tools.t2capture` runner later judges the port against.
//!
//! Results, one JSON line per event: `{"index": i, "event": "start"}` before a run (a host that dies or hangs names
//! its input), then `{"index": i, "event": "end", "end": "returned"|"fault"|"panic", ...}`; an original still running
//! after `--input-ms` is stopped by a SIGALRM watchdog and reported as a fault with signal 14. A fault ends the process
//! with exit code 4 after its line, so the next input starts in a fresh process with clean capture state (the
//! driver resumes with the remaining inputs). Exit codes: 0 every input ran, 2 harness error, 4 stopped on a fault.
use std::io::Write;
use std::process::ExitCode;

use tsfp_port_runtime::abi::ExtRegisters;
use tsfp_t2_offline::arena::{Arena, GUEST_WINDOW, RunEnd};
use tsfp_t2_offline::host::{HOST_API_VERSION, HostApi, host_api};
use tsfp_t2_offline::spool::{self, Implicit};

pub const EXIT_FAULT: u8 = 4;

/// The code under test: the lifted original through the host API, or a test double.
pub trait Original {
    fn bind(&mut self, base: usize);
    fn set_implicit(&mut self, implicit: &Implicit);
    /// Run the original of `va` with its callees for real. False when the VA has no lifted function.
    fn run(&mut self, va: u32, regs: &mut ExtRegisters) -> bool;
}

pub struct HostOriginal {
    api: &'static HostApi,
}

impl Original for HostOriginal {
    fn bind(&mut self, base: usize) {
        unsafe { *self.api.mem_offset = base as isize };
    }
    fn set_implicit(&mut self, implicit: &Implicit) {
        unsafe { (self.api.set_implicit)(implicit.as_ptr()) };
    }
    fn run(&mut self, va: u32, regs: &mut ExtRegisters) -> bool {
        // No hook: callees, tail calls and kernel thunks run for real, as in the game.
        unsafe {
            *self.api.call_hook = None;
            *self.api.lifted_call_hook = None;
            (self.api.run_lifted)(va, regs) != 0
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    pub input: String,
    pub va: u32,
    pub out: String,
    /// Per-input watchdog: an original still running after this many milliseconds ends as a SIGALRM fault.
    pub input_ms: u64,
}

pub const DEFAULT_INPUT_MS: u64 = 2000;

pub fn parse_args(args: &[String]) -> Result<Args, String> {
    let (mut input, mut va, mut out) = (None, None, None);
    let mut input_ms = DEFAULT_INPUT_MS;
    let mut at = 0;
    while at < args.len() {
        let value = args
            .get(at + 1)
            .cloned()
            .ok_or(format!("{} needs a value", args[at]));
        match args[at].as_str() {
            "--t2-construct" => input = Some(value?),
            "--out" => out = Some(value?),
            "--va" => {
                let text = value?;
                let digits = text.trim_start_matches("0x").trim_start_matches("0X");
                va = Some(u32::from_str_radix(digits, 16).map_err(|_| format!("bad --va {text}"))?);
            }
            "--input-ms" => {
                let text = value?;
                input_ms = text.parse().map_err(|_| format!("bad --input-ms {text}"))?;
            }
            other => return Err(format!("unknown argument {other}")),
        }
        at += 2;
    }
    Ok(Args {
        input: input.ok_or("--t2-construct needs <inputs.bin>")?,
        va: va.ok_or("--va is required")?,
        out: out.ok_or("--out is required")?,
        input_ms,
    })
}

/// How a constructed run ended, as one result line.
pub fn end_line(index: usize, end: &RunEnd, extra_reads: u32, regs: &ExtRegisters) -> String {
    match end {
        RunEnd::Returned => format!(
            "{{\"index\": {index}, \"event\": \"end\", \"end\": \"returned\", \"extra_reads\": {extra_reads}, \"exit_esp\": \"{:#010x}\"}}",
            regs.esp
        ),
        RunEnd::Fault { signal, address } => format!(
            "{{\"index\": {index}, \"event\": \"end\", \"end\": \"fault\", \"signal\": {signal}, \"address\": \"{address:#x}\", \"extra_reads\": {extra_reads}}}"
        ),
        RunEnd::Panic(text) => format!(
            "{{\"index\": {index}, \"event\": \"end\", \"end\": \"panic\", \"detail\": {:?}, \"extra_reads\": {extra_reads}}}",
            text.replace(['"', '\\'], "'")
        ),
    }
}

unsafe extern "C" {
    /// `crates/tsfp-t2-offline/c/t2o_glue.c`: leave the armed guarded run (returns when none is armed).
    fn t2o_escape(signal: i32, address: usize);
}

extern "C" fn on_alarm(signal: libc::c_int) {
    // Delivered to the thread that armed the timer (SIGEV_THREAD_ID); a late one finds no guard and returns.
    unsafe { t2o_escape(signal, 0) };
}

/// A one-shot timer that sends SIGALRM to the creating thread only (tests run several runners at once).
struct Watchdog {
    timer: libc::timer_t,
}

impl Watchdog {
    fn new() -> Result<Self, String> {
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = on_alarm as *const () as usize;
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(libc::SIGALRM, &action, std::ptr::null_mut());
            let mut event: libc::sigevent = std::mem::zeroed();
            event.sigev_notify = libc::SIGEV_THREAD_ID;
            event.sigev_signo = libc::SIGALRM;
            event.sigev_notify_thread_id = libc::gettid();
            let mut timer: libc::timer_t = std::ptr::null_mut();
            if libc::timer_create(libc::CLOCK_MONOTONIC, &mut event, &mut timer) != 0 {
                return Err(format!("timer_create: {}", std::io::Error::last_os_error()));
            }
            Ok(Self { timer })
        }
    }

    /// Arm (ms > 0) or disarm (0).
    fn set(&self, ms: u64) {
        let value = libc::itimerspec {
            it_interval: libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            },
            it_value: libc::timespec {
                tv_sec: (ms / 1000) as libc::time_t,
                tv_nsec: ((ms % 1000) * 1_000_000) as libc::c_long,
            },
        };
        unsafe { libc::timer_settime(self.timer, 0, &value, std::ptr::null_mut()) };
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        unsafe { libc::timer_delete(self.timer) };
    }
}

/// Run every input record of `data` through `original`, each under an `input_ms` watchdog (0: none).
/// Ok(true) when it stopped on a fault.
pub fn run_inputs(
    data: &[u8],
    va: u32,
    input_ms: u64,
    arena: &mut Arena,
    original: &mut dyn Original,
    out: &mut dyn Write,
) -> Result<bool, String> {
    let parsed = spool::parse(data).map_err(|e| e.to_string())?;
    original.bind(arena.base());
    let watchdog = if input_ms > 0 {
        Some(Watchdog::new()?)
    } else {
        None
    };
    let arm = |ms: u64| {
        if let Some(watchdog) = &watchdog {
            watchdog.set(ms);
        }
    };
    for (index, record) in parsed.records.into_iter().enumerate() {
        let entry = record.map_err(|e| format!("input {index}: {e}"))?;
        if entry.va != va {
            return Err(format!("input {index} is for {:#x}, not {va:#x}", entry.va));
        }
        writeln!(out, "{{\"index\": {index}, \"event\": \"start\"}}").map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
        arena.prepare(&entry.read_runs)?;
        original.set_implicit(&entry.entry_implicit.unwrap_or_default());
        let mut regs = entry.entry_regs;
        let mut known = true;
        arm(input_ms);
        let end = arena.run(&mut || known = original.run(va, &mut regs));
        arm(0);
        if !known {
            return Err(format!("no lifted function for {va:#x}"));
        }
        writeln!(out, "{}", end_line(index, &end, arena.extra_reads(), &regs))
            .map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
        if end != RunEnd::Returned {
            return Ok(true);
        }
    }
    Ok(false)
}

fn run(args: &Args, original: &mut dyn Original) -> Result<bool, String> {
    let data = std::fs::read(&args.input).map_err(|e| format!("{}: {e}", args.input))?;
    let mut arena = Arena::reserve(GUEST_WINDOW)?;
    let file = std::fs::File::create(&args.out).map_err(|e| format!("{}: {e}", args.out))?;
    let mut out = std::io::BufWriter::new(file);
    run_inputs(
        &data,
        args.va,
        args.input_ms,
        &mut arena,
        original,
        &mut out,
    )
}

/// `tsfp-bevy --t2-construct ...` (`args` starts at `--t2-construct`).
pub fn main(args: &[String]) -> ExitCode {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(problem) => {
            eprintln!("t2-construct: {problem}");
            return ExitCode::from(2);
        }
    };
    let Some(api) = host_api() else {
        eprintln!("t2-construct: this binary does not link the C host (lifted C, dispatch)");
        return ExitCode::from(2);
    };
    if api.version != HOST_API_VERSION || unsafe { (api.capture_build)() } == 0 {
        // Only the capture runtime records the reference entry; any other host would run inputs and record nothing.
        eprintln!(
            "t2-construct: needs a capture build host (TSFP_BUILD_CAPTURE=1, host API v{HOST_API_VERSION})"
        );
        return ExitCode::from(2);
    }
    // Fabricated inputs kill the host now and then (native stack overflow in a recursive original): never hand
    // those deaths to the system core-dump handler (a pipe core_pattern ignores RLIMIT_CORE 0).
    unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0) };
    if std::env::var_os("TSFP_T2_CAPTURE").is_none() {
        eprintln!("t2-construct: TSFP_T2_CAPTURE=<spool> is not set, nothing would be recorded");
        return ExitCode::from(2);
    }
    match run(&parsed, &mut HostOriginal { api }) {
        Ok(false) => ExitCode::SUCCESS,
        Ok(true) => ExitCode::from(EXIT_FAULT),
        Err(problem) => {
            eprintln!("t2-construct: {problem}");
            ExitCode::from(2)
        }
    }
}

/// The host's offline modes (`--t2-offline`, T3742; `--t2-construct`, T3796); `args` starts at the mode flag.
pub fn dispatch(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("--t2-construct") => main(args),
        _ => tsfp_t2_offline::host::main(args),
    }
}

#[cfg(test)]
mod tests;

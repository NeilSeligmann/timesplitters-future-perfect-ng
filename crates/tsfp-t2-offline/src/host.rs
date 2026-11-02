// SPDX-License-Identifier: GPL-3.0-or-later
//! `tsfp-bevy --t2-offline <entries.bin> --va 0x<VA> --out <results.jsonl> [--lifted-only|--rust-only]
//! [--from <index>] [--ret-dead eax,edx] [--fpo-leaf] [--inlined-callees <vas>] [--callee-arity <file>]` (`--from`, T3740: start at that record, lines keep their
//! absolute index; `--ret-dead`, compare v3: port-side return registers no caller reads).
//! (interface agreed with T3743, claude-1 2026-10-25). `<entries.bin>` is the output of
//! `python3 -m tools.t2corpus export --va 0x<VA> --out <file>`.
//!
//! Exit codes: 0 when every entry was judged (diverged entries included), 2 on a harness error
//! (bad arguments, unreadable input, a binary without the C host), 3 when the VA has no Rust
//! dispatch entry (every line then says `"rust": "no_abi_entry"`; the lifted control still runs).
//!
//! The host side is `tsfp_t2o_host_api` in `src/host/rust_ports.c`: the lifted original by VA
//! (`tsfp_original_<va>`, else `recomp_lookup`), the port through its dispatch entry
//! (`tsfp_rust_try`, `tsfp_rust_try_lane`, `tsfp_rust_try_weapon`, `tsfp_rust_try_ext`) with
//! only that VA selected, and a nested-call hook that `tsfp_lane_guest_call`,
//! `tsfp_lane_x_call` and the ext trampoline consult first. Version 2 (T3740 fidelity): the
//! implicit thread state (`g_seh_ebp`, DF, ...) is loaded before both sides and read back after,
//! and every nested `RECOMP_ABI_CALL` of the lifted control (lifted callees, kernel imports, HLE
//! thunks) reaches the stub through `recomp_direct_call_intercept`, so both sides are stubbed.
//! Direct tail calls (`sub_X(); return;`) still run their target for real.
use std::cell::Cell;
use std::ffi::{CStr, c_char, c_void};
use std::process::ExitCode;
use std::ptr::null_mut;

use tsfp_port_runtime::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};

use crate::arena::{Arena, GUEST_WINDOW};
use crate::compare::RetDead;
use crate::compare::Side;
use crate::runner::{Executor, Options, run_spool};
use crate::spool::Implicit;
use crate::stub::{CalleeArity, CalleeStub, Convention, Inlined};

type CallHook = unsafe extern "C" fn(u32, *mut ExtRegisters, i32);
type LiftedCallHook = unsafe extern "C" fn(u32, *mut ExtRegisters, *mut u32);

unsafe extern "C" {
    /// T2238 offline-only scope record. The C trace writer is inert unless TSFP_RENDER_TRACE is
    /// open, so these markers never alter live renderer routing or output.
    fn tsfp_render_trace_enabled() -> i32;
    fn tsfp_render_trace_line(text: *const c_char, length: usize);
}

fn render_scope_marker(va: u32, index: usize, side: Side, begin: bool) -> String {
    let side = match side {
        Side::Lifted => "lifted",
        Side::Port => "rust",
    };
    let phase = if begin { "begin" } else { "end" };
    format!("scope {va:08x} {side} {index} {phase}")
}

// This memory is the offline runner's fault-tracked guest window. An uncaptured
// access faults in the arena rather than manufacturing an input value.
struct CaptureMemory(usize);
impl GuestMemory for CaptureMemory {
    fn read8(&self, address: u32) -> u8 {
        unsafe { ((self.0.wrapping_add(address as usize)) as *const u8).read_volatile() }
    }
    fn write8(&mut self, address: u32, value: u8) {
        unsafe { ((self.0.wrapping_add(address as usize)) as *mut u8).write_volatile(value) }
    }
}

struct CaptureGuest;
impl GuestCall for CaptureGuest {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        unsafe { nested_call(va, regs, 0) }
    }
}

/// C host lookup for capture-only roots. These entries never enter a live selector.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_t2_lane_has_root(va: u32) -> i32 {
    i32::from(tsfp_t2_lanes::cleanup(va).is_some())
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_t2_lane_capture_preferred(va: u32) -> i32 {
    i32::from(tsfp_t2_lanes::capture_preferred(va))
}

/// # Safety
/// `regs` is a valid register record and `offset` is the active fault-tracked guest window.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_t2_lane_run(va: u32, regs: *mut ExtRegisters, offset: usize) -> i32 {
    let Some(cleanup) = tsfp_t2_lanes::cleanup(va) else {
        return 0;
    };
    let regs = unsafe { &mut *regs };
    let entry = regs.esp;
    if !tsfp_t2_lanes::run(va, &mut CaptureMemory(offset), &mut CaptureGuest, regs) {
        return 0;
    }
    regs.esp = entry.wrapping_add(4).wrapping_add(cleanup);
    1
}

/// Mirrors `tsfp_t2o_host_api` in src/host/rust_ports.c (version 5).
#[repr(C)]
pub struct HostApi {
    pub version: u32,
    pub mem_offset: *mut isize,
    /// 0 none, 1 four-register root, 2 lane root, 3 ext root, 4 weapon lane root.
    pub port_kind: unsafe extern "C" fn(u32) -> i32,
    /// 0 when the VA has no lifted function.
    pub run_lifted: unsafe extern "C" fn(u32, *mut ExtRegisters) -> i32,
    /// 0 when the dispatch declined.
    pub run_port: unsafe extern "C" fn(u32, *mut ExtRegisters) -> i32,
    pub call_hook: *mut Option<CallHook>,
    /// Version 2: implicit state for the next run (8 words, `spool::IMPLICIT_NAMES`).
    pub set_implicit: unsafe extern "C" fn(*const u32),
    /// Version 2: the implicit state now.
    pub get_implicit: unsafe extern "C" fn(*mut u32),
    /// Version 2: nested calls of the lifted control while set.
    pub lifted_call_hook: *mut Option<LiftedCallHook>,
    /// Version 3 (T3765): lane-guard events so far; None without the c2r lane crate.
    pub guard_events: Option<unsafe extern "C" fn() -> u64>,
    /// Version 4 (T3767): 1 in a capture build (`TSFP_BUILD_CAPTURE=1`), the only build whose
    /// lifted control replays every nested call from the capture.
    pub capture_build: unsafe extern "C" fn() -> i32,
    /// Version 5 (T3738 compare v8): the `index`-th default-off A/B switch gating a VA (env name,
    /// enabling value; `tsfp_ports_opt_in`, the modules' own `OPT_IN` declarations), 0 when none.
    pub opt_in: OptInLookup,
}

type OptInLookup = unsafe extern "C" fn(u32, u32, *mut *const c_char, *mut *const c_char) -> i32;

pub const HOST_API_VERSION: u32 = 5;

/// v8: the A/B switches that gate `va` (`lookup` = `HostApi::opt_in`), as (env, value).
pub fn opt_ins(va: u32, lookup: OptInLookup) -> Vec<(String, String)> {
    let mut out = Vec::new();
    // Memory ABI roots are selected by TSFP_REA_MEMORY_NATIVE inside tsfp-memory-abi, rather
    // than through the tsfp-ports root OPT_IN list. The offline checker owns this opt-in so the
    // selected native memory route is compared instead of being misreported as unavailable.
    if matches!(va, 0x3df00 | 0x3e420 | 0x3e430) {
        out.push(("TSFP_REA_MEMORY_NATIVE".into(), "1".into()));
    }
    // `memory_atexit` is selected by TSFP_RUST_GAME_ARENA directly in the lane adapter, rather
    // than through the tsfp-ports root OPT_IN list. Keep the offline host aware of that switch.
    if va == 0x3c8a84 {
        out.push(("TSFP_RUST_GAME_ARENA".into(), "1".into()));
    }
    for index in 0u32.. {
        let (mut env, mut value) = (std::ptr::null(), std::ptr::null());
        // SAFETY: the host writes two static C strings when it returns 1.
        if unsafe { lookup(va, index, &mut env, &mut value) } == 0
            || env.is_null()
            || value.is_null()
        {
            break;
        }
        let text = |ptr| {
            unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned()
        };
        out.push((text(env), text(value)));
    }
    out
}

/// v8, `--t2-offline` only (called from `main`, never in play): turn on the switches that gate
/// the selected VA so its port runs instead of declining to the retained original.
fn enable_opt_ins(va: u32, switches: &[(String, String)]) {
    for (env, value) in switches {
        // SAFETY: single-threaded, before any port runs.
        unsafe { std::env::set_var(env, value) };
        eprintln!("t2-offline: {env}={value} (opt-in switch of {va:#x})");
    }
    OPT_IN_DECLARED.with(|declared| declared.set(!switches.is_empty()));
}

thread_local! {
    /// Whether a module declared an opt-in switch for the selected VA (the decline message).
    static OPT_IN_DECLARED: Cell<bool> = const { Cell::new(false) };
}

/// T3767: what a host must offer for the requested sides. The lifted fidelity control needs a
/// capture build: in a normal build the retained originals (`tsfp_*original_<va>`, the lifted
/// body of every lane and ext root) call their callees directly and tail calls are not hooked, so
/// the control runs real callees. Measured on the gate prebuilt (rounds 0 and 1): `callee_mismatch`
/// on faithful captures, XDK "read that never armed" host deaths, tail jumps to 0xCDCDCDCD.
pub fn check_host(version: u32, capture_build: bool, options: &Options) -> Result<(), String> {
    if version != HOST_API_VERSION {
        return Err(format!(
            "host API version {version} (want {HOST_API_VERSION})"
        ));
    }
    if options.lifted && !capture_build {
        return Err("the lifted fidelity control needs a capture build host \
             (TSFP_BUILD_CAPTURE=1): this host runs the callees of retained originals and tail \
             calls for real (T3767); use a capture host or --rust-only"
            .into());
    }
    Ok(())
}

unsafe extern "C" {
    fn t2o_host_api() -> *const HostApi;
}

/// The host API when this binary links the C host, else None.
pub fn host_api() -> Option<&'static HostApi> {
    let api = unsafe { t2o_host_api() };
    // SAFETY: a non-null pointer is the host's static definition.
    (!api.is_null()).then(|| unsafe { &*api })
}

thread_local! {
    static STUB: Cell<*mut c_void> = const { Cell::new(null_mut()) };
}

unsafe extern "C" fn nested_call(va: u32, regs: *mut ExtRegisters, convention: i32) {
    let stub = STUB.with(Cell::get).cast::<CalleeStub<'static>>();
    let convention = if convention == 1 {
        Convention::ReturnPushed
    } else {
        Convention::BridgePushesReturn
    };
    if stub.is_null() {
        // The hook is set only around a port run; anything else is a host wiring defect.
        eprintln!("t2-offline: nested guest call {va:#x} without an active stub");
        std::process::abort();
    }
    let regs = unsafe { &mut *regs };
    if unsafe { (*stub).should_dispatch_native(va) }
        && let Some(api) = host_api()
        && unsafe { (api.port_kind)(va) != 0 }
    {
        let before = *regs;
        // The host port entry expects the guest RET slot that tsfp_lane_x_call normally pushes.
        regs.esp = regs.esp.wrapping_sub(4);
        if unsafe { (api.run_port)(va, regs) } != 0 {
            return;
        }
        *regs = before;
    }
    unsafe { (*stub).dispatch(va, regs, convention) };
}

unsafe extern "C" fn lifted_nested_call(va: u32, regs: *mut ExtRegisters, implicit: *mut u32) {
    let stub = STUB.with(Cell::get).cast::<CalleeStub<'static>>();
    if stub.is_null() {
        eprintln!("t2-offline: lifted nested call {va:#x} without an active stub");
        std::process::abort();
    }
    // SAFETY: the host passes its 8-word implicit array, valid for the call.
    let implicit = unsafe { &mut *implicit.cast::<Implicit>() };
    unsafe { (*stub).dispatch_lifted(va, &mut *regs, implicit) };
}

pub struct HostExecutor {
    api: &'static HostApi,
}

impl HostExecutor {
    pub fn new(api: &'static HostApi) -> Self {
        // The C host's references are weak, so retain the offline exports in
        // the executable even under link-time garbage collection.
        std::hint::black_box(tsfp_t2_lane_has_root as extern "C" fn(u32) -> i32);
        std::hint::black_box(tsfp_t2_lane_capture_preferred as extern "C" fn(u32) -> i32);
        std::hint::black_box(
            tsfp_t2_lane_run as unsafe extern "C" fn(u32, *mut ExtRegisters, usize) -> i32,
        );
        Self { api }
    }
}

impl Executor for HostExecutor {
    fn bind(&mut self, base: usize) {
        unsafe { *self.api.mem_offset = base as isize };
    }
    fn has_port(&self, va: u32) -> bool {
        unsafe { (self.api.port_kind)(va) != 0 }
    }
    fn lifted_stubs_callees(&self) -> bool {
        true
    }
    fn render_scope(&mut self, va: u32, index: usize, side: Side, begin: bool) {
        if unsafe { tsfp_render_trace_enabled() } == 0 {
            return;
        }
        let marker = render_scope_marker(va, index, side, begin);
        unsafe { tsfp_render_trace_line(marker.as_ptr().cast(), marker.len()) };
    }
    fn guard_events(&self) -> Option<u64> {
        let read = if self.api.version >= 3 {
            self.api.guard_events
        } else {
            None
        };
        read.map(|events| {
            let (degraded, skipped, rolled_back) =
                tsfp_port_engine_anim_adapter::anim_lane::guard_counters();
            (unsafe { events() })
                + u64::from(degraded)
                + u64::from(skipped)
                + u64::from(rolled_back)
        })
    }
    fn run_lifted(&mut self, va: u32, regs: &mut ExtRegisters, stub: &mut CalleeStub<'_>) {
        let previous = STUB.with(|s| s.replace((stub as *mut CalleeStub<'_>).cast()));
        unsafe { *self.api.lifted_call_hook = Some(lifted_nested_call) };
        let ran = unsafe { (self.api.run_lifted)(va, regs) };
        unsafe { *self.api.lifted_call_hook = None };
        STUB.with(|s| s.set(previous));
        if ran == 0 {
            panic!("no lifted function for {va:#x}");
        }
    }
    fn set_implicit(&mut self, implicit: &Implicit) {
        unsafe { (self.api.set_implicit)(implicit.as_ptr()) };
    }
    fn implicit(&self) -> Option<Implicit> {
        let mut words: Implicit = [0; 8];
        unsafe { (self.api.get_implicit)(words.as_mut_ptr()) };
        Some(words)
    }
    fn run_port(&mut self, va: u32, regs: &mut ExtRegisters, stub: &mut CalleeStub<'_>) {
        let previous = STUB.with(|s| s.replace((stub as *mut CalleeStub<'_>).cast()));
        unsafe { *self.api.call_hook = Some(nested_call) };
        let taken = unsafe { (self.api.run_port)(va, regs) };
        unsafe { *self.api.call_hook = None };
        STUB.with(|s| s.set(previous));
        if taken == 0 {
            if OPT_IN_DECLARED.with(Cell::get) {
                panic!("dispatch declined {va:#x} (its opt-in switch is on)");
            }
            panic!("dispatch declined {va:#x} (no dispatching module declares an opt-in switch)");
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    pub input: String,
    pub va: u32,
    pub out: String,
    pub options: Options,
}

pub fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut input = None;
    let mut va = None;
    let mut out = None;
    let mut options = Options::default();
    let mut at = 0;
    while at < args.len() {
        let value = |at: usize| {
            args.get(at + 1)
                .cloned()
                .ok_or(format!("{} needs a value", args[at]))
        };
        match args[at].as_str() {
            "--t2-offline" => {
                input = Some(value(at)?);
                at += 1;
            }
            "--va" => {
                let text = value(at)?;
                let digits = text.trim_start_matches("0x").trim_start_matches("0X");
                va = Some(u32::from_str_radix(digits, 16).map_err(|_| format!("bad --va {text}"))?);
                at += 1;
            }
            "--out" => {
                out = Some(value(at)?);
                at += 1;
            }
            "--from" => {
                let text = value(at)?;
                options.from = text.parse().map_err(|_| format!("bad --from {text}"))?;
                at += 1;
            }
            "--ret-dead" => {
                options.ret_dead = RetDead::parse(&value(at)?)?;
                at += 1;
            }
            "--fpo-leaf" => options.ret_dead.fpo_leaf = true,
            "--inlined-callees" => {
                options.inlined = Inlined::parse(&value(at)?)?;
                at += 1;
            }
            "--callee-arity" => {
                let path = value(at)?;
                let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
                // One table per process (the host judges one VA per invocation).
                options.arity = Some(Box::leak(Box::new(CalleeArity::parse(&text)?)));
                at += 1;
            }
            "--lifted-only" => options.rust = false,
            "--rust-only" => options.lifted = false,
            other => return Err(format!("unknown argument {other}")),
        }
        at += 1;
    }
    if !options.lifted && !options.rust {
        return Err("--lifted-only and --rust-only exclude each other".into());
    }
    Ok(Args {
        input: input.ok_or("--t2-offline needs <entries.bin>")?,
        va: va.ok_or("--va is required")?,
        out: out.ok_or("--out is required")?,
        options,
    })
}

/// Run a whole `--t2-offline` invocation with an executor (the host's, or a test double).
pub fn run(args: &Args, exec: &mut dyn Executor) -> Result<bool, String> {
    let data = std::fs::read(&args.input).map_err(|e| format!("{}: {e}", args.input))?;
    let mut arena = Arena::reserve(GUEST_WINDOW)?;
    let file = std::fs::File::create(&args.out).map_err(|e| format!("{}: {e}", args.out))?;
    let mut out = std::io::BufWriter::new(file);
    let summary = run_spool(exec, &mut arena, &data, args.va, args.options, &mut out)?;
    std::io::Write::flush(&mut out).map_err(|e| format!("{}: {e}", args.out))?;
    eprintln!("{} va={:#x}", summary.line(), args.va);
    Ok(summary.no_abi_entry)
}

/// The `tsfp-bevy --t2-offline ...` entry point (`args` starts at `--t2-offline`).
pub fn main(args: &[String]) -> ExitCode {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(problem) => {
            eprintln!("t2-offline: {problem}");
            return ExitCode::from(2);
        }
    };
    let Some(api) = host_api() else {
        eprintln!(
            "t2-offline: this binary does not link the C host (lifted C, Rust ports, dispatch)"
        );
        return ExitCode::from(2);
    };
    // The version is checked before the capture_build field (absent below version 4) is read.
    let capture_build = api.version == HOST_API_VERSION && unsafe { (api.capture_build)() } != 0;
    if let Err(problem) = check_host(api.version, capture_build, &parsed.options) {
        eprintln!("t2-offline: {problem}");
        return ExitCode::from(2);
    }
    enable_opt_ins(parsed.va, &opt_ins(parsed.va, api.opt_in));
    match run(&parsed, &mut HostExecutor::new(api)) {
        Ok(true) => ExitCode::from(3),
        Ok(false) => ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("t2-offline: {problem}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferred_ret4_audio_root_uses_the_capture_frame_cleanup() {
        let mut arena = Box::new([0u8; 0x4000]);
        // Guarded lane calls reject guest addresses below 64 KB. Map this
        // synthetic guest window onto the start of the test arena.
        let guest_base = 0x1_0000usize;
        let offset = (arena.as_mut_ptr() as usize).wrapping_sub(guest_base);
        let entry_esp = (guest_base + 0x1010) as u32;
        let arg = 0x1234_5678u32;
        unsafe {
            (offset.wrapping_add(entry_esp as usize + 4) as *mut u32).write_unaligned(arg);
        }
        let mut regs = ExtRegisters {
            eax: 0,
            ecx: (guest_base + 0x1200) as u32,
            edx: 0,
            esp: entry_esp,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: tsfp_port_runtime::abi::Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [tsfp_port_runtime::abi::Xmm::default(); 5],
            xmm_hi: [tsfp_port_runtime::abi::Xmm::default(); 3],
        };

        assert_eq!(tsfp_t2_lane_capture_preferred(0x29e40), 1);
        assert_eq!(unsafe { tsfp_t2_lane_run(0x29e40, &mut regs, offset) }, 1);
        assert_eq!(regs.eax, arg);
        assert_eq!(regs.esp, entry_esp + 8);
        assert_eq!(
            unsafe {
                (offset.wrapping_add(regs.ecx as usize + 0x64) as *const u32).read_unaligned()
            },
            arg
        );
    }

    #[test]
    fn the_lifted_control_refuses_a_host_that_is_not_a_capture_build() {
        let both = Options::default();
        let rust_only = Options {
            lifted: false,
            ..Options::default()
        };
        assert!(check_host(HOST_API_VERSION, true, &both).is_ok());
        let refused = check_host(HOST_API_VERSION, false, &both).unwrap_err();
        assert!(refused.contains("capture build"), "{refused}");
        assert!(check_host(HOST_API_VERSION, false, &rust_only).is_ok());
        assert!(
            check_host(2, true, &both)
                .unwrap_err()
                .contains("version 2")
        );
    }

    unsafe extern "C" fn two_switches(
        va: u32,
        index: u32,
        env: *mut *const c_char,
        value: *mut *const c_char,
    ) -> i32 {
        let table: &[(&CStr, &CStr)] = match va {
            0x160f50 => &[(c"TSFP_A", c"1"), (c"TSFP_B", c"rust")],
            _ => &[],
        };
        let Some((name, on)) = table.get(index as usize) else {
            return 0;
        };
        unsafe {
            *env = name.as_ptr();
            *value = on.as_ptr();
        }
        1
    }

    #[test]
    fn v8_opt_ins_list_every_declared_switch_and_none_for_an_undeclared_va() {
        let found = opt_ins(0x160f50, two_switches);
        assert_eq!(
            found,
            vec![
                ("TSFP_A".into(), "1".into()),
                ("TSFP_B".into(), "rust".into())
            ]
        );
        assert!(opt_ins(0x2784d0, two_switches).is_empty());
    }

    #[test]
    fn v8_opt_ins_enable_native_crt_atexit_route() {
        assert_eq!(
            opt_ins(0x3c8a84, two_switches),
            vec![("TSFP_RUST_GAME_ARENA".into(), "1".into())]
        );
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn arguments_follow_the_t3743_contract() {
        let args = parse_args(&strings(&[
            "--t2-offline",
            "e.bin",
            "--va",
            "0x1531D0",
            "--out",
            "r.jsonl",
            "--lifted-only",
        ]))
        .unwrap();
        assert_eq!(args.input, "e.bin");
        assert_eq!(args.va, 0x1531d0);
        assert_eq!(args.out, "r.jsonl");
        assert_eq!(
            args.options,
            Options {
                lifted: true,
                rust: false,
                from: 0,
                ret_dead: RetDead::default(),
                inlined: Inlined::default(),
                arity: None,
            }
        );
        assert!(parse_args(&strings(&["--t2-offline", "e.bin", "--out", "r"])).is_err());
        assert!(parse_args(&strings(&["--t2-offline", "e", "--va", "zz", "--out", "r"])).is_err());
        assert!(
            parse_args(&strings(&[
                "--t2-offline",
                "e",
                "--va",
                "1",
                "--out",
                "r",
                "--lifted-only",
                "--rust-only"
            ]))
            .is_err()
        );
    }

    #[test]
    fn without_the_c_host_the_api_is_absent() {
        assert!(host_api().is_none());
        assert_eq!(
            main(&strings(&["--t2-offline", "e", "--va", "1", "--out", "r"])),
            ExitCode::from(2)
        );
    }
}

#[cfg(test)]
mod render_scope_tests {
    use super::render_scope_marker;
    use crate::compare::Side;

    #[test]
    fn offline_scope_marker_names_root_side_index_and_boundary() {
        assert_eq!(
            render_scope_marker(0x21450, 7, Side::Lifted, true),
            "scope 00021450 lifted 7 begin"
        );
        assert_eq!(
            render_scope_marker(0x21450, 7, Side::Port, false),
            "scope 00021450 rust 7 end"
        );
    }
}

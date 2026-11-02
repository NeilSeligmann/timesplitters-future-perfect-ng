// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-entry driving and the JSON-lines output agreed with T3743 (claude-1, 2026-10-25):
//!
//! `{"index", "digest", "lifted", "rust", "detail", "extra_read", "dead_stack_writes", "no_work"}`, one line
//! per input record in input order. `lifted` is the capture-fidelity control (anything but
//! `equal` makes the entry `capture_unfaithful`, its `rust` result is then not evidence).
//! `lifted`: equal|exit_regs|write_set|callee_mismatch|fault. `rust`: the same plus fp_ulp and
//! no_abi_entry. A side that was not run (`--lifted-only` / `--rust-only`) is `null`. A record the
//! runner must not judge is `skipped` on both sides with the reason in `detail`: a flagged record
//! (`capture_incomplete:<reason>`, normally filtered before export), a record of another VA, or a
//! malformed record. `no_work` (v9, T3832): the port ran, did nothing (no write outside the dead
//! stack, no call) and the original wrote memory or had callees; the Python aggregate counts it as
//! diverged even on an entry whose lifted control is unfaithful.
use std::io::Write;

use tsfp_port_runtime::abi::ExtRegisters;

use crate::arena::Arena;
use crate::compare::{
    Class, Judgement, RetDead, Side, compare_implicit, dead_stack_writes, judge, original_worked,
    run_worked,
};
use crate::digest::input_digest;
use crate::frames::NestedFrames;
use crate::spool::{self, Entry, Implicit};
use crate::stub::{ArgPolicy, CalleeArity, CalleeStub, Inlined};

/// What links the code under test: the host binary (lifted C plus Rust dispatch) or a test double.
pub trait Executor {
    /// Point the code under test at the arena (`g_xbox_mem_offset` = `base`).
    fn bind(&mut self, base: usize);
    /// Whether `va` has a Rust dispatch entry.
    fn has_port(&self, va: u32) -> bool;
    /// Whether nested calls of the lifted original reach `stub` (false: the lifted callees run
    /// for real, only exit registers and write-set are checked for the control).
    fn lifted_stubs_callees(&self) -> bool;
    /// Run the retained lifted original of `va`. Exit ESP as after its `ret`.
    fn run_lifted(&mut self, va: u32, regs: &mut ExtRegisters, stub: &mut CalleeStub<'_>);
    /// Run the Rust port of `va` through its dispatch entry. Exit ESP as after the `ret` the
    /// host wrapper performs (entry + 4 + callee-cleaned argument bytes).
    fn run_port(&mut self, va: u32, regs: &mut ExtRegisters, stub: &mut CalleeStub<'_>);
    /// Bracket each side of one captured root invocation for offline renderer stream comparison.
    /// Live executors do not implement this hook; the t2 offline host writes it to the trace file.
    fn render_scope(&mut self, _va: u32, _index: usize, _side: Side, _begin: bool) {}
    /// T3765: lane-guard events so far (None: no guard in this executor). A port run that moves
    /// the count had an access refused and repaired by the guard, so it is never judged equal.
    fn guard_events(&self) -> Option<u64> {
        None
    }
    /// T3740: load the implicit thread state (`g_seh_ebp`, DF, ...) before either side runs.
    fn set_implicit(&mut self, _implicit: &Implicit) {}
    /// The implicit thread state after a run (None: the executor has none, not compared).
    fn implicit(&self) -> Option<Implicit> {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub lifted: bool,
    pub rust: bool,
    /// T3740: first record index to run (`--from`): the T3743 runner resumes after a record that
    /// took the whole host down (an abort or exit in host code), which it then records as `fault`.
    pub from: usize,
    /// v3: return registers the port side need not reproduce (`--ret-dead`, default none).
    pub ret_dead: RetDead,
    /// v6: callees the port runs natively (`--inlined-callees`, default none).
    pub inlined: Inlined,
    /// v8: proven callee stack arity (`--callee-arity <file>`, default none = strict).
    pub arity: Option<&'static CalleeArity>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            lifted: true,
            rust: true,
            from: 0,
            ret_dead: RetDead::default(),
            inlined: Inlined::default(),
            arity: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    NotRun,
    Skipped,
    NoAbiEntry,
    Judged(Judgement),
}

impl Verdict {
    fn json(&self) -> String {
        match self {
            Verdict::NotRun => "null".into(),
            Verdict::Skipped => "\"skipped\"".into(),
            Verdict::NoAbiEntry => "\"no_abi_entry\"".into(),
            Verdict::Judged(j) => format!("\"{}\"", j.class.name()),
        }
    }
    fn class(&self) -> Option<Class> {
        match self {
            Verdict::Judged(j) => Some(j.class),
            _ => None,
        }
    }
    fn detail(&self) -> Option<&str> {
        match self {
            Verdict::Judged(j) if !j.detail.is_empty() => Some(&j.detail),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryResult {
    pub index: usize,
    pub digest: String,
    pub lifted: Verdict,
    pub rust: Verdict,
    pub detail: String,
    pub extra_read: u32,
    pub dead_stack_writes: u32,
    /// v9: the port did nothing where the original did work (see the module doc).
    pub no_work: bool,
}

fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

impl EntryResult {
    pub fn json_line(&self) -> String {
        format!(
            "{{\"index\": {}, \"digest\": {}, \"lifted\": {}, \"rust\": {}, \"detail\": {}, \"extra_read\": {}, \"dead_stack_writes\": {}, \"no_work\": {}}}",
            self.index,
            json_string(&self.digest),
            self.lifted.json(),
            self.rust.json(),
            json_string(&self.detail),
            self.extra_read,
            self.dead_stack_writes,
            self.no_work
        )
    }

    fn skipped(index: usize, digest: String, detail: String) -> Self {
        Self {
            index,
            digest,
            lifted: Verdict::Skipped,
            rust: Verdict::Skipped,
            detail,
            extra_read: 0,
            dead_stack_writes: 0,
            no_work: false,
        }
    }
}

struct SideRun {
    judgement: Judgement,
    extra_reads: u32,
    /// T3767: the lifted control's per-call masks of argument bytes it had written (`Observe`).
    arg_masks: Vec<u64>,
    /// v9: the run made a call or wrote outside the dead stack.
    worked: bool,
}

#[allow(clippy::too_many_arguments)] // v8: the arity table joins the port rules
fn run_side(
    exec: &mut dyn Executor,
    arena: &mut Arena,
    index: usize,
    entry: &Entry,
    side: Side,
    ret_dead: RetDead,
    policy: ArgPolicy,
    inlined: Inlined,
    arity: Option<&'static CalleeArity>,
    frames: &NestedFrames<'_>,
) -> SideRun {
    if let Err(problem) = arena.prepare(&entry.read_runs) {
        return SideRun {
            judgement: Judgement {
                class: Class::Fault,
                detail: problem,
            },
            extra_reads: 0,
            arg_masks: Vec::new(),
            worked: false,
        };
    }
    let arena: &Arena = arena;
    let mut regs = entry.entry_regs;
    exec.set_implicit(&entry.entry_implicit.unwrap_or_default());
    let mut stub = CalleeStub::with_policy(entry, arena, policy);
    if side == Side::Port {
        stub = stub
            .port_rules(ret_dead.fpo_leaf, inlined)
            .callee_arity(arity)
            .recursion(frames);
    }
    let stubbed = side == Side::Port || exec.lifted_stubs_callees();
    let guard_before = exec.guard_events();
    exec.render_scope(entry.va, index, side, true);
    let end = arena.run(&mut || match side {
        Side::Lifted => exec.run_lifted(entry.va, &mut regs, &mut stub),
        Side::Port => exec.run_port(entry.va, &mut regs, &mut stub),
    });
    exec.render_scope(entry.va, index, side, false);
    let callees = if stubbed { stub.finish() } else { None };
    let mut judgement = judge(side, entry, arena, &end, callees, &regs, ret_dead);
    let guard_after = exec.guard_events();
    if side == Side::Port && guard_after != guard_before {
        // T3765 (I-T3765-2): the lane guard refused an access and degraded the run, skipped a
        // callee or ran the original instead; whatever the exit state, it is not the port's.
        judgement = Judgement {
            class: Class::Fault,
            detail: format!("lane_guard_fired: {guard_before:?} -> {guard_after:?} events"),
        };
    }
    if side == Side::Lifted && judgement.class == Class::Equal {
        if let Some(detail) = exec
            .implicit()
            .and_then(|got| compare_implicit(entry, &got))
        {
            judgement = Judgement {
                class: Class::ExitRegs,
                detail,
            };
        }
    }
    SideRun {
        judgement,
        extra_reads: arena.extra_reads(),
        arg_masks: stub.observed().to_vec(),
        worked: run_worked(entry, arena, stub.calls()),
    }
}

/// Judge one record. `va` is the function the run is for.
pub fn run_entry(
    exec: &mut dyn Executor,
    arena: &mut Arena,
    index: usize,
    entry: &Entry,
    va: u32,
    options: Options,
) -> EntryResult {
    run_entry_in(
        exec,
        arena,
        index,
        entry,
        va,
        options,
        &NestedFrames::default(),
    )
}

/// `run_entry` with the spool's records of `va` as candidate inner activations (v10, T3839):
/// a port that runs its recursion natively has the inner frames' calls matched in their frame.
pub fn run_entry_in(
    exec: &mut dyn Executor,
    arena: &mut Arena,
    index: usize,
    entry: &Entry,
    va: u32,
    options: Options,
    frames: &NestedFrames<'_>,
) -> EntryResult {
    let digest = input_digest(entry);
    // Until the executor consumes these events, never judge a format-3 capture.
    if entry.clock_events.is_some() {
        return EntryResult::skipped(
            index,
            digest,
            "capture_incomplete:clock_events_unsupported".into(),
        );
    }
    if entry.flags != 0 {
        let reasons = spool::flag_names(entry.flags).join(",");
        return EntryResult::skipped(index, digest, format!("capture_incomplete:{reasons}"));
    }
    if entry.va != va {
        return EntryResult::skipped(
            index,
            digest,
            format!("record va {:#x} is not {va:#x}", entry.va),
        );
    }
    let mut extra_read = 0;
    // T3767: the port is held to the argument bytes the lifted control had written at each call.
    let mut port_policy = ArgPolicy::Strict;
    let lifted = if options.lifted {
        let run = run_side(
            exec,
            arena,
            index,
            entry,
            Side::Lifted,
            options.ret_dead,
            ArgPolicy::Observe,
            Inlined::default(),
            None,
            &NestedFrames::default(),
        );
        extra_read = run.extra_reads;
        port_policy = ArgPolicy::FromLifted(run.arg_masks);
        Verdict::Judged(run.judgement)
    } else {
        Verdict::NotRun
    };
    let mut no_work = false;
    let rust = if !options.rust {
        Verdict::NotRun
    } else if !exec.has_port(va) {
        Verdict::NoAbiEntry
    } else {
        let run = run_side(
            exec,
            arena,
            index,
            entry,
            Side::Port,
            options.ret_dead,
            port_policy,
            options.inlined,
            options.arity,
            frames,
        );
        extra_read = run.extra_reads;
        // v9: a run that returned, or a dispatch that declined, having done nothing (any other
        // fault is its own verdict).
        let declined = run.judgement.detail.contains("dispatch declined");
        no_work = (run.judgement.class != Class::Fault || declined)
            && !run.worked
            && original_worked(entry);
        Verdict::Judged(run.judgement)
    };
    let detail = match (lifted.detail(), rust.detail()) {
        (Some(l), Some(r)) => format!("lifted: {l}; rust: {r}"),
        (Some(l), None) => format!("lifted: {l}"),
        (None, Some(r)) => r.to_string(),
        (None, None) => String::new(),
    };
    EntryResult {
        index,
        digest,
        lifted,
        rust,
        detail,
        extra_read,
        dead_stack_writes: dead_stack_writes(entry),
        no_work,
    }
}

/// Totals over a run, for the stderr summary and the tests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub entries: usize,
    pub skipped: usize,
    pub lifted_equal: usize,
    pub capture_unfaithful: usize,
    /// Port verdicts on faithful entries only, by class name.
    pub rust: std::collections::BTreeMap<&'static str, usize>,
    pub no_abi_entry: bool,
}

impl Summary {
    fn add(&mut self, result: &EntryResult) {
        self.entries += 1;
        if result.lifted == Verdict::Skipped {
            self.skipped += 1;
            return;
        }
        let faithful = match result.lifted.class() {
            Some(Class::Equal) => {
                self.lifted_equal += 1;
                true
            }
            Some(_) => {
                self.capture_unfaithful += 1;
                false
            }
            None => true,
        };
        if result.rust == Verdict::NoAbiEntry {
            self.no_abi_entry = true;
        }
        if let (true, Some(class)) = (faithful, result.rust.class()) {
            *self.rust.entry(class.name()).or_default() += 1;
        }
    }

    /// Lifted-fidelity rate over the judged entries (None when the control did not run).
    pub fn faithful_rate(&self) -> Option<f64> {
        let judged = self.lifted_equal + self.capture_unfaithful;
        (judged > 0).then(|| self.lifted_equal as f64 / judged as f64)
    }

    pub fn line(&self) -> String {
        let rate = self
            .faithful_rate()
            .map_or("n/a".to_string(), |r| format!("{:.1}%", r * 100.0));
        format!(
            "t2-offline: entries={} skipped={} lifted_equal={} capture_unfaithful={} faithful_rate={} rust={:?}{}",
            self.entries,
            self.skipped,
            self.lifted_equal,
            self.capture_unfaithful,
            rate,
            self.rust,
            if self.no_abi_entry {
                " no_abi_entry"
            } else {
                ""
            }
        )
    }
}

/// Run every record of a spool (or raw shard) for `va`, writing one JSON line each.
pub fn run_spool(
    exec: &mut dyn Executor,
    arena: &mut Arena,
    data: &[u8],
    va: u32,
    options: Options,
    out: &mut dyn Write,
) -> Result<Summary, String> {
    let spool = spool::parse(data).map_err(|e| format!("unreadable spool: {e}"))?;
    exec.bind(arena.base());
    let mut summary = Summary::default();
    let frames = NestedFrames::new(
        va,
        spool
            .records
            .iter()
            .filter_map(|record| record.as_ref().ok()),
    );
    for (index, record) in spool.records.iter().enumerate().skip(options.from) {
        let result = match record {
            Ok(entry) => run_entry_in(exec, arena, index, entry, va, options, &frames),
            Err(problem) => {
                EntryResult::skipped(index, String::new(), format!("malformed: {problem}"))
            }
        };
        summary.add(&result);
        writeln!(out, "{}", result.json_line()).map_err(|e| format!("write: {e}"))?;
        // Line by line: a record that kills the host leaves every earlier verdict on disk.
        out.flush().map_err(|e| format!("write: {e}"))?;
    }
    summary.no_abi_entry = options.rust && !exec.has_port(va);
    Ok(summary)
}

// SPDX-License-Identifier: GPL-3.0-or-later
//! T3975 slice 6: `src/host/host_route.c` in Rust. The host's route duties (the wait, nav and poke specs of the command
//! line and of the record, the route's event logging, the `--poke-at-poll` triggers, the nav recorder and the installed
//! route handle) are Rust-owned state behind the same `host_route_*` C ABI. Guest memory, the route probe, the record
//! reader and the nav recorder stay in C (TEMPORARY-SHIM: the record reader is the baseline until the Rust reader is
//! EQUAL on recorded owner inputs). `TSFP_RUST_INPUT_HLE=0` builds keep the C file.
use std::ffi::{CStr, c_char, c_int, c_void};
use std::sync::{Mutex, MutexGuard};

use tsfp_input_route_spec::nav_spec::{self, Mode, Step};
use tsfp_input_route_spec::poke::{PokeEntry, PokeSet, PokeWhere, parse as poke_parse};
use tsfp_input_route_spec::{MAX_WAITS, POKE_MAX, Wait, wait_parse};

use crate::abi::{
    Event, Hooks, NavConfig, NavProbe, PadState, Probe, ReadBytes, ReadMem, SourceFn, event,
};
use crate::ffi;
use crate::route::Route;

const MAX_NAV_STEPS: usize = nav_spec::MAX_STEPS;

/// `host_route_config`, same layout.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Config {
    waits: *const *const c_char,
    wait_count: usize,
    event_waits: *const *const c_char,
    event_wait_count: usize,
    record_waits: *const *const c_char,
    record_wait_count: usize,
    probe: *const Probe,
    pokes: *const *const c_char,
    poke_count: usize,
    live: SourceFn,
    live_user: *mut c_void,
    on_failure: Option<unsafe extern "C" fn(*const c_char)>,
    read_mem: ReadMem,
    poke: Option<unsafe extern "C" fn(*const c_char) -> bool>,
    test_user: *mut c_void,
    record_navs: *const *const c_char,
    record_nav_count: usize,
    nav_mode: *const c_char,
    nav_menus_file: *const c_char,
    nav_menus_text: *const c_char,
    nav_timing: *const c_char,
    read_bytes: ReadBytes,
}

impl Config {
    const EMPTY: Config = Config {
        waits: std::ptr::null(),
        wait_count: 0,
        event_waits: std::ptr::null(),
        event_wait_count: 0,
        record_waits: std::ptr::null(),
        record_wait_count: 0,
        probe: std::ptr::null(),
        pokes: std::ptr::null(),
        poke_count: 0,
        live: None,
        live_user: std::ptr::null_mut(),
        on_failure: None,
        read_mem: None,
        poke: None,
        test_user: std::ptr::null_mut(),
        record_navs: std::ptr::null(),
        record_nav_count: 0,
        nav_mode: std::ptr::null(),
        nav_menus_file: std::ptr::null(),
        nav_menus_text: std::ptr::null(),
        nav_timing: std::ptr::null(),
        read_bytes: None,
    };
}

/// The state of `host_route.c` (`g_route`, `g_pokes`, `g_config`, `g_nav_recorder`).
struct State {
    route: *mut Route,
    pokes: PokeSet,
    config: Config,
    recorder: *mut c_void,
}

// SAFETY: the pointers are handles owned by this module; every access goes through the lock.
unsafe impl Send for State {}

static STATE: Mutex<State> = Mutex::new(State {
    route: std::ptr::null_mut(),
    pokes: PokeSet {
        entries: [PokeEntry::ZERO; POKE_MAX],
        count: 0,
    },
    config: Config::EMPTY,
    recorder: std::ptr::null_mut(),
});

fn state() -> MutexGuard<'static, State> {
    STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// `nav_record_hooks`.
#[repr(C)]
struct RecordHooks {
    probe: NavProbe,
    log: Option<unsafe extern "C" fn(*const c_char, *mut c_void)>,
    log_user: *mut c_void,
    add: Option<
        unsafe extern "C" fn(
            u64,
            u64,
            u64,
            *const c_char,
            c_int,
            u16,
            *mut c_void,
        ) -> *const c_char,
    >,
    add_user: *mut c_void,
}

unsafe extern "C" {
    fn xinput_replay_marks(marks: *mut *const u64) -> usize;
    fn xinput_replay_script() -> *const c_void;
    fn xinput_replay_wait_count() -> usize;
    fn xinput_replay_wait(index: usize) -> *const c_char;
    fn xinput_replay_nav_count() -> usize;
    fn xinput_replay_nav(index: usize) -> *const c_char;
    fn xinput_record_add_nav(
        at: u64,
        to: u64,
        edge: u64,
        body: *const c_char,
        analog: c_int,
        digital: u16,
    ) -> *const c_char;
    fn xinput_record_set_nav_hooks(
        observer: Option<unsafe extern "C" fn(u64, *const PadState, *mut c_void)>,
        closer: Option<unsafe extern "C" fn(u64, *mut c_void)>,
        logger: Option<unsafe extern "C" fn(*const c_char, *mut c_void)>,
        user: *mut c_void,
    );
    fn nav_recorder_create(
        table: *const nav_spec::MenuTable,
        hooks: *const RecordHooks,
    ) -> *mut c_void;
    fn nav_recorder_free(recorder: *mut c_void);
    fn nav_recorder_observe(recorder: *mut c_void, poll: u64, recorded: *const PadState);
    fn nav_recorder_close(recorder: *mut c_void, total: u64);
    fn route_probe_enable();
    fn route_probe_view() -> *const Probe;
    fn route_probe_log_active() -> bool;
    fn route_probe_log_line(format: *const c_char, ...);
    fn guest_dump_poke_now(label: *const c_char) -> bool;
    fn kernel_guest_read_bytes(address: u32, out: *mut c_void, length: usize) -> bool;
}

fn text(pointer: *const c_char) -> Option<&'static [u8]> {
    // SAFETY: a non-null pointer is a C string that outlives the call.
    (!pointer.is_null()).then(|| unsafe { CStr::from_ptr(pointer) }.to_bytes())
}

fn lossy(pointer: *const c_char) -> String {
    String::from_utf8_lossy(text(pointer).unwrap_or(b"")).into_owned()
}

fn strings(array: *const *const c_char, count: usize) -> Vec<*const c_char> {
    if count == 0 || array.is_null() {
        return Vec::new();
    }
    // SAFETY: `count` entries.
    unsafe { std::slice::from_raw_parts(array, count) }.to_vec()
}

fn write_error(error: *mut c_char, size: usize, message: &str) {
    if error.is_null() || size == 0 {
        return;
    }
    let kept = message.len().min(size - 1);
    // SAFETY: `error` is writable for `size` bytes.
    unsafe {
        std::ptr::copy_nonoverlapping(message.as_ptr(), error.cast::<u8>(), kept);
        *error.add(kept) = 0;
    }
}

fn read_guest(address: u32, width: u32) -> Option<u32> {
    let mut raw = [0u8; 4];
    if width == 0 || width > 4 {
        return None;
    }
    // SAFETY: `raw` holds `width` bytes.
    if !unsafe { kernel_guest_read_bytes(address, raw.as_mut_ptr().cast(), width as usize) } {
        return None;
    }
    Some(u32::from_le_bytes(raw))
}

unsafe extern "C" fn mem_adapter(
    address: u32,
    width: u32,
    value: *mut u32,
    _user: *mut c_void,
) -> bool {
    let config = state().config;
    let read = match config.read_mem {
        // SAFETY: the test seam has the `read_mem` contract.
        Some(read) => unsafe { read(address, width, value, config.test_user) },
        None => read_guest(address, width).is_some_and(|word| {
            // SAFETY: `value` is writable.
            unsafe { *value = word };
            true
        }),
    };
    read
}

unsafe extern "C" fn bytes_adapter(
    address: u32,
    buffer: *mut c_void,
    length: usize,
    _user: *mut c_void,
) -> bool {
    // SAFETY: `buffer` holds `length` bytes.
    length != 0 && unsafe { kernel_guest_read_bytes(address, buffer, length) }
}

fn probe_log(line: &str) {
    // SAFETY: plain C calls; the line is NUL-free text passed through "%s".
    if !unsafe { route_probe_log_active() } {
        return;
    }
    let mut owned = line.as_bytes().to_vec();
    owned.retain(|byte| *byte != 0);
    owned.push(0);
    unsafe { route_probe_log_line(c"%s".as_ptr(), owned.as_ptr().cast::<c_char>()) };
}

fn log_event(line: &str) {
    probe_log(&format!("route {line}"));
}

fn fire(label: &str, why: &str, poll: u64, cursor: u64) {
    let config = state().config;
    let mut owned = label.as_bytes().to_vec();
    owned.push(0);
    let pointer = owned.as_ptr().cast::<c_char>();
    // SAFETY: `pointer` is a NUL-terminated copy.
    let ran = match config.poke {
        Some(poke) => unsafe { poke(pointer) },
        None => unsafe { guest_dump_poke_now(pointer) },
    };
    eprintln!(
        "route poke (T1616, FORCED-STATE, FABRICATED): label {label} at {why}, host poll {poll}, record position {cursor} -> {}",
        if ran {
            "served (see guestpoke.log for the result)"
        } else {
            "NOT served (dump/poke not started)"
        }
    );
}

/// At most `POKE_MAX` triggers exist, so a take that does not retire its entry cannot spin.
fn fire_due(where_: u32, value: u64, why: &str, poll: u64, cursor: u64) {
    for _ in 0..POKE_MAX {
        let label = {
            let mut guard = state();
            guard.pokes.take(where_, value).map(|index| {
                String::from_utf8_lossy(nav_spec::c_text(&guard.pokes.entries[index].label))
                    .into_owned()
            })
        };
        match label {
            Some(label) => fire(&label, why, poll, cursor),
            None => return,
        }
    }
}

unsafe extern "C" fn on_poll(poll: u64, cursor: u64, _user: *mut c_void) {
    fire_due(PokeWhere::POLL, poll, "poll", poll, cursor);
}

fn failure(reason: &str) {
    if let Some(on_failure) = state().config.on_failure {
        let mut owned = reason.as_bytes().to_vec();
        owned.push(0);
        // SAFETY: NUL-terminated copy.
        unsafe { on_failure(owned.as_ptr().cast()) };
    }
}

unsafe extern "C" fn on_event(event: *const Event, _user: *mut c_void) {
    // SAFETY: the route passes a valid event.
    let event = unsafe { &*event };
    let detail = lossy(event.detail);
    let has_detail = !detail.is_empty();
    let seconds = event.stalled_ms as f64 / 1000.0;
    match event.kind {
        event::MARK => {
            eprintln!(
                "route: mark {} reached at host poll {}, record position {}",
                event.index, event.poll, event.cursor
            );
            log_event(&format!(
                "mark {} reached record-position={}",
                event.index, event.cursor
            ));
            fire_due(
                PokeWhere::MARK,
                u64::from(event.index),
                "mark",
                event.poll,
                event.cursor,
            );
        }
        event::WAIT_OK => {
            eprintln!(
                "route: wait for mark {} satisfied after {} stalled poll(s), {seconds:.1} s, at host poll {}",
                event.index, event.stalled, event.poll
            );
            if has_detail {
                eprintln!("route:   {detail}");
            }
            log_event(&format!(
                "wait-ok mark {} stalled-polls={} stalled-ms={} {detail}",
                event.index, event.stalled, event.stalled_ms
            ));
        }
        event::WAIT_PROGRESS => {
            eprintln!(
                "route: still waiting for mark {} after {seconds:.1} s ({} polls): {detail}",
                event.index, event.stalled
            );
            log_event(&format!(
                "wait-progress mark {} stalled-ms={} {detail}",
                event.index, event.stalled_ms
            ));
        }
        event::WAIT_TIMEOUT => {
            let reason = format!(
                "route FAILED: the wait for mark {} did not hold within {} polls / {seconds:.1} s (record position {}, host poll {}){}{detail}",
                event.index,
                event.stalled,
                event.cursor,
                event.poll,
                if has_detail { "; unmet: " } else { "" }
            );
            eprintln!(
                "{reason}\nroute: the screen or event the route waits for never appeared (wrong state or disk, or the condition is wrong: see --route-event-log and tools.route_events)"
            );
            log_event(&format!("wait-timeout mark {} {detail}", event.index));
            failure(&reason);
        }
        event::NAV_START | event::NAV_OK => {
            eprintln!("route nav: {detail} (host poll {})", event.poll);
            let tag = if event.kind == event::NAV_START {
                "nav-start"
            } else {
                "nav-ok"
            };
            log_event(&format!("{tag} {detail}"));
        }
        event::NAV_FALLBACK | event::NAV_PROGRESS => {
            eprintln!("{detail}");
            let tag = if event.kind == event::NAV_FALLBACK {
                "nav-fallback"
            } else {
                "nav-progress"
            };
            log_event(&format!("{tag} {detail}"));
        }
        event::NAV_FAIL => {
            let shown = if has_detail {
                detail.as_str()
            } else {
                "route FAILED: nav step"
            };
            eprintln!(
                "{shown} (record position {}, host poll {})",
                event.cursor, event.poll
            );
            log_event(&format!("nav-fail {detail}"));
            failure(shown);
        }
        event::END => {
            let live = state().config.live.is_some();
            eprintln!(
                "route: the recorded inputs are exhausted at host poll {}, the {} pad has the controls now",
                event.poll,
                if live { "live" } else { "(none: rest)" }
            );
            log_event("end record-exhausted");
            fire_due(PokeWhere::END, 0, "replay-end", event.poll, event.cursor);
        }
        _ => {}
    }
}

/// A menu table is a few KB: bigger files are refused like the C reader did.
fn read_text_file(path: *const c_char) -> Option<Vec<u8>> {
    let path = std::str::from_utf8(text(path)?).ok()?;
    let bytes = std::fs::read(path).ok()?;
    (bytes.len() < (1 << 20) - 1).then_some(bytes)
}

fn menus_source(config: &Config) -> Result<Option<Vec<u8>>, String> {
    if let Some(inline) = text(config.nav_menus_text) {
        return Ok(Some(inline.to_vec()));
    }
    if config.nav_menus_file.is_null() {
        return Ok(None);
    }
    read_text_file(config.nav_menus_file)
        .map(Some)
        .ok_or_else(|| {
            format!(
                "--route-nav-menus {}: cannot read the menu table",
                lossy(config.nav_menus_file)
            )
        })
}

fn menus_label(config: &Config) -> String {
    if config.nav_menus_file.is_null() {
        "(text)".to_string()
    } else {
        lossy(config.nav_menus_file)
    }
}

/// T1640: the record's nav lines, the menu table, the mode and the timing become the route's nav steps.
fn setup_nav(
    route: *mut Route,
    config: &Config,
    script: *const c_void,
    marks: &[u64],
) -> Result<(), String> {
    if config.record_nav_count == 0 {
        return Ok(());
    }
    let mut steps: Vec<Step> = Vec::new();
    for (i, line) in strings(config.record_navs, config.record_nav_count)
        .into_iter()
        .enumerate()
    {
        steps.push(
            nav_spec::line_parse(text(line))
                .map_err(|reason| format!("record '# nav:' line {}: {reason}", i + 1))?,
        );
    }
    // SAFETY: the script is the loaded record.
    let total = unsafe { crate::sys::xinput_script_total_frames(script) };
    nav_spec::steps_check(&steps, marks, total)
        .map_err(|reason| format!("record '# nav:' lines: {reason}"))?;
    let table = match menus_source(config)? {
        Some(source) => Some(
            nav_spec::menus_parse(&source)
                .map_err(|reason| format!("--route-nav-menus {}: {reason}", menus_label(config)))?,
        ),
        None => None,
    };
    let have_table = table.is_some();
    let mode = match text(config.nav_mode) {
        Some(requested) => nav_spec::mode_parse(Some(requested)).ok_or_else(|| {
            format!(
                "--route-nav {}: want on, off or strict",
                lossy(config.nav_mode)
            )
        })?,
        None if have_table => Mode::ON,
        None => Mode::OFF,
    };
    let timing = match text(config.nav_timing) {
        Some(spec) => nav_spec::timing_parse(Some(spec))?,
        None => nav_spec::timing_default(),
    };
    let nav = NavConfig {
        steps: steps.as_ptr(),
        step_count: steps.len(),
        menus: table
            .as_deref()
            .map_or(std::ptr::null(), std::ptr::from_ref),
        mode,
        timing: &timing,
        read_bytes: config.read_bytes.or(Some(bytes_adapter)),
        bytes_user: config.test_user,
    };
    let mut error = [0 as c_char; 300];
    // SAFETY: the route is live, `nav` and the buffers outlive the call.
    if !unsafe { ffi::xinput_route_set_nav(route, &nav, error.as_mut_ptr(), error.len()) } {
        return Err(lossy(error.as_ptr()));
    }
    if mode == Mode::OFF {
        eprintln!(
            "route nav (T1640): off{}, the record's {} nav step(s) are ignored and the recorded presses replay open loop",
            if config.nav_mode.is_null() {
                " (no --route-nav-menus table)"
            } else {
                ""
            },
            steps.len()
        );
    } else {
        eprintln!(
            "route nav (T1640, FABRICATED input): {} step(s), mode {}, {} menu(s) from {}, hold {} gap {} polls",
            steps.len(),
            if mode == Mode::STRICT { "strict" } else { "on" },
            table.as_ref().map_or(0, |table| table.count),
            if !config.nav_menus_file.is_null() {
                lossy(config.nav_menus_file)
            } else if have_table {
                "(text)".to_string()
            } else {
                "(no table)".to_string()
            },
            timing.hold,
            timing.gap
        );
    }
    Ok(())
}

fn build_waits(config: &Config) -> Result<Vec<Wait>, String> {
    if config.wait_count + config.event_wait_count > MAX_WAITS || config.poke_count > POKE_MAX {
        return Err("too many --route-wait/--route-wait-event or --poke-at-poll specs".to_string());
    }
    let mut waits: Vec<Wait> = Vec::new();
    for spec in strings(config.waits, config.wait_count) {
        waits.push(wait_parse(text(spec), false)?);
    }
    for spec in strings(config.event_waits, config.event_wait_count) {
        waits.push(wait_parse(text(spec), true)?);
    }
    let command_line = waits.len();
    for (i, spec) in strings(config.record_waits, config.record_wait_count)
        .into_iter()
        .enumerate()
    {
        let wait = wait_parse(text(spec), true)
            .map_err(|reason| format!("record '# wait:' line {}: {reason}", i + 1))?;
        if waits[..command_line]
            .iter()
            .any(|other| other.mark == wait.mark)
        {
            eprintln!(
                "route: the record's wait for mark {} is replaced by the command line's",
                wait.mark
            );
            continue;
        }
        if waits.len() >= MAX_WAITS {
            return Err("too many waits (command line plus the record's)".to_string());
        }
        if waits.iter().any(|other| other.mark == wait.mark) {
            return Err(format!(
                "the record has two '# wait:' lines for mark{}",
                wait.mark
            ));
        }
        waits.push(wait);
    }
    Ok(waits)
}

fn setup_from(config: &Config, script: *const c_void, marks: &[u64]) -> Result<(), String> {
    let waits = build_waits(config)?;
    let mut pokes = PokeSet {
        entries: [PokeEntry::ZERO; POKE_MAX],
        count: 0,
    };
    for (i, spec) in strings(config.pokes, config.poke_count)
        .into_iter()
        .enumerate()
    {
        let entry = poke_parse(text(spec))?;
        if entry.where_ == PokeWhere::MARK && entry.value > marks.len() as u64 {
            return Err(format!(
                "--poke-at-poll names mark{} but the record has {} mark(s)",
                entry.value,
                marks.len()
            ));
        }
        pokes.entries[i] = entry;
        pokes.count = i as u32 + 1;
    }
    let probe = if config.probe.is_null() {
        // SAFETY: the host probe is a static view.
        unsafe {
            route_probe_enable();
            *route_probe_view()
        }
    } else {
        // SAFETY: a non-null probe is valid.
        unsafe { *config.probe }
    };
    let hooks = Hooks {
        read_mem: Some(mem_adapter),
        mem_user: std::ptr::null_mut(),
        live: config.live,
        live_user: config.live_user,
        on_event: Some(on_event),
        event_user: std::ptr::null_mut(),
        on_poll: Some(on_poll),
        poll_user: std::ptr::null_mut(),
        probe,
    };
    let mut error = [0 as c_char; 300];
    // SAFETY: counted arrays; `hooks` is valid for the call.
    let route = unsafe {
        ffi::xinput_route_create(
            script,
            marks.as_ptr(),
            marks.len(),
            waits.as_ptr(),
            waits.len(),
            &hooks,
            error.as_mut_ptr(),
            error.len(),
        )
    };
    if route.is_null() {
        return Err(lossy(error.as_ptr()));
    }
    if let Err(message) = setup_nav(route, config, script, marks) {
        // SAFETY: the route was just created.
        unsafe { ffi::xinput_route_free(route) };
        return Err(message);
    }
    host_route_teardown();
    let mut guard = state();
    guard.route = route;
    guard.config = *config;
    guard.pokes = pokes;
    drop(guard);
    // SAFETY: the route is live and owned by the state.
    unsafe { ffi::xinput_route_install(route) };
    Ok(())
}

/// # Safety
/// `config` is valid; `script`/`marks` (`mark_count`) are the loaded record; `error` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn host_route_setup_from(
    config: *const Config,
    script: *const c_void,
    marks: *const u64,
    mark_count: usize,
    error: *mut c_char,
    error_size: usize,
) -> bool {
    let marks = if mark_count == 0 || marks.is_null() {
        &[][..]
    } else {
        // SAFETY: counted array.
        unsafe { std::slice::from_raw_parts(marks, mark_count) }
    };
    // SAFETY: valid per the contract.
    match setup_from(unsafe { &*config }, script, marks) {
        Ok(()) => true,
        Err(message) => {
            write_error(error, error_size, &message);
            false
        }
    }
}

/// # Safety
/// `config` is valid; `error` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn host_route_setup(
    config: *const Config,
    error: *mut c_char,
    error_size: usize,
) -> bool {
    let mut marks: *const u64 = std::ptr::null();
    // SAFETY: the record reader owns the arrays it returns.
    let mark_count = unsafe { xinput_replay_marks(&mut marks) };
    let script = unsafe { xinput_replay_script() };
    if script.is_null() {
        write_error(error, error_size, "no replay is loaded");
        return false;
    }
    let mut merged = unsafe { *config };
    // SAFETY: indexed reads of the loaded record's lines.
    let waits: Vec<*const c_char> = (0..unsafe { xinput_replay_wait_count() }.min(MAX_WAITS))
        .map(|i| unsafe { xinput_replay_wait(i) })
        .collect();
    merged.record_waits = waits.as_ptr();
    merged.record_wait_count = waits.len();
    let navs: Vec<*const c_char> = (0..unsafe { xinput_replay_nav_count() }.min(MAX_NAV_STEPS))
        .map(|i| unsafe { xinput_replay_nav(i) })
        .collect();
    merged.record_navs = navs.as_ptr();
    merged.record_nav_count = navs.len();
    unsafe { host_route_setup_from(&merged, script, marks, mark_count, error, error_size) }
}

/// T3975 production contract. Route state and behavior are owned by this Rust module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_route_setup(
    config: *const Config,
    error: *mut c_char,
    error_size: usize,
) -> bool {
    // SAFETY: forwarded unchanged to the implementation with the same ABI contract.
    unsafe { host_route_setup(config, error, error_size) }
}

/// Decide whether route setup is requested from host options and baseline-reader metadata.
/// The reader remains the C compatibility baseline; route activation policy and installed route state live here.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_route_requested(
    replay: bool,
    handover: bool,
    wait_count: usize,
    event_wait_count: usize,
    poke_count: usize,
) -> bool {
    replay
        && (handover
            || wait_count != 0
            || event_wait_count != 0
            || poke_count != 0
            // SAFETY: the loaded baseline reader exposes immutable route metadata for the host session.
            || unsafe { xinput_replay_wait_count() } != 0
            // SAFETY: as above; replay metadata remains live for this host session.
            || unsafe { xinput_replay_nav_count() } != 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_route_recorded_wait_count() -> usize {
    // SAFETY: the loaded baseline reader exposes immutable route metadata for the host session.
    unsafe { xinput_replay_wait_count() }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_route_current() -> *mut Route {
    state().route
}

unsafe extern "C" fn recorder_read(
    address: u32,
    width: u32,
    value: *mut u32,
    user: *mut c_void,
) -> bool {
    // SAFETY: same contract as the replay adapter.
    unsafe { mem_adapter(address, width, value, user) }
}

unsafe extern "C" fn recorder_bytes(
    address: u32,
    buffer: *mut c_void,
    length: usize,
    user: *mut c_void,
) -> bool {
    let config = state().config;
    match config.read_bytes {
        // SAFETY: the test seam has the `read_bytes` contract.
        Some(read) => unsafe { read(address, buffer, length, config.test_user) },
        None => unsafe { bytes_adapter(address, buffer, length, user) },
    }
}

unsafe extern "C" fn recorder_now(_user: *mut c_void) -> u64 {
    0
}

unsafe extern "C" fn recorder_log(line: *const c_char, _user: *mut c_void) {
    probe_log(&lossy(line));
}

unsafe extern "C" fn recorder_add(
    at: u64,
    to: u64,
    edge: u64,
    body: *const c_char,
    analog: c_int,
    digital: u16,
    _user: *mut c_void,
) -> *const c_char {
    // SAFETY: forwards to the record's pending list.
    unsafe { xinput_record_add_nav(at, to, edge, body, analog, digital) }
}

unsafe extern "C" fn recorder_observe(poll: u64, recorded: *const PadState, user: *mut c_void) {
    unsafe { nav_recorder_observe(user, poll, recorded) };
}

unsafe extern "C" fn recorder_close(total: u64, user: *mut c_void) {
    unsafe { nav_recorder_close(user, total) };
}

#[unsafe(no_mangle)]
pub extern "C" fn host_route_nav_record_stop() {
    let recorder = std::mem::replace(&mut state().recorder, std::ptr::null_mut());
    if recorder.is_null() {
        return;
    }
    // SAFETY: the recorder was created by this module and is no longer referenced by the record.
    unsafe {
        xinput_record_set_nav_hooks(None, None, None, std::ptr::null_mut());
        nav_recorder_free(recorder);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_route_nav_record_stop() {
    host_route_nav_record_stop();
}

fn nav_record_start(config: &Config) -> Result<(), String> {
    host_route_nav_record_stop();
    // The read seams; a replay and a recording never run together.
    state().config = *config;
    let label = if config.nav_menus_file.is_null() {
        "(none)".to_string()
    } else {
        lossy(config.nav_menus_file)
    };
    let source = match text(config.nav_menus_text) {
        Some(inline) => inline.to_vec(),
        None if !config.nav_menus_file.is_null() => read_text_file(config.nav_menus_file)
            .ok_or_else(|| format!("--route-nav-menus {label}: cannot read the menu table"))?,
        None => {
            return Err(format!(
                "--route-nav-menus {label}: cannot read the menu table"
            ));
        }
    };
    let table = nav_spec::menus_parse(&source)
        .map_err(|reason| format!("--route-nav-menus {}: {reason}", menus_label(config)))?;
    if table.count == 0 {
        return Err(format!(
            "--route-nav-menus {}: the table has no menu",
            menus_label(config)
        ));
    }
    let hooks = RecordHooks {
        probe: NavProbe {
            read_mem: Some(recorder_read),
            read_bytes: Some(recorder_bytes),
            now_ms: Some(recorder_now),
            expect_arm: None,
            expect_holds: None,
            progress: None,
            user: std::ptr::null_mut(),
        },
        log: Some(recorder_log),
        log_user: std::ptr::null_mut(),
        add: Some(recorder_add),
        add_user: std::ptr::null_mut(),
    };
    // SAFETY: the recorder copies the table and hooks it needs.
    let recorder = unsafe { nav_recorder_create(&*table, &hooks) };
    if recorder.is_null() {
        return Err("out of memory".to_string());
    }
    state().recorder = recorder;
    // SAFETY: the recorder outlives the hooks (removed in `host_route_nav_record_stop`).
    unsafe {
        xinput_record_set_nav_hooks(
            Some(recorder_observe),
            Some(recorder_close),
            Some(recorder_log),
            recorder,
        );
    }
    Ok(())
}

/// # Safety
/// `config` is valid; `error` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn host_route_nav_record_start(
    config: *const Config,
    error: *mut c_char,
    error_size: usize,
) -> bool {
    // SAFETY: valid per the contract.
    match nav_record_start(unsafe { &*config }) {
        Ok(()) => true,
        Err(message) => {
            write_error(error, error_size, &message);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_route_nav_record_start(
    config: *const Config,
    error: *mut c_char,
    error_size: usize,
) -> bool {
    // SAFETY: forwarded unchanged to the implementation with the same ABI contract.
    unsafe { host_route_nav_record_start(config, error, error_size) }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_route_handed_over() -> bool {
    let route = state().route;
    // SAFETY: a non-null route is live until `host_route_teardown`.
    !route.is_null() && unsafe { ffi::xinput_route_ended(route) }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_route_handed_over() -> bool {
    host_route_handed_over()
}

#[unsafe(no_mangle)]
pub extern "C" fn host_route_teardown() {
    let route = std::mem::replace(&mut state().route, std::ptr::null_mut());
    if route.is_null() {
        return;
    }
    // SAFETY: the route is owned by this module and no longer installed after the first call.
    unsafe {
        crate::sys::xinput_source_install(None, std::ptr::null_mut());
        ffi::xinput_route_free(route);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_route_teardown() {
    host_route_teardown()
}

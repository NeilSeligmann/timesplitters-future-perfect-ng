// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use tsfp_t2_offline::spool::{Entry, Run, decode_regs, encode_record, encode_regs, encode_spool};

const VA: u32 = 0x12340;
const STACK: u32 = 0x0e00_f000;

fn input(arg: u32) -> Vec<u8> {
    let mut regs = decode_regs(&[0u8; spool::REGS_SIZE]);
    regs.esp = STACK;
    let mut stack = 0x1f00_u32.to_le_bytes().to_vec(); // return address tag
    stack.extend(arg.to_le_bytes());
    let entry = Entry {
        va: VA,
        caller_va: 0,
        return_address: 0x1f00,
        poll: 0,
        guest_thread: 0,
        flags: 0,
        entry_raw: encode_regs(&regs),
        entry_regs: regs,
        exit_regs: regs,
        read_runs: vec![Run {
            addr: STACK,
            data: stack,
        }],
        write_runs: Vec::new(),
        callees: Vec::new(),
        entry_implicit: Some([0; 8]),
        exit_implicit: None,
    };
    encode_record(&entry)
}

fn inputs(args: &[u32]) -> Vec<u8> {
    let records: Vec<Vec<u8>> = args.iter().map(|&arg| input(arg)).collect();
    encode_spool("{}", &records)
}

/// A stand-in original: reads its stack argument and stores twice it at 0x2000; an argument of 0 makes it
/// fault outside the guest arena (as a trap or a host-death path of a real original would).
struct Double {
    base: usize,
    implicit: Implicit,
    runs: u32,
}

impl Original for Double {
    fn bind(&mut self, base: usize) {
        self.base = base;
    }
    fn set_implicit(&mut self, implicit: &Implicit) {
        self.implicit = *implicit;
    }
    fn run(&mut self, va: u32, regs: &mut ExtRegisters) -> bool {
        if va != VA {
            return false;
        }
        self.runs += 1;
        unsafe {
            let arg = ((self.base + regs.esp as usize + 4) as *const u32).read_volatile();
            if arg == 0 {
                (16usize as *mut u32).write_volatile(1);
            }
            if arg == 7 {
                // An original that never returns on this input.
                while ((self.base + regs.esp as usize + 4) as *const u32).read_volatile() == 7 {}
            }
            ((self.base + 0x2000) as *mut u32).write_volatile(arg * 2);
        }
        regs.esp += 4;
        regs.eax = 1;
        true
    }
}

fn double() -> Double {
    Double {
        base: 0,
        implicit: [1; 8],
        runs: 0,
    }
}

fn run_text(data: &[u8], original: &mut Double) -> (Result<bool, String>, String) {
    let mut arena = Arena::reserve(GUEST_WINDOW).expect("reserve");
    let mut out = Vec::new();
    let result = run_inputs(data, VA, 50, &mut arena, original, &mut out);
    (result, String::from_utf8(out).unwrap())
}

#[test]
fn every_input_runs_the_original_and_reports_start_then_end() {
    let mut original = double();
    let (result, text) = run_text(&inputs(&[3, 5]), &mut original);
    assert_eq!(result, Ok(false));
    assert_eq!(original.runs, 2);
    assert_eq!(
        original.implicit, [0; 8],
        "the input's implicit state is loaded"
    );
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4, "{text}");
    assert_eq!(lines[0], "{\"index\": 0, \"event\": \"start\"}");
    assert!(lines[1].contains("\"end\": \"returned\""), "{text}");
    assert!(lines[1].contains("\"exit_esp\": \"0x0e00f004\""), "{text}");
    assert!(
        lines[3].starts_with("{\"index\": 1, \"event\": \"end\""),
        "{text}"
    );
}

#[test]
fn a_faulting_input_is_reported_and_stops_the_process_before_the_next_input() {
    let mut original = double();
    let (result, text) = run_text(&inputs(&[3, 0, 5]), &mut original);
    assert_eq!(result, Ok(true), "stopped on the fault");
    assert_eq!(
        original.runs, 2,
        "the input after the fault is left to a fresh process"
    );
    let last = text.lines().last().unwrap();
    assert!(
        last.starts_with("{\"index\": 1, \"event\": \"end\", \"end\": \"fault\""),
        "{text}"
    );
    assert!(last.contains("\"address\": \"0x10\""), "{text}");
}

#[test]
fn an_original_that_hangs_is_stopped_by_the_watchdog_and_reported_as_a_fault() {
    let mut original = double();
    let started = std::time::Instant::now();
    let (result, text) = run_text(&inputs(&[3, 7, 5]), &mut original);
    assert_eq!(result, Ok(true), "stopped on the watchdog fault");
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    let last = text.lines().last().unwrap();
    assert!(
        last.starts_with("{\"index\": 1, \"event\": \"end\", \"end\": \"fault\", \"signal\": 14"),
        "{text}"
    );
}

#[test]
fn an_input_for_another_function_or_an_unknown_function_is_a_harness_error() {
    let mut original = double();
    let mut arena = Arena::reserve(GUEST_WINDOW).expect("reserve");
    let mut out = Vec::new();
    let error = run_inputs(
        &inputs(&[3]),
        VA + 1,
        0,
        &mut arena,
        &mut original,
        &mut out,
    )
    .unwrap_err();
    assert!(error.contains("not 0x12341"), "{error}");
}

#[test]
fn arguments_follow_the_documented_contract() {
    let strings = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let args = parse_args(&strings(&[
        "--t2-construct",
        "i.bin",
        "--va",
        "0x2D1330",
        "--out",
        "r",
    ]))
    .unwrap();
    assert_eq!(
        args,
        Args {
            input: "i.bin".into(),
            va: 0x2d1330,
            out: "r".into(),
            input_ms: DEFAULT_INPUT_MS,
        }
    );
    assert!(parse_args(&strings(&["--t2-construct", "i.bin", "--out", "r"])).is_err());
    assert!(
        parse_args(&strings(&[
            "--t2-construct",
            "i",
            "--va",
            "1",
            "--out",
            "r",
            "--x",
            "y"
        ]))
        .is_err()
    );
}

#[test]
fn without_the_c_host_both_modes_refuse() {
    let strings = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(
        dispatch(&strings(&[
            "--t2-construct",
            "i",
            "--va",
            "1",
            "--out",
            "r"
        ])),
        ExitCode::from(2)
    );
    assert_eq!(
        dispatch(&strings(&["--t2-offline", "e", "--va", "1", "--out", "r"])),
        ExitCode::from(2)
    );
}

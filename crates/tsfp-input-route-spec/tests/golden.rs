// SPDX-License-Identifier: GPL-3.0-or-later
//! Replays `golden.tsv` (the C parser's output, regenerated and checked by tests/test_t3975_route_spec_golden.py)
//! through the Rust parser: every accepted spec must print the same fields, every rejected one the same message.
use tsfp_input_route_spec::cond::{self, Cond};
use tsfp_input_route_spec::{poke, wait_parse};

fn cond_line(c: &Cond) -> String {
    format!(
        "[{}|{}|{:x}|{:x}|{}|{}|{:x}|{:x}|{}|{:x}|{}|{}]",
        c.kind,
        u8::from(c.indirect),
        c.address,
        c.offset,
        c.width,
        c.cmp,
        c.value,
        c.mask,
        String::from_utf8_lossy(c.text_bytes()),
        c.va,
        c.amount,
        cond::format(c)
    )
}

fn run(mode: &str, spec: &str) -> String {
    match mode {
        "event" | "legacy" => match wait_parse(Some(spec.as_bytes()), mode == "event") {
            Err(message) => format!("ERR {message}"),
            Ok(w) => {
                let mut line = format!(
                    "OK mark={} any={} event={} minp={} minms={} maxp={} tmo={} mem={},{:x},{},{:x},{:x} conds={}",
                    w.mark,
                    u8::from(w.any),
                    u8::from(w.event),
                    w.min_polls,
                    w.min_ms,
                    w.max_polls,
                    w.timeout_ms,
                    u8::from(w.has_mem),
                    w.address,
                    w.width,
                    w.value,
                    w.mask,
                    w.cond_count
                );
                for c in &w.conds[..w.cond_count as usize] {
                    line.push(' ');
                    line.push_str(&cond_line(c));
                }
                line
            }
        },
        "mem" | "memref" => {
            let parsed = if mode == "mem" {
                cond::mem_event_condition(spec.as_bytes())
            } else {
                cond::mem_ref(spec.as_bytes())
            };
            parsed.map_or("ERR".to_string(), |c| format!("OK {}", cond_line(&c)))
        }
        "poke" => match poke::parse(Some(spec.as_bytes())) {
            Err(message) => format!("ERR {message}"),
            Ok(e) => {
                let end = e.label.iter().position(|&b| b == 0).unwrap();
                format!(
                    "OK where={} value={} label={}",
                    e.where_,
                    e.value,
                    String::from_utf8_lossy(&e.label[..end])
                )
            }
        },
        other => panic!("unknown mode {other}"),
    }
}

#[test]
fn rust_parser_equals_the_c_golden() {
    let golden = include_str!("golden.tsv");
    let mut checked = 0;
    let mut failures = Vec::new();
    for line in golden.lines() {
        let mut parts = line.splitn(3, '\t');
        let (mode, spec, expected) = (
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap(),
        );
        let got = run(mode, spec);
        if got != expected {
            failures.push(format!(
                "{mode} {spec:?}\n  C:    {expected}\n  Rust: {got}"
            ));
        }
        checked += 1;
    }
    assert!(checked > 4000, "golden file too small: {checked}");
    assert!(
        failures.is_empty(),
        "{} of {checked} differ:\n{}",
        failures.len(),
        failures[..failures.len().min(8)].join("\n")
    );
}

//! T1868: the particle template definers (`0xDD1B0`, `0x207740`, `0x1F1260`) rebuilt from a caller supplied XBE image by
//! the ported setters and the definer interpreter, against emulated-original golden vectors (hash of the whole template region
//! plus the target records). The image is read from `TSFP_XBE` or `../../build/default.xbe`; the tests skip when it is absent.
//! No game bytes are committed. Golden: `tests/data/t1868/template_*.golden` (`tools/t1846_gold/svc_template_defs.py`).
mod common;
use common::parse;
use std::path::PathBuf;
use tsfp_gameplay::story_events::svc_template_defs::*;
use tsfp_gameplay::story_events::svc_wiring::GuestRam;

const REGION: u32 = 0x72_F000;
const REGION_WORDS: u32 = 0x82000 / 4;

fn real_image() -> Option<Vec<u8>> {
    let path = std::env::var_os("TSFP_XBE")
        .map_or_else(|| PathBuf::from("../../build/default.xbe"), PathBuf::from);
    std::fs::read(path).ok()
}

fn fnv(ram: &GuestRam) -> (u32, u32) {
    let (mut digest, mut nonzero) = (0x811C_9DC5u32, 0);
    for k in 0..REGION_WORDS {
        let word = ram.word(REGION + 4 * k);
        nonzero += u32::from(word != 0);
        for byte in word.to_le_bytes() {
            digest = (digest ^ u32::from(byte)).wrapping_mul(0x0100_0193);
        }
    }
    (digest, nonzero)
}

fn check(name: &str, golden: &str, entry: u32, expect_skipped: usize) {
    let Some(image) = real_image() else {
        eprintln!("skip {name}: no XBE image (set TSFP_XBE)");
        return;
    };
    let cases = parse(golden);
    let case = &cases[0];
    let mut ram = GuestRam::default();
    let report =
        run_definer(&image, &mut ram, entry, None).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    assert_eq!(report.skipped.len(), expect_skipped, "{name} skipped calls");
    assert_eq!(
        report.skipped.len(),
        case.calls.len(),
        "{name} stubbed calls"
    );
    for (skipped, call) in report.skipped.iter().zip(&case.calls) {
        assert_eq!(skipped.0, call.callee, "{name} call target");
        let n = call.args.len().min(3); // args 3 and 4 are pointers to stack locals (the stack bases differ)
        assert_eq!(
            skipped.1[..n],
            call.args[..n],
            "{name} call args {:#x}",
            skipped.0
        );
    }
    for (address, want) in &case.outs {
        if *address >= 0xF000_0000 {
            continue;
        }
        assert_eq!(ram.word(*address), *want, "{name} word at {address:#x}");
    }
    let (digest, nonzero) = fnv(&ram);
    assert_eq!(
        [digest, nonzero],
        [case.output(0xF000_0000), case.output(0xF000_0004)],
        "{name} region hash"
    );
    assert!(report.setters > 5, "{name} ran {} setters", report.setters);
    eprintln!(
        "{name}: {} steps, {} setters, {} skipped calls, {nonzero} non zero words",
        report.steps,
        report.setters,
        report.skipped.len()
    );
}

#[test]
fn boot_effect_table_defines_the_scratch_and_ember_templates() {
    check(
        "boot",
        include_str!("data/t1868/template_boot.golden"),
        BOOT_EFFECTS,
        0,
    );
}

#[test]
fn level_11_table_particle_template() {
    check(
        "level11",
        include_str!("data/t1868/template_level11.golden"),
        LEVEL11_TABLE,
        0,
    );
}

#[test]
fn barrier_init_defines_the_barrier_template_and_skips_its_spawns() {
    check(
        "barrier",
        include_str!("data/t1868/template_barrier.golden"),
        BARRIER_INIT,
        12,
    );
}

#[test]
fn definers_reject_unmodelled_code_and_missing_images() {
    let mut ram = GuestRam::default();
    assert!(run_definer(&[0u8; 16], &mut ram, 0x1000, None).is_err());
    assert!(!apply_setter(&mut ram, 0x1234, &|_| 0));
}

#[test]
fn setters_write_the_documented_fields_on_synthetic_memory() {
    let mut ram = GuestRam::default();
    let sprite = 0x9000_0000u32;
    let words = [sprite, 0x1111, 7, 1, 3.0f32.to_bits(), 5, 0.01f32.to_bits()];
    assert!(apply_setter(&mut ram, 0x1113E0, &|n| words[n as usize]));
    assert_eq!(ram.word(CURRENT_SPRITE), sprite);
    assert_eq!(ram.word(sprite + 0x4C), 0x1111, "spec pointer");
    assert_eq!(ram.word(sprite), 7);
    assert_eq!(ram.word(sprite + 4), 1);
    assert_eq!(ram.word(sprite + 0x20), 3.0f32.to_bits());
    assert_eq!(ram.word(sprite + 0x30), 0.01f32.to_bits());
    assert_eq!(ram.word(sprite + 0x54), u32::MAX);
    assert_eq!(ram.word(sprite + 0x58), 1.0f32.to_bits());
    // family B gradient: positive first colour stop
    let spec = 0x9100_0000u32;
    assert!(apply_setter(&mut ram, 0x1115F0, &|n| [
        spec,
        0,
        0,
        1,
        1.0f32.to_bits(),
        0x329,
        0,
        1.0f32.to_bits()
    ][n as usize]));
    assert!(apply_setter(&mut ram, 0x111680, &|n| [
        1,
        2,
        3,
        4,
        2.0f32.to_bits(),
        0.5f32.to_bits(),
        0.25f32.to_bits()
    ][n as usize]));
    assert_eq!(ram.word(spec + 0x34), 4, "three stops plus the terminator");
    assert_eq!(ram.word(spec + 0x38), 2.0f32.to_bits());
    assert_eq!(ram.word(spec + 0x40), 0.5f32.to_bits());
}

#[test]
fn loaded_templates_feed_the_emitter_creation_of_the_cell() {
    use tsfp_gameplay::story_events::svc_wiring::HookServices;
    let Some(image) = real_image() else {
        eprintln!("skip: no XBE image (set TSFP_XBE)");
        return;
    };
    let mut services = HookServices::new(1);
    // before loading, the zero template creates an emitter with zero flags
    assert_eq!(
        services.create_emitter(0x78_DBA0, 0, [1.0, 2.0, 3.0], [0.0, 1.0, 0.0]),
        1
    );
    let before = services.ram.borrow().word(0x7A_D28C);
    assert_eq!(
        services.ram.borrow().word(before + 4) & 0xFFF,
        0,
        "zero template flags"
    );
    let reports = services.load_templates(&image).expect("definers run");
    assert_eq!(reports.len(), 3);
    for (template, _) in TARGETS {
        assert_ne!(
            services.ram.borrow().word(template + 4),
            0,
            "template {template:#x} flags defined"
        );
    }
    for template in [0x78_DBA0u32, 0x78_ED40, 0x7B_0A80, 0x73_5FC8, 0x73_5DE8] {
        assert_eq!(
            services.create_emitter(template, 0, [1.0, 2.0, 3.0], [0.0, 1.0, 0.0]),
            1,
            "{template:#x}"
        );
        let emitter = services.ram.borrow().word(0x7A_D28C);
        let want = services.ram.borrow().word(template + 4);
        assert_eq!(
            services.ram.borrow().word(emitter + 0xC),
            template,
            "emitter points at its template"
        );
        let got = services.ram.borrow().word(emitter + 4);
        eprintln!("template {template:#x} flags {want:#x} emitter flags {got:#x}");
        assert_eq!(
            got & 0xFFF,
            want & 0xFFF,
            "emitter flags come from the loaded template"
        );
    }
}

#[test]
fn effect_definition_table_has_nineteen_entries_and_five_are_defined() {
    use tsfp_gameplay::story_events::svc_kind_table::{
        EFFECT_ENTRIES, load_effect_defined, load_kind_rows,
    };
    let Some(image) = real_image() else {
        eprintln!("skip: no XBE image (set TSFP_XBE)");
        return;
    };
    let defined = load_effect_defined(&image).expect("table mapped");
    let ids: Vec<usize> = (0..EFFECT_ENTRIES).filter(|&id| defined[id]).collect();
    assert_eq!(ids, vec![7, 8, 9, 10, 11], "statically defined effect ids");
    // every id a kind row references is one of the 19 entries (-1 = none)
    for row in load_kind_rows(&image).expect("rows mapped") {
        for id in [row.effect_a, row.effect_b] {
            assert!(
                id == -1 || (0..EFFECT_ENTRIES as i32).contains(&id),
                "row effect id {id}"
            );
        }
    }
}

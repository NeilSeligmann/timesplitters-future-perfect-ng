//! T1856: particle and sound request services against emulated-original golden vectors (MEASURED), plus unit tests.
mod common;
use common::{Case, parse};
use std::collections::HashMap;
use tsfp_gameplay::story_events::strip_update::{GuestMemory, Service, StripServices};
use tsfp_gameplay::story_events::svc_particles::*;

const ARGS: u32 = 0x1F0_0000;

#[derive(Default)]
struct Memory(HashMap<u32, u8>);

impl Memory {
    fn from_case(case: &Case) -> Self {
        let mut memory = Memory::default();
        for (&address, &word) in &case.ins {
            memory.write_u32(address, word);
        }
        memory
    }
}

impl GuestMemory for Memory {
    fn read_u8(&self, address: u32) -> u8 {
        *self.0.get(&address).unwrap_or(&0)
    }
    fn write_u8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
    fn read_u32(&self, address: u32) -> u32 {
        u32::from_le_bytes([0, 1, 2, 3].map(|i| self.read_u8(address.wrapping_add(i))))
    }
    fn write_u32(&mut self, address: u32, value: u32) {
        for (i, byte) in value.to_le_bytes().iter().enumerate() {
            self.0.insert(address.wrapping_add(i as u32), *byte);
        }
    }
}

fn check_outputs(case: &Case, memory: &Memory, n: usize) {
    assert!(!case.outs.is_empty());
    for (&address, &value) in &case.outs {
        assert_eq!(
            memory.read_u32(address),
            value,
            "case {n} memory {address:#x}"
        );
    }
}

#[derive(Default)]
struct Recorder {
    records: Vec<[u32; 12]>,
    handle: u32,
}

impl EffectSink for Recorder {
    fn submit(&mut self, record: &[u32; 12]) -> u32 {
        self.records.push(*record);
        self.handle
    }
}

#[test]
fn emitter_offset_matches_original() {
    let cases = parse(include_str!("data/t1856/particles_emitter.golden"));
    assert!(cases.len() >= 60);
    let (mut packed, mut floats) = (0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut memory = Memory::from_case(case);
        let table = 0x1C0_0000;
        if case.input(table + 0xC) & 0x10 != 0 {
            packed += 1;
        } else {
            floats += 1;
        }
        emitter_offset(
            &mut memory,
            0x1C4_0000,
            table,
            0x1C1_0000,
            0x1C2_0000,
            case.input(ARGS),
        );
        check_outputs(case, &memory, n);
    }
    assert!(packed > 10 && floats > 10);
}

#[test]
fn spawn_particle_matches_original() {
    let cases = parse(include_str!("data/t1856/particles_spawn.golden"));
    assert!(cases.len() >= 60);
    let mut wrapped = 0;
    for (n, case) in cases.iter().enumerate() {
        let mut memory = Memory::from_case(case);
        spawn_particle(&mut memory, 0x1D0_0000, 0x1D0_0020, case.input(ARGS));
        check_outputs(case, &memory, n);
        if memory.read_u32(PARTICLE_INDEX) == 0 {
            wrapped += 1;
        }
    }
    assert!(wrapped > 5, "wrap branch");
}

#[test]
fn box_overlap_matches_original() {
    let cases = parse(include_str!("data/t1856/particles_overlap.golden"));
    assert!(cases.len() >= 60);
    let (mut yes, mut no) = (0, 0);
    for (n, case) in cases.iter().enumerate() {
        let base = 0x1E0_0000;
        let got = boxes_overlap(
            case.vec3(base),
            case.vec3(base + 0x20),
            case.vec3(base + 0x40),
            case.vec3(base + 0x60),
        );
        assert_eq!(u32::from(got), case.eax, "case {n}");
        if got {
            yes += 1;
        } else {
            no += 1;
        }
    }
    assert!(yes > 10 && no > 10);
}

#[test]
fn region_query_matches_original() {
    let cases = parse(include_str!("data/t1856/particles_region.golden"));
    assert!(cases.len() >= 60);
    let (mut full, mut empty, mut null, mut limited) = (0, 0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut memory = Memory::from_case(case);
        let ring_len = case.input(ARGS) as usize;
        let mut ring = vec![0u32; ring_len];
        let ring_base = 0x1A5_0000;
        for (k, slot) in ring.iter_mut().enumerate() {
            *slot = memory.read_u32(ring_base + 4 * k as u32);
        }
        let count = region_query_checked(&mut memory, 0x1A4_0000, &mut ring);
        for (k, slot) in ring.iter().enumerate() {
            memory.write_u32(ring_base + 4 * k as u32, *slot);
        }
        assert_eq!(count, case.eax, "case {n} count");
        check_outputs(case, &memory, n);
        match (case.input(REGION_TABLE) == 0, count) {
            (true, _) => null += 1,
            (_, 0) => empty += 1,
            _ => full += 1,
        }
        if count as usize == ring_len {
            limited += 1;
        }
    }
    assert!(
        full > 10 && empty > 3 && null > 3 && limited > 3,
        "coverage {full} {empty} {null} {limited}"
    );
}

#[test]
fn refresh_regions_matches_original() {
    let cases = parse(include_str!("data/t1856/particles_refresh.golden"));
    assert!(cases.len() >= 60);
    let (mut def_path, mut box_path, mut hits) = (0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut memory = Memory::from_case(case);
        let object = 0x1B0_0000;
        refresh_regions(&mut memory, object);
        check_outputs(case, &memory, n);
        let flags = case.input(object + 4);
        if case.input(object + 0x88) != 0 && !(flags & 0x2_0000 != 0 && flags & 0x1000_0000 != 0) {
            def_path += 1;
        } else {
            box_path += 1;
            if memory.read_u32(object + 0xE0) as i32 > 0 {
                hits += 1;
            }
        }
    }
    assert!(
        def_path > 10 && box_path > 10 && hits > 5,
        "coverage {def_path} {box_path} {hits}"
    );
}

fn wrapper_check(golden: &str, run: impl Fn(&Memory, &mut Recorder, &[u32]) -> u32) {
    let cases = parse(golden);
    assert!(cases.len() >= 60);
    for (n, case) in cases.iter().enumerate() {
        let memory = Memory::from_case(case);
        let args: Vec<u32> = (0..3).map(|i| case.input(ARGS + 4 * i)).collect();
        let mut sink = Recorder {
            handle: case.eax,
            ..Recorder::default()
        };
        let result = run(&memory, &mut sink, &args);
        assert_eq!(result, case.eax, "case {n} handle");
        assert_eq!(case.calls.len(), 1, "case {n}");
        assert_eq!(case.calls[0].callee, 0x45820);
        assert_eq!(sink.records.len(), 1);
        assert_eq!(
            &case.calls[0].args[1..13],
            &sink.records[0][..],
            "case {n} record"
        );
    }
}

#[test]
fn start_effect_matches_original() {
    wrapper_check(
        include_str!("data/t1856/particles_217b0.golden"),
        |memory, sink, args| start_effect(memory, sink, args[0], args[1]),
    );
}

#[test]
fn sound_one_matches_original() {
    wrapper_check(
        include_str!("data/t1856/particles_218a0.golden"),
        |_, sink, args| sound_1(sink, args[0], args[1]),
    );
}

#[test]
fn sound_two_matches_original() {
    wrapper_check(
        include_str!("data/t1856/particles_21860.golden"),
        |_, sink, args| sound_2(sink, args[0], args[1], args[2]),
    );
}

struct Passive;
impl StripServices for Passive {
    fn call(&mut self, _: &mut dyn GuestMemory, service: Service, _: &[u32]) -> u32 {
        if service == Service::Destroy { 7 } else { 0 }
    }
}

#[test]
fn strip_adapter_maps_services() {
    let mut memory = Memory::default();
    memory.write_u32(0x2000, 1.5f32.to_bits());
    let mut sink = Recorder {
        handle: 0x99,
        ..Recorder::default()
    };
    let mut inner = Passive;
    let mut services = ParticleStripServices {
        sink: &mut sink,
        inner: &mut inner,
    };
    assert_eq!(
        services.call(&mut memory, Service::StartEffect, &[0x3A4, 0x2000]),
        0x99
    );
    assert_eq!(services.call(&mut memory, Service::Destroy, &[1]), 7);
    services.call(
        &mut memory,
        Service::SpawnParticle,
        &[0x2000, 0x2000, 0x3DCC_CCCD],
    );
    assert_eq!(memory.read_u32(PARTICLE_RING), 3);
    assert_eq!(memory.read_u32(PARTICLE_INDEX), 1);
    assert_eq!(sink.records[0][0], 0x3A4);
    assert_eq!(sink.records[0][7], 1.5f32.to_bits());
    assert_eq!(sink.records[0][10], 1);
}

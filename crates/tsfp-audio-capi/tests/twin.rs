//! Compare twin against a stand-in C side (the same Rust model run in a second world with other heap addresses), so the
//! shadow overlay, the alias translation, the snapshot flattening and the fault injection are exercised without the C host.
//! The live check against the real C host is docs/t-audio-replace.md "Live compare mode". A stand-in made of the same code can
//! only find plumbing errors, never model errors: that is what the live run is for.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tsfp_audio::adapter::GuestMemory;
use tsfp_audio::frames::{self, stream_frame};
use tsfp_audio::lifecycle::*;
use tsfp_audio::routing::StreamHooks;
use tsfp_audio::setters::{NoHooks, Policy, Setters};
use tsfp_audio_capi::compare::*;
use tsfp_audio_capi::world::{Host, Shadow};

const OUT: u32 = 0x0010_0100;
const DESC: u32 = 0x0010_0200;
const FMT: u32 = 0x0010_0300;
const C_STACK: u32 = 0x0010_0900;
const SLOT: u32 = 0x0058_18E8;

/// Title image of the stand-in: mapped zero bytes.
struct Image(HashMap<u32, u8>);
impl Host for Image {
    fn read(&self, a: u32, out: &mut [u8]) -> bool {
        for (i, b) in out.iter_mut().enumerate() {
            match self.0.get(&(a + i as u32)) {
                Some(v) => *b = *v,
                None => return false,
            }
        }
        true
    }
    fn readable(&self, a: u32, len: u32) -> bool {
        (0..len).all(|i| self.0.contains_key(&(a + i)))
    }
    fn irql0(&self) -> bool {
        true
    }
    fn codec_ready(&self) -> bool {
        true
    }
    fn stream_snapshot(&self, _: u32) -> Option<Vec<u32>> {
        None
    }
    fn buffer_snapshot(&self, _: u32) -> Option<Vec<u32>> {
        None
    }
}
fn image() -> Image {
    let mut m = HashMap::new();
    for (a, n) in [
        (SINGLETON, 4),
        (DEVICE_VTABLE, 12),
        (STREAM_SECONDARY, 60),
        (BUFFER_VTABLE, 16),
        (0x0010_0000, 0x1000),
        (GLOBAL_STATE, 4),
        (SLOT, 160),
    ] {
        for i in 0..n {
            m.insert(a + i, 0);
        }
    }
    for i in 0..15u32 {
        let target = STREAM_STOPPED_TARGETS[(i % 14) as usize].to_le_bytes();
        for (j, byte) in target.iter().enumerate() {
            m.insert(STREAM_SECONDARY + i * 4 + j as u32, *byte);
        }
    }
    Image(m)
}

/// The stand-in C host.
struct CSide {
    w: Shadow<Image>,
    d: Device,
    objs: Objects,
    s: Setters,
}
struct Fake(Rc<RefCell<CSide>>);
impl Host for Fake {
    fn read(&self, a: u32, out: &mut [u8]) -> bool {
        match self.0.borrow().w.read_bytes(a, out.len()) {
            Some(b) => {
                out.copy_from_slice(&b);
                true
            }
            None => false,
        }
    }
    fn readable(&self, a: u32, len: u32) -> bool {
        self.0.borrow().w.read_bytes(a, len as usize).is_some()
    }
    fn irql0(&self) -> bool {
        true
    }
    fn codec_ready(&self) -> bool {
        true
    }
    fn stream_snapshot(&self, st: u32) -> Option<Vec<u32>> {
        let c = self.0.borrow();
        stream_snapshot_words(&c.s, &c.objs, st)
    }
    fn buffer_snapshot(&self, b: u32) -> Option<Vec<u32>> {
        let c = self.0.borrow();
        buffer_snapshot_words(&c.s, &c.objs, b)
    }
}

struct NoStream;
impl StreamHooks for NoStream {}

struct Rig {
    c: Rc<RefCell<CSide>>,
    twin: Twin<Fake>,
}

fn words(w: &[u32]) -> Vec<u8> {
    w.iter().flat_map(|x| x.to_le_bytes()).collect()
}
fn stream_format() -> Vec<u8> {
    let mut f = vec![0u8; 20];
    f[0..2].copy_from_slice(&0x69u16.to_le_bytes());
    f[2..4].copy_from_slice(&2u16.to_le_bytes());
    f[4..8].copy_from_slice(&44100u32.to_le_bytes());
    f[8..12].copy_from_slice(&((72u32 * 44100) >> 6).to_le_bytes());
    f[12..14].copy_from_slice(&72u16.to_le_bytes());
    f[14..16].copy_from_slice(&4u16.to_le_bytes());
    f[16..18].copy_from_slice(&2u16.to_le_bytes());
    f[18..20].copy_from_slice(&64u16.to_le_bytes());
    f
}
fn mono_format() -> Vec<u8> {
    let mut f = vec![0u8; 20];
    f[0..2].copy_from_slice(&0x69u16.to_le_bytes());
    f[2..4].copy_from_slice(&1u16.to_le_bytes());
    f[4..8].copy_from_slice(&44000u32.to_le_bytes());
    f[8..12].copy_from_slice(&((36u32 * 44000) >> 6).to_le_bytes());
    f[12..14].copy_from_slice(&36u16.to_le_bytes());
    f[14..16].copy_from_slice(&4u16.to_le_bytes());
    f[16..18].copy_from_slice(&2u16.to_le_bytes());
    f[18..20].copy_from_slice(&64u16.to_le_bytes());
    f
}

impl Rig {
    fn new() -> Self {
        let c = Rc::new(RefCell::new(CSide {
            w: Shadow::new(image(), 0xE000_0000),
            d: Device::new(),
            objs: Objects::new(),
            s: Setters::new(Policy {
                enabled: true,
                completion: false,
                irql0: true,
            }),
        }));
        let cfg = Config {
            streams_enabled: true,
            buffers_enabled: true,
            ..Config::default()
        };
        let twin = Twin::with_heap_base(Fake(c.clone()), cfg, 0xF100_0000);
        Self { c, twin }
    }
    fn put(&self, a: u32, b: &[u8]) {
        assert!(self.c.borrow_mut().w.write_bytes(a, b));
    }
    fn create_device(&mut self) {
        let r = {
            let mut c = self.c.borrow_mut();
            let CSide { w, d, .. } = &mut *c;
            d.create(w, 0, OUT, 0, false, true).unwrap()
        };
        self.twin
            .observe(DS_CREATE, &[0x279D5, 0, OUT, 0], r, &[], 0);
    }
    fn stream_call(&mut self, entry: u32, caller: u32, args: &[u32]) -> u32 {
        let mut stack = vec![caller];
        stack.extend_from_slice(args);
        self.put(C_STACK, &words(&stack));
        let mut notes = Vec::new();
        let r = {
            let mut c = self.c.borrow_mut();
            let CSide { w, d, objs, s } = &mut *c;
            let mut note = |st: u32, mode: u32| notes.push((st, mode));
            stream_frame(s, objs, d, w, &mut NoStream, entry, C_STACK, &mut note).unwrap()
        };
        let recs: Vec<Rec> = notes
            .into_iter()
            .map(|(st, mode)| Rec {
                kind: REC_STREAM_PAUSE,
                a: [st, mode, 0, 0, 0],
                serial: 0,
                result: true,
                blob: [0; 40],
            })
            .collect();
        self.twin.observe(entry, &stack, r, &recs, 0);
        r
    }
    fn create_stream(&mut self) -> u32 {
        self.put(DESC, &words(&[0, 3, FMT, 0, 0, 0]));
        self.put(FMT, &stream_format());
        let out = OUT + 8;
        assert_eq!(self.stream_call(frames::CREATE, 0x29943, &[DESC, out]), 0);
        self.c.borrow().w.read_u32(out).unwrap()
    }
    fn create_buffer(&mut self) -> u32 {
        self.put(DESC, &words(&[24, 0, 0, FMT, 0, 0]));
        self.put(FMT, &mono_format());
        let (iface, out) = (self.c.borrow().d.owned_address() + 8, SLOT);
        let r = {
            let mut c = self.c.borrow_mut();
            let CSide { w, d, objs, s } = &mut *c;
            s.create_buffer(objs, d, w, iface, DESC, out, 0).unwrap()
        };
        self.twin
            .observe(B_CREATE, &[0x27B54, iface, DESC, out, 0], r, &[], 0);
        assert_eq!(r, 0);
        self.c.borrow().w.read_u32(out).unwrap()
    }
    fn buffer_set_data(&mut self, b: u32) {
        let r = {
            let mut c = self.c.borrow_mut();
            let CSide { w, d, objs, s } = &mut *c;
            s.buffer_set_data(objs, d, &*w, b, 0x0010_0800, 72).unwrap()
        };
        self.twin
            .observe(B_SET_DATA, &[0x28000, b, 0x0010_0800, 72], r, &[], 0);
    }
    fn buffer_set_volume(&mut self, b: u32, volume: u32) {
        let r = {
            let mut c = self.c.borrow_mut();
            let CSide { w, d, objs, s } = &mut *c;
            s.buffer_volume(objs, d, &*w, b, volume, &mut NoHooks)
                .unwrap()
        };
        self.twin
            .observe(B_SET_VOLUME, &[0x28348, b, volume], r, &[], 0);
    }
}

fn session(rig: &mut Rig) {
    rig.create_device();
    rig.create_device(); // cached path: reference count 7
    let st = rig.create_stream();
    rig.stream_call(frames::VOLUME, 0x29F68, &[st, 0xFFFF_D8F0]);
    rig.stream_call(frames::PAUSE, 0x29B5A, &[st, 1]);
    rig.stream_call(frames::FLUSH_EX, 0x29B69, &[st, 0, 0, 1]);
    let b = rig.create_buffer();
    rig.buffer_set_data(b);
    rig.buffer_set_volume(b, 0xFFFF_D8F0);
    // a movie-style foreign stream pointer is skipped, not a mismatch
    rig.twin
        .observe(frames::VOLUME, &[0x29F68, 0xDEAD_0000, 0], 0, &[], 0);
}

#[cfg(not(feature = "fault-inject"))]
#[test]
fn twin_agrees_with_a_matching_c_side() {
    let mut rig = Rig::new();
    session(&mut rig);
    assert_eq!(rig.twin.total_mismatches, 0, "{:?}", rig.twin.log);
    let checks: u64 = rig.twin.stats.values().map(|s| s.checks).sum();
    assert!(
        checks >= 40,
        "only {checks} checks ran: a compare that checks nothing agrees with anything"
    );
    assert_eq!(rig.twin.stats[&frames::VOLUME].foreign, 1);
    assert_eq!(rig.twin.stats[&B_SET_VOLUME].compared, 1);
    assert_eq!(rig.twin.stats[&B_SET_DATA].compared, 1);
    assert!(rig.twin.summary().len() > 5);
}

#[cfg(not(feature = "fault-inject"))]
#[test]
fn twin_catches_a_wrong_c_result_and_a_wrong_c_state() {
    let mut rig = Rig::new();
    rig.create_device();
    let st = rig.create_stream();
    // the C returned a wrong code
    rig.twin
        .observe(frames::PAUSE, &[0x29B5A, st, 1], 0x8000_4005, &[], 0);
    assert!(
        rig.twin.total_mismatches >= 1 && rig.twin.log[0].contains("return C"),
        "{:?}",
        rig.twin.log
    );
    // the C state diverges silently (a Pause the twin never saw): the next compare of the object catches it
    rig.stream_call(frames::VOLUME, 0x29F68, &[st, 0xFFFF_D8F0]);
    let before = rig.twin.total_mismatches;
    {
        let mut c = rig.c.borrow_mut();
        let CSide { w, d, objs, s } = &mut *c;
        s.stream_pause(objs, d, &*w, st, 1, false, &mut |_, _| {})
            .unwrap();
    }
    rig.twin
        .observe(frames::FLUSH_EX, &[0x29B69, st, 0, 0, 1], 0, &[], 0);
    assert!(rig.twin.total_mismatches > before, "{:?}", rig.twin.log);
}

#[cfg(feature = "fault-inject")]
#[test]
fn every_fault_mode_is_detected() {
    for fault in [
        Fault::Ret,
        Fault::State,
        Fault::Refcount,
        Fault::Header,
        Fault::Hook,
    ] {
        let mut rig = Rig::new();
        rig.twin.fault = Some(fault);
        rig.twin.log_limit = 0;
        session(&mut rig);
        if fault == Fault::Hook {
            // no hooks in this session: the hook fault flips nothing, the others must show
            assert_eq!(rig.twin.total_mismatches, 0);
        } else {
            assert!(rig.twin.total_mismatches > 0, "{fault:?} not detected");
        }
    }
}

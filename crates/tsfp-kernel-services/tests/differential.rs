// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(feature = "c-oracle")]
use tsfp_kernel_services::{
    identity::{ConsoleIdentity, DiskIdentity},
    settings::UnknownPolicy,
    *,
};

unsafe extern "C" {
    fn oracle_reset();
    fn oracle_stored(args: *const u32, count: u32) -> u32;
    fn oracle_memory(bytes: *const u8);
    fn oracle_snapshot(bytes: *mut u8);
    fn oracle_limits(read: u32, write: u32);
    fn oracle_call(ordinal: u32, args: *const u32, count: u32) -> u32;
    fn oracle_config_set(index: u32, bytes: *const u8, count: u32) -> u32;
    fn oracle_config_policy(policy: u32);
    fn oracle_eeprom(bytes: *const u8, key: *const u8) -> u32;
    fn oracle_av(pack: u32, region: u32, widescreen: u32) -> u32;
    fn oracle_av_install() -> u32;
    fn oracle_identity(hardware: u32, version: u32) -> u32;
    fn oracle_disk(
        model: u32,
        serial: u32,
        storage: u32,
        m: *const u8,
        nm: u32,
        s: *const u8,
        ns: u32,
    ) -> u32;
    fn oracle_counter(id: u32) -> u32;
    fn oracle_debug(bytes: *mut u8) -> u32;
    fn oracle_absorbed() -> u64;
    fn oracle_queried(out: *mut u32) -> u32;
    fn oracle_key(ordinal: u32, bytes: *mut u8) -> u32;
    fn oracle_disk_read(model: u32, serial: u32, bytes: *mut u8) -> u32;
    fn oracle_av_image(pack: u32, base: u32) -> u32;
}
#[derive(Clone)]
struct Memory {
    bytes: [u8; 8192],
    read_limit: usize,
    write_limit: usize,
}
impl Default for Memory {
    fn default() -> Self {
        Self {
            bytes: [0; 8192],
            read_limit: 8192,
            write_limit: 8192,
        }
    }
}
impl GuestMemory for Memory {
    fn readable(&self, address: u32, length: usize) -> bool {
        address != 0
            && (address as usize)
                .checked_add(length)
                .is_some_and(|end| end <= self.read_limit)
    }
    fn read(&self, address: u32, output: &mut [u8]) -> bool {
        if !self.readable(address, output.len()) {
            return false;
        }
        output.copy_from_slice(&self.bytes[address as usize..address as usize + output.len()]);
        true
    }
    fn write(&mut self, address: u32, input: &[u8]) -> bool {
        let address = address as usize;
        if address == 0
            || address
                .checked_add(input.len())
                .is_none_or(|end| end > self.bytes.len())
        {
            return false;
        }
        let n = input.len().min(self.write_limit.saturating_sub(address));
        self.bytes[address..address + n].copy_from_slice(&input[..n]);
        n == input.len()
    }
}
struct Harness {
    rust: RustKernelServices,
    memory: Memory,
    calls: usize,
}
impl Harness {
    fn new() -> Self {
        // All oracle use is serialized in the single differential test.
        unsafe {
            oracle_reset();
        }
        Self {
            rust: RustKernelServices::default(),
            memory: Memory::default(),
            calls: 0,
        }
    }
    fn call(&mut self, ordinal: u32, args: &[u32]) -> Answer {
        // SAFETY: buffers have the exact adapter extents, args remains live for the synchronous call.
        let c = unsafe {
            oracle_memory(self.memory.bytes.as_ptr());
            oracle_limits(
                self.memory.read_limit as u32,
                self.memory.write_limit as u32,
            );
            oracle_call(ordinal, args.as_ptr(), args.len() as u32)
        };
        let answer = self.rust.call(ordinal, args, 1, &mut self.memory).unwrap();
        assert_eq!(answer.eax, c, "ordinal {ordinal}, args {args:?}");
        let mut actual = [0; 8192];
        unsafe {
            oracle_snapshot(actual.as_mut_ptr());
        }
        // Avoid printing complete memory snapshots (could be a future private fixture).
        assert!(
            actual == self.memory.bytes,
            "memory mismatch ordinal {ordinal}, first offset {:?}",
            actual
                .iter()
                .zip(&self.memory.bytes)
                .position(|(a, b)| a != b)
        );
        self.counters();
        self.calls += 1;
        if ordinal == 8 && args.first().is_some_and(|&p| p != 0) {
            let mut bytes = [0; 1024];
            let n = unsafe { oracle_debug(bytes.as_mut_ptr()) } as usize;
            assert_eq!(
                answer
                    .effects
                    .iter()
                    .find_map(|e| if let Effect::Debug(b) = e {
                        Some(b.as_slice())
                    } else {
                        None
                    }),
                Some(&bytes[..n])
            );
        }
        answer
    }
    fn counters(&self) {
        let r = &self.rust;
        let expected = [
            r.hal.registrations().len() as u32,
            r.hal.duplicate,
            r.hal.unmatched,
            r.hal.overflow,
            r.fp.total() as u32,
            r.fp.anomalies,
            r.settings.served,
            r.settings.fabricated,
            r.settings.refused,
            r.av.fabricated,
            r.av.refused,
            r.av.ignored,
            r.debug_prints,
            r.debug_refused,
        ];
        let c = &r.crypto;
        for (i, count) in [
            c.sha_init,
            c.sha_update,
            c.sha_final,
            c.hmac,
            c.des,
            c.stale,
            c.oversize,
            r.settings.queried_total,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                unsafe { oracle_counter(19 + i as u32) },
                count,
                "crypto/query counter {i}"
            );
        }
        assert_eq!(unsafe { oracle_absorbed() }, c.absorbed);
        let mut queried = [0; 64];
        let total = unsafe { oracle_queried(queried.as_mut_ptr()) };
        assert_eq!(total, r.settings.queried_total);
        assert_eq!(
            &queried[..r.settings.queried.len()],
            r.settings.queried.as_slice()
        );
        for (i, count) in expected.into_iter().enumerate() {
            assert_eq!(unsafe { oracle_counter(i as u32) }, count, "counter {i}");
        }
    }
    fn set(&mut self, index: u32, data: &[u8]) {
        assert_eq!(
            self.rust.settings.set(index, data),
            unsafe { oracle_config_set(index, data.as_ptr(), data.len() as u32) } != 0
        );
    }
    fn eeprom(&mut self, bytes: &[u8; 256], key: Option<&[u8; 16]>) -> bool {
        let rust = self.rust.settings.set_eeprom(bytes, key);
        assert_eq!(
            rust,
            unsafe { oracle_eeprom(bytes.as_ptr(), key.map_or(std::ptr::null(), |k| k.as_ptr())) }
                != 0
        );
        for ordinal in [321, 323, 324] {
            let mut bytes = [0; 16];
            let available = unsafe { oracle_key(ordinal, bytes.as_mut_ptr()) } != 0;
            assert_eq!(available, self.rust.settings.key(ordinal).is_some());
            if available {
                assert!(self.rust.settings.key(ordinal).unwrap() == &bytes);
            }
        }
        rust
    }
    fn configure(&mut self, pack: AvPack, region: u32, widescreen: bool) {
        self.rust.av.widescreen = widescreen;
        let id = match pack {
            AvPack::Composite => 0,
            AvPack::Svideo => 1,
            AvPack::Hdtv => 2,
        };
        assert_eq!(
            self.rust.av.configure(pack, region),
            unsafe { oracle_av(id, region, u32::from(widescreen)) } != 0
        );
        assert_eq!(self.rust.av.capabilities(), unsafe { oracle_counter(17) });
        assert_eq!(self.rust.av.smc(), unsafe { oracle_counter(18) });
        assert_eq!(
            self.rust.av.install_settings(&mut self.rust.settings),
            unsafe { oracle_av_install() } as usize
        );
    }
}
fn random(seed: &mut u64) -> u32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed as u32
}

#[test]
fn seeded_c_oracle_all_k3_services() {
    let mut calls = 0;
    let mut h = Harness::new();
    // Missing frames, fatal effects and explicit firmware return handling.
    for &(ordinal, arity) in ORDINALS {
        for n in 0..arity {
            h.call(ordinal, &vec![0; n]);
        }
    }
    assert!(matches!(
        h.call(95, &[0xc0000144]).effects[0],
        Effect::BugCheck {
            code: Some(0xc0000144)
        }
    ));
    assert_eq!(unsafe { oracle_counter(14) }, 95);
    h.call(49, &[3]);
    assert_eq!(unsafe { oracle_counter(15) }, 3);
    calls += h.calls;
    let mut h = Harness::new();
    let mut seed = 0x2208_c0ae_51d9u64;
    // Stateful HAL and FP streams exercise duplicate, overflow, low-byte BOOLEAN and restore order.
    for i in 0..70 {
        h.call(47, &[0x1000 + i * 16, 1]);
    }
    for i in 0..256 {
        h.call(142, &[64 + (i % 8) * 32]);
    }
    h.call(142, &[4096]);
    for _ in 0..3000 {
        let address = 0x1000 + random(&mut seed) % 80 * 16;
        let flag = [0, 1, 0x100, 0x101][random(&mut seed) as usize % 4];
        h.call(47, &[address, flag]);
        let area = 64 + random(&mut seed) % 10 * 32;
        h.call(if random(&mut seed) & 1 == 0 { 142 } else { 139 }, &[area]);
    }
    let a = h.call(49, &[1]);
    assert!(
        matches!(a.effects[0],Effect::Firmware {pending_notifications,..} if pending_notifications == unsafe{oracle_counter(16)} as usize)
    );
    h.call(142, &[0]);
    h.call(142, &[8190]);
    h.call(252, &[7]);
    h.call(253, &[0, 0]);
    calls += h.calls;
    let mut h = Harness::new();
    for i in 0..32 {
        h.set(i, &i.to_le_bytes());
    }
    h.set(99, &[1]); // Table full.
    h.set(2, &[]);
    h.set(3, &[3; 256]);
    h.set(4, &[0; 257]);
    for _ in 0..3000 {
        let index = random(&mut seed) % 48;
        let capacity = [0, 1, 3, 4, 8, 255, 256, 257][random(&mut seed) as usize % 8];
        let address = [0, 64, 128, 8190][random(&mut seed) as usize % 4];
        let t = [0, address, 512][random(&mut seed) as usize % 3];
        let n = [0, address, t, 516][random(&mut seed) as usize % 4];
        let fail = random(&mut seed) & 1 != 0;
        h.rust.settings.policy = if fail {
            UnknownPolicy::Fail
        } else {
            UnknownPolicy::FabricatedZeros
        };
        unsafe {
            oracle_config_policy(u32::from(fail));
        }
        h.call(24, &[index, t, address, capacity, n]);
    }
    for index in 1000..1080 {
        h.call(24, &[index, 64, 128, 4, 80]);
    }
    h.call(24, &[1079, 64, 128, 4, 80]);
    // Value/Type/ResultLength all alias; ordered writes and partial faults.
    h.call(24, &[0, 64, 64, 4, 64]);
    h.memory.write_limit = 66;
    h.call(24, &[0, 64, 64, 4, 64]);
    calls += h.calls;
    let mut h = Harness::new();
    let mut bytes = [0u8; 256];
    for b in &mut bytes {
        *b = random(&mut seed) as u8;
    }
    assert!(h.eeprom(&bytes, None));
    h.call(24, &[0x101, 64, 128, 6, 80]); // Invalid factory checksum.
    bytes[0x30..0x60].fill(0);
    bytes[0x30..0x34].fill(0xff);
    assert!(h.eeprom(&bytes, None));
    for index in [0xffff, 0x101] {
        for capacity in [0, 1, 5, 6, 255, 256, 300] {
            for t in [0, 64, 128] {
                for n in [0, 64, 128] {
                    h.call(24, &[index, t, 128, capacity, n]);
                }
            }
        }
    }
    // Source write faults preserve the order-specific prefix.
    h.memory.write_limit = 130;
    h.call(24, &[0xffff, 64, 128, 256, 80]);
    h.call(24, &[0x101, 64, 128, 6, 80]);
    h.memory.write_limit = 8192;
    // Authenticated synthetic EEPROM. No owner source bytes or keys are used.
    let key = [0x71; 16];
    let plain = [0x39; 28];
    bytes[..20].copy_from_slice(&crypto::hmac(&key, &plain, &[], true));
    let stream_key = crypto::hmac(&key, &bytes[..20], &[], true);
    let mut temp = Memory::default();
    temp.bytes[1..21].copy_from_slice(&stream_key);
    temp.bytes[512..540].copy_from_slice(&plain);
    crypto::rc4_key(&mut temp, 64, 20, 1);
    crypto::rc4_crypt(&mut temp, 64, 28, 512);
    bytes[20..48].copy_from_slice(&temp.bytes[512..540]);
    assert!(h.eeprom(&bytes, Some(&key)));
    assert_eq!(h.rust.settings.key(323), Some(&[0x39; 16]));
    bytes[0] ^= 1;
    assert!(!h.eeprom(&bytes, Some(&key)));
    assert_eq!(h.rust.settings.key(323), Some(&[0x39; 16]));
    calls += h.calls;
    let mut h = Harness::new();
    h.call(2, &[0, 6, 0, 64]);
    for pack in [AvPack::Composite, AvPack::Svideo, AvPack::Hdtv] {
        for region in 0..16 {
            for widescreen in [false, true] {
                h.configure(pack, region, widescreen);
                h.call(24, &[8, 64, 128, 4, 80]);
                h.call(24, &[0x103, 64, 128, 4, 80]);
                for option in [6, 9, 0xb, 0xe, 0xf, 0x10, 0, 0xff] {
                    for param in [0, 1, 5, 6] {
                        for result in [0, 64, 8191] {
                            h.call(2, &[0, option, param, result]);
                        }
                    }
                }
                h.call(3, &[0, 4, 640, 0x12, 2560, 4096]);
                h.call(4, &[random(&mut seed)]);
                h.call(1, &[]);
            }
        }
    }
    calls += h.calls;
    let mut h = Harness::new();
    // Full state and memory comparison, including guest aliases and SHA invalidation.
    for case in 0..2048 {
        for b in &mut h.memory.bytes[1..4096] {
            *b = random(&mut seed) as u8;
        }
        let n = random(&mut seed) % 768;
        h.call(335, &[4096]);
        h.call(336, &[4096, 128, n]);
        h.call(336, &[4096, 128 + n, case % 65]);
        h.call(337, &[4096, if case % 5 == 0 { 4096 } else { 6000 }]);
        h.call(337, &[4096, 6000]);
        let length = [0, 1, 16, 64, 65, 255, 256, 257, 0x10000][case as usize % 9];
        let key = if case % 3 == 0 { 64 + case % 258 } else { 1000 };
        h.call(338, &[64, length, key]);
        let data = if case % 2 == 0 {
            64 + case % 258
        } else {
            1000 + case % 4
        };
        h.call(339, &[64, case % 128, data]);
        h.call(
            340,
            &[
                1000,
                case % 129,
                128,
                n,
                2048,
                case % 100,
                if case % 3 == 0 { 1000 } else { 6000 },
            ],
        );
        h.call(346, &[if case % 8 == 0 { 0 } else { 128 }, case % 64]);
    }
    for ordinal in [335, 336, 337, 338, 339, 340, 346] {
        let arity = ORDINALS.iter().find(|&&(o, _)| o == ordinal).unwrap().1;
        h.call(ordinal, &vec![0; arity]);
        h.call(ordinal, &vec![u32::MAX; arity]);
    }
    calls += h.calls;
    let mut h = Harness::new();
    for case in 0..1024 {
        let formats: &[&[u8]] = &[
            b"%d %i %u %x %X %p %c %s %%\n\0",
            b"%08x %ld %-5d %q %u\0",
            b"trailing%\0",
            b"%s %s %s\r\n\0",
        ];
        let fmt = formats[case % formats.len()];
        h.memory.bytes[64..64 + fmt.len()].copy_from_slice(fmt);
        h.memory.bytes[128..133].copy_from_slice(b"text\0");
        let args = [
            64,
            random(&mut seed),
            random(&mut seed),
            random(&mut seed),
            random(&mut seed),
            random(&mut seed),
            random(&mut seed),
            random(&mut seed),
            128,
        ];
        h.call(8, &args[..1 + case % 9]);
    }
    h.memory.read_limit = 100;
    h.call(8, &[8190]);
    h.call(8, &[0]);
    calls += h.calls;
    let mut h = Harness::new();
    for (hw, ver) in [(64, 80), (64, 64), (8190, 80), (64, 8190), (0, 80)] {
        unsafe {
            oracle_memory(h.memory.bytes.as_ptr());
        }
        assert_eq!(
            ConsoleIdentity::default().publish(&mut h.memory, hw, ver),
            unsafe { oracle_identity(hw, ver) } != 0
        );
        let mut c = [0; 8192];
        unsafe {
            oracle_snapshot(c.as_mut_ptr());
        }
        assert!(c == h.memory.bytes);
    }
    for case in 0..512 {
        let id = DiskIdentity {
            model: vec![b'M'; case % 41],
            serial: vec![b'S'; case % 21],
            device: (0, 0),
        };
        let model = [64, 128, 256, 8190, 0][case % 5];
        let serial = [80, 64, 256, 8190, 0][case % 5];
        let storage = 256;
        unsafe {
            oracle_memory(h.memory.bytes.as_ptr());
        }
        assert_eq!(
            id.publish(&mut h.memory, model, serial, storage),
            unsafe {
                oracle_disk(
                    model,
                    serial,
                    storage,
                    id.model.as_ptr(),
                    id.model.len() as u32,
                    id.serial.as_ptr(),
                    id.serial.len() as u32,
                )
            } != 0
        );
        let mut c = [0; 8192];
        unsafe {
            oracle_snapshot(c.as_mut_ptr());
        }
        assert!(c == h.memory.bytes);
    }
    // Disk descriptor decoding, valid source publication and malformed lengths.
    let mut h = Harness::new();
    let id = DiskIdentity {
        model: b"synthetic model".to_vec(),
        serial: b"synthetic serial".to_vec(),
        device: (0, 0),
    };
    assert!(id.publish(&mut h.memory, 64, 80, 256));
    for malformed in 0..4 {
        if malformed == 1 {
            h.memory.put_word(64, 0);
        }
        if malformed == 2 {
            h.memory.put_word(64, 0x00010002);
        }
        if malformed == 3 {
            h.memory.put_word(64, 0x0040003f);
        }
        unsafe {
            oracle_memory(h.memory.bytes.as_ptr());
        }
        let mut bytes = [0; 72];
        let c = unsafe { oracle_disk_read(64, 80, bytes.as_mut_ptr()) } != 0;
        let rust = DiskIdentity::read(&h.memory, 64, 80);
        assert_eq!(rust.is_some(), c);
        if let Some(rust) = rust {
            assert_eq!(rust, id);
        }
    }
    // Synthetic header shape only, no original XBE bytes.
    let mut h = Harness::new();
    for region in 0..16 {
        h.memory.put_word(64, 0x48454258);
        h.memory.put_word(64 + 0x108, 1024);
        h.memory.put_word(64 + 0x118, 512);
        h.memory.put_word(512 + 0xa0, region);
        unsafe {
            oracle_memory(h.memory.bytes.as_ptr());
        }
        assert_eq!(
            h.rust.av.configure_from_image(AvPack::Hdtv, 64, &h.memory),
            unsafe { oracle_av_image(2, 64) } != 0
        );
        assert_eq!(h.rust.av.capabilities(), unsafe { oracle_counter(17) });
    }
    // Atomic EEPROM installation refuses a store with only one available slot.
    let mut full = Harness::new();
    for index in 0..31 {
        full.set(index, &[7]);
    }
    assert!(!full.eeprom(&[0; 256], None));
    assert_eq!(full.rust.settings.count(), 31);
    full.configure(AvPack::Hdtv, 1, false); // Only region fits; flags remain absent.
    full.call(24, &[0x103, 64, 128, 4, 80]);
    full.call(24, &[8, 64, 128, 4, 80]);
    let args = [99, 64, 128, 4, 80];
    unsafe {
        oracle_memory(full.memory.bytes.as_ptr());
    }
    let c = unsafe { oracle_stored(args.as_ptr(), 5) };
    let (r, fabricated) = full.rust.settings.query(&args, true, &mut full.memory);
    assert_eq!(r, c);
    assert!(!fabricated);
    assert_eq!(r, 0xc0000034);
    full.counters();
    // The default-off A/B selector actually invokes the C partner, then Rust when selected.
    struct COracle;
    impl KernelServices for COracle {
        fn call(
            &mut self,
            ordinal: u32,
            args: &[u32],
            _thread: u64,
            _memory: &mut dyn GuestMemory,
        ) -> Option<Answer> {
            ORDINALS.iter().find(|&&(o, _)| o == ordinal)?;
            Some(Answer::status(unsafe {
                oracle_call(ordinal, args.as_ptr(), args.len() as u32)
            }))
        }
    }
    let mut selection = Selection::default();
    let mut oracle = COracle;
    assert!(!selection.rust(253));
    assert_eq!(
        selection
            .call(&mut h.rust, &mut oracle, 253, &[0, 0], 1, &mut h.memory)
            .unwrap()
            .eax,
        0xc000000e
    );
    assert_eq!(h.rust.hal.phy_initializations, 0);
    selection.select(253, true);
    assert_eq!(
        selection
            .call(&mut h.rust, &mut oracle, 253, &[0, 0], 1, &mut h.memory)
            .unwrap()
            .eax,
        0xc000000e
    );
    assert_eq!(h.rust.hal.phy_initializations, 1);
    println!(
        "{calls} C differential calls plus 517 data publication comparisons (seed 0x2208c0ae51d9)"
    );
}

// SPDX-License-Identifier: GPL-3.0-or-later
//! T1697 Tier 1: the native A/V Accept write set against lifted original 0x2C3AC0.
//! Profiles and menu globals are fabricated; guest calls are stubbed to return the same profile
//! view. This checks the typed profile publication boundary, not live page rendering or audio.
#![cfg(t1697_lifted)]

use std::ptr;
use tsfp_format_profile::{Profile, settings::SettingsEdit};

unsafe extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    fn shim_set_base(base: *mut u8);
    fn shim_call(va: u32, regs: *mut u32) -> i32;
    fn shim_script(words: *const u32, entries: u32);
}

const ARENA: usize = (1 << 32) + 0x10000;
const STACK: u32 = 0x0030_0000;
const PROFILE: u32 = 0x0100_0000;
const SETTINGS_ACCEPT: u32 = 0x002C_3AC0;

fn arena() -> *mut u8 {
    let base = unsafe { mmap(ptr::null_mut(), ARENA, 3, 0x2 | 0x20 | 0x4000, -1, 0) };
    assert_ne!(base as isize, -1, "lifted test arena mmap failed");
    base
}

fn profile(seed: u32) -> Profile {
    let mut payload = vec![seed as u8; 0x1f54];
    payload[0..4].fill(0);
    payload[6] = 0;
    payload[0x6c..0x70].copy_from_slice(&0u32.to_le_bytes());
    payload[0x78..0x7c].copy_from_slice(&0u32.to_le_bytes());
    payload[0x7c..0x80].copy_from_slice(&0.5f32.to_le_bytes());
    payload[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
    Profile {
        payload,
        signature: [0xA5; 20],
    }
}

fn put32(base: *mut u8, address: u32, value: u32) {
    unsafe {
        ptr::copy_nonoverlapping(value.to_le_bytes().as_ptr(), base.add(address as usize), 4);
    }
}

fn read(base: *mut u8, address: u32, len: usize) -> Vec<u8> {
    unsafe { std::slice::from_raw_parts(base.add(address as usize), len).to_vec() }
}

/// Seeded Tier 1 differential against the lifted original's six-word A/V profile write set.
#[test]
fn audio_video_accept_matches_lifted_2c3ac0() {
    let base = arena();
    unsafe { shim_set_base(base) };
    let mut seed = 0x1697_2c3au32;
    for case in 0..4096 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;

        let mut source = profile(seed);
        unsafe {
            ptr::copy_nonoverlapping(
                source.payload.as_ptr(),
                base.add(PROFILE as usize),
                source.payload.len(),
            );
        }
        let av = tsfp_format_profile::settings::AudioVideo {
            music_enabled: seed.rotate_left(3),
            sound_enabled: seed.rotate_right(7),
            music_volume: seed % 17,
            sound_volume: seed.rotate_left(9) % 17,
            slider_4: seed.rotate_right(5) % 65,
            option_5: !seed,
        };
        for (address, value) in [
            (0x0052_36B4, av.music_enabled),
            (0x0052_36B8, av.sound_enabled),
            (0x0052_36BC, av.music_volume),
            (0x0052_36C0, av.sound_volume),
            (0x0076_13D8, av.slider_4),
            (0x0076_13DC, av.option_5),
        ] {
            put32(base, address, value);
        }
        // The first six calls are profile-view lookups; the final two are the audio apply and
        // page-pop boundaries. The original body is run, with only these neighbours stubbed.
        let mut script = vec![PROFILE + 0x80, 0, 0].repeat(8);
        script[3 * 6..].fill(0);
        unsafe { shim_script(script.as_ptr(), 8) };
        let mut regs = [0u32, 0, 0, 0, STACK];
        assert_eq!(unsafe { shim_call(SETTINGS_ACCEPT, regs.as_mut_ptr()) }, 1);

        let mut edit = SettingsEdit::open(&source).unwrap();
        edit.audio_video = av;
        edit.accept_audio_video(&mut source).unwrap();
        let lifted = read(base, PROFILE, source.payload.len());
        assert_eq!(lifted, source.payload, "case {case}");
    }
}

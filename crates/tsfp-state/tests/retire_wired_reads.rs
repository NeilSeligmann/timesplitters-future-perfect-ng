// SPDX-License-Identifier: GPL-3.0-or-later
//! T2135 adapter differential, FABRICATED inputs. No lifted oracle or Tier 2 claim.
use std::collections::BTreeMap;
use tsfp_ports::{
    memory::GuestMemory,
    state_wire::{
        self as gs, Wired,
        game_state::{Cell, RETIRE_AUDIT, RETIRE_OFF, RETIRE_OWN},
    },
};

#[derive(Default)]
struct Image(BTreeMap<u32, u8>, bool);
impl GuestMemory for Image {
    fn read8(&self, address: u32) -> u8 {
        assert!(
            !self.1 || Cell::containing_byte(address).is_none(),
            "own reader touched guest cell at {address:#x}"
        );
        *self.0.get(&address).unwrap_or(&0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}

// One test owns the process-wide GameState selector; no PlayerSet state is accessed.
#[test]
fn wired_reads_follow_authority_at_every_width_and_boundary() {
    assert_eq!(gs::RETIRE.mode(), RETIRE_OFF);
    let mut random = 0x2135_u32;
    for _seed in 0..2000 {
        let mut image = Image::default();
        for cell in Cell::ALL {
            for offset in -4_i32..8 {
                random = random.wrapping_mul(1664525).wrapping_add(1013904223);
                image.write8(
                    cell.address().wrapping_add_signed(offset),
                    (random >> 24) as u8,
                );
            }
        }
        gs::RETIRE.reset();
        gs::RETIRE.set_mode(RETIRE_OFF);
        // Differential against the unwrapped memory contract; mode off must preserve every byte.
        for cell in Cell::ALL {
            for offset in -3_i32..5 {
                let a = cell.address().wrapping_add_signed(offset);
                let expected = (image.read8(a), image.read16(a), image.read32(a));
                let wired = Wired(&mut image);
                assert_eq!((wired.read8(a), wired.read16(a), wired.read32(a)), expected);
            }
        }
        assert!(!gs::RETIRE.is_init());
        gs::RETIRE.set_mode(RETIRE_OWN);
        gs::RETIRE.ensure_init(&image);
        // Corrupt each guest mirror. Owned reads must remain stable, guest-only neighbours must not.
        let mut expected_owned = Image(image.0.clone(), false);
        for cell in Cell::ALL {
            let value = gs::RETIRE.owned(cell);
            for offset in 0..cell.width() {
                image.write8(
                    cell.address() + offset,
                    ((value >> (8 * offset)) as u8) ^ 0xff,
                );
            }
            if Cell::containing_byte(cell.address() - 1).is_none() {
                image.write8(cell.address() - 1, 0x5a);
                expected_owned.write8(cell.address() - 1, 0x5a);
            }
        }
        image.1 = true; // Owned reads must not even access the corresponding guest bytes.
        for cell in Cell::ALL {
            for offset in -3_i32..5 {
                let a = cell.address().wrapping_add_signed(offset);
                let expected = (
                    expected_owned.read8(a),
                    expected_owned.read16(a),
                    expected_owned.read32(a),
                );
                let mut bytes = [0; 7];
                let wired = Wired(&mut image);
                assert_eq!((wired.read8(a), wired.read16(a), wired.read32(a)), expected);
                wired.read_bytes(a, &mut bytes);
                let mut expected_bytes = [0; 7];
                expected_owned.read_bytes(a, &mut expected_bytes);
                assert_eq!(bytes, expected_bytes);
                assert!(wired.as_slice(a, 4).is_none());
            }
        }
        image.1 = false;
        assert!(gs::RETIRE.check(&image).is_some());
        gs::RETIRE.set_mode(RETIRE_AUDIT);
        let before = gs::RETIRE.drift_count();
        for cell in Cell::ALL {
            let a = cell.address();
            let expected = (image.read8(a), image.read16(a), image.read32(a));
            let wired = Wired(&mut image);
            assert_eq!((wired.read8(a), wired.read16(a), wired.read32(a)), expected);
        }
        assert!(gs::RETIRE.drift_count() > before);
        // Wired stores must subsequently repair both authority and mirror.
        gs::RETIRE.set_mode(RETIRE_OWN);
        for cell in Cell::ALL {
            let mut wired = Wired(&mut image);
            if cell == Cell::Mode {
                wired.write8(cell.address(), 10);
            } else {
                wired.write32(cell.address(), random);
            }
        }
        assert_eq!(gs::RETIRE.check(&image), None);
    }
    gs::RETIRE.set_mode(RETIRE_OFF);
    gs::RETIRE.reset();
}

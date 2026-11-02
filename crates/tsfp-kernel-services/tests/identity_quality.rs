// SPDX-License-Identifier: GPL-3.0-or-later
use tsfp_kernel_services::identity::DiskIdentity;

#[path = "support/memory.rs"]
mod memory;
use memory::Memory;

#[test]
fn descriptor_lengths_padding_and_round_trip() {
    for model_len in 1..=40 {
        for serial_len in 1..=20 {
            let disk = DiskIdentity {
                model: (0..model_len).map(|i| b'A' + (i % 26) as u8).collect(),
                serial: (0..serial_len).map(|i| b'0' + (i % 10) as u8).collect(),
                device: (3, 4),
            };
            let mut memory = Memory(vec![0xa5; 256]);
            assert!(disk.publish(&mut memory, 16, 32, 64));
            assert_eq!(&memory.0[16..18], &(model_len as u16).to_le_bytes());
            assert_eq!(&memory.0[18..20], &40u16.to_le_bytes());
            assert_eq!(&memory.0[20..24], &64u32.to_le_bytes());
            assert_eq!(&memory.0[32..34], &(serial_len as u16).to_le_bytes());
            assert_eq!(&memory.0[34..36], &20u16.to_le_bytes());
            assert_eq!(&memory.0[36..40], &104u32.to_le_bytes());
            assert_eq!(&memory.0[64..64 + model_len], disk.model);
            assert!(memory.0[64 + model_len..104].iter().all(|&b| b == b' '));
            assert_eq!(&memory.0[104..104 + serial_len], disk.serial);
            assert!(memory.0[104 + serial_len..124].iter().all(|&b| b == b' '));
            assert_eq!(&memory.0[124..128], &[0; 4]);
            let read = DiskIdentity::read(&memory, 16, 32).unwrap();
            assert_eq!(read.model, disk.model);
            assert_eq!(read.serial, disk.serial);
            assert_eq!(read.device, (0, 0));
        }
    }
}

#[test]
fn invalid_lengths_and_overlaps_leave_memory_unchanged() {
    for (model_len, serial_len) in [(0, 1), (41, 1), (1, 0), (1, 21)] {
        let disk = DiskIdentity {
            model: vec![b'M'; model_len],
            serial: vec![b'S'; serial_len],
            device: (0, 0),
        };
        let mut memory = Memory(vec![0xa5; 256]);
        assert!(!disk.publish(&mut memory, 16, 32, 64));
        assert!(memory.0.iter().all(|&b| b == 0xa5));
    }
    let disk = DiskIdentity {
        model: b"M".to_vec(),
        serial: b"S".to_vec(),
        device: (0, 0),
    };
    for (model, serial, storage) in [(16, 20, 64), (16, 32, 20), (64, 32, 64), (0, 32, 64)] {
        let mut memory = Memory(vec![0xa5; 256]);
        assert!(!disk.publish(&mut memory, model, serial, storage));
        assert!(memory.0.iter().all(|&b| b == 0xa5));
    }
}

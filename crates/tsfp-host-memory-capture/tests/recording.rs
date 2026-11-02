// SPDX-License-Identifier: GPL-3.0-or-later
use std::{ffi::CString, fs, io};
use tsfp_host_memory_capture::{events, read, recording::*};

#[test]
fn nested_host_recording_roundtrips_and_concurrent_overlap_poisoned() {
    let dir = (0..100)
        .find_map(|index| {
            let path = std::env::temp_dir().join(format!(
                "tsfp-hostmem-recording-{}-{index}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => Some(path),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => None,
                Err(e) => panic!("{e}"),
            }
        })
        .unwrap();
    let path = |name: &str| CString::new(dir.join(name).to_str().unwrap()).unwrap();
    let ops = [
        Operation {
            id: 90,
            inputs: 1,
            outputs: 0,
        },
        Operation {
            id: 12,
            inputs: 2,
            outputs: 1,
        },
    ];
    let identity = b"build-route";
    let dest = path("nested");
    assert!(unsafe {
        tsfp_hostmem_recording_start(
            dest.as_ptr(),
            identity.as_ptr(),
            identity.len(),
            true,
            ops.as_ptr(),
            ops.len(),
            1024,
        )
    });
    let outer = unsafe { tsfp_hostmem_recording_begin(90, [7].as_ptr(), 1) };
    let inner = unsafe { tsfp_hostmem_recording_begin(12, [1, 2].as_ptr(), 2) };
    assert_ne!(outer, 0);
    assert_ne!(inner, 0);
    assert!(unsafe { tsfp_hostmem_recording_memory(false, 0x1000, [0xaa, 0xbb].as_ptr(), 2) });
    assert!(tsfp_hostmem_recording_placement(0, 4096, 0, 0x1000));
    assert!(unsafe { tsfp_hostmem_recording_end(inner, 0x1000, [0].as_ptr(), 1) });
    assert!(unsafe { tsfp_hostmem_recording_end(outer, 0x1000, std::ptr::null(), 0) });
    assert!(tsfp_hostmem_recording_seal());
    let bytes = fs::read(dir.join("nested")).unwrap();
    let events = read(bytes.as_slice(), 1024).unwrap();
    assert_eq!(
        events::validate(&events, identity, &[(90, 1, 0), (12, 2, 1)]),
        Ok(2)
    );

    let overlap = path("overlap");
    assert!(unsafe {
        tsfp_hostmem_recording_start(
            overlap.as_ptr(),
            identity.as_ptr(),
            identity.len(),
            true,
            ops.as_ptr(),
            ops.len(),
            1024,
        )
    });
    let outer = unsafe { tsfp_hostmem_recording_begin(90, [7].as_ptr(), 1) };
    assert_ne!(outer, 0);
    assert_eq!(
        std::thread::spawn(|| unsafe { tsfp_hostmem_recording_begin(12, [1, 2].as_ptr(), 2) })
            .join()
            .unwrap(),
        0
    );
    assert!(!unsafe { tsfp_hostmem_recording_end(outer, 0, std::ptr::null(), 0) });
    assert!(!tsfp_hostmem_recording_seal());
    assert!(!dir.join("overlap").exists());
    assert!(dir.join("overlap.part").exists());
    fs::remove_dir_all(dir).unwrap();
}

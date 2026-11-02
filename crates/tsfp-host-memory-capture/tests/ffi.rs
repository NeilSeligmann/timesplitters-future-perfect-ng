// SPDX-License-Identifier: GPL-3.0-or-later
use std::ffi::CString;
use std::fs;
use std::io;
use tsfp_host_memory_capture::ffi::{
    tsfp_hostmem_capture_incomplete, tsfp_hostmem_capture_record, tsfp_hostmem_capture_seal,
    tsfp_hostmem_capture_start,
};
use tsfp_host_memory_capture::read;

#[test]
fn c_boundary_only_publishes_nonempty_complete_recordings() {
    let dir = (0..100)
        .find_map(|index| {
            let path = std::env::temp_dir()
                .join(format!("tsfp-hostmem-ffi-{}-{index}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => Some(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => None,
                Err(error) => panic!("cannot create test directory: {error}"),
            }
        })
        .unwrap();
    let identity = b"route-and-build-identity";
    let filename = |name: &str| CString::new(dir.join(name).to_str().unwrap()).unwrap();

    let empty = filename("empty");
    assert!(unsafe {
        tsfp_hostmem_capture_start(empty.as_ptr(), identity.as_ptr(), identity.len(), 1024)
    });
    assert!(!tsfp_hostmem_capture_seal());
    assert!(!dir.join("empty").exists());
    assert!(dir.join("empty.part").exists());

    let failed = filename("failed");
    assert!(unsafe {
        tsfp_hostmem_capture_start(failed.as_ptr(), identity.as_ptr(), identity.len(), 1024)
    });
    assert!(!unsafe { tsfp_hostmem_capture_record(1, std::ptr::null(), 1) });
    assert!(!unsafe { tsfp_hostmem_capture_record(1, b"later".as_ptr(), 5) });
    assert!(!tsfp_hostmem_capture_seal());
    assert!(!dir.join("failed").exists());

    let omitted = filename("omitted");
    assert!(unsafe {
        tsfp_hostmem_capture_start(omitted.as_ptr(), identity.as_ptr(), identity.len(), 1024)
    });
    assert!(unsafe { tsfp_hostmem_capture_record(1, b"event".as_ptr(), 5) });
    tsfp_hostmem_capture_incomplete();
    assert!(!tsfp_hostmem_capture_seal());
    assert!(!dir.join("omitted").exists());

    let duplicate = filename("duplicate");
    assert!(unsafe {
        tsfp_hostmem_capture_start(duplicate.as_ptr(), identity.as_ptr(), identity.len(), 1024)
    });
    assert!(unsafe { tsfp_hostmem_capture_record(1, b"event".as_ptr(), 5) });
    assert!(!unsafe {
        tsfp_hostmem_capture_start(duplicate.as_ptr(), identity.as_ptr(), identity.len(), 1024)
    });
    assert!(!tsfp_hostmem_capture_seal());
    assert!(!dir.join("duplicate").exists());

    let complete = filename("complete");
    assert!(unsafe {
        tsfp_hostmem_capture_start(complete.as_ptr(), identity.as_ptr(), identity.len(), 1024)
    });
    assert!(unsafe { tsfp_hostmem_capture_record(1, b"event".as_ptr(), 5) });
    assert!(tsfp_hostmem_capture_seal());
    let bytes = fs::read(dir.join("complete")).unwrap();
    assert_eq!(
        read(bytes.as_slice(), 1024).unwrap(),
        vec![(0, identity.to_vec()), (1, b"event".to_vec())]
    );
    fs::remove_dir_all(dir).unwrap();
}

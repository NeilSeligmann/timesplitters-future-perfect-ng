use tsfp_audio::effects_metadata::Metadata;
use tsfp_audio_capi::effects_metadata_ffi::{
    dsound_effects_snapshot_known_image_rust, dsound_effects_validate_known_image_rust,
    dsound_effects_validate_layout_rust,
};

#[test]
fn validator_abi_publishes_success_and_preserves_output_on_refusal() {
    let mut image = vec![0u8; 0x890];
    image[0x804..0x808].copy_from_slice(&4u32.to_le_bytes());
    image[0x80c..0x810].copy_from_slice(&4u32.to_le_bytes());
    image[0x838..0x83c].copy_from_slice(&1u32.to_le_bytes());
    image[0x83c..0x840].copy_from_slice(&16u32.to_le_bytes());
    for (index, value) in [0x818u32, 8, 0x828, 8, 0, 0, 0xc000, 16]
        .into_iter()
        .enumerate()
    {
        let offset = 0x840 + index * 4;
        image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    let mut output = Metadata::default();
    assert!(unsafe {
        dsound_effects_validate_layout_rust(image.as_ptr(), image.len(), &mut output)
    });
    assert_eq!(output.map_count, 1);
    assert_eq!(output.maps[0].workspace_offset, 0);

    let before = output;
    image[0x838..0x83c].copy_from_slice(&0u32.to_le_bytes());
    assert!(!unsafe {
        dsound_effects_validate_layout_rust(image.as_ptr(), image.len(), &mut output)
    });
    assert_eq!(output, before);
    assert!(!unsafe { dsound_effects_validate_layout_rust(std::ptr::null(), 0x890, &mut output) });
}

#[test]
fn known_image_abi_preserves_output_on_synthetic_identity_refusal() {
    let image = vec![0u8; tsfp_audio::effects_identity::IMAGE_BYTES];
    let mut output = Metadata {
        map_count: 0xA5A5_A5A5,
        ..Metadata::default()
    };
    let before = output;
    assert!(!unsafe {
        dsound_effects_validate_known_image_rust(image.as_ptr(), image.len(), &mut output)
    });
    assert_eq!(output, before);
    assert!(!unsafe {
        dsound_effects_validate_known_image_rust(std::ptr::null(), image.len(), &mut output)
    });
    assert_eq!(output, before);
}

#[cfg(target_os = "linux")]
#[test]
fn known_image_snapshot_refuses_unreadable_guest_span_without_touching_outputs() {
    use std::ffi::c_void;

    unsafe extern "C" {
        fn mmap(
            address: *mut c_void,
            length: usize,
            protection: i32,
            flags: i32,
            fd: i32,
            offset: isize,
        ) -> *mut c_void;
        fn munmap(address: *mut c_void, length: usize) -> i32;
    }

    const IMAGE_BYTES: usize = tsfp_audio::effects_identity::IMAGE_BYTES;
    const PROT_NONE: i32 = 0;
    const MAP_PRIVATE: i32 = 2;
    const MAP_ANONYMOUS: i32 = 0x20;
    // SAFETY: mmap returns an owned inaccessible mapping; it is unmapped below.
    let inaccessible = unsafe {
        mmap(
            std::ptr::null_mut(),
            IMAGE_BYTES,
            PROT_NONE,
            MAP_PRIVATE | MAP_ANONYMOUS,
            -1,
            0,
        )
    };
    assert_ne!(inaccessible as isize, -1);
    let mut image = vec![0xA5; IMAGE_BYTES];
    let mut metadata = Metadata {
        map_count: 0xA5A5_A5A5,
        ..Metadata::default()
    };
    let before_metadata = metadata;
    assert!(!unsafe {
        dsound_effects_snapshot_known_image_rust(
            inaccessible.cast(),
            IMAGE_BYTES,
            image.as_mut_ptr(),
            &mut metadata,
        )
    });
    assert!(image.iter().all(|byte| *byte == 0xA5));
    assert_eq!(metadata, before_metadata);
    // SAFETY: this test owns the mapping and releases it exactly once.
    assert_eq!(unsafe { munmap(inaccessible, IMAGE_BYTES) }, 0);
}

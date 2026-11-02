use tsfp_audio::effects_metadata::{EffectMap, Metadata};

#[test]
fn descriptor_ffi_publishes_only_complete_valid_results() {
    let mut metadata = Metadata {
        code_offset: 0x818,
        code_bytes: 0x100,
        state_offset: 0x918,
        state_bytes: 0x100,
        map_count: 1,
        workspace_bytes: 0x1000,
        ..Metadata::default()
    };
    metadata.maps[0] = EffectMap {
        code_offset: 0x828,
        code_bytes: 0x20,
        state_offset: 0x928,
        state_bytes: 0x20,
        workspace_offset: 0x80,
        workspace_bytes: 0x40,
        ..EffectMap::default()
    };
    let mut output = [0xa5; 296];
    // SAFETY: all pointers refer to live values and output spans the required exact length.
    let accepted = unsafe {
        tsfp_audio_capi::effects_binding_ffi::tsfp_effects_binding_build_descriptor_rust(
            &metadata,
            0x20_000,
            0x30_000,
            0x40_000,
            0x50_000,
            output.as_mut_ptr(),
            output.len(),
        )
    };
    assert!(accepted);
    assert_eq!(u32::from_le_bytes(output[0..4].try_into().unwrap()), 1);
    assert_eq!(
        u32::from_le_bytes(output[8..12].try_into().unwrap()),
        0x20_010
    );
    assert_eq!(
        u32::from_le_bytes(output[24..28].try_into().unwrap()),
        0x50_000
    );

    metadata.maps[0].code_offset = 0x700;
    output.fill(0xa5);
    // SAFETY: pointers are valid; refusal is expected and must preserve output.
    let refused = unsafe {
        tsfp_audio_capi::effects_binding_ffi::tsfp_effects_binding_build_descriptor_rust(
            &metadata,
            0x20_000,
            0x30_000,
            0x40_000,
            0x50_000,
            output.as_mut_ptr(),
            output.len(),
        )
    };
    assert!(!refused);
    assert!(output.iter().all(|byte| *byte == 0xa5));
}

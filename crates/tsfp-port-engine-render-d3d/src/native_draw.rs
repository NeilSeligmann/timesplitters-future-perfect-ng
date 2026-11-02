// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024: native primitive expansion for the live D3D8 draw-assembly path.
//!
//! The C implementation remains the default oracle. The opt-in C adapter calls this function
//! with the draw's index list and caller-owned output buffer; no allocation or ownership crosses
//! the ABI. Values are the NV2A BEGIN_END operations in `gpu_pgraph.h`.

const INVALID: u32 = u32::MAX;

/// Snapshot-backed vertex array input shared with the C draw adapter.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeAttributeSlot {
    pub bytes: *const u8,
    pub snapshot_address: u64,
    pub snapshot_length: u64,
    pub array_address: u64,
    pub stride: u32,
    pub lanes: u32,
    pub element: u32,
    pub mode: u32,
}

const FAST_DISABLED: u32 = 1;
const FAST_D3D: u32 = 2;
const FAST_CMP: u32 = 3;
const FAST_S32K: u32 = 4;
const FAST_FLOAT: u32 = 5;

/// Return the ordered RenderDevice inference bits required by an attribute descriptor.
/// The first requirement occupies the high word; C still authorizes and reports each one in
/// slot order. Unknown/slow descriptors return `u64::MAX` for the C fallback.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_render_native_attribute_inference_mask(mode: u32, lanes: u32) -> u64 {
    const COMPONENT_DEFAULTS: u32 = 0x4;
    const D3DCOLOR_ORDER: u32 = 0x8;
    const S32K_UNNORMALISED: u32 = 0x40;
    const CMP_PACKED: u32 = 0x400;
    let (first, second) = match mode {
        FAST_DISABLED => (COMPONENT_DEFAULTS, 0),
        FAST_D3D => (D3DCOLOR_ORDER, 0),
        FAST_CMP => (CMP_PACKED, COMPONENT_DEFAULTS),
        FAST_S32K => (S32K_UNNORMALISED, COMPONENT_DEFAULTS),
        FAST_FLOAT if lanes < 4 => (COMPONENT_DEFAULTS, 0),
        FAST_FLOAT => (0, 0),
        _ => return u64::MAX,
    };
    (u64::from(first) << 32) | u64::from(second)
}

/// Describe which inference a point/line draw needs before it can be submitted.
/// The C boundary retains authorization and diagnostics; this is only the state decision.
pub const NATIVE_PRIMITIVE_NEEDS_POINT_SIZE: u32 = 0x10;
pub const NATIVE_PRIMITIVE_NEEDS_LINE_WIDTH: u32 = 0x20;
pub const NATIVE_PRIMITIVE_UNSUPPORTED_LINE_WIDTH: u32 = u32::MAX;

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_render_native_resolve_primitive(topology: u32, line_width: f32) -> u32 {
    match topology {
        1 => NATIVE_PRIMITIVE_NEEDS_POINT_SIZE,
        2 if line_width == 0.0 => NATIVE_PRIMITIVE_NEEDS_LINE_WIDTH,
        2 if line_width != 1.0 => NATIVE_PRIMITIVE_UNSUPPORTED_LINE_WIDTH,
        _ => 0,
    }
}

/// Materialize D3D draw constants after the C adapter has authorized any required inferences.
/// This owns only the deterministic copy/viewport transformation; inference policy and refusal
/// remain at the C boundary.
/// # Safety
/// `input` and `out` each reference 768 readable/writable floats; `out` does not overlap any
/// input. `scale` and `offset` each reference four readable floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_render_native_materialize_constants(
    input: *const f32,
    written_58: u32,
    written_59: u32,
    scale_set: u32,
    offset_set: u32,
    scale: *const f32,
    offset: *const f32,
    viewport_from_target: u32,
    window_clip_modules: u32,
    width: u32,
    height: u32,
    out: *mut f32,
) -> bool {
    if input.is_null() || scale.is_null() || offset.is_null() || out.is_null() {
        return false;
    }
    // SAFETY: the caller contract provides the declared fixed-size ranges.
    let input = unsafe { core::slice::from_raw_parts(input, 768) };
    // SAFETY: both register vectors contain four floats.
    let scale = unsafe { core::slice::from_raw_parts(scale, 4) };
    // SAFETY: both register vectors contain four floats.
    let offset = unsafe { core::slice::from_raw_parts(offset, 4) };
    // SAFETY: the caller contract provides the declared writable range.
    let out = unsafe { core::slice::from_raw_parts_mut(out, 768) };
    out.copy_from_slice(input);

    let written_58 = written_58 != 0;
    let written_59 = written_59 != 0;
    let scale_set = scale_set != 0;
    let offset_set = offset_set != 0;
    if scale_set && !written_58 {
        out[58 * 4..59 * 4].copy_from_slice(scale);
    }
    if offset_set && !written_59 {
        out[59 * 4..60 * 4].copy_from_slice(offset);
    }

    if viewport_from_target != 0 && !scale_set && !offset_set && (!written_58 || !written_59) {
        let half_width = width as f32 * 0.5;
        let half_height = height as f32 * 0.5;
        if !written_58 {
            out[58 * 4..59 * 4].copy_from_slice(&[half_width, -half_height, 1.0, 0.0]);
        }
        if !written_59 {
            out[59 * 4..60 * 4].copy_from_slice(&[half_width, half_height, 0.0, 0.0]);
        }
    }

    if window_clip_modules != 0 && viewport_from_target != 0 && !written_58 && !scale_set {
        let half_width = width as f32 * 0.5;
        let half_height = height as f32 * 0.5;
        out[58 * 4..59 * 4].copy_from_slice(&[half_width, -half_height, 1.0, 0.0]);
        if !written_59 {
            out[59 * 4..60 * 4].copy_from_slice(&[half_width, half_height, 0.0, 0.0]);
        }
    }
    true
}

/// Move the simple NV2A vertex-array format classification into Rust. `false` leaves the caller
/// free to use its C oracle; a successful slow-mode descriptor is distinct from ABI refusal.
/// # Safety
/// `out` must point to one writable `NativeAttributeSlot`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_render_native_prepare_attribute_slot(
    format_word: u32,
    format_set: u32,
    address_set: u32,
    array_address: u64,
    bytes: *const u8,
    snapshot_address: u64,
    snapshot_length: u64,
    out: *mut NativeAttributeSlot,
) -> bool {
    if out.is_null() {
        return false;
    }
    let word = if format_set != 0 { format_word } else { 0 };
    let kind = word & 0x0f;
    let lanes = (word >> 4) & 0x0f;
    let stride = word >> 8;
    let element = match (kind, lanes) {
        (2, 1..=4) => lanes * 4,
        (0, 4) | (5, 2) | (6, 1) => 4,
        (_, 0) => 0,
        _ => 0,
    };
    let mode = if lanes == 0 {
        FAST_DISABLED
    } else if element == 0 || address_set == 0 || bytes.is_null() {
        0
    } else {
        match kind {
            0 => FAST_D3D,
            2 => FAST_FLOAT,
            5 => FAST_S32K,
            6 => FAST_CMP,
            _ => 0,
        }
    };
    if snapshot_address.checked_add(snapshot_length).is_none() {
        return false;
    }
    // SAFETY: the caller guarantees `out` is a writable descriptor.
    unsafe {
        out.write(NativeAttributeSlot {
            bytes,
            snapshot_address,
            snapshot_length,
            array_address,
            stride,
            lanes,
            element,
            mode,
        });
    }
    true
}

/// Decode snapshot-backed vertex attributes in bulk. Returns false without writing output when
/// any range is unsupported; the caller then runs the C oracle, including its read/error path.
/// # Safety
/// `slots` references `slot_count` descriptors, `indices` references `vertex_count` indices and
/// `out` references `vertex_count * slot_count * 4` writable floats. Each active descriptor's
/// byte range covers every addressed element. Input ranges do not overlap `out`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_render_native_decode_attributes(
    slots: *const NativeAttributeSlot,
    slot_count: u32,
    indices: *const u32,
    vertex_count: u32,
    out: *mut f32,
) -> bool {
    if slots.is_null() || (vertex_count != 0 && (indices.is_null() || out.is_null())) {
        return false;
    }
    // SAFETY: caller provides the declared descriptor range.
    let slots = unsafe { core::slice::from_raw_parts(slots, slot_count as usize) };
    // SAFETY: caller provides the declared index range.
    let indices = if vertex_count == 0 {
        &[]
    } else {
        unsafe { core::slice::from_raw_parts(indices, vertex_count as usize) }
    };
    // Validate the entire batch before writing so a refusal leaves the C oracle free to overwrite
    // the output without observing partial candidate results.
    for slot in slots {
        if slot.mode == FAST_DISABLED {
            continue;
        }
        if !(FAST_D3D..=FAST_FLOAT).contains(&slot.mode) || slot.bytes.is_null() {
            return false;
        }
        for &index in indices {
            let Some(at) = slot
                .array_address
                .checked_add(u64::from(slot.stride) * u64::from(index))
            else {
                return false;
            };
            let Some(end) = at.checked_add(u64::from(slot.element)) else {
                return false;
            };
            let Some(snapshot_end) = slot.snapshot_address.checked_add(slot.snapshot_length) else {
                return false;
            };
            if at < slot.snapshot_address || end > snapshot_end {
                return false;
            }
        }
    }
    if vertex_count == 0 {
        return true;
    }
    // SAFETY: caller provides the declared output range.
    let output = unsafe {
        core::slice::from_raw_parts_mut(out, vertex_count as usize * slot_count as usize * 4)
    };
    for (vertex, &index) in indices.iter().enumerate() {
        for (slot_index, slot) in slots.iter().enumerate() {
            let dst = &mut output[(vertex * slot_count as usize + slot_index) * 4..][..4];
            dst.copy_from_slice(&[0.0, 0.0, 0.0, 1.0]);
            if slot.mode == FAST_DISABLED {
                continue;
            }
            let at = slot.array_address + u64::from(slot.stride) * u64::from(index);
            // SAFETY: complete batch range validation above covers this element.
            let raw = unsafe {
                core::slice::from_raw_parts(
                    slot.bytes.add((at - slot.snapshot_address) as usize),
                    slot.element as usize,
                )
            };
            match slot.mode {
                FAST_D3D => {
                    dst[0] = f32::from(raw[2]) / 255.0;
                    dst[1] = f32::from(raw[1]) / 255.0;
                    dst[2] = f32::from(raw[0]) / 255.0;
                    dst[3] = f32::from(raw[3]) / 255.0;
                }
                FAST_CMP => {
                    let word = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
                    dst[0] = ((word << 21) as i32 >> 21) as f32 / 1023.0;
                    dst[1] = ((word << 10) as i32 >> 21) as f32 / 1023.0;
                    dst[2] = (word as i32 >> 22) as f32 / 511.0;
                }
                FAST_S32K => {
                    for lane in 0..slot.lanes as usize {
                        dst[lane] = i16::from_le_bytes([raw[lane * 2], raw[lane * 2 + 1]]) as f32;
                    }
                }
                FAST_FLOAT => {
                    for lane in 0..slot.lanes as usize {
                        dst[lane] = f32::from_bits(u32::from_le_bytes(
                            raw[lane * 4..lane * 4 + 4].try_into().unwrap(),
                        ));
                    }
                }
                _ => return false,
            }
        }
    }
    true
}

/// Expand one NV2A primitive into point, line, or triangle-list indices.
///
/// # Safety
/// For nonzero `count`, `indices` must reference `count` readable `u32`s. For nonzero
/// `capacity`, `out` must reference `capacity` writable `u32`s. The input and output ranges
/// must not overlap.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_render_native_expand_primitive(
    primitive: u32,
    indices: *const u32,
    count: u32,
    out: *mut u32,
    capacity: u32,
) -> u32 {
    if (count != 0 && indices.is_null()) || (capacity != 0 && out.is_null()) {
        return INVALID;
    }
    // SAFETY: caller contract above supplies exactly the declared ranges.
    let input = if count == 0 {
        &[]
    } else {
        // SAFETY: caller contract above supplies exactly the declared range.
        unsafe { core::slice::from_raw_parts(indices, count as usize) }
    };
    // SAFETY: caller contract above supplies exactly the declared ranges.
    let output = if capacity == 0 {
        &mut []
    } else {
        // SAFETY: caller contract above supplies exactly the declared range.
        unsafe { core::slice::from_raw_parts_mut(out, capacity as usize) }
    };
    expand(primitive, input, output).unwrap_or(INVALID)
}

fn expand(primitive: u32, indices: &[u32], out: &mut [u32]) -> Option<u32> {
    let mut used = 0usize;
    let mut emit = |values: &[u32]| {
        let end = used.checked_add(values.len())?;
        let target = out.get_mut(used..end)?;
        target.copy_from_slice(values);
        used = end;
        Some(())
    };

    match primitive {
        1 => emit(indices)?, // POINTS
        2 => {
            for pair in indices.chunks_exact(2) {
                emit(pair)?;
            }
        }
        4 => {
            for pair in indices.windows(2) {
                emit(pair)?;
            }
        }
        5 => {
            for triangle in indices.chunks_exact(3) {
                emit(triangle)?;
            }
        }
        6 => {
            for (i, strip) in indices.windows(3).enumerate() {
                if i & 1 == 0 {
                    emit(strip)?;
                } else {
                    emit(&[strip[1], strip[0], strip[2]])?;
                }
            }
        }
        7 => {
            if let Some((&pivot, rest)) = indices.split_first() {
                for pair in rest.windows(2) {
                    emit(&[pivot, pair[0], pair[1]])?;
                }
            }
        }
        8 => {
            for quad in indices.chunks_exact(4) {
                emit(&[quad[0], quad[1], quad[2]])?;
                emit(&[quad[0], quad[2], quad[3]])?;
            }
        }
        _ => return None,
    }
    u32::try_from(used).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        FAST_CMP, FAST_D3D, FAST_DISABLED, FAST_FLOAT, FAST_S32K, INVALID, NativeAttributeSlot,
        expand,
    };
    use std::vec;
    use std::vec::Vec;

    #[test]
    fn native_viewport_constants_preserve_stream_and_materialize_target_pair() {
        let mut input = [0.0f32; 768];
        input[58 * 4..59 * 4].copy_from_slice(&[11.0, 12.0, 13.0, 14.0]);
        input[59 * 4..60 * 4].copy_from_slice(&[21.0, 22.0, 23.0, 24.0]);
        let scale = [1.0, 2.0, 3.0, 4.0];
        let offset = [5.0, 6.0, 7.0, 8.0];
        let mut out = [f32::NAN; 768];
        // SAFETY: all arguments reference the fixed-size buffers required by the ABI.
        assert!(unsafe {
            super::tsfp_render_native_materialize_constants(
                input.as_ptr(),
                0,
                0,
                0,
                0,
                scale.as_ptr(),
                offset.as_ptr(),
                1,
                0,
                640,
                480,
                out.as_mut_ptr(),
            )
        });
        assert_eq!(&out[58 * 4..59 * 4], &[320.0, -240.0, 1.0, 0.0]);
        assert_eq!(&out[59 * 4..60 * 4], &[320.0, 240.0, 0.0, 0.0]);
        assert_eq!(out[0], 0.0);

        // A stream-authored scale survives target derivation; only the missing offset is filled.
        // SAFETY: same valid fixed-size buffers as above.
        assert!(unsafe {
            super::tsfp_render_native_materialize_constants(
                input.as_ptr(),
                1,
                0,
                0,
                0,
                scale.as_ptr(),
                offset.as_ptr(),
                1,
                0,
                640,
                480,
                out.as_mut_ptr(),
            )
        });
        assert_eq!(&out[58 * 4..59 * 4], &[11.0, 12.0, 13.0, 14.0]);
        assert_eq!(&out[59 * 4..60 * 4], &[320.0, 240.0, 0.0, 0.0]);
    }

    #[test]
    fn native_viewport_constants_window_clip_replaces_lone_offset() {
        let mut input = [0.0f32; 768];
        input[59 * 4..60 * 4].copy_from_slice(&[21.0, 22.0, 23.0, 24.0]);
        let register_scale = [0.0; 4];
        let register_offset = [1.0, 2.0, 3.0, 4.0];
        let mut out = [0.0f32; 768];
        // SAFETY: all arguments reference the fixed-size buffers required by the ABI.
        assert!(unsafe {
            super::tsfp_render_native_materialize_constants(
                input.as_ptr(),
                0,
                1,
                0,
                0,
                register_scale.as_ptr(),
                register_offset.as_ptr(),
                1,
                1,
                800,
                600,
                out.as_mut_ptr(),
            )
        });
        assert_eq!(&out[58 * 4..59 * 4], &[400.0, -300.0, 1.0, 0.0]);
        assert_eq!(&out[59 * 4..60 * 4], &[21.0, 22.0, 23.0, 24.0]);
    }

    #[test]
    fn native_attribute_classifier_preserves_supported_and_fallback_modes() {
        let bytes = [0u8; 16];
        let mut slot = NativeAttributeSlot {
            bytes: core::ptr::null(),
            snapshot_address: 0,
            snapshot_length: 0,
            array_address: 0,
            stride: 0,
            lanes: 0,
            element: 0,
            mode: 99,
        };
        // SAFETY: `slot` is writable and `bytes` stays alive for the call.
        assert!(unsafe {
            super::tsfp_render_native_prepare_attribute_slot(
                (16 << 8) | (4 << 4) | 2,
                1,
                1,
                0x1000,
                bytes.as_ptr(),
                0x1000,
                bytes.len() as u64,
                &mut slot,
            )
        });
        assert_eq!(
            (slot.mode, slot.stride, slot.lanes, slot.element),
            (FAST_FLOAT, 16, 4, 16)
        );

        for (kind, lanes, mode) in [(0, 4, FAST_D3D), (6, 1, FAST_CMP), (5, 2, FAST_S32K)] {
            let format = (16 << 8) | (lanes << 4) | kind;
            // SAFETY: `slot` is writable and `bytes` stays alive for the call.
            assert!(unsafe {
                super::tsfp_render_native_prepare_attribute_slot(
                    format,
                    1,
                    1,
                    0x1000,
                    bytes.as_ptr(),
                    0x1000,
                    bytes.len() as u64,
                    &mut slot,
                )
            });
            assert_eq!(
                (slot.mode, slot.stride, slot.lanes, slot.element),
                (mode, 16, lanes, 4)
            );
        }

        // SAFETY: `slot` is writable; disabled formats need no byte source.
        assert!(unsafe {
            super::tsfp_render_native_prepare_attribute_slot(
                0,
                0,
                0,
                0,
                core::ptr::null(),
                0,
                0,
                &mut slot,
            )
        });
        assert_eq!((slot.mode, slot.lanes, slot.element), (FAST_DISABLED, 0, 0));

        // SAFETY: `slot` is writable and the source range is valid for this call.
        assert!(unsafe {
            super::tsfp_render_native_prepare_attribute_slot(
                (4 << 8) | (4 << 4) | 4,
                1,
                1,
                0x1000,
                bytes.as_ptr(),
                0x1000,
                bytes.len() as u64,
                &mut slot,
            )
        });
        assert_eq!(
            slot.mode, 0,
            "unsupported UB_OGL remains on the C slow path"
        );
    }

    #[test]
    fn native_attribute_inference_requirements_preserve_order() {
        use super::{
            FAST_CMP, FAST_D3D, FAST_DISABLED, FAST_FLOAT, FAST_S32K,
            tsfp_render_native_attribute_inference_mask,
        };
        assert_eq!(
            tsfp_render_native_attribute_inference_mask(FAST_DISABLED, 4),
            0x4_0000_0000
        );
        assert_eq!(
            tsfp_render_native_attribute_inference_mask(FAST_D3D, 4),
            0x8_0000_0000
        );
        assert_eq!(
            tsfp_render_native_attribute_inference_mask(FAST_CMP, 3),
            0x400_0000_0004
        );
        assert_eq!(
            tsfp_render_native_attribute_inference_mask(FAST_S32K, 2),
            0x40_0000_0004
        );
        assert_eq!(
            tsfp_render_native_attribute_inference_mask(FAST_FLOAT, 3),
            0x4_0000_0000
        );
        assert_eq!(
            tsfp_render_native_attribute_inference_mask(FAST_FLOAT, 4),
            0
        );
        assert_eq!(tsfp_render_native_attribute_inference_mask(0, 0), u64::MAX);
    }

    fn output(primitive: u32, input: &[u32], capacity: usize) -> Result<Vec<u32>, u32> {
        let mut output = vec![0; capacity];
        match expand(primitive, input, &mut output) {
            Some(used) => {
                output.truncate(used as usize);
                Ok(output)
            }
            None => Err(INVALID),
        }
    }

    #[test]
    fn native_draw_expansion_matches_nv2a_list_rules() {
        assert_eq!(output(1, &[4, 7], 2), Ok(vec![4, 7]));
        assert_eq!(output(2, &[4, 7, 9], 2), Ok(vec![4, 7]));
        assert_eq!(output(4, &[4, 7, 9], 4), Ok(vec![4, 7, 7, 9]));
        assert_eq!(output(5, &[4, 7, 9, 10], 3), Ok(vec![4, 7, 9]));
        assert_eq!(output(6, &[4, 7, 9, 10], 6), Ok(vec![4, 7, 9, 9, 7, 10]));
        assert_eq!(output(7, &[4, 7, 9, 10], 6), Ok(vec![4, 7, 9, 4, 9, 10]));
        assert_eq!(output(8, &[4, 7, 9, 10], 6), Ok(vec![4, 7, 9, 4, 9, 10]));
    }

    #[test]
    fn native_draw_expansion_refuses_unsupported_or_short_output() {
        assert_eq!(output(3, &[0, 1, 2], 8), Err(INVALID));
        assert_eq!(output(8, &[0, 1, 2, 3], 5), Err(INVALID));
    }

    #[test]
    fn native_primitive_resolution_preserves_point_line_and_triangle_rules() {
        assert_eq!(
            super::tsfp_render_native_resolve_primitive(1, 1.0),
            super::NATIVE_PRIMITIVE_NEEDS_POINT_SIZE
        );
        assert_eq!(
            super::tsfp_render_native_resolve_primitive(2, 0.0),
            super::NATIVE_PRIMITIVE_NEEDS_LINE_WIDTH
        );
        assert_eq!(super::tsfp_render_native_resolve_primitive(2, 1.0), 0);
        assert_eq!(
            super::tsfp_render_native_resolve_primitive(2, 2.0),
            super::NATIVE_PRIMITIVE_UNSUPPORTED_LINE_WIDTH
        );
        assert_eq!(super::tsfp_render_native_resolve_primitive(0, 0.0), 0);
    }

    #[test]
    fn native_draw_abi_writes_only_to_the_caller_buffer() {
        let input = [2, 5, 7, 11];
        let mut output = [u32::MAX; 6];
        // SAFETY: both pointers reference the declared nonoverlapping stack arrays.
        let count = unsafe {
            super::tsfp_render_native_expand_primitive(
                6,
                input.as_ptr(),
                input.len() as u32,
                output.as_mut_ptr(),
                output.len() as u32,
            )
        };
        assert_eq!(count, 6);
        assert_eq!(output, [2, 5, 7, 7, 5, 11]);
    }

    #[test]
    fn native_attribute_decoder_matches_c_format_rules() {
        let floats = [1.25f32, -2.5, 9.0, 0.5, 7.0, 8.0, 9.0, 10.0];
        let colours = [3u8, 20, 240, 128, 40, 80, 120, 160];
        let packed = [0x00, 0x02, 0x04, 0x80, 0x00, 0xFC, 0xFF, 0x7F];
        let shorts = [0xFF, 0xFF, 0x00, 0x80, 0x34, 0x12, 0xFE, 0xFF];
        let slots = [
            NativeAttributeSlot {
                bytes: floats.as_ptr().cast(),
                snapshot_address: 100,
                snapshot_length: 32,
                array_address: 100,
                stride: 16,
                lanes: 4,
                element: 16,
                mode: FAST_FLOAT,
            },
            NativeAttributeSlot {
                bytes: colours.as_ptr(),
                snapshot_address: 200,
                snapshot_length: 8,
                array_address: 200,
                stride: 4,
                lanes: 4,
                element: 4,
                mode: FAST_D3D,
            },
            NativeAttributeSlot {
                bytes: packed.as_ptr(),
                snapshot_address: 300,
                snapshot_length: 8,
                array_address: 300,
                stride: 4,
                lanes: 3,
                element: 4,
                mode: FAST_CMP,
            },
            NativeAttributeSlot {
                bytes: shorts.as_ptr(),
                snapshot_address: 400,
                snapshot_length: 8,
                array_address: 400,
                stride: 4,
                lanes: 2,
                element: 4,
                mode: FAST_S32K,
            },
            NativeAttributeSlot {
                bytes: core::ptr::null(),
                snapshot_address: 0,
                snapshot_length: 0,
                array_address: 0,
                stride: 0,
                lanes: 0,
                element: 0,
                mode: FAST_DISABLED,
            },
        ];
        let indices = [0u32, 1];
        let mut out = [0.0f32; 2 * 5 * 4];
        // SAFETY: descriptors, indices, source bytes and output all live for this call.
        assert!(unsafe {
            super::tsfp_render_native_decode_attributes(
                slots.as_ptr(),
                5,
                indices.as_ptr(),
                2,
                out.as_mut_ptr(),
            )
        });
        assert_eq!(&out[0..4], &[1.25, -2.5, 9.0, 0.5]);
        assert_eq!(&out[20..24], &[7.0, 8.0, 9.0, 10.0]);
        assert_eq!(
            &out[24..28],
            &[120.0 / 255.0, 80.0 / 255.0, 40.0 / 255.0, 160.0 / 255.0]
        );
        let packed_second = u32::from_le_bytes([0x00, 0xFC, 0xFF, 0x7F]);
        assert_eq!(
            out[28],
            ((packed_second << 21) as i32 >> 21) as f32 / 1023.0
        );
        assert_eq!(&out[32..36], &[4660.0, -2.0, 0.0, 1.0]);
        assert_eq!(&out[36..40], &[0.0, 0.0, 0.0, 1.0]);
    }
}

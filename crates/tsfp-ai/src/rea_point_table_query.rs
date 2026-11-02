//! Typed record/query boundary for original helper `0x9BCF0`.
//!
//! The Rust implementation receives table rows and globals as inputs rather
//! than reading the guest table at `0x715F2C` itself. Semantic labels for the
//! row flags and coordinates remain unknown.

/// One 0x20-byte row from the original point table.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointEntryRaw {
    pub opaque_00: u32,
    pub flags_04: u8,
    pub reserved_05_0b: [u8; 7],
    pub coordinates_c_10_14: [f32; 3],
    pub reserved_18_1f: [u8; 8],
}

const _: [(); 0x20] = [(); core::mem::size_of::<PointEntryRaw>()];

/// Raw inputs corresponding to `0x9BCF0(arg4, arg8)` and its globals.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointQueryRaw {
    pub coordinates_00_08: [f32; 3],
    pub arg8_value: f32,
    pub min_squared_bias_475d1c: f32,
    pub initial_best_475d5c: f32,
}

/// Select the first nearest active row whose measured squared distance meets
/// the helper's lower threshold. `None` corresponds to original EAX `-1`.
pub fn select_nearest_entry_9bcf0(
    query: PointQueryRaw,
    entries: &[PointEntryRaw],
) -> Option<usize> {
    // COMISS 0,arg8 followed by JB takes this branch for positive or unordered
    // arg8 values. NaN therefore forms a NaN threshold and rejects all rows.
    let threshold = if query.arg8_value > 0.0 || query.arg8_value.is_nan() {
        query.arg8_value * query.arg8_value + query.min_squared_bias_475d1c
    } else {
        0.0
    };
    let mut best_distance = query.initial_best_475d5c;
    let mut best_index = None;

    for (index, entry) in entries.iter().enumerate() {
        if entry.flags_04 & 1 == 0 {
            continue;
        }

        let delta_x = entry.coordinates_c_10_14[0] - query.coordinates_00_08[0];
        let delta_z = entry.coordinates_c_10_14[2] - query.coordinates_00_08[2];
        let delta_y = entry.coordinates_c_10_14[1] - query.coordinates_00_08[1];
        let mut distance_squared = delta_z * delta_z;
        distance_squared += delta_x * delta_x;
        distance_squared += delta_y * delta_y;

        // These ordered comparisons reproduce the original COMISS/Jcc skips:
        // unordered distances, ties, and rows closer than the lower threshold
        // are rejected.
        if !(best_distance > distance_squared) || !(distance_squared >= threshold) {
            continue;
        }

        best_distance = distance_squared;
        best_index = Some(index);
    }

    best_index
}

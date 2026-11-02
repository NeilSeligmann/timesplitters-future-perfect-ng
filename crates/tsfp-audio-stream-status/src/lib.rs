//! Admission for the measured passive stream GetStatus response.

/// The arguments are snapshots from the Rust-owned stream lease. A true result
/// admits only the original facade's measured startup and started shapes.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_status_admit_rust(
    flags: u32,
    mask: u32,
    rate: u32,
    channels: u32,
    block: u32,
    average: u32,
    packets: u32,
    volume_seen: u32,
    volume: i32,
    pause: u32,
    flush: u32,
    discontinuity: u32,
) -> bool {
    let spatial = flags == 0x10 && mask == 7;
    let stereo = flags == 0
        && mask == 0
        && rate == 44100
        && channels == 2
        && block == 72
        && average == 49612;
    let fresh = pause == 0 && flush == 0 && discontinuity == 0;
    (spatial || stereo)
        && packets == 3
        && volume_seen != 0
        && volume == -10000
        && (fresh || discontinuity != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_accepts_only_measured_shapes_and_lifecycle() {
        let admit = |flags,
                     mask,
                     rate,
                     channels,
                     block,
                     average,
                     packets,
                     seen,
                     volume,
                     pause,
                     flush,
                     disc| {
            tsfp_audio_stream_status_admit_rust(
                flags, mask, rate, channels, block, average, packets, seen, volume, pause, flush,
                disc,
            )
        };
        assert!(admit(0, 0, 44100, 2, 72, 49612, 3, 1, -10000, 0, 0, 0));
        assert!(admit(16, 7, 0, 0, 0, 0, 3, 1, -10000, 1, 1, 1));
        assert!(!admit(0, 0, 44100, 2, 72, 49612, 3, 1, -10000, 1, 0, 0));
        assert!(!admit(0, 0, 44100, 2, 72, 49612, 2, 1, -10000, 0, 0, 0));
        assert!(!admit(16, 3, 0, 0, 0, 0, 3, 1, -10000, 0, 0, 0));
        assert!(!admit(0, 0, 44100, 2, 72, 49612, 3, 0, -10000, 0, 0, 0));
    }
}

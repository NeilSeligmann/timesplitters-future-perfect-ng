//! T1873: the typed sampler (`tsfp_engine_anim::pose::sample`) on the real clips of the owner disc against the
//! ORIGINAL `0x59320` run under the Unicorn oracle (golden hashes only, generator `tools/t1873_anim_gold.py`).
//! Skips (SKIP line) when the private ISO is absent.
use tsfp_engine_anim::pose::sample;
use tsfp_format_anim::Clip;

struct Frame {
    path: String,
    ordinal: u32,
    bytes: Vec<u8>,
}

fn disc_clips() -> Option<Vec<Frame>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let python = std::env::var("TSFP_TEST_PYTHON").unwrap_or("python3".into());
    let output = std::process::Command::new(python)
        .current_dir(root)
        .args(["-m", "tools.rust_anim_sample"])
        .output()
        .expect("Python clip reader");
    if output.status.code() == Some(77) {
        eprintln!("SKIP anim: private ISO absent");
        return None;
    }
    assert!(output.status.success(), "reader failed: {}", String::from_utf8_lossy(&output.stderr));
    let b = output.stdout;
    let (mut at, mut frames) = (0usize, Vec::new());
    let u32_at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    while at < b.len() {
        let n = u16::from_le_bytes([b[at], b[at + 1]]) as usize;
        let path = String::from_utf8(b[at + 2..at + 2 + n].to_vec()).unwrap();
        at += 2 + n;
        let (ordinal, len) = (u32_at(at), u32_at(at + 8) as usize);
        frames.push(Frame { path, ordinal, bytes: b[at + 12..at + 12 + len].to_vec() });
        at += 12 + len;
    }
    Some(frames)
}

fn key_set(keys: i32) -> Vec<i32> {
    let mut set = vec![0, keys / 2, keys - 1, keys + 3];
    set.sort_unstable();
    set.dedup();
    set
}

#[path = "support/fnv.rs"]
mod digest;
use digest::fnv;

#[test]
fn sampler_matches_the_original_on_every_disc_clip() {
    let Some(frames) = disc_clips() else { return };
    let golden = std::fs::read_to_string(format!(
        "{}/tests/data/t1873/clip_samples.golden",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("golden file");
    let expected: std::collections::BTreeMap<(String, u32), (usize, usize, u64)> = golden
        .lines()
        .map(|line| {
            let f: Vec<&str> = line.split(' ').collect();
            (
                (f[0].to_string(), f[1].parse().unwrap()),
                (f[2].parse().unwrap(), f[3].parse().unwrap(), u64::from_str_radix(f[4], 16).unwrap()),
            )
        })
        .collect();
    assert_eq!(expected.len(), frames.len(), "golden covers a different clip set");
    let (mut samples, mut unit, mut finite) = (0usize, 0usize, 0usize);
    let mut odd = std::collections::BTreeMap::new();
    for frame in &frames {
        let clip = Clip::parse(&frame.bytes).unwrap();
        let keys = clip.key_count() as i32;
        let (mut state, mut count) = (0xCBF2_9CE4_8422_2325u64, 0usize);
        for node in 0..clip.nodes.len() {
            for (index, key) in key_set(keys).into_iter().enumerate() {
                let scale = if index % 2 == 0 { f32::from_bits(0x3F80_0000) } else { f32::from_bits(0x3EBD_70A4) };
                let sample = sample(&clip, node, key, scale)
                    .unwrap_or_else(|e| panic!("{}#{} node {node} key {key}: {e:?}", frame.path, frame.ordinal));
                for word in sample.bits {
                    state = fnv(state, &word.to_le_bytes());
                }
                count += 1;
                let q = sample.quaternion();
                if q.iter().all(|c| c.is_finite()) {
                    finite += 1;
                    let is_unit = (q.iter().map(|c| c * c).sum::<f32>() - 1.0).abs() < 0.02;
                    unit += usize::from(is_unit);
                    if !is_unit {
                        *odd.entry((clip.nodes[node].selector(), clip.flags())).or_insert(0usize) += 1;
                    }
                }
            }
        }
        samples += count;
        let want = expected.get(&(frame.path.clone(), frame.ordinal)).expect("clip in golden");
        assert_eq!(*want, (clip.nodes.len(), count, state), "{}#{}", frame.path, frame.ordinal);
    }
    eprintln!("MEASURED anim sampler: {} clips, {samples} samples identical to the original; unit quaternions {unit}/{finite}", frames.len());
    eprintln!("non-unit by (selector, flags): {odd:?}");
    // Only the fixed form (selector 0: unused nodes hold a zero quaternion) and the float selector 2 (the original reads its
    // quaternion at D + 12 * key, T1273) produce non-unit quaternions; every keyed selector decodes to unit length.
    assert!(odd.keys().all(|(selector, _)| matches!(selector, 0 | 2)), "keyed track not unit length: {odd:?}");
    assert!(unit * 100 >= finite * 98, "quaternions are not unit length: layout wrong");
}

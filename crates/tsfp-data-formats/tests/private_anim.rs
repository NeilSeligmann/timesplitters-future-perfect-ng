//! T1873: real animation clips of the owner disc. Skips (SKIP line) when the private ISO is absent.
use tsfp_data_formats::anim::Clip;

pub struct Frame {
    pub path: String,
    pub ordinal: u32,
    pub key: u32,
    pub bytes: Vec<u8>,
}

pub fn disc_clips(only: &str) -> Option<Vec<Frame>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let python = std::env::var("TSFP_TEST_PYTHON").unwrap_or("python3".into());
    let output = std::process::Command::new(python)
        .current_dir(root)
        .args(["-m", "tools.rust_anim_sample", "--only", only])
        .output()
        .expect("Python clip reader");
    if output.status.code() == Some(77) {
        eprintln!("SKIP anim: private ISO absent");
        return None;
    }
    assert!(output.status.success(), "reader failed: {}", String::from_utf8_lossy(&output.stderr));
    let b = output.stdout;
    let (mut at, mut frames) = (0usize, Vec::new());
    let u32_at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as usize;
    while at < b.len() {
        let n = u16::from_le_bytes([b[at], b[at + 1]]) as usize;
        let path = String::from_utf8(b[at + 2..at + 2 + n].to_vec()).unwrap();
        at += 2 + n;
        let (ordinal, key, len) = (u32_at(at), u32_at(at + 4), u32_at(at + 8));
        frames.push(Frame { path, ordinal: ordinal as u32, key: key as u32, bytes: b[at + 12..at + 12 + len].to_vec() });
        at += 12 + len;
    }
    Some(frames)
}

#[test]
fn every_disc_clip_round_trips_byte_identical() {
    let Some(frames) = disc_clips("") else { return };
    assert!(frames.len() > 3200, "only {} resident clips", frames.len());
    let (mut tails, mut distance_clips, mut nodes) = (std::collections::BTreeMap::new(), 0, 0usize);
    for f in &frames {
        let clip = Clip::parse(&f.bytes).unwrap_or_else(|e| panic!("{}#{}: {e}", f.path, f.ordinal));
        let encoded = clip.encode().unwrap();
        assert!(encoded == f.bytes, "{}#{} differs", f.path, f.ordinal);
        *tails.entry(clip.tail.len()).or_insert(0usize) += 1;
        distance_clips += usize::from(!clip.distances.is_empty());
        nodes += clip.nodes.len();
    }
    eprintln!("MEASURED anim: {} clips, {nodes} nodes, {distance_clips} with distance table, tail length histogram {tails:?}", frames.len());
    assert!(tails.keys().all(|t| *t == 0 || *t == 8), "unexpected tail sizes {tails:?}");
}

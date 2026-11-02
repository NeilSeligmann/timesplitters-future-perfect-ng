//! PAK member name recovery. A PAK stores only the CRC32 of the lower-case logical path, so a
//! name is recovered by hashing candidate strings found in other game data (the XBE string
//! table, name lists passed with `--names`) and keeping those that equal a member key.
use std::collections::{BTreeMap, HashMap, HashSet};
use tsfp_format_core::crc32;

/// Printable ASCII runs of at least `min` bytes (no spaces split: paths have none).
pub fn ascii_runs(b: &[u8], min: usize) -> Vec<&str> {
    let mut out = Vec::new();
    let mut s = 0;
    for i in 0..=b.len() {
        let printable = i < b.len() && (0x21..0x7f).contains(&b[i]);
        if !printable {
            if i - s >= min {
                out.push(std::str::from_utf8(&b[s..i]).unwrap());
            }
            s = i + 1;
        }
    }
    out
}

/// Normalised forms of a candidate that the game could have hashed.
fn variants(c: &str) -> [String; 2] {
    [c.to_string(), c.to_ascii_lowercase().replace('\\', "/")]
}

/// Recover `key -> name` for every candidate string whose CRC32 is in `keys`.
pub fn recover<'a>(
    keys: &HashSet<u32>,
    candidates: impl IntoIterator<Item = &'a str>,
) -> BTreeMap<u32, String> {
    let mut found = BTreeMap::new();
    for c in candidates {
        for v in variants(c) {
            let k = crc32(v.as_bytes());
            if keys.contains(&k) {
                found.entry(k).or_insert(v);
            }
        }
    }
    found
}

/// Path-like strings only (a slash or dot, a restricted alphabet): this cuts the chance matches
/// that a 32-bit hash gives over ~1.3M candidate strings (about 8 expected without it).
pub fn plausible(name: &str) -> bool {
    name.len() >= 5
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_./-".contains(&b))
        && (name.contains('/') || name.contains('.'))
}

/// Build the key table from builtin names, a user list and XBE-style binaries.
pub fn table(builtin: &[&str], extra: &[String], binaries: &[Vec<u8>]) -> HashMap<u32, String> {
    let mut cands: Vec<&str> = builtin.to_vec();
    cands.extend(extra.iter().map(String::as_str));
    for b in binaries {
        cands.extend(ascii_runs(b, 5));
    }
    let mut t = HashMap::new();
    for c in cands {
        for v in variants(c).into_iter().filter(|v| plausible(v)) {
            t.entry(crc32(v.as_bytes())).or_insert(v);
        }
    }
    t
}

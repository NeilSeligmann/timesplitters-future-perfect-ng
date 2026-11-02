//! Manifests: stage A (source files) and stage B (converted outputs), one pair per source.
//!
//! Layout under the output root `OUT`:
//!  - `OUT/source/<id>/<path>`: stage A loose files, byte-identical to the image.
//!  - `OUT/converted/<id>/<category>/<name>`: stage B open-format outputs.
//!  - `OUT/manifest/<id>.stage-a.json`, `OUT/manifest/<id>.stage-b.json`.
//!  - `OUT/overlay/`: user/mod files shadowing imported ones by logical id (`<category>/<name>`).
//!
//! Several sources (xbox-usa, ps2-usa, gc-usa) coexist as siblings. Their logical ids line up so
//! the runtime picks overlay, then the highest-priority source that has the id.
use crate::Result;
use crate::json::quote;
use std::collections::BTreeMap;

pub const SCHEMA: u32 = 1;

pub struct StageAFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

pub struct Output {
    pub logical_id: String,
    pub source_path: String,
    pub source_sha256: String,
    pub converter: String,
    pub converter_version: u32,
    pub sha256: String,
    pub size: u64,
    /// `exact`: re-encoding reproduces the source member byte for byte. Otherwise the loss note.
    pub lossless: bool,
    pub note: String,
}

pub fn stage_a_json(id: &str, platform: &str, layout: &str, files: &[StageAFile]) -> String {
    let mut s = format!(
        "{{\"schema\":{SCHEMA},\"stage\":\"A\",\"source\":{{\"id\":{},\"platform\":{},\"layout\":{}}},\"files\":[\n",
        quote(id),
        quote(platform),
        quote(layout)
    );
    let rows: Vec<String> = files
        .iter()
        .map(|f| {
            format!(
                "{{\"path\":{},\"size\":{},\"sha256\":{}}}",
                quote(&f.path),
                f.size,
                quote(&f.sha256)
            )
        })
        .collect();
    s.push_str(&rows.join(",\n"));
    s.push_str("\n]}\n");
    s
}

pub fn stage_b_json(id: &str, fingerprint: &str, outputs: &[Output]) -> String {
    let mut s = format!(
        "{{\"schema\":{SCHEMA},\"stage\":\"B\",\"source\":{},\"fingerprint\":{},\"outputs\":[\n",
        quote(id),
        quote(fingerprint)
    );
    let rows: Vec<String> = outputs
        .iter()
        .map(|o| {
            format!(
                "{{\"logical_id\":{},\"source_path\":{},\"source_sha256\":{},\"converter\":{},\"converter_version\":{},\"size\":{},\"sha256\":{},\"lossless\":{},\"note\":{}}}",
                quote(&o.logical_id), quote(&o.source_path), quote(&o.source_sha256), quote(&o.converter), o.converter_version, o.size, quote(&o.sha256), o.lossless, quote(&o.note)
            )
        })
        .collect();
    s.push_str(&rows.join(",\n"));
    s.push_str("\n]}\n");
    s
}

/// Extract `(key, value)` pairs from the flat row objects this module writes, for `verify`.
/// Values are returned unescaped for strings and verbatim for numbers and booleans.
pub fn parse_rows(text: &str) -> Result<Vec<BTreeMap<String, String>>> {
    let mut rows = Vec::new();
    for line in text.lines() {
        let l = line.trim().trim_end_matches(',');
        if !l.starts_with("{\"") || !l.ends_with('}') || l.starts_with("{\"schema\"") {
            continue;
        }
        let c: Vec<char> = l.chars().collect();
        let (mut i, mut m) = (1, BTreeMap::new());
        while i < c.len() && c[i] == '"' {
            let (k, n) = string(&c, i)?;
            i = n + 2;
            let v = if c[i] == '"' {
                let (v, n) = string(&c, i)?;
                i = n + 1;
                v
            } else {
                let s = i;
                while c[i] != ',' && c[i] != '}' {
                    i += 1;
                }
                c[s..i].iter().collect()
            };
            m.insert(k, v);
            if c[i] == ',' {
                i += 1;
            }
        }
        rows.push(m);
    }
    Ok(rows)
}

fn string(c: &[char], start: usize) -> Result<(String, usize)> {
    let (mut i, mut o) = (start + 1, String::new());
    while i < c.len() {
        match c[i] {
            '"' => return Ok((o, i)),
            '\\' => {
                i += 1;
                match c.get(i) {
                    Some('n') => o.push('\n'),
                    Some('r') => o.push('\r'),
                    Some('t') => o.push('\t'),
                    Some('u') => {
                        let h: String = c.get(i + 1..i + 5).ok_or("bad escape")?.iter().collect();
                        o.push(
                            char::from_u32(u32::from_str_radix(&h, 16).map_err(|e| e.to_string())?)
                                .ok_or("bad escape")?,
                        );
                        i += 4;
                    }
                    Some(x) => o.push(*x),
                    None => return Err("bad escape".into()),
                }
            }
            x => o.push(x),
        }
        i += 1;
    }
    Err("unterminated string".into())
}

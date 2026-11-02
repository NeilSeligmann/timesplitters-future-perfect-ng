//! Lossless structured sidecars for recognized script and cutscene members.
//!
//! A converter is selected only by a known path or a format signature. The game-format codec
//! must re-encode the input byte-for-byte before a JSON sidecar is returned. Unsupported or
//! malformed candidates are reported to the caller so the original member can remain raw.

use crate::json;

pub const CSINFO_JSON: (&str, u32) = ("csinfo-json", 1);
pub const IGCS_JSON: (&str, u32) = ("igcs-json", 1);
pub const MKR_JSON: (&str, u32) = ("mkr-json", 1);

pub struct Conversion {
    pub converter: (&'static str, u32),
    pub bytes: Vec<u8>,
    pub note: &'static str,
}

#[derive(Clone, Copy)]
enum Kind {
    Csinfo,
    Igcs,
    Mkr,
}

pub fn supports_path(path: &str) -> bool {
    path_kind(path).is_some()
}

/// Try formats identified by the logical member path. `Some(Err(_))` means that the path
/// identifies a supported format but its bytes failed validation and should be kept raw.
pub fn convert_path(path: &str, bytes: &[u8]) -> Option<Result<Conversion, String>> {
    path_kind(path).map(|kind| convert(kind, bytes))
}

/// Try formats with unambiguous binary signatures when a PAK name was not recovered. CSINFO is
/// path-selected because its text tokens overlap ordinary text members.
pub fn convert_sniffed(bytes: &[u8]) -> Option<Result<Conversion, String>> {
    if bytes.starts_with(b"ANRS") {
        return Some(convert(Kind::Igcs, bytes));
    }
    if bytes.starts_with(&tsfp_format_script::mkr::MAGIC.to_le_bytes()) {
        return Some(convert(Kind::Mkr, bytes));
    }
    None
}

pub fn output_path(path: &str) -> String {
    let lower = path.to_ascii_lowercase();
    let suffix = [".txt", ".csinfo", ".mkr"]
        .into_iter()
        .find(|suffix| lower.ends_with(suffix))
        .unwrap_or("");
    if suffix.is_empty() {
        format!("{path}.json")
    } else {
        format!("{}.json", &path[..path.len() - suffix.len()])
    }
}

pub fn converter_known(name: &str) -> bool {
    [CSINFO_JSON.0, IGCS_JSON.0, MKR_JSON.0].contains(&name)
}

pub fn rebuild(converter: &str, original: &[u8]) -> Result<Vec<u8>, String> {
    let kind = match converter {
        value if value == CSINFO_JSON.0 => Kind::Csinfo,
        value if value == IGCS_JSON.0 => Kind::Igcs,
        value if value == MKR_JSON.0 => Kind::Mkr,
        _ => return Err(format!("unknown structured converter {converter}")),
    };
    convert(kind, original).map(|converted| converted.bytes)
}

fn path_kind(path: &str) -> Option<Kind> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".mkr") {
        Some(Kind::Mkr)
    } else if lower.ends_with(".csinfo") || lower.ends_with("csinfo.txt") {
        Some(Kind::Csinfo)
    } else {
        None
    }
}

fn convert(kind: Kind, original: &[u8]) -> Result<Conversion, String> {
    match kind {
        Kind::Csinfo => csinfo(original),
        Kind::Igcs => igcs(original),
        Kind::Mkr => mkr(original),
    }
}

fn csinfo(original: &[u8]) -> Result<Conversion, String> {
    let doc = tsfp_format_csinfo::Csinfo::parse(original)
        .map_err(|error| format!("csinfo parser rejected member: {error}"))?;
    let encoded = doc
        .encode()
        .map_err(|error| format!("csinfo encoder rejected records: {error}"))?;
    if encoded != original {
        return Err("csinfo canonical encoding changes source whitespace or terminator".into());
    }
    let records = doc
        .records
        .iter()
        .map(|record| {
            let args = record
                .args
                .iter()
                .map(|arg| json::quote(arg))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"directive\":{},\"args\":[{}]}}",
                json::quote(&record.directive),
                args
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let sidecar = format!(
        "{{\n\"format\":\"tsfp-csinfo\",\n\"version\":1,\n\"records\":[\n{}\n]\n}}\n",
        records
    );
    Ok(Conversion {
        converter: CSINFO_JSON,
        bytes: sidecar.into_bytes(),
        note: "csinfo records serialized; canonical codec output equals the original bytes",
    })
}

fn igcs(original: &[u8]) -> Result<Conversion, String> {
    let doc = tsfp_format_igcs::Animation::parse(original)
        .map_err(|error| format!("ANRS parser rejected member: {error}"))?;
    if doc
        .encode()
        .map_err(|error| format!("ANRS encoder rejected member: {error}"))?
        != original
    {
        return Err("ANRS codec output differs from the original bytes".into());
    }
    let words = |values: &[u32]| {
        values
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    let descriptors = |items: &[tsfp_format_igcs::Descriptor]| {
        items
            .iter()
            .map(|item| {
                format!(
                    "{{\"offset\":{},\"words\":[{}]}}",
                    item.offset,
                    words(&item.words)
                )
            })
            .collect::<Vec<_>>()
            .join(",\n")
    };
    let sidecar = format!(
        "{{\n\"format\":\"tsfp-igcs-anrs\",\n\"version\":1,\n\"header_words\":[{}],\n\"nodes\":[\n{}\n],\n\"animations\":[\n{}\n],\n\"original_bytes_hex\":\"{}\"\n}}\n",
        words(&doc.header),
        descriptors(&doc.nodes),
        descriptors(&doc.animations),
        hex(original)
    );
    Ok(Conversion {
        converter: IGCS_JSON,
        bytes: sidecar.into_bytes(),
        note: "ANRS descriptors serialized; retained original bytes preserve opaque regions",
    })
}

fn mkr(original: &[u8]) -> Result<Conversion, String> {
    use tsfp_format_script::mkr::{Mkr, TAG_RULE, TAG_SETUP};
    use tsfp_format_script::{Kind, Records};

    let doc =
        Mkr::parse(original).map_err(|error| format!("MKR parser rejected member: {error}"))?;
    if doc
        .encode()
        .map_err(|error| format!("MKR encoder rejected member: {error}"))?
        != original
    {
        return Err("MKR codec output differs from the original bytes".into());
    }
    let setups = doc
        .setups()
        .map_err(|error| format!("MKR setup record chunk rejected: {error}"))?;
    let rules = doc
        .rules()
        .map_err(|error| format!("MKR rule record chunk rejected: {error}"))?;
    for record in setups {
        let parsed = Records::parse(record, Kind::Setup)
            .map_err(|error| format!("MKR setup record rejected: {error}"))?;
        if parsed.encode().map_err(|error| error.to_string())? != record {
            return Err("MKR setup record did not re-encode exactly".into());
        }
    }
    for record in rules {
        let parsed = Records::parse(record, Kind::Rule)
            .map_err(|error| format!("MKR rule record rejected: {error}"))?;
        if parsed.encode().map_err(|error| error.to_string())? != record {
            return Err("MKR rule record did not re-encode exactly".into());
        }
    }
    let chunks = doc
        .chunks
        .iter()
        .map(|chunk| {
            let payload = hex(&chunk.payload);
            if chunk.tag == TAG_SETUP {
                format!(
                    "{{\"tag\":{},\"kind\":\"setup\",\"record_hex\":\"{}\",\"copy_overrun_hex\":\"{}\"}}",
                    chunk.tag,
                    hex(&chunk.payload[..0xb4]),
                    hex(&chunk.payload[0xb4..])
                )
            } else if chunk.tag == TAG_RULE {
                format!(
                    "{{\"tag\":{},\"kind\":\"rule\",\"record_hex\":\"{}\"}}",
                    chunk.tag, payload
                )
            } else {
                format!(
                    "{{\"tag\":{},\"kind\":\"opaque\",\"payload_hex\":\"{}\"}}",
                    chunk.tag, payload
                )
            }
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let sidecar = format!(
        "{{\n\"format\":\"tsfp-mapmaker-mkr\",\n\"version\":{},\n\"chunks\":[\n{}\n],\n\"tail_hex\":\"{}\"\n}}\n",
        doc.version,
        chunks,
        hex(&doc.tail)
    );
    Ok(Conversion {
        converter: MKR_JSON,
        bytes: sidecar.into_bytes(),
        note: "MKR chunks serialized; setup/rule records validated and opaque bytes retained",
    })
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

//! Minimal JSON string emission (no parser needed by the importer itself).
use std::fmt::Write;

/// Quote `s` as a JSON string.
pub fn quote(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Quote raw bytes as Latin-1 text (each byte maps to the code point of equal value, lossless).
pub fn quote_latin1(b: &[u8]) -> String {
    quote(&b.iter().map(|x| *x as char).collect::<String>())
}

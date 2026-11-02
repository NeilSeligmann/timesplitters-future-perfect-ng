//! Shared payload decoding for the live and replay audio command-log readers.

/// Preserve the log readers' existing policy: ignore a trailing nibble and
/// decode an invalid pair as zero. Byte indexing retains their UTF-8 panic
/// behaviour; these logs carry ASCII hexadecimal payloads.
pub(crate) fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap_or(0))
        .collect()
}

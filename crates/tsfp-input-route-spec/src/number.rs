// SPDX-License-Identifier: GPL-3.0-or-later
//! `number()` of the C grammar: `strtoull(text, base 0)` over the whole field, no sign, below a limit.

const SPACE: &[u8] = b" \t\n\x0b\x0c\r";

/// Parse like `strtoull(.., 0)` consuming the whole text (leading white space allowed, as libc does).
pub fn number(text: &[u8], limit: u64) -> Option<u64> {
    if text.is_empty() || text.len() >= 32 || text[0] == b'-' || text[0] == b'+' {
        return None;
    }
    let mut at = 0;
    while at < text.len() && SPACE.contains(&text[at]) {
        at += 1;
    }
    let rest = &text[at..];
    let (radix, digits) = match rest {
        [b'0', b'x' | b'X', tail @ ..] if tail.first().is_some_and(u8::is_ascii_hexdigit) => {
            (16, tail)
        }
        [b'0', ..] => (8, rest),
        _ => (10, rest),
    };
    if digits.is_empty() {
        return None;
    }
    let mut value: u64 = 0;
    for &byte in digits {
        let digit = (byte as char).to_digit(radix)?;
        value = value
            .checked_mul(u64::from(radix))?
            .checked_add(u64::from(digit))?;
    }
    (value <= limit).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::number;

    #[test]
    fn matches_strtoull_base_zero() {
        assert_eq!(number(b"10", 99), Some(10));
        assert_eq!(number(b"0x1F", 99), Some(31));
        assert_eq!(number(b"010", 99), Some(8));
        assert_eq!(number(b"09", 99), None);
        assert_eq!(number(b"0x", 99), None);
        assert_eq!(number(b"-1", 99), None);
        assert_eq!(number(b"+1", 99), None);
        assert_eq!(number(b"", 99), None);
        assert_eq!(number(b"100", 99), None);
        assert_eq!(number(b" 7", 99), Some(7));
        assert_eq!(number(b"7 ", 99), None);
    }
}

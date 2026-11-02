//! CSINFO whitespace token grammar. Encoding preserves semantic records while
//! normalizing separators to one space between tokens and one newline per record.
use tsfp_format_core::*;

/// A directive and its ordered arguments. Constructed records are checked on encode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub directive: String,
    pub args: Vec<String>,
}

/// A nonempty sequence of CSINFO records.
///
/// The parser accepts ASCII whitespace and optional zero padding after a single
/// terminator. It rejects unsupported directives and invalid frame counts.
///
/// ```
/// use tsfp_format_csinfo::Csinfo;
/// let info = Csinfo::parse(b"FRAMES 42\r\nSTREAM scene track\0\0")?;
/// assert_eq!(info.records.len(), 2);
/// assert_eq!(Csinfo::parse(&info.encode()?)?.records, info.records);
/// # Ok::<(), &'static str>(())
/// ```
#[derive(Debug, Clone)]
pub struct Csinfo {
    pub records: Vec<Record>,
}

fn arity(s: &str) -> Option<usize> {
    match s {
        "STREAM" | "CHR" => Some(2),
        "FRAMES" | "CAMERA" | "PROP" => Some(1),
        "ANIM" => Some(3),
        _ => None,
    }
}

impl Csinfo {
    pub fn parse(b: &[u8]) -> Result<Self> {
        let n = b.iter().position(|x| *x == 0).unwrap_or(b.len());
        if b[n..].iter().any(|x| *x != 0) || !b[..n].is_ascii() {
            return Err("non-ASCII or terminator tail");
        }
        let s = std::str::from_utf8(&b[..n]).unwrap();
        let t: Vec<_> = s.split_ascii_whitespace().collect();
        let mut records = Vec::new();
        let mut i = 0;
        while i < t.len() {
            let n = arity(t[i]).ok_or("unsupported directive")?;
            let args = t.get(i + 1..i + 1 + n).ok_or("truncated directive")?;
            if args.iter().any(|a| arity(a).is_some()) {
                return Err("truncated directive");
            }
            if t[i] == "FRAMES" && args[0].parse::<i32>().ok().is_none_or(|f| f <= 0) {
                return Err("invalid frame count");
            }
            records.push(Record {
                directive: t[i].into(),
                args: args.iter().map(|x| (*x).into()).collect(),
            });
            i += n + 1;
        }
        if records.is_empty() {
            return Err("empty csinfo");
        }
        Ok(Self { records })
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        // Public records can be edited after parsing. Reject malformed structure
        // before allocating a serialized copy of potentially large arguments.
        if self.records.is_empty() {
            return Err("empty csinfo");
        }
        for record in &self.records {
            let expected = arity(&record.directive).ok_or("unsupported directive")?;
            if record.args.len() != expected || record.args.iter().any(|arg| arity(arg).is_some()) {
                return Err("truncated directive");
            }
            if record.directive == "FRAMES"
                && record.args[0]
                    .parse::<i32>()
                    .ok()
                    .is_none_or(|frames| frames <= 0)
            {
                return Err("invalid frame count");
            }
            // Tokens are public strings. Check their byte grammar before building
            // a second, potentially large serialization of malformed input.
            for arg in &record.args {
                if !arg.is_ascii() || arg.contains('\0') {
                    return Err("non-ASCII or terminator tail");
                }
                if arg.is_empty() || arg.bytes().any(|byte| byte.is_ascii_whitespace()) {
                    return Err("invalid record tokens");
                }
            }
        }
        let mut s = String::new();
        for r in &self.records {
            s.push_str(&r.directive);
            for a in &r.args {
                s.push(' ');
                s.push_str(a);
            }
            s.push('\n');
        }
        let b = s.into_bytes();
        if Self::parse(&b)?.records != self.records {
            return Err("invalid record tokens");
        }
        Ok(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar() {
        let c = Csinfo::parse(b"FRAMES 42\r\nSTREAM a b\0\0").unwrap();
        assert_eq!(
            Csinfo::parse(&c.encode().unwrap()).unwrap().records,
            c.records
        );
        for b in [b"FRAMES 0".as_slice(), b"PROP", b"PROP x\0x", b"# nope"] {
            assert!(Csinfo::parse(b).is_err());
        }
    }

    #[test]
    fn separators_and_padding_preserve_records() {
        let tokens = [
            "FRAMES", "42", "STREAM", "scene", "track", "ANIM", "a", "b", "c",
        ];
        let expected = Csinfo::parse(tokens.join(" ").as_bytes()).unwrap().records;
        for separator in [" ", "\t", "\n", "\r\n", " \t\n"] {
            for padding in 0..=8 {
                let mut input = tokens.join(separator).into_bytes();
                input.extend(std::iter::repeat_n(0, padding));
                let parsed = Csinfo::parse(&input).unwrap();
                assert_eq!(parsed.records, expected);
                assert_eq!(
                    Csinfo::parse(&parsed.encode().unwrap()).unwrap().records,
                    expected
                );
            }
        }
    }

    #[test]
    fn invalid_frame_values_and_terminator_tails() {
        for frame in ["0", "-1", "x", "2147483648"] {
            assert!(Csinfo::parse(format!("FRAMES {frame}").as_bytes()).is_err());
        }
        for tail in [b"x".as_slice(), b"\n", b"\xff"] {
            let mut input = b"FRAMES 1\0".to_vec();
            input.extend_from_slice(tail);
            assert!(Csinfo::parse(&input).is_err());
        }
    }

    #[test]
    fn edited_records_are_checked_before_serialization() {
        let mut info = Csinfo::parse(b"FRAMES 2").unwrap();
        assert_eq!(Csinfo { records: vec![] }.encode(), Err("empty csinfo"));
        info.records[0].directive = "UNKNOWN".into();
        assert_eq!(info.encode(), Err("unsupported directive"));
        info.records[0].directive = "FRAMES".into();
        info.records[0].args.clear();
        assert_eq!(info.encode(), Err("truncated directive"));
        info.records[0].args.push("STREAM".into());
        assert_eq!(info.encode(), Err("truncated directive"));
        info.records[0].args.clear();
        info.records[0].args.push("0".into());
        assert_eq!(info.encode(), Err("invalid frame count"));
        info.records[0].args[0] = "3".into();
        assert_eq!(
            Csinfo::parse(&info.encode().unwrap()).unwrap().records,
            info.records
        );
    }

    #[test]
    fn edited_argument_tokens_are_checked_before_serialization() {
        let mut info = Csinfo::parse(b"STREAM scene track").unwrap();
        for invalid in ["", "two words", "two\twords", "two\nwords"] {
            info.records[0].args[0] = invalid.into();
            assert_eq!(info.encode(), Err("invalid record tokens"));
        }
        for invalid in ["café", "a\0b"] {
            info.records[0].args[0] = invalid.into();
            assert_eq!(info.encode(), Err("non-ASCII or terminator tail"));
        }
    }
}

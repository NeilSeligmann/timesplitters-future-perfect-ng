//! Minimal JSON reader for the menu-tree dump (no external dependency).
//! Numbers are held as `f64`, which is exact for every integer in the dump (below 2^53).

use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(BTreeMap<String, Json>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonError {
    pub offset: usize,
    pub message: &'static str,
}

impl Json {
    pub fn parse(text: &str) -> Result<Json, JsonError> {
        let mut parser = Parser {
            bytes: text.as_bytes(),
            pos: 0,
            depth: 0,
        };
        parser.skip_ws();
        let value = parser.value()?;
        parser.skip_ws();
        if parser.pos != parser.bytes.len() {
            return Err(parser.err("trailing characters"));
        }
        Ok(value)
    }
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(map) => map.get(key),
            _ => None,
        }
    }
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Json::Num(n) if *n >= 0.0 && n.fract() == 0.0 && *n < 9.0e15 => Some(*n as u64),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }
    pub fn as_object(&self) -> Option<&BTreeMap<String, Json>> {
        match self {
            Json::Obj(o) => Some(o),
            _ => None,
        }
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    depth: usize,
}

const MAX_DEPTH: usize = 64;

impl Parser<'_> {
    fn err(&self, message: &'static str) -> JsonError {
        JsonError {
            offset: self.pos,
            message,
        }
    }
    fn skip_ws(&mut self) {
        while matches!(self.bytes.get(self.pos), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }
    fn eat(&mut self, literal: &str, value: Json) -> Result<Json, JsonError> {
        if self.bytes[self.pos..].starts_with(literal.as_bytes()) {
            self.pos += literal.len();
            Ok(value)
        } else {
            Err(self.err("bad literal"))
        }
    }
    fn value(&mut self) -> Result<Json, JsonError> {
        match self.bytes.get(self.pos) {
            None => Err(self.err("unexpected end")),
            Some(b'n') => self.eat("null", Json::Null),
            Some(b't') => self.eat("true", Json::Bool(true)),
            Some(b'f') => self.eat("false", Json::Bool(false)),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b'[') => self.nested(|p| p.array()),
            Some(b'{') => self.nested(|p| p.object()),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.err("unexpected character")),
        }
    }
    fn nested(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<Json, JsonError>,
    ) -> Result<Json, JsonError> {
        if self.depth >= MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        self.depth += 1;
        let result = parse(self);
        self.depth -= 1;
        result
    }
    fn number(&mut self) -> Result<Json, JsonError> {
        let start = self.pos;
        if self.bytes.get(self.pos) == Some(&b'-') {
            self.pos += 1;
        }
        match self.bytes.get(self.pos) {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => self.digits(),
            _ => return Err(self.err("bad number")),
        }
        if self.bytes.get(self.pos) == Some(&b'.') {
            self.pos += 1;
            if !matches!(self.bytes.get(self.pos), Some(b'0'..=b'9')) {
                return Err(self.err("bad number"));
            }
            self.digits();
        }
        if matches!(self.bytes.get(self.pos), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.bytes.get(self.pos), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !matches!(self.bytes.get(self.pos), Some(b'0'..=b'9')) {
                return Err(self.err("bad number"));
            }
            self.digits();
        }
        let text =
            std::str::from_utf8(&self.bytes[start..self.pos]).map_err(|_| self.err("utf8"))?;
        match text.parse::<f64>() {
            Ok(n) if n.is_finite() => Ok(Json::Num(n)),
            _ => Err(self.err("bad number")),
        }
    }
    fn digits(&mut self) {
        while matches!(self.bytes.get(self.pos), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
    }
    fn hex_quad(&mut self) -> Result<u16, JsonError> {
        let hex = self
            .bytes
            .get(self.pos..self.pos + 4)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u16::from_str_radix(h, 16).ok())
            .ok_or_else(|| self.err("bad \\u escape"))?;
        self.pos += 4;
        Ok(hex)
    }
    fn string(&mut self) -> Result<String, JsonError> {
        self.pos += 1;
        let mut out = String::new();
        loop {
            let start = self.pos;
            while let Some(&b) = self.bytes.get(self.pos) {
                if b == b'"' || b == b'\\' {
                    break;
                }
                if b < 0x20 {
                    return Err(self.err("unescaped control character"));
                }
                self.pos += 1;
            }
            out.push_str(
                std::str::from_utf8(&self.bytes[start..self.pos]).map_err(|_| self.err("utf8"))?,
            );
            match self.bytes.get(self.pos) {
                None => return Err(self.err("unterminated string")),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(_) => {
                    self.pos += 1;
                    let escape = *self.bytes.get(self.pos).ok_or_else(|| self.err("escape"))?;
                    self.pos += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'n' => out.push('\n'),
                        b't' => out.push('\t'),
                        b'r' => out.push('\r'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'u' => {
                            let first = self.hex_quad()?;
                            let code = if (0xd800..=0xdbff).contains(&first) {
                                if self.bytes.get(self.pos..self.pos + 2) != Some(b"\\u") {
                                    return Err(self.err("missing low surrogate"));
                                }
                                self.pos += 2;
                                let second = self.hex_quad()?;
                                if !(0xdc00..=0xdfff).contains(&second) {
                                    return Err(self.err("bad low surrogate"));
                                }
                                0x10000 + ((u32::from(first) - 0xd800) << 10) + u32::from(second)
                                    - 0xdc00
                            } else if (0xdc00..=0xdfff).contains(&first) {
                                return Err(self.err("unexpected low surrogate"));
                            } else {
                                u32::from(first)
                            };
                            out.push(char::from_u32(code).expect("validated Unicode scalar"));
                        }
                        _ => return Err(self.err("bad escape")),
                    }
                }
            }
        }
    }
    fn array(&mut self) -> Result<Json, JsonError> {
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_ws();
        if self.bytes.get(self.pos) == Some(&b']') {
            self.pos += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value()?);
            self.skip_ws();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Arr(items));
                }
                _ => return Err(self.err("expected , or ]")),
            }
        }
    }
    fn object(&mut self) -> Result<Json, JsonError> {
        self.pos += 1;
        let mut map = BTreeMap::new();
        self.skip_ws();
        if self.bytes.get(self.pos) == Some(&b'}') {
            self.pos += 1;
            return Ok(Json::Obj(map));
        }
        loop {
            self.skip_ws();
            if self.bytes.get(self.pos) != Some(&b'"') {
                return Err(self.err("expected key"));
            }
            let key = self.string()?;
            self.skip_ws();
            if self.bytes.get(self.pos) != Some(&b':') {
                return Err(self.err("expected :"));
            }
            self.pos += 1;
            self.skip_ws();
            let value = self.value()?;
            if map.insert(key, value).is_some() {
                return Err(self.err("duplicate key"));
            }
            self.skip_ws();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Obj(map));
                }
                _ => return Err(self.err("expected , or }")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Json;

    #[test]
    fn escaped_unicode_pair_is_one_scalar() {
        assert_eq!(
            Json::parse(r#""\uD83D\uDE80""#).unwrap(),
            Json::Str("🚀".into())
        );
    }

    #[test]
    fn malformed_json_is_rejected() {
        for input in [
            "01",
            "-",
            "1.",
            "1e",
            "1e+",
            "+1",
            "[1,]",
            "{\"a\":1,\"a\":2}",
            "\"\u{0001}\"",
            r#""\uD83D""#,
            r#""\uDE80""#,
            r#""\uD83D\u0041""#,
        ] {
            assert!(Json::parse(input).is_err(), "accepted {input:?}");
        }
    }
}

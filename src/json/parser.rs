/* -----------------------------------------------------------------------------
 * json/parser.rs
 * Recursive-descent JSON parser supporting strings, numbers, booleans,
 * null, arrays, objects, and unicode surrogate pairs.
 * -------------------------------------------------------------------------- */

use std::collections::HashMap;
use std::fmt;

use super::value::Value;

// Error

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub pos: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error at byte {}: {}", self.pos, self.message)
    }
}

impl std::error::Error for ParseError {}

pub type Result<T> = std::result::Result<T, ParseError>;

// Parser

struct Parser<'a> {
    source: &'a [u8],
    position: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Parser {
            source: src.as_bytes(),
            position: 0,
        }
    }

    fn err(&self, msg: &str) -> ParseError {
        ParseError {
            message: msg.to_string(),
            pos: self.position,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.source.get(self.position).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.source.get(self.position).copied();
        if b.is_some() {
            self.position += 1;
        }
        b
    }

    fn expect(&mut self, byte: u8) -> Result<()> {
        if self.peek() == Some(byte) {
            self.position += 1;
            Ok(())
        } else {
            Err(self.err(&format!("expected '{}'", byte as char)))
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.position += 1;
        }
    }

    fn matches_literal(&mut self, lit: &[u8]) -> bool {
        if self.source.get(self.position..self.position + lit.len()) == Some(lit) {
            self.position += lit.len();
            true
        } else {
            false
        }
    }

    // Parse Dispatch

    fn parse_value(&mut self) -> Result<Value> {
        self.skip_ws();
        match self
            .peek()
            .ok_or_else(|| self.err("unexpected end of input"))?
        {
            b'"' => self.parse_string().map(Value::String),
            b'{' => self.parse_object(),
            b'[' => self.parse_array(),
            b't' => {
                if self.matches_literal(b"true") {
                    Ok(Value::Bool(true))
                } else {
                    Err(self.err("invalid token"))
                }
            }
            b'f' => {
                if self.matches_literal(b"false") {
                    Ok(Value::Bool(false))
                } else {
                    Err(self.err("invalid token"))
                }
            }
            b'n' => {
                if self.matches_literal(b"null") {
                    Ok(Value::Null)
                } else {
                    Err(self.err("invalid token"))
                }
            }
            b'-' | b'0'..=b'9' => self.parse_number(),
            c => Err(self.err(&format!("unexpected byte '{}'", c as char))),
        }
    }

    // String

    fn parse_string(&mut self) -> Result<String> {
        self.expect(b'"')?;
        let mut s = String::new();
        loop {
            match self.bump().ok_or_else(|| self.err("unterminated string"))? {
                b'"' => return Ok(s),
                b'\\' => s.push(self.parse_escape()?),
                b => s.push(b as char),
            }
        }
    }

    fn parse_escape(&mut self) -> Result<char> {
        match self.bump().ok_or_else(|| self.err("unterminated escape"))? {
            b'"' => Ok('"'),
            b'\\' => Ok('\\'),
            b'/' => Ok('/'),
            b'b' => Ok('\x08'),
            b'f' => Ok('\x0C'),
            b'n' => Ok('\n'),
            b'r' => Ok('\r'),
            b't' => Ok('\t'),
            b'u' => {
                let n = self.parse_hex4()?;
                // High surrogate (U+D800–U+DBFF): must be followed by \uDC00–\uDFFF
                if (0xD800..=0xDBFF).contains(&n) {
                    if self.peek() != Some(b'\\') {
                        return Err(
                            self.err("high surrogate must be followed by \\uXXXX low surrogate")
                        );
                    }
                    self.position += 1; // consume '\'
                    if self.bump() != Some(b'u') {
                        return Err(self.err("expected \\u after high surrogate"));
                    }
                    let n2 = self.parse_hex4()?;
                    if !(0xDC00..=0xDFFF).contains(&n2) {
                        return Err(self.err("expected low surrogate (U+DC00–U+DFFF)"));
                    }
                    // Decode: U+10000 + (high - 0xD800) * 0x400 + (low - 0xDC00)
                    let codepoint = 0x10000 + ((n - 0xD800) << 10) + (n2 - 0xDC00);
                    char::from_u32(codepoint)
                        .ok_or_else(|| self.err("invalid surrogate pair codepoint"))
                } else if (0xDC00..=0xDFFF).contains(&n) {
                    // Lone low surrogate — invalid per spec
                    Err(self.err("unexpected lone low surrogate"))
                } else {
                    char::from_u32(n).ok_or_else(|| self.err("invalid unicode codepoint"))
                }
            }
            c => Err(self.err(&format!("unknown escape '\\{}'", c as char))),
        }
    }

    /// Parse exactly 4 hex digits and return their value as u32.
    fn parse_hex4(&mut self) -> Result<u32> {
        let hex = self.take_n::<4>()?;
        let s = std::str::from_utf8(&hex).map_err(|_| self.err("invalid utf-8 in \\u escape"))?;
        u32::from_str_radix(s, 16).map_err(|_| self.err("invalid \\u hex digits"))
    }

    fn take_n<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut buf = [0u8; N];
        for b in &mut buf {
            *b = self
                .bump()
                .ok_or_else(|| self.err("unexpected end in escape"))?;
        }
        Ok(buf)
    }

    // Number

    fn parse_number(&mut self) -> Result<Value> {
        let start = self.position;
        if self.peek() == Some(b'-') {
            self.position += 1;
        }
        self.eat_digits();
        if self.peek() == Some(b'.') {
            self.position += 1;
            self.eat_digits();
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.position += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.position += 1;
            }
            self.eat_digits();
        }
        let slice = std::str::from_utf8(&self.source[start..self.position])
            .map_err(|_| self.err("invalid number bytes"))?;
        slice
            .parse::<f64>()
            .map(Value::Number)
            .map_err(|_| self.err(&format!("invalid number '{slice}'")))
    }

    fn eat_digits(&mut self) {
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.position += 1;
        }
    }

    // Array

    fn parse_array(&mut self) -> Result<Value> {
        self.expect(b'[')?;
        let mut arr = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.position += 1;
            return Ok(Value::Array(arr));
        }
        loop {
            arr.push(self.parse_value()?);
            self.skip_ws();
            match self.bump().ok_or_else(|| self.err("unterminated array"))? {
                b']' => return Ok(Value::Array(arr)),
                b',' => {}
                _ => return Err(self.err("expected ',' or ']'")),
            }
        }
    }

    // Object

    fn parse_object(&mut self) -> Result<Value> {
        self.expect(b'{')?;
        let mut map = HashMap::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.position += 1;
            return Ok(Value::Object(map));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect(b':')?;
            let val = self.parse_value()?;
            map.insert(key, val);
            self.skip_ws();
            match self.bump().ok_or_else(|| self.err("unterminated object"))? {
                b'}' => return Ok(Value::Object(map)),
                b',' => {}
                _ => return Err(self.err("expected ',' or '}'")),
            }
        }
    }
}

// Public API

/// Parse a JSON string into a [`Value`].
pub fn parse(src: &str) -> Result<Value> {
    let mut p = Parser::new(src);
    let v = p.parse_value()?;
    p.skip_ws();
    if p.position != p.source.len() {
        return Err(p.err("trailing content after JSON value"));
    }
    Ok(v)
}

/// Serialize a [`Value`] back to a compact JSON string.
pub fn stringify(v: &Value) -> String {
    v.to_string()
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives() {
        assert_eq!(parse("null").unwrap(), Value::Null);
        assert_eq!(parse("true").unwrap(), Value::Bool(true));
        assert_eq!(parse("false").unwrap(), Value::Bool(false));
        assert_eq!(parse("42").unwrap(), Value::Number(42.0));
        assert_eq!(parse("-3.14").unwrap(), Value::Number(-3.14));
        assert_eq!(parse("1e2").unwrap(), Value::Number(100.0));
    }

    #[test]
    fn strings() {
        assert_eq!(parse(r#""hello""#).unwrap(), Value::String("hello".into()));
        assert_eq!(
            parse(r#""\n\t\\\"\/""#).unwrap(),
            Value::String("\n\t\\\"/".into())
        );
        assert_eq!(parse(r#""\u0041""#).unwrap(), Value::String("A".into()));
        // Surrogate pair \uD83D\uDE00 → 😀 (U+1F600)
        assert_eq!(
            parse(r#""\uD83D\uDE00""#).unwrap(),
            Value::String("😀".into())
        );
        // Null byte is valid JSON
        assert_eq!(parse(r#""\u0000""#).unwrap(), Value::String("\0".into()));
    }

    #[test]
    fn surrogate_errors() {
        // Lone high surrogate
        assert!(parse(r#""\uD83D""#).is_err());
        // Lone low surrogate
        assert!(parse(r#""\uDE00""#).is_err());
        // High surrogate followed by non-surrogate \u
        assert!(parse(r#""\uD83D\u0041""#).is_err());
    }

    #[test]
    fn array() {
        let v = parse("[1,2,3]").unwrap();
        assert_eq!(v.as_array().unwrap().len(), 3);
        assert_eq!(v.get("1").unwrap().as_f64(), Some(2.0));
    }

    #[test]
    fn object() {
        let v = parse(r#"{"x":1,"y":true}"#).unwrap();
        assert_eq!(v.get("x").unwrap().as_i64(), Some(1));
        assert_eq!(v.get("y").unwrap().as_bool(), Some(true));
    }

    #[test]
    fn nested() {
        let src = r#"{"users":[{"name":"alice","age":30},{"name":"bob","age":25}]}"#;
        let v = parse(src).unwrap();
        let name = v
            .get("users")
            .unwrap()
            .get("0")
            .unwrap()
            .get("name")
            .unwrap();
        assert_eq!(name.as_str(), Some("alice"));
    }

    #[test]
    fn roundtrip() {
        let src = r#"{"a":1,"b":[2,3],"c":null}"#;
        let v = parse(src).unwrap();
        let back = stringify(&v);
        assert_eq!(parse(&back).unwrap(), v);
    }

    #[test]
    fn errors() {
        assert!(parse("{bad}").is_err());
        assert!(parse(r#"{"a":}"#).is_err());
        assert!(parse("[1,2,]").is_err());
        assert!(parse("").is_err());
    }
}

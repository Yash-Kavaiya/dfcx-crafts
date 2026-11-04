//! A small, order-preserving JSON implementation.

use std::fmt;

/// An insertion-ordered JSON object.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Map {
    entries: Vec<(String, Value)>,
}

impl Map {
    pub fn new() -> Self {
        Map { entries: Vec::new() }
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.entries.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }
    /// Inserts or replaces a key, keeping the original position on replace.
    pub fn insert(&mut self, key: impl Into<String>, value: Value) -> Option<Value> {
        let key = key.into();
        if let Some(slot) = self.get_mut(&key) {
            return Some(std::mem::replace(slot, value));
        }
        self.entries.push((key, value));
        None
    }
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let idx = self.entries.iter().position(|(k, _)| k == key)?;
        Some(self.entries.remove(idx).1)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&String, &mut Value)> {
        self.entries.iter_mut().map(|(k, v)| (&*k, v))
    }
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.iter().map(|(k, _)| k)
    }
    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.entries.iter().map(|(_, v)| v)
    }
    /// Returns the entry for `key`, inserting `default` first if absent.
    pub fn entry_or(&mut self, key: &str, default: Value) -> &mut Value {
        if !self.contains_key(key) {
            self.entries.push((key.to_string(), default));
        }
        self.get_mut(key).unwrap()
    }
}

impl IntoIterator for Map {
    type Item = (String, Value);
    type IntoIter = std::vec::IntoIter<(String, Value)>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl FromIterator<(String, Value)> for Map {
    fn from_iter<T: IntoIterator<Item = (String, Value)>>(iter: T) -> Self {
        let mut m = Map::new();
        for (k, v) in iter {
            m.insert(k, v);
        }
        m
    }
}

/// A JSON value.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(Map),
}

pub static NULL: Value = Value::Null;

impl Value {
    pub fn object() -> Value {
        Value::Object(Map::new())
    }
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
    pub fn is_object(&self) -> bool {
        matches!(self, Value::Object(_))
    }
    pub fn is_array(&self) -> bool {
        matches!(self, Value::Array(_))
    }
    pub fn is_string(&self) -> bool {
        matches!(self, Value::String(_))
    }
    pub fn is_number(&self) -> bool {
        matches!(self, Value::Number(_))
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        self.as_f64().map(|n| n as i64)
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }
    pub fn as_object(&self) -> Option<&Map> {
        match self {
            Value::Object(m) => Some(m),
            _ => None,
        }
    }
    pub fn as_object_mut(&mut self) -> Option<&mut Map> {
        match self {
            Value::Object(m) => Some(m),
            _ => None,
        }
    }
    /// Object field lookup; returns `None` for non-objects.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_object().and_then(|m| m.get(key))
    }
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.as_object_mut().and_then(|m| m.get_mut(key))
    }
    /// Field lookup returning `&NULL` when missing.
    pub fn at(&self, key: &str) -> &Value {
        self.get(key).unwrap_or(&NULL)
    }
    /// String field or empty string.
    pub fn str(&self, key: &str) -> &str {
        self.get(key).and_then(Value::as_str).unwrap_or("")
    }
    /// Array field or empty slice.
    pub fn arr(&self, key: &str) -> &[Value] {
        self.get(key).and_then(Value::as_array).map(|v| v.as_slice()).unwrap_or(&[])
    }
    pub fn bool_field(&self, key: &str) -> bool {
        self.get(key).and_then(Value::as_bool).unwrap_or(false)
    }
    pub fn f64_field(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(Value::as_f64)
    }
    /// Sets a field, converting `self` into an object if needed.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<Value>) {
        if !self.is_object() {
            *self = Value::object();
        }
        self.as_object_mut().unwrap().insert(key, value.into());
    }
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.as_object_mut().and_then(|m| m.remove(key))
    }
    pub fn push(&mut self, value: impl Into<Value>) {
        if !self.is_array() {
            *self = Value::Array(Vec::new());
        }
        self.as_array_mut().unwrap().push(value.into());
    }
    /// Dotted path lookup (`a.b.0.c`).
    pub fn path(&self, path: &str) -> Option<&Value> {
        let mut cur = self;
        for seg in path.split('.').filter(|s| !s.is_empty()) {
            cur = match cur {
                Value::Object(m) => m.get(seg)?,
                Value::Array(a) => a.get(seg.parse::<usize>().ok()?)?,
                _ => return None,
            };
        }
        Some(cur)
    }
    /// Mutable object field, creating intermediate objects.
    pub fn entry(&mut self, key: &str) -> &mut Value {
        if !self.is_object() {
            *self = Value::object();
        }
        self.as_object_mut().unwrap().entry_or(key, Value::Null)
    }
    /// JSON "truthiness" used by the condition language.
    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Object(_) => true,
        }
    }
    /// Human readable rendering used for text templates.
    pub fn to_display(&self) -> String {
        match self {
            Value::Null => String::new(),
            Value::String(s) => s.clone(),
            Value::Number(n) => format_number(*n),
            Value::Bool(b) => b.to_string(),
            Value::Array(a) => a.iter().map(|v| v.to_display()).collect::<Vec<_>>().join(", "),
            Value::Object(_) => self.to_string(),
        }
    }
    /// Deep merge of `other` into `self` (objects merge recursively; others replace).
    pub fn merge(&mut self, other: &Value) {
        match (self.as_object_mut(), other) {
            (Some(dst), Value::Object(src)) => {
                for (k, v) in src.iter() {
                    match dst.get_mut(k) {
                        Some(existing) if existing.is_object() && v.is_object() => existing.merge(v),
                        _ => {
                            dst.insert(k.clone(), v.clone());
                        }
                    }
                }
            }
            _ => *self = other.clone(),
        }
    }
    pub fn to_pretty(&self) -> String {
        let mut out = String::new();
        write_value(&mut out, self, Some(0));
        out
    }
    pub fn parse(text: &str) -> Result<Value, ParseError> {
        parse(text)
    }
}

pub fn format_number(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else if n.is_finite() {
        let s = format!("{}", n);
        s
    } else {
        "null".to_string()
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        write_value(&mut out, self, None);
        f.write_str(&out)
    }
}

fn write_indent(out: &mut String, level: usize) {
    out.push('\n');
    for _ in 0..level {
        out.push_str("  ");
    }
}

fn write_value(out: &mut String, v: &Value, indent: Option<usize>) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&format_number(*n)),
        Value::String(s) => write_string(out, s),
        Value::Array(a) => {
            if a.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if let Some(l) = indent {
                    write_indent(out, l + 1);
                }
                write_value(out, item, indent.map(|l| l + 1));
            }
            if let Some(l) = indent {
                write_indent(out, l);
            }
            out.push(']');
        }
        Value::Object(m) => {
            if m.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (k, item)) in m.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if let Some(l) = indent {
                    write_indent(out, l + 1);
                }
                write_string(out, k);
                out.push(':');
                if indent.is_some() {
                    out.push(' ');
                }
                write_value(out, item, indent.map(|l| l + 1));
            }
            if let Some(l) = indent {
                write_indent(out, l);
            }
            out.push('}');
        }
    }
}

fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub offset: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JSON parse error at byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for ParseError {}

pub fn parse(text: &str) -> Result<Value, ParseError> {
    let mut p = Parser { bytes: text.as_bytes(), pos: 0, depth: 0 };
    p.skip_ws();
    let v = p.value()?;
    p.skip_ws();
    if p.pos != p.bytes.len() {
        return Err(p.err("trailing characters"));
    }
    Ok(v)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn err(&self, msg: &str) -> ParseError {
        ParseError { message: msg.to_string(), offset: self.pos }
    }
    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() && matches!(self.bytes[self.pos], b' ' | b'\n' | b'\r' | b'\t') {
            self.pos += 1;
        }
    }
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }
    fn expect_lit(&mut self, lit: &str, v: Value) -> Result<Value, ParseError> {
        if self.bytes[self.pos..].starts_with(lit.as_bytes()) {
            self.pos += lit.len();
            Ok(v)
        } else {
            Err(self.err("invalid literal"))
        }
    }
    fn value(&mut self) -> Result<Value, ParseError> {
        self.depth += 1;
        if self.depth > 512 {
            return Err(self.err("nesting too deep"));
        }
        let r = match self.peek() {
            None => Err(self.err("unexpected end of input")),
            Some(b'n') => self.expect_lit("null", Value::Null),
            Some(b't') => self.expect_lit("true", Value::Bool(true)),
            Some(b'f') => self.expect_lit("false", Value::Bool(false)),
            Some(b'"') => self.string().map(Value::String),
            Some(b'[') => self.array(),
            Some(b'{') => self.object(),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            Some(_) => Err(self.err("unexpected character")),
        };
        self.depth -= 1;
        r
    }
    fn number(&mut self) -> Result<Value, ParseError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E' | b'+' | b'-') {
                self.pos += 1;
            } else {
                break;
            }
        }
        let s = std::str::from_utf8(&self.bytes[start..self.pos]).unwrap();
        s.parse::<f64>().map(Value::Number).map_err(|_| ParseError { message: "invalid number".into(), offset: start })
    }
    fn hex4(&mut self) -> Result<u32, ParseError> {
        if self.pos + 4 > self.bytes.len() {
            return Err(self.err("truncated unicode escape"));
        }
        let s = std::str::from_utf8(&self.bytes[self.pos..self.pos + 4]).map_err(|_| self.err("bad escape"))?;
        let v = u32::from_str_radix(s, 16).map_err(|_| self.err("bad unicode escape"))?;
        self.pos += 4;
        Ok(v)
    }
    fn string(&mut self) -> Result<String, ParseError> {
        self.pos += 1; // opening quote
        let mut out = String::new();
        loop {
            // copy a run of plain bytes
            let run_start = self.pos;
            while let Some(c) = self.peek() {
                if c == b'"' || c == b'\\' || c < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            if self.pos > run_start {
                out.push_str(std::str::from_utf8(&self.bytes[run_start..self.pos]).map_err(|_| self.err("invalid utf-8"))?);
            }
            match self.peek() {
                None => return Err(self.err("unterminated string")),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let c = self.peek().ok_or_else(|| self.err("unterminated escape"))?;
                    self.pos += 1;
                    match c {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{08}'),
                        b'f' => out.push('\u{0c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut cp = self.hex4()?;
                            if (0xD800..0xDC00).contains(&cp) && self.bytes[self.pos..].starts_with(b"\\u") {
                                self.pos += 2;
                                let lo = self.hex4()?;
                                if (0xDC00..0xE000).contains(&lo) {
                                    cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                }
                            }
                            out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                        }
                        _ => return Err(self.err("invalid escape")),
                    }
                }
                Some(_) => {
                    // raw control character: be lenient and keep it
                    out.push(self.bytes[self.pos] as char);
                    self.pos += 1;
                }
            }
        }
    }
    fn array(&mut self) -> Result<Value, ParseError> {
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.err("expected ',' or ']'")),
            }
        }
    }
    fn object(&mut self) -> Result<Value, ParseError> {
        self.pos += 1;
        let mut map = Map::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(map));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(self.err("expected object key"));
            }
            let key = self.string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(self.err("expected ':'"));
            }
            self.pos += 1;
            self.skip_ws();
            let v = self.value()?;
            map.insert(key, v);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(map));
                }
                _ => return Err(self.err("expected ',' or '}'")),
            }
        }
    }
}

// ---- conversions -----------------------------------------------------------

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}
impl From<&String> for Value {
    fn from(s: &String) -> Self {
        Value::String(s.clone())
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}
macro_rules! num_from {
    ($($t:ty),*) => {$(
        impl From<$t> for Value {
            fn from(n: $t) -> Self { Value::Number(n as f64) }
        }
    )*};
}
num_from!(i8, i16, i32, i64, u8, u16, u32, u64, usize, isize, f32, f64);
impl From<Map> for Value {
    fn from(m: Map) -> Self {
        Value::Object(m)
    }
}
impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Value::Array(v.into_iter().map(Into::into).collect())
    }
}
impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(o: Option<T>) -> Self {
        match o {
            Some(v) => v.into(),
            None => Value::Null,
        }
    }
}
impl From<&Value> for Value {
    fn from(v: &Value) -> Self {
        v.clone()
    }
}

impl std::ops::Index<&str> for Value {
    type Output = Value;
    fn index(&self, key: &str) -> &Value {
        self.at(key)
    }
}
impl std::ops::Index<usize> for Value {
    type Output = Value;
    fn index(&self, i: usize) -> &Value {
        self.as_array().and_then(|a| a.get(i)).unwrap_or(&NULL)
    }
}

/// Builds a JSON object: `obj! { "a" => 1, "b" => "x" }`.
#[macro_export]
macro_rules! obj {
    () => { $crate::json::Value::Object($crate::json::Map::new()) };
    ($($k:expr => $v:expr),+ $(,)?) => {{
        let mut m = $crate::json::Map::new();
        $( m.insert($k, $crate::json::Value::from($v)); )+
        $crate::json::Value::Object(m)
    }};
}

/// Builds a JSON array: `arr![1, "x", obj!{}]`.
#[macro_export]
macro_rules! arr {
    () => { $crate::json::Value::Array(Vec::new()) };
    ($($v:expr),+ $(,)?) => { $crate::json::Value::Array(vec![$($crate::json::Value::from($v)),+]) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let src = r#"{"a":1,"b":[true,false,null],"c":{"d":"x\"yé😀"},"e":-1.5e3}"#;
        let v = parse(src).unwrap();
        assert_eq!(v["a"], Value::Number(1.0));
        assert_eq!(v["c"]["d"].as_str().unwrap(), "x\"yé😀");
        assert_eq!(v["e"].as_f64().unwrap(), -1500.0);
        let again = parse(&v.to_string()).unwrap();
        assert_eq!(v, again);
        let pretty = parse(&v.to_pretty()).unwrap();
        assert_eq!(v, pretty);
    }

    #[test]
    fn order_preserved_and_macros() {
        let v = obj! {"z" => 1, "a" => arr![1, "two"], "m" => obj!{}};
        assert_eq!(v.to_string(), r#"{"z":1,"a":[1,"two"],"m":{}}"#);
        assert_eq!(v.path("a.1").unwrap().as_str(), Some("two"));
    }

    #[test]
    fn errors() {
        assert!(parse("{").is_err());
        assert!(parse("[1,]").is_err());
        assert!(parse("1 2").is_err());
    }

    #[test]
    fn merge() {
        let mut a = obj! {"x" => obj!{"y" => 1, "z" => 2}};
        a.merge(&obj! {"x" => obj!{"y" => 5}, "w" => true});
        assert_eq!(a.to_string(), r#"{"x":{"y":5,"z":2},"w":true}"#);
    }
}

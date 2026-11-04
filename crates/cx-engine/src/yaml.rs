//! A small YAML subset reader, sufficient for OpenAPI tool schemas:
//! block maps and sequences, plain/quoted scalars, `|` / `>` block scalars,
//! inline flow collections and comments.

use cx_core::{Map, Value};

#[derive(Debug, Clone)]
struct Line {
    indent: usize,
    text: String,
}

fn strip_comment(s: &str) -> &str {
    let mut in_s = false;
    let mut in_d = false;
    let b = s.as_bytes();
    for i in 0..b.len() {
        match b[i] {
            b'\'' if !in_d => in_s = !in_s,
            b'"' if !in_s => in_d = !in_d,
            b'#' if !in_s && !in_d && (i == 0 || b[i - 1] == b' ' || b[i - 1] == b'\t') => return s[..i].trim_end(),
            _ => {}
        }
    }
    s.trim_end()
}

pub fn parse(src: &str) -> Result<Value, String> {
    let raw: Vec<&str> = src.lines().collect();
    let mut lines = Vec::new();
    for l in &raw {
        if l.trim() == "---" || l.trim() == "..." {
            continue;
        }
        let indent = l.len() - l.trim_start_matches(' ').len();
        lines.push(Line { indent, text: l[indent..].to_string() });
    }
    let mut p = P { lines, i: 0 };
    p.skip_blank();
    if p.i >= p.lines.len() {
        return Ok(Value::Null);
    }
    let indent = p.lines[p.i].indent;
    let v = p.block(indent)?;
    Ok(v)
}

struct P {
    lines: Vec<Line>,
    i: usize,
}

fn find_colon(s: &str) -> Option<usize> {
    let mut in_s = false;
    let mut in_d = false;
    let b = s.as_bytes();
    for i in 0..b.len() {
        match b[i] {
            b'\'' if !in_d => in_s = !in_s,
            b'"' if !in_s => in_d = !in_d,
            b':' if !in_s && !in_d && (i + 1 == b.len() || b[i + 1] == b' ') => return Some(i),
            _ => {}
        }
    }
    None
}

impl P {
    fn skip_blank(&mut self) {
        while self.i < self.lines.len() && strip_comment(&self.lines[self.i].text).trim().is_empty() {
            self.i += 1;
        }
    }

    fn block(&mut self, indent: usize) -> Result<Value, String> {
        self.skip_blank();
        if self.i >= self.lines.len() {
            return Ok(Value::Null);
        }
        let t = strip_comment(&self.lines[self.i].text).to_string();
        if t == "-" || t.starts_with("- ") {
            self.seq(indent)
        } else if find_colon(&t).is_some() {
            self.map(indent)
        } else {
            self.i += 1;
            Ok(scalar(&t))
        }
    }

    fn seq(&mut self, indent: usize) -> Result<Value, String> {
        let mut items = Vec::new();
        loop {
            self.skip_blank();
            if self.i >= self.lines.len() || self.lines[self.i].indent != indent {
                break;
            }
            let t = strip_comment(&self.lines[self.i].text).to_string();
            if !(t == "-" || t.starts_with("- ")) {
                break;
            }
            let rest = t[1..].trim_start().to_string();
            if rest.is_empty() {
                self.i += 1;
                self.skip_blank();
                if self.i < self.lines.len() && self.lines[self.i].indent > indent {
                    let ni = self.lines[self.i].indent;
                    items.push(self.block(ni)?);
                } else {
                    items.push(Value::Null);
                }
            } else if find_colon(&rest).is_some() && !rest.starts_with('{') && !rest.starts_with('"') && !rest.starts_with('\'') {
                // "- key: value" starts an inline map at a deeper indent
                let offset = t.len() - rest.len();
                self.lines[self.i] = Line { indent: indent + offset, text: rest };
                items.push(self.map(indent + offset)?);
            } else {
                self.i += 1;
                items.push(scalar(&rest));
            }
        }
        Ok(Value::Array(items))
    }

    fn map(&mut self, indent: usize) -> Result<Value, String> {
        let mut m = Map::new();
        loop {
            self.skip_blank();
            if self.i >= self.lines.len() || self.lines[self.i].indent != indent {
                break;
            }
            let t = strip_comment(&self.lines[self.i].text).to_string();
            if t.starts_with("- ") || t == "-" {
                break;
            }
            let Some(c) = find_colon(&t) else { return Err(format!("expected 'key: value' at line {}", self.i + 1)) };
            let key = unquote(t[..c].trim());
            let rest = t[c + 1..].trim().to_string();
            self.i += 1;
            let value = if rest.is_empty() {
                self.skip_blank();
                if self.i < self.lines.len() && self.lines[self.i].indent > indent {
                    let ni = self.lines[self.i].indent;
                    self.block(ni)?
                } else if self.i < self.lines.len()
                    && self.lines[self.i].indent == indent
                    && (self.lines[self.i].text.starts_with("- ") || self.lines[self.i].text.trim() == "-")
                {
                    self.seq(indent)?
                } else {
                    Value::Null
                }
            } else if rest == "|" || rest == ">" || rest.starts_with("|-") || rest.starts_with(">-") || rest.starts_with("|+") {
                let folded = rest.starts_with('>');
                let mut parts = Vec::new();
                let mut block_indent = None;
                while self.i < self.lines.len() {
                    let l = &self.lines[self.i];
                    if l.text.trim().is_empty() {
                        parts.push(String::new());
                        self.i += 1;
                        continue;
                    }
                    if l.indent <= indent {
                        break;
                    }
                    let bi = *block_indent.get_or_insert(l.indent);
                    parts.push(format!("{}{}", " ".repeat(l.indent.saturating_sub(bi)), l.text));
                    self.i += 1;
                }
                while parts.last().map_or(false, |p| p.is_empty()) {
                    parts.pop();
                }
                let joined = if folded { parts.join(" ") } else { parts.join("\n") };
                Value::String(if rest.ends_with('-') { joined } else { format!("{}\n", joined) })
            } else {
                scalar(&rest)
            };
            m.insert(key, value);
        }
        Ok(Value::Object(m))
    }
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''))) {
        if s.starts_with('"') {
            if let Ok(Value::String(v)) = Value::parse(s) {
                return v;
            }
        }
        return s[1..s.len() - 1].replace("''", "'");
    }
    s.to_string()
}

fn split_flow(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    let mut q: Option<char> = None;
    for c in s.chars() {
        match q {
            Some(qc) => {
                cur.push(c);
                if c == qc {
                    q = None;
                }
            }
            None => match c {
                '"' | '\'' => {
                    q = Some(c);
                    cur.push(c);
                }
                '[' | '{' => {
                    depth += 1;
                    cur.push(c);
                }
                ']' | '}' => {
                    depth -= 1;
                    cur.push(c);
                }
                ',' if depth == 0 => {
                    out.push(cur.trim().to_string());
                    cur.clear();
                }
                _ => cur.push(c),
            },
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn scalar(s: &str) -> Value {
    let s = s.trim();
    if s.starts_with('[') && s.ends_with(']') {
        return Value::Array(split_flow(&s[1..s.len() - 1]).iter().map(|x| scalar(x)).collect());
    }
    if s.starts_with('{') && s.ends_with('}') {
        let mut m = Map::new();
        for part in split_flow(&s[1..s.len() - 1]) {
            if let Some(c) = part.find(':') {
                m.insert(unquote(&part[..c]), scalar(&part[c + 1..]));
            }
        }
        return Value::Object(m);
    }
    if s.starts_with('"') || s.starts_with('\'') {
        return Value::String(unquote(s));
    }
    match s {
        "" | "~" | "null" | "Null" | "NULL" => return Value::Null,
        "true" | "True" | "TRUE" => return Value::Bool(true),
        "false" | "False" | "FALSE" => return Value::Bool(false),
        _ => {}
    }
    if let Ok(n) = s.parse::<f64>() {
        if !s.starts_with('+') && !(s.starts_with('0') && s.len() > 1 && !s.contains('.')) {
            return Value::Number(n);
        }
    }
    Value::String(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn openapi_like() {
        let y = r#"
openapi: 3.0.0
info:
  title: Orders API   # comment
  version: "1.0"
servers:
  - url: http://localhost:9000/api
paths:
  /orders/{id}:
    get:
      operationId: getOrder
      parameters:
        - name: id
          in: path
          required: true
      description: |
        Gets an order.
        Second line.
tags: [a, "b c"]
empty:
"#;
        let v = parse(y).unwrap();
        assert_eq!(v.str("openapi"), "3.0.0");
        assert_eq!(v.at("info").str("version"), "1.0");
        assert_eq!(v.at("servers")[0].str("url"), "http://localhost:9000/api");
        let op = v.at("paths").at("/orders/{id}").at("get");
        assert_eq!(op.str("operationId"), "getOrder");
        assert_eq!(op.at("parameters")[0].str("in"), "path");
        assert_eq!(op.at("parameters")[0].at("required"), &Value::Bool(true));
        assert_eq!(op.str("description"), "Gets an order.\nSecond line.\n");
        assert_eq!(v.at("tags")[1].as_str(), Some("b c"));
        assert!(v.at("empty").is_null());
    }
}

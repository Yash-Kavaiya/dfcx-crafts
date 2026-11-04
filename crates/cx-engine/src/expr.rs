//! The condition / expression language and text templating.
//!
//! Supports references (`$session.params.x`, `$page.params.status`,
//! `$intent.params.size.original`, `$request.user-utterance`, `$flow.x`),
//! literals, comparison operators (`= != < > <= >=`, `:` "has"), boolean
//! `AND` / `OR` / `NOT`, parentheses and inline system functions
//! (`$sys.func.ADD(1, 2)`).

use cx_core::time::Date;
use cx_core::{json::format_number, Value};

/// Resolves `$` references (the path is given without the leading `$`).
pub trait Scope {
    fn lookup(&self, path: &str) -> Value;
}

impl<F: Fn(&str) -> Value> Scope for F {
    fn lookup(&self, path: &str) -> Value {
        self(path)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ref(String),
    Func(String),
    Str(String),
    Num(f64),
    Word(String),
    Op(String),
    LParen,
    RParen,
    Comma,
}

fn is_ref_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' || c == '.'
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '$' => {
                let start = i + 1;
                i += 1;
                while i < chars.len() && is_ref_char(chars[i]) {
                    i += 1;
                }
                let mut path: String = chars[start..i].iter().collect();
                while path.ends_with('.') {
                    path.pop();
                }
                if path.starts_with("sys.func.") && i < chars.len() && chars[i] == '(' {
                    out.push(Tok::Func(path["sys.func.".len()..].to_ascii_uppercase()));
                } else {
                    out.push(Tok::Ref(path));
                }
            }
            '"' | '\'' => {
                let q = c;
                i += 1;
                let mut s = String::new();
                while i < chars.len() && chars[i] != q {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                        s.push(match chars[i] {
                            'n' => '\n',
                            't' => '\t',
                            other => other,
                        });
                    } else {
                        s.push(chars[i]);
                    }
                    i += 1;
                }
                if i >= chars.len() {
                    return Err("unterminated string literal".into());
                }
                i += 1;
                out.push(Tok::Str(s));
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            '=' => {
                i += if i + 1 < chars.len() && chars[i + 1] == '=' { 2 } else { 1 };
                out.push(Tok::Op("=".into()));
            }
            '!' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                out.push(Tok::Op("!=".into()));
                i += 2;
            }
            '!' => {
                out.push(Tok::Word("NOT".into()));
                i += 1;
            }
            '<' | '>' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    out.push(Tok::Op(format!("{}=", c)));
                    i += 2;
                } else {
                    out.push(Tok::Op(c.to_string()));
                    i += 1;
                }
            }
            ':' => {
                out.push(Tok::Op(":".into()));
                i += 1;
            }
            '&' if i + 1 < chars.len() && chars[i + 1] == '&' => {
                out.push(Tok::Word("AND".into()));
                i += 2;
            }
            '|' if i + 1 < chars.len() && chars[i + 1] == '|' => {
                out.push(Tok::Word("OR".into()));
                i += 2;
            }
            '*' => {
                out.push(Tok::Word("*".into()));
                i += 1;
            }
            c if c.is_ascii_digit() || (c == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) => {
                let start = i;
                i += 1;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                out.push(Tok::Num(s.parse().map_err(|_| format!("invalid number '{}'", s))?));
            }
            c if c.is_alphanumeric() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '-' || chars[i] == '.') {
                    i += 1;
                }
                out.push(Tok::Word(chars[start..i].iter().collect()));
            }
            other => return Err(format!("unexpected character '{}'", other)),
        }
    }
    Ok(out)
}

#[derive(Debug, Clone)]
enum Expr {
    Lit(Value),
    Ref(String),
    Call(String, Vec<Expr>),
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Cmp(String, Box<Expr>, Box<Expr>),
    Wildcard,
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Tok::Word(x)) if x.eq_ignore_ascii_case(w))
    }
    fn or(&mut self) -> Result<Expr, String> {
        let mut l = self.and()?;
        while self.is_word("OR") {
            self.pos += 1;
            let r = self.and()?;
            l = Expr::Or(Box::new(l), Box::new(r));
        }
        Ok(l)
    }
    fn and(&mut self) -> Result<Expr, String> {
        let mut l = self.not()?;
        while self.is_word("AND") {
            self.pos += 1;
            let r = self.not()?;
            l = Expr::And(Box::new(l), Box::new(r));
        }
        Ok(l)
    }
    fn not(&mut self) -> Result<Expr, String> {
        if self.is_word("NOT") {
            self.pos += 1;
            return Ok(Expr::Not(Box::new(self.not()?)));
        }
        self.cmp()
    }
    fn cmp(&mut self) -> Result<Expr, String> {
        let l = self.primary()?;
        if let Some(Tok::Op(op)) = self.peek().cloned() {
            self.pos += 1;
            let r = self.primary()?;
            return Ok(Expr::Cmp(op, Box::new(l), Box::new(r)));
        }
        Ok(l)
    }
    fn primary(&mut self) -> Result<Expr, String> {
        let t = self.peek().cloned().ok_or("unexpected end of expression")?;
        self.pos += 1;
        match t {
            Tok::Str(s) => Ok(Expr::Lit(Value::String(s))),
            Tok::Num(n) => Ok(Expr::Lit(Value::Number(n))),
            Tok::Ref(r) => Ok(Expr::Ref(r)),
            Tok::Word(w) if w == "*" => Ok(Expr::Wildcard),
            Tok::Word(w) => Ok(match w.to_ascii_lowercase().as_str() {
                "true" => Expr::Lit(Value::Bool(true)),
                "false" => Expr::Lit(Value::Bool(false)),
                "null" => Expr::Lit(Value::Null),
                _ => Expr::Lit(Value::String(w)),
            }),
            Tok::LParen => {
                let e = self.or()?;
                if self.peek() != Some(&Tok::RParen) {
                    return Err("expected ')'".into());
                }
                self.pos += 1;
                Ok(e)
            }
            Tok::Func(name) => {
                if self.peek() != Some(&Tok::LParen) {
                    return Err("expected '(' after function name".into());
                }
                self.pos += 1;
                let mut args = Vec::new();
                if self.peek() == Some(&Tok::RParen) {
                    self.pos += 1;
                } else {
                    loop {
                        args.push(self.or()?);
                        match self.peek() {
                            Some(Tok::Comma) => self.pos += 1,
                            Some(Tok::RParen) => {
                                self.pos += 1;
                                break;
                            }
                            _ => return Err(format!("expected ',' or ')' in call to {}", name)),
                        }
                    }
                }
                Ok(Expr::Call(name, args))
            }
            other => Err(format!("unexpected token {:?}", other)),
        }
    }
}

fn parse(src: &str) -> Result<Expr, String> {
    let toks = lex(src)?;
    if toks.is_empty() {
        return Ok(Expr::Lit(Value::Bool(true)));
    }
    let mut p = Parser { toks, pos: 0 };
    let e = p.or()?;
    if p.pos != p.toks.len() {
        return Err(format!("unexpected trailing input in expression '{}'", src));
    }
    Ok(e)
}

/// Checks that an expression parses.
pub fn validate(src: &str) -> Result<(), String> {
    parse(src).map(|_| ())
}

/// Evaluates a condition string to a boolean. An empty condition is true.
pub fn eval_condition(src: &str, scope: &dyn Scope) -> Result<bool, String> {
    if src.trim().is_empty() {
        return Ok(true);
    }
    Ok(eval(&parse(src)?, scope)?.truthy())
}

/// Evaluates an expression to a value.
pub fn eval_expr(src: &str, scope: &dyn Scope) -> Result<Value, String> {
    eval(&parse(src)?, scope)
}

fn as_num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => Some(*n),
        Value::String(s) => s.trim().parse().ok(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Null, _) | (_, Value::Null) => false,
        (Value::Number(x), Value::Number(y)) => (x - y).abs() < 1e-9,
        (Value::Number(_), Value::String(_)) | (Value::String(_), Value::Number(_)) => match (as_num(a), as_num(b)) {
            (Some(x), Some(y)) => (x - y).abs() < 1e-9,
            _ => false,
        },
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Bool(x), Value::String(y)) | (Value::String(y), Value::Bool(x)) => y.eq_ignore_ascii_case(&x.to_string()),
        _ => a == b,
    }
}

fn compare(op: &str, a: &Value, b: &Value) -> bool {
    match op {
        "=" => values_equal(a, b),
        "!=" => !values_equal(a, b),
        ":" => match (a, b) {
            (Value::String(x), Value::String(y)) => x.to_lowercase().contains(&y.to_lowercase()),
            (Value::Array(items), v) => items.iter().any(|i| values_equal(i, v)),
            (Value::Object(m), Value::String(k)) => m.contains_key(k),
            (x, y) => values_equal(x, y),
        },
        _ => {
            let ord = match (as_num(a), as_num(b)) {
                (Some(x), Some(y)) => x.partial_cmp(&y),
                _ => match (a, b) {
                    (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
                    _ => None,
                },
            };
            let Some(ord) = ord else { return false };
            use std::cmp::Ordering::*;
            match op {
                "<" => ord == Less,
                ">" => ord == Greater,
                "<=" => ord != Greater,
                ">=" => ord != Less,
                _ => false,
            }
        }
    }
}

fn eval(e: &Expr, scope: &dyn Scope) -> Result<Value, String> {
    Ok(match e {
        Expr::Lit(v) => v.clone(),
        Expr::Wildcard => Value::String("*".into()),
        Expr::Ref(r) => scope.lookup(r),
        Expr::Not(x) => Value::Bool(!eval(x, scope)?.truthy()),
        Expr::And(a, b) => Value::Bool(eval(a, scope)?.truthy() && eval(b, scope)?.truthy()),
        Expr::Or(a, b) => Value::Bool(eval(a, scope)?.truthy() || eval(b, scope)?.truthy()),
        Expr::Cmp(op, a, b) => {
            let av = eval(a, scope)?;
            if matches!(**b, Expr::Wildcard) {
                return Ok(Value::Bool(match op.as_str() {
                    ":" | "=" => !av.is_null(),
                    "!=" => av.is_null(),
                    _ => false,
                }));
            }
            let bv = eval(b, scope)?;
            Value::Bool(compare(op, &av, &bv))
        }
        Expr::Call(name, args) => {
            let mut vals = Vec::with_capacity(args.len());
            for a in args {
                vals.push(eval(a, scope)?);
            }
            call(name, vals, scope)?
        }
    })
}

/// Names of the supported inline system functions.
pub const FUNCTIONS: &[&str] = &[
    "ADD", "MINUS", "MULTIPLY", "DIVIDE", "MOD", "ROUND", "RAND", "IF", "CONCATENATE", "CONTAIN", "COUNT", "LEN",
    "LOWER", "UPPER", "TO_TEXT", "TO_NUMBER", "JOIN", "SPLIT", "SUBSTITUTE", "MID", "GET", "GET_FIELD", "IDENTITY",
    "APPEND", "REMOVE", "IS_DATE", "NOW", "TODAY", "FORMAT_DATE", "ISNULL", "COALESCE", "TRIM",
];

fn num_arg(vals: &[Value], i: usize, f: &str) -> Result<f64, String> {
    vals.get(i).and_then(as_num).ok_or_else(|| format!("$sys.func.{}: argument {} is not a number", f, i + 1))
}

fn call(name: &str, vals: Vec<Value>, scope: &dyn Scope) -> Result<Value, String> {
    let s = |i: usize| vals.get(i).map(display).unwrap_or_default();
    Ok(match name {
        "ADD" => Value::from(vals.iter().map(|v| as_num(v).unwrap_or(0.0)).sum::<f64>()),
        "MINUS" => Value::from(num_arg(&vals, 0, name)? - num_arg(&vals, 1, name)?),
        "MULTIPLY" => Value::from(vals.iter().map(|v| as_num(v).unwrap_or(0.0)).product::<f64>()),
        "DIVIDE" => {
            let d = num_arg(&vals, 1, name)?;
            if d == 0.0 {
                return Err("$sys.func.DIVIDE: division by zero".into());
            }
            Value::from(num_arg(&vals, 0, name)? / d)
        }
        "MOD" => Value::from(num_arg(&vals, 0, name)?.rem_euclid(num_arg(&vals, 1, name)?)),
        "ROUND" => {
            let places = vals.get(1).and_then(as_num).unwrap_or(0.0) as i32;
            let m = 10f64.powi(places);
            Value::from((num_arg(&vals, 0, name)? * m).round() / m)
        }
        "RAND" => {
            let r = (cx_core::uuid::random_u64() >> 11) as f64 / (1u64 << 53) as f64;
            match (vals.first().and_then(as_num), vals.get(1).and_then(as_num)) {
                (Some(lo), Some(hi)) => Value::from((lo + r * (hi - lo + 1.0)).floor()),
                (Some(hi), None) => Value::from((r * hi).floor()),
                _ => Value::from(r),
            }
        }
        "IF" => {
            let cond = match vals.first() {
                Some(Value::String(c)) => eval_condition(c, scope)?,
                Some(v) => v.truthy(),
                None => false,
            };
            if cond {
                vals.get(1).cloned().unwrap_or_default()
            } else {
                vals.get(2).cloned().unwrap_or_default()
            }
        }
        "CONCATENATE" => Value::from(vals.iter().map(display).collect::<String>()),
        "CONTAIN" => Value::Bool(match vals.first() {
            Some(Value::Array(a)) => a.iter().any(|x| values_equal(x, vals.get(1).unwrap_or(&Value::Null))),
            Some(Value::String(t)) => t.contains(&s(1)),
            _ => false,
        }),
        "COUNT" | "LEN" => Value::from(match vals.first() {
            Some(Value::Array(a)) => a.len(),
            Some(Value::String(t)) => t.chars().count(),
            Some(Value::Object(m)) => m.len(),
            Some(Value::Null) | None => 0,
            Some(_) => 1,
        }),
        "LOWER" => Value::from(s(0).to_lowercase()),
        "UPPER" => Value::from(s(0).to_uppercase()),
        "TRIM" => Value::from(s(0).trim()),
        "TO_TEXT" => Value::from(s(0)),
        "TO_NUMBER" => as_num(vals.first().unwrap_or(&Value::Null)).map(Value::from).unwrap_or(Value::Null),
        "JOIN" => {
            let sep = s(0);
            let items: Vec<String> = match vals.get(1) {
                Some(Value::Array(a)) => a.iter().map(display).collect(),
                _ => vals.iter().skip(1).map(display).collect(),
            };
            Value::from(items.join(&sep))
        }
        "SPLIT" => Value::Array(s(0).split(&s(1)).map(|p| Value::from(p.trim())).collect()),
        "SUBSTITUTE" => Value::from(s(0).replace(&s(1), &s(2))),
        "MID" => {
            let text: Vec<char> = s(0).chars().collect();
            let start = (num_arg(&vals, 1, name)? as usize).saturating_sub(1).min(text.len());
            let len = vals.get(2).and_then(as_num).map(|n| n as usize).unwrap_or(text.len());
            Value::from(text[start..(start + len).min(text.len())].iter().collect::<String>())
        }
        "GET" => match (vals.first(), vals.get(1).and_then(as_num)) {
            (Some(Value::Array(a)), Some(i)) => {
                let idx = if i < 0.0 { a.len() as i64 + i as i64 } else { i as i64 };
                a.get(idx.max(0) as usize).cloned().unwrap_or_default()
            }
            _ => Value::Null,
        },
        "GET_FIELD" => vals.first().and_then(|o| o.path(&s(1))).cloned().unwrap_or_default(),
        "IDENTITY" => vals.first().cloned().unwrap_or_default(),
        "APPEND" => {
            let mut a = match vals.first() {
                Some(Value::Array(a)) => a.clone(),
                Some(Value::Null) | None => Vec::new(),
                Some(v) => vec![v.clone()],
            };
            a.extend(vals.iter().skip(1).cloned());
            Value::Array(a)
        }
        "REMOVE" => match vals.first() {
            Some(Value::Array(a)) => {
                let target = vals.get(1).cloned().unwrap_or_default();
                Value::Array(a.iter().filter(|x| !values_equal(x, &target)).cloned().collect())
            }
            _ => Value::Null,
        },
        "IS_DATE" => Value::Bool(match vals.first() {
            Some(v @ Value::Object(_)) => v.get("year").is_some() && v.get("month").is_some() && v.get("day").is_some(),
            Some(Value::String(t)) => Date::parse_iso(t).is_some(),
            _ => false,
        }),
        "NOW" => Value::from(cx_core::time::now_rfc3339()),
        "TODAY" => Value::from(Date::today().iso()),
        "FORMAT_DATE" => Value::from(display(vals.first().unwrap_or(&Value::Null))),
        "ISNULL" => Value::Bool(vals.first().map_or(true, |v| v.is_null())),
        "COALESCE" => vals.into_iter().find(|v| !v.is_null()).unwrap_or_default(),
        other => return Err(format!("unknown system function $sys.func.{}", other)),
    })
}

/// Human-friendly rendering of a parameter value (dates, times, money…).
pub fn display(v: &Value) -> String {
    match v {
        Value::Object(_) => {
            if let (Some(y), Some(m), Some(d)) = (v.f64_field("year"), v.f64_field("month"), v.f64_field("day")) {
                return format!("{:04}-{:02}-{:02}", y as i64, m as i64, d as i64);
            }
            if let (Some(h), Some(mi)) = (v.f64_field("hours"), v.f64_field("minutes")) {
                return format!("{:02}:{:02}", h as i64, mi as i64);
            }
            if let (Some(a), Some(c)) = (v.f64_field("amount"), v.get("currency").and_then(Value::as_str)) {
                return format!("{} {}", format_number(a), c);
            }
            if let Some(n) = v.get("name").and_then(Value::as_str) {
                return n.to_string();
            }
            v.to_string()
        }
        Value::Array(a) => a.iter().map(display).collect::<Vec<_>>().join(", "),
        other => other.to_display(),
    }
}

/// Finds the end (char index, exclusive) of a function call starting at
/// `open` (index of '(') respecting nesting and quotes.
fn call_end(chars: &[char], open: usize) -> Option<usize> {
    let mut depth = 0;
    let mut quote: Option<char> = None;
    let mut i = open;
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = quote {
            if c == '\\' {
                i += 1;
            } else if c == q {
                quote = None;
            }
        } else if c == '"' || c == '\'' {
            quote = Some(c);
        } else if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
            if depth == 0 {
                return Some(i + 1);
            }
        }
        i += 1;
    }
    None
}

/// Replaces `$references` and `$sys.func.X(...)` calls inside text.
pub fn render_template(text: &str, scope: &dyn Scope) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() && chars[i + 1] == '$' {
            out.push('$');
            i += 2;
            continue;
        }
        if c != '$' || i + 1 >= chars.len() || !chars[i + 1].is_alphabetic() {
            out.push(c);
            i += 1;
            continue;
        }
        let start = i;
        let mut j = i + 1;
        while j < chars.len() && is_ref_char(chars[j]) {
            j += 1;
        }
        // trailing punctuation dots are not part of the path
        let mut end = j;
        while end > start + 1 && chars[end - 1] == '.' {
            end -= 1;
        }
        let path: String = chars[start + 1..end].iter().collect();
        if path.starts_with("sys.func.") && j < chars.len() && chars[j] == '(' {
            if let Some(close) = call_end(&chars, j) {
                let src: String = chars[start..close].iter().collect();
                match eval_expr(&src, scope) {
                    Ok(v) => out.push_str(&display(&v)),
                    Err(_) => out.push_str(&src),
                }
                i = close;
                continue;
            }
        }
        out.push_str(&display(&scope.lookup(&path)));
        i = end;
    }
    out
}

/// Evaluates a parameter-preset value: strings that are exactly a
/// reference or function call keep their type; other strings containing `$`
/// are rendered as templates; everything else is used as-is (recursively
/// for arrays/objects).
pub fn eval_value(v: &Value, scope: &dyn Scope) -> Value {
    match v {
        Value::String(s) => {
            let t = s.trim();
            if t.starts_with('$') {
                if let Ok(toks) = lex(t) {
                    let single = match toks.as_slice() {
                        [Tok::Ref(_)] => true,
                        [Tok::Func(_), ..] => {
                            let chars: Vec<char> = t.chars().collect();
                            chars.iter().position(|c| *c == '(').and_then(|p| call_end(&chars, p)) == Some(chars.len())
                        }
                        _ => false,
                    };
                    if single {
                        if let Ok(val) = eval_expr(t, scope) {
                            return val;
                        }
                    }
                }
            }
            if s.contains('$') {
                Value::String(render_template(s, scope))
            } else {
                v.clone()
            }
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| eval_value(x, scope)).collect()),
        Value::Object(m) => Value::Object(m.iter().map(|(k, x)| (k.clone(), eval_value(x, scope))).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cx_core::obj;

    fn scope() -> impl Fn(&str) -> Value {
        let params = obj! {"size" => "large", "count" => 3, "toppings" => cx_core::arr!["ham", "olive"], "date" => obj!{"year"=>2024,"month"=>3,"day"=>9}};
        move |p: &str| {
            if let Some(rest) = p.strip_prefix("session.params.") {
                params.path(rest).cloned().unwrap_or_default()
            } else if p == "page.params.status" {
                Value::from("FINAL")
            } else {
                Value::Null
            }
        }
    }

    #[test]
    fn conditions() {
        let s = scope();
        assert!(eval_condition("$session.params.size = \"large\"", &s).unwrap());
        assert!(eval_condition("$session.params.count > 2 AND $page.params.status = \"FINAL\"", &s).unwrap());
        assert!(!eval_condition("$session.params.missing != null", &s).unwrap());
        assert!(eval_condition("$session.params.missing = null OR false", &s).unwrap());
        assert!(eval_condition("NOT ($session.params.count < 1)", &s).unwrap());
        assert!(eval_condition("$session.params.toppings : \"ham\"", &s).unwrap());
        assert!(eval_condition("$session.params.size : *", &s).unwrap());
        assert!(eval_condition("true", &s).unwrap());
        assert!(eval_condition("", &s).unwrap());
        assert!(eval_condition("$sys.func.ADD($session.params.count, 2) = 5", &s).unwrap());
        assert!(eval_condition("$sys.func.COUNT($session.params.toppings) = 2", &s).unwrap());
        assert!(eval_condition("$session.params.size = \"x", &s).is_err());
    }

    #[test]
    fn templates() {
        let s = scope();
        assert_eq!(render_template("A $session.params.size pizza.", &s), "A large pizza.");
        assert_eq!(render_template("Total: $sys.func.MULTIPLY($session.params.count, 2.5)!", &s), "Total: 7.5!");
        assert_eq!(render_template("Toppings: $session.params.toppings", &s), "Toppings: ham, olive");
        assert_eq!(render_template("On $session.params.date.", &s), "On 2024-03-09.");
        assert_eq!(render_template("Costs \\$5 or $9", &s), "Costs $5 or $9");
        assert_eq!(
            render_template("$sys.func.IF(\"$session.params.count > 2\", \"many\", \"few\")", &s),
            "many"
        );
    }

    #[test]
    fn values() {
        let s = scope();
        assert_eq!(eval_value(&Value::from("$session.params.count"), &s), Value::from(3));
        assert_eq!(eval_value(&Value::from("$sys.func.ADD($session.params.count, 1)"), &s), Value::from(4));
        assert_eq!(eval_value(&Value::from("n=$session.params.count"), &s), Value::from("n=3"));
        assert_eq!(eval_value(&Value::from("$sys.func.APPEND($session.params.toppings, \"x\")"), &s).as_array().unwrap().len(), 3);
    }
}

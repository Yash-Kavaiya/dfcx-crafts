//! A compact regular-expression engine.
//!
//! Supported syntax: literals, `.`, character classes (`[a-z]`, `[^...]`),
//! shorthand classes (`\d \w \s \D \W \S`), anchors (`^ $ \b \B`), groups
//! (`( )`, `(?: )`, named `(?P<n> )` / `(?<n> )`), alternation `|`,
//! quantifiers `* + ? {n} {n,} {n,m}` (greedy and lazy) and the inline
//! case-insensitive flag `(?i)`.
//!
//! Matching uses a backtracking VM with a visited bitmap (the "bit-state"
//! technique), giving leftmost-first semantics in O(len(text) * len(prog)).

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "regex error: {}", self.0)
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Clone)]
enum ClassItem {
    Range(char, char),
    Digit(bool),
    Word(bool),
    Space(bool),
}

#[derive(Debug, Clone)]
struct Class {
    items: Vec<ClassItem>,
    negated: bool,
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl ClassItem {
    fn matches(&self, c: char) -> bool {
        match *self {
            ClassItem::Range(a, b) => a <= c && c <= b,
            ClassItem::Digit(neg) => c.is_ascii_digit() != neg,
            ClassItem::Word(neg) => is_word(c) != neg,
            ClassItem::Space(neg) => c.is_whitespace() != neg,
        }
    }
}

impl Class {
    fn matches(&self, c: char, ci: bool) -> bool {
        let hit = |ch: char| self.items.iter().any(|i| i.matches(ch));
        let mut m = hit(c);
        if !m && ci {
            m = c.to_lowercase().any(hit) || c.to_uppercase().any(hit);
        }
        m != self.negated
    }
}

#[derive(Debug, Clone)]
enum Node {
    Empty,
    Char(char),
    Any,
    Class(Class),
    Start,
    End,
    WordBoundary(bool),
    Group(Box<Node>, Option<usize>),
    Concat(Vec<Node>),
    Alt(Vec<Node>),
    Repeat { node: Box<Node>, min: u32, max: Option<u32>, greedy: bool },
}

struct Parser<'a> {
    chars: Vec<char>,
    pos: usize,
    ngroups: usize,
    names: Vec<(String, usize)>,
    ci: bool,
    _src: &'a str,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn err<T>(&self, m: &str) -> Result<T, Error> {
        Err(Error(format!("{} at position {}", m, self.pos)))
    }

    fn parse_alt(&mut self) -> Result<Node, Error> {
        let mut branches = vec![self.parse_concat()?];
        while self.eat('|') {
            branches.push(self.parse_concat()?);
        }
        Ok(if branches.len() == 1 { branches.pop().unwrap() } else { Node::Alt(branches) })
    }

    fn parse_concat(&mut self) -> Result<Node, Error> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.parse_atom()?;
            let atom = self.parse_quant(atom)?;
            items.push(atom);
        }
        Ok(match items.len() {
            0 => Node::Empty,
            1 => items.pop().unwrap(),
            _ => Node::Concat(items),
        })
    }

    fn parse_number(&mut self) -> Option<u32> {
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.pos += 1;
        }
        if self.pos == start {
            return None;
        }
        self.chars[start..self.pos].iter().collect::<String>().parse().ok()
    }

    fn parse_quant(&mut self, atom: Node) -> Result<Node, Error> {
        let (min, max) = match self.peek() {
            Some('*') => {
                self.pos += 1;
                (0, None)
            }
            Some('+') => {
                self.pos += 1;
                (1, None)
            }
            Some('?') => {
                self.pos += 1;
                (0, Some(1))
            }
            Some('{') => {
                let save = self.pos;
                self.pos += 1;
                match self.parse_number() {
                    Some(n) => {
                        let max = if self.eat(',') {
                            if self.peek() == Some('}') {
                                None
                            } else {
                                match self.parse_number() {
                                    Some(m) => Some(m),
                                    None => return self.err("invalid repetition"),
                                }
                            }
                        } else {
                            Some(n)
                        };
                        if !self.eat('}') {
                            return self.err("unclosed repetition");
                        }
                        if let Some(m) = max {
                            if m < n {
                                return self.err("invalid repetition range");
                            }
                        }
                        if n > 1000 || max.unwrap_or(0) > 1000 {
                            return self.err("repetition too large");
                        }
                        (n, max)
                    }
                    None => {
                        // literal '{'
                        self.pos = save;
                        return Ok(atom);
                    }
                }
            }
            _ => return Ok(atom),
        };
        if matches!(atom, Node::Start | Node::End | Node::WordBoundary(_)) {
            return Ok(atom);
        }
        let greedy = !self.eat('?');
        let node = Node::Repeat { node: Box::new(atom), min, max, greedy };
        // Allow stacked quantifiers like a** gracefully.
        if matches!(self.peek(), Some('*') | Some('+')) {
            return self.err("nested quantifier");
        }
        Ok(node)
    }

    fn parse_escape_item(&mut self) -> Result<Result<ClassItem, char>, Error> {
        // after backslash; returns either a class item or a literal char
        let c = match self.peek() {
            Some(c) => c,
            None => return self.err("trailing backslash"),
        };
        self.pos += 1;
        Ok(match c {
            'd' => Ok(ClassItem::Digit(false)),
            'D' => Ok(ClassItem::Digit(true)),
            'w' => Ok(ClassItem::Word(false)),
            'W' => Ok(ClassItem::Word(true)),
            's' => Ok(ClassItem::Space(false)),
            'S' => Ok(ClassItem::Space(true)),
            'n' => Err('\n'),
            't' => Err('\t'),
            'r' => Err('\r'),
            'f' => Err('\u{0c}'),
            'v' => Err('\u{0b}'),
            '0' => Err('\0'),
            'u' => {
                let hex: String = self.chars.get(self.pos..self.pos + 4).map(|s| s.iter().collect()).unwrap_or_default();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(ch) => {
                        self.pos += 4;
                        Err(ch)
                    }
                    None => return self.err("invalid \\u escape"),
                }
            }
            other => Err(other),
        })
    }

    fn parse_class(&mut self) -> Result<Node, Error> {
        // '[' already consumed
        let negated = self.eat('^');
        let mut items = Vec::new();
        let mut first = true;
        loop {
            let c = match self.peek() {
                Some(c) => c,
                None => return self.err("unclosed character class"),
            };
            if c == ']' && !first {
                self.pos += 1;
                break;
            }
            first = false;
            self.pos += 1;
            let lo = if c == '\\' {
                match self.parse_escape_item()? {
                    Ok(item) => {
                        items.push(item);
                        continue;
                    }
                    Err(ch) => ch,
                }
            } else {
                c
            };
            if self.peek() == Some('-') && self.chars.get(self.pos + 1).map_or(false, |&n| n != ']') {
                self.pos += 1;
                let hc = self.peek().unwrap();
                self.pos += 1;
                let hi = if hc == '\\' {
                    match self.parse_escape_item()? {
                        Ok(_) => return self.err("invalid class range"),
                        Err(ch) => ch,
                    }
                } else {
                    hc
                };
                if hi < lo {
                    return self.err("invalid class range");
                }
                items.push(ClassItem::Range(lo, hi));
            } else {
                items.push(ClassItem::Range(lo, lo));
            }
        }
        Ok(Node::Class(Class { items, negated }))
    }

    fn parse_atom(&mut self) -> Result<Node, Error> {
        let c = self.peek().unwrap();
        self.pos += 1;
        match c {
            '.' => Ok(Node::Any),
            '^' => Ok(Node::Start),
            '$' => Ok(Node::End),
            '[' => self.parse_class(),
            '(' => {
                let mut capture = true;
                let mut name = None;
                if self.eat('?') {
                    if self.eat(':') {
                        capture = false;
                    } else if self.peek() == Some('i') && self.chars.get(self.pos + 1) == Some(&')') {
                        self.pos += 2;
                        self.ci = true;
                        return Ok(Node::Empty);
                    } else if self.eat('P') || self.peek() == Some('<') {
                        if !self.eat('<') {
                            return self.err("invalid group");
                        }
                        let start = self.pos;
                        while matches!(self.peek(), Some(ch) if ch != '>') {
                            self.pos += 1;
                        }
                        name = Some(self.chars[start..self.pos].iter().collect::<String>());
                        if !self.eat('>') {
                            return self.err("unclosed group name");
                        }
                    } else {
                        return self.err("unsupported group syntax");
                    }
                }
                let idx = if capture {
                    self.ngroups += 1;
                    if let Some(n) = name {
                        self.names.push((n, self.ngroups));
                    }
                    Some(self.ngroups)
                } else {
                    None
                };
                let inner = self.parse_alt()?;
                if !self.eat(')') {
                    return self.err("unclosed group");
                }
                Ok(Node::Group(Box::new(inner), idx))
            }
            ')' => self.err("unmatched ')'"),
            '*' | '+' | '?' => self.err("nothing to repeat"),
            '\\' => {
                match self.peek() {
                    Some('b') => {
                        self.pos += 1;
                        return Ok(Node::WordBoundary(true));
                    }
                    Some('B') => {
                        self.pos += 1;
                        return Ok(Node::WordBoundary(false));
                    }
                    _ => {}
                }
                Ok(match self.parse_escape_item()? {
                    Ok(item) => Node::Class(Class { items: vec![item], negated: false }),
                    Err(ch) => Node::Char(ch),
                })
            }
            other => Ok(Node::Char(other)),
        }
    }
}

#[derive(Debug, Clone)]
enum Inst {
    Char(char),
    Any,
    Class(usize),
    Start,
    End,
    WordBoundary(bool),
    Split(usize, usize),
    Jmp(usize),
    Save(usize),
    Match,
}

struct Compiler {
    prog: Vec<Inst>,
    classes: Vec<Class>,
}

impl Compiler {
    fn emit(&mut self, i: Inst) -> usize {
        self.prog.push(i);
        self.prog.len() - 1
    }
    fn compile(&mut self, n: &Node) {
        match n {
            Node::Empty => {}
            Node::Char(c) => {
                self.emit(Inst::Char(*c));
            }
            Node::Any => {
                self.emit(Inst::Any);
            }
            Node::Class(c) => {
                self.classes.push(c.clone());
                let idx = self.classes.len() - 1;
                self.emit(Inst::Class(idx));
            }
            Node::Start => {
                self.emit(Inst::Start);
            }
            Node::End => {
                self.emit(Inst::End);
            }
            Node::WordBoundary(b) => {
                self.emit(Inst::WordBoundary(*b));
            }
            Node::Group(inner, idx) => {
                if let Some(i) = idx {
                    self.emit(Inst::Save(2 * i));
                    self.compile(inner);
                    self.emit(Inst::Save(2 * i + 1));
                } else {
                    self.compile(inner);
                }
            }
            Node::Concat(items) => {
                for it in items {
                    self.compile(it);
                }
            }
            Node::Alt(branches) => {
                let mut jumps = Vec::new();
                for (i, b) in branches.iter().enumerate() {
                    if i + 1 < branches.len() {
                        let split = self.emit(Inst::Split(0, 0));
                        self.compile(b);
                        jumps.push(self.emit(Inst::Jmp(0)));
                        let next = self.prog.len();
                        self.prog[split] = Inst::Split(split + 1, next);
                    } else {
                        self.compile(b);
                    }
                }
                let end = self.prog.len();
                for j in jumps {
                    self.prog[j] = Inst::Jmp(end);
                }
            }
            Node::Repeat { node, min, max, greedy } => {
                for _ in 0..*min {
                    self.compile(node);
                }
                match max {
                    None => {
                        let split = self.emit(Inst::Split(0, 0));
                        self.compile(node);
                        self.emit(Inst::Jmp(split));
                        let out = self.prog.len();
                        self.prog[split] = if *greedy { Inst::Split(split + 1, out) } else { Inst::Split(out, split + 1) };
                    }
                    Some(m) => {
                        let mut splits = Vec::new();
                        for _ in *min..*m {
                            splits.push(self.emit(Inst::Split(0, 0)));
                            self.compile(node);
                        }
                        let out = self.prog.len();
                        for s in splits {
                            self.prog[s] = if *greedy { Inst::Split(s + 1, out) } else { Inst::Split(out, s + 1) };
                        }
                    }
                }
            }
        }
    }
}

/// A compiled regular expression.
#[derive(Debug, Clone)]
pub struct Regex {
    prog: Vec<Inst>,
    classes: Vec<Class>,
    ngroups: usize,
    names: Vec<(String, usize)>,
    ci: bool,
    source: String,
}

/// A match: byte offsets into the haystack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    pub start: usize,
    pub end: usize,
}

impl Regex {
    pub fn new(pattern: &str) -> Result<Regex, Error> {
        Self::build(pattern, false)
    }
    pub fn new_case_insensitive(pattern: &str) -> Result<Regex, Error> {
        Self::build(pattern, true)
    }
    fn build(pattern: &str, ci: bool) -> Result<Regex, Error> {
        let mut p = Parser { chars: pattern.chars().collect(), pos: 0, ngroups: 0, names: Vec::new(), ci, _src: pattern };
        let node = p.parse_alt()?;
        if p.pos != p.chars.len() {
            return p.err("unexpected ')'");
        }
        let mut c = Compiler { prog: Vec::new(), classes: Vec::new() };
        c.emit(Inst::Save(0));
        c.compile(&node);
        c.emit(Inst::Save(1));
        c.emit(Inst::Match);
        if c.prog.len() > 100_000 {
            return Err(Error("pattern too large".into()));
        }
        Ok(Regex { prog: c.prog, classes: c.classes, ngroups: p.ngroups, names: p.names, ci: p.ci, source: pattern.to_string() })
    }

    pub fn as_str(&self) -> &str {
        &self.source
    }
    pub fn group_count(&self) -> usize {
        self.ngroups
    }
    pub fn group_index(&self, name: &str) -> Option<usize> {
        self.names.iter().find(|(n, _)| n == name).map(|(_, i)| *i)
    }

    pub fn is_match(&self, text: &str) -> bool {
        self.find(text).is_some()
    }

    pub fn find(&self, text: &str) -> Option<Match> {
        self.captures_at(text, 0).and_then(|c| c[0])
    }

    /// All non-overlapping matches.
    pub fn find_iter(&self, text: &str) -> Vec<Match> {
        let mut out = Vec::new();
        let mut start = 0;
        while start <= text.len() {
            match self.captures_at(text, start).and_then(|c| c[0]) {
                Some(m) => {
                    out.push(m);
                    start = if m.end == m.start {
                        // advance by one char
                        match text[m.end..].chars().next() {
                            Some(ch) => m.end + ch.len_utf8(),
                            None => break,
                        }
                    } else {
                        m.end
                    };
                }
                None => break,
            }
        }
        out
    }

    pub fn captures(&self, text: &str) -> Option<Vec<Option<Match>>> {
        self.captures_at(text, 0)
    }

    /// Searches starting at byte offset `start`; returns capture groups
    /// (group 0 = whole match).
    pub fn captures_at(&self, text: &str, start: usize) -> Option<Vec<Option<Match>>> {
        let chars: Vec<(usize, char)> = text.char_indices().collect();
        let n = chars.len();
        let start_ci = chars.iter().position(|(b, _)| *b >= start).unwrap_or(n);
        let plen = self.prog.len();
        let mut visited = vec![0u64; ((plen * (n + 1)) + 63) / 64];
        let byte_at = |i: usize| if i < n { chars[i].0 } else { text.len() };
        for s in start_ci..=n {
            if let Some(slots) = self.run(&chars, s, &mut visited) {
                let mut caps = Vec::with_capacity(self.ngroups + 1);
                for g in 0..=self.ngroups {
                    match (slots[2 * g], slots[2 * g + 1]) {
                        (Some(a), Some(b)) => caps.push(Some(Match { start: byte_at(a), end: byte_at(b) })),
                        _ => caps.push(None),
                    }
                }
                return Some(caps);
            }
            // If the program begins with '^' (after Save 0), no later start can match.
            if matches!(self.prog.get(1), Some(Inst::Start)) {
                break;
            }
        }
        None
    }

    fn char_eq(&self, a: char, b: char) -> bool {
        a == b || (self.ci && a.to_lowercase().eq(b.to_lowercase()))
    }

    fn run(&self, chars: &[(usize, char)], start: usize, visited: &mut [u64]) -> Option<Vec<Option<usize>>> {
        let n = chars.len();
        let nslots = 2 * (self.ngroups + 1);
        enum Job {
            Pc(usize, usize),
            Restore(usize, Option<usize>),
        }
        let mut slots: Vec<Option<usize>> = vec![None; nslots];
        let mut stack = vec![Job::Pc(0, start)];
        while let Some(job) = stack.pop() {
            let (mut pc, mut pos) = match job {
                Job::Restore(slot, val) => {
                    slots[slot] = val;
                    continue;
                }
                Job::Pc(pc, pos) => (pc, pos),
            };
            loop {
                let key = pc * (n + 1) + pos;
                if visited[key / 64] & (1 << (key % 64)) != 0 {
                    break;
                }
                visited[key / 64] |= 1 << (key % 64);
                match &self.prog[pc] {
                    Inst::Match => return Some(slots),
                    Inst::Char(c) => {
                        if pos < n && self.char_eq(chars[pos].1, *c) {
                            pc += 1;
                            pos += 1;
                        } else {
                            break;
                        }
                    }
                    Inst::Any => {
                        if pos < n && chars[pos].1 != '\n' {
                            pc += 1;
                            pos += 1;
                        } else {
                            break;
                        }
                    }
                    Inst::Class(i) => {
                        if pos < n && self.classes[*i].matches(chars[pos].1, self.ci) {
                            pc += 1;
                            pos += 1;
                        } else {
                            break;
                        }
                    }
                    Inst::Start => {
                        if pos == 0 {
                            pc += 1
                        } else {
                            break;
                        }
                    }
                    Inst::End => {
                        if pos == n {
                            pc += 1
                        } else {
                            break;
                        }
                    }
                    Inst::WordBoundary(want) => {
                        let before = pos > 0 && is_word(chars[pos - 1].1);
                        let after = pos < n && is_word(chars[pos].1);
                        if (before != after) == *want {
                            pc += 1
                        } else {
                            break;
                        }
                    }
                    Inst::Jmp(t) => pc = *t,
                    Inst::Split(a, b) => {
                        stack.push(Job::Pc(*b, pos));
                        pc = *a;
                    }
                    Inst::Save(s) => {
                        stack.push(Job::Restore(*s, slots[*s]));
                        slots[*s] = Some(pos);
                        pc += 1;
                    }
                }
            }
        }
        // Visited bits are intentionally kept for later start positions: a
        // (pc, pos) state that failed once can never lead to a match.
        None
    }
}

/// Escapes regex metacharacters in `s`.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "\\.+*?()|[]{}^$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(p: &str, t: &str) -> Option<(usize, usize)> {
        Regex::new(p).unwrap().find(t).map(|m| (m.start, m.end))
    }

    #[test]
    fn basics() {
        assert_eq!(m("abc", "xxabcxx"), Some((2, 5)));
        assert_eq!(m("a.c", "abc"), Some((0, 3)));
        assert_eq!(m("^abc$", "abc"), Some((0, 3)));
        assert_eq!(m("^abc$", "abcd"), None);
        assert_eq!(m("a|b|c", "zzc"), Some((2, 3)));
        assert_eq!(m("colou?r", "my color"), Some((3, 8)));
        assert_eq!(m("\\d+", "order 12345 now"), Some((6, 11)));
        assert_eq!(m("[A-Z]{3}-\\d{2,4}", "id ABC-123x"), Some((3, 10)));
        assert_eq!(m("a*?b", "aaab"), Some((0, 4)));
        assert_eq!(m("\\bcat\\b", "concat cat"), Some((7, 10)));
        assert_eq!(m("[^a-z ]+", "abc DEF"), Some((4, 7)));
        assert_eq!(m("x{2}", "x xx"), Some((2, 4)));
    }

    #[test]
    fn email_like() {
        let r = Regex::new(r"[\w.+-]+@[\w-]+\.[\w.]+").unwrap();
        let f = r.find("mail me at john.doe+x@example.co.uk please").unwrap();
        assert_eq!(&"mail me at john.doe+x@example.co.uk please"[f.start..f.end], "john.doe+x@example.co.uk");
    }

    #[test]
    fn captures_and_ci() {
        let r = Regex::new(r"(?i)order (?P<id>[a-z]+\d+)").unwrap();
        let t = "ORDER Ab12 ok";
        let c = r.captures(t).unwrap();
        let g = c[r.group_index("id").unwrap()].unwrap();
        assert_eq!(&t[g.start..g.end], "Ab12");
    }

    #[test]
    fn pathological_is_fast() {
        let r = Regex::new("(a*)*b").unwrap();
        let t = "a".repeat(2000);
        assert!(r.find(&t).is_none());
    }

    #[test]
    fn unicode_and_iter() {
        let r = Regex::new("é+").unwrap();
        let t = "caféé et é";
        let all = r.find_iter(t);
        assert_eq!(all.len(), 2);
        assert_eq!(&t[all[0].start..all[0].end], "éé");
        assert!(Regex::new("(").is_err());
        assert!(Regex::new("a{3,1}").is_err());
    }
}

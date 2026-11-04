//! Custom entity types (map, list and regexp kinds) and mention extraction.

use crate::system;
use crate::text::{fuzzy_eq, tokenize, Token};
use cx_core::Value;
use cx_regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Map,
    List,
    Regexp,
}

impl Kind {
    pub fn parse(s: &str) -> Kind {
        match s {
            "KIND_LIST" => Kind::List,
            "KIND_REGEXP" => Kind::Regexp,
            _ => Kind::Map,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub value: String,
    pub synonyms: Vec<String>,
}

/// A compiled custom entity type.
#[derive(Debug, Clone)]
pub struct EntityTypeDef {
    /// Lookup key (usually the resource name).
    pub key: String,
    pub display_name: String,
    pub kind: Kind,
    pub entities: Vec<Entity>,
    pub excluded: Vec<String>,
    pub fuzzy: bool,
    synonyms: Vec<(Vec<String>, usize)>,
    regexes: Vec<(Regex, usize)>,
}

impl EntityTypeDef {
    pub fn new(key: &str, display_name: &str, kind: Kind, entities: Vec<Entity>, excluded: Vec<String>, fuzzy: bool) -> Self {
        let mut synonyms = Vec::new();
        let mut regexes = Vec::new();
        for (i, e) in entities.iter().enumerate() {
            let mut all: Vec<&String> = e.synonyms.iter().collect();
            if !e.synonyms.iter().any(|s| s == &e.value) {
                all.push(&e.value);
            }
            for s in all {
                match kind {
                    Kind::Regexp => {
                        if let Ok(r) = Regex::new_case_insensitive(s) {
                            regexes.push((r, i));
                        }
                    }
                    _ => {
                        let toks: Vec<String> = tokenize(s).into_iter().map(|t| t.norm).collect();
                        if !toks.is_empty() {
                            synonyms.push((toks, i));
                        }
                    }
                }
            }
        }
        // longest synonyms first so the scan prefers them
        synonyms.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        let excluded = excluded.iter().map(|e| crate::text::normalize(e)).collect();
        EntityTypeDef { key: key.to_string(), display_name: display_name.to_string(), kind, entities, excluded, fuzzy, synonyms, regexes }
    }

    /// Builds from a Conversational Agents `EntityType` JSON resource.
    pub fn from_json(key: &str, v: &Value) -> Self {
        let entities = v
            .arr("entities")
            .iter()
            .map(|e| Entity {
                value: e.str("value").to_string(),
                synonyms: e.arr("synonyms").iter().filter_map(|s| s.as_str().map(String::from)).collect(),
            })
            .collect();
        let excluded = v.arr("excludedPhrases").iter().map(|p| p.str("value").to_string()).collect();
        EntityTypeDef::new(key, v.str("displayName"), Kind::parse(v.str("kind")), entities, excluded, v.bool_field("enableFuzzyExtraction"))
    }

    /// Finds all mentions of this type in `text`.
    pub fn find(&self, text: &str, tokens: &[Token]) -> Vec<Mention> {
        let mut out = Vec::new();
        match self.kind {
            Kind::Regexp => {
                for (r, idx) in &self.regexes {
                    for m in r.find_iter(text) {
                        if m.end > m.start {
                            let original = &text[m.start..m.end];
                            out.push(Mention::new(&self.key, m.start, m.end, original, Value::from(original), 2));
                            let _ = idx;
                        }
                    }
                }
            }
            Kind::Map | Kind::List => {
                let mut i = 0;
                while i < tokens.len() {
                    let mut best: Option<(usize, usize, bool)> = None; // (len, entity, exact)
                    for (syn, idx) in &self.synonyms {
                        let n = syn.len();
                        if i + n > tokens.len() || best.map_or(false, |(l, _, ex)| l > n || (l == n && ex)) {
                            continue;
                        }
                        let mut exact = true;
                        let ok = syn.iter().zip(&tokens[i..i + n]).all(|(s, t)| {
                            if *s == t.norm || *s == t.stem || crate::text::stem(s) == t.stem {
                                true
                            } else if self.fuzzy && fuzzy_eq(s, &t.norm) {
                                exact = false;
                                true
                            } else {
                                false
                            }
                        });
                        if ok && best.map_or(true, |(l, _, ex)| n > l || (n == l && exact && !ex)) {
                            best = Some((n, *idx, exact));
                        }
                    }
                    if let Some((n, idx, exact)) = best {
                        let start = tokens[i].start;
                        let end = tokens[i + n - 1].end;
                        let original = &text[start..end];
                        let norm = crate::text::normalize(original);
                        if !self.excluded.contains(&norm) {
                            let value = match self.kind {
                                Kind::List => Value::from(self.entities[idx].value.clone()),
                                _ => Value::from(self.entities[idx].value.clone()),
                            };
                            out.push(Mention::new(&self.key, start, end, original, value, if exact { 3 } else { 1 }));
                        }
                        i += n;
                    } else {
                        i += 1;
                    }
                }
            }
        }
        out
    }
}

/// An entity mention found in an utterance.
#[derive(Debug, Clone, PartialEq)]
pub struct Mention {
    /// Entity type key (`sys.number` or a custom key).
    pub entity_type: String,
    pub start: usize,
    pub end: usize,
    pub original: String,
    pub value: Value,
    /// Preference when overlapping: higher wins on equal length.
    pub priority: u8,
}

impl Mention {
    pub fn new(entity_type: &str, start: usize, end: usize, original: &str, value: Value, priority: u8) -> Self {
        Mention { entity_type: entity_type.to_string(), start, end, original: original.to_string(), value, priority }
    }
    pub fn overlaps(&self, o: &Mention) -> bool {
        self.start < o.end && o.start < self.end
    }
}

/// Registry of entity types available for extraction.
#[derive(Debug, Clone, Default)]
pub struct EntityRegistry {
    pub custom: Vec<EntityTypeDef>,
}

impl EntityRegistry {
    pub fn get(&self, key: &str) -> Option<&EntityTypeDef> {
        self.custom.iter().find(|e| e.key == key || e.display_name == key)
    }

    /// All candidate mentions (possibly overlapping) for the given types.
    pub fn candidates(&self, text: &str, tokens: &[Token], types: &[String], overrides: &[EntityTypeDef]) -> Vec<Mention> {
        let mut out = Vec::new();
        for t in types {
            if system::is_system(t) {
                out.extend(system::find(t, text, tokens));
            } else {
                let def = overrides.iter().find(|e| e.key == *t).or_else(|| self.get(t));
                if let Some(def) = def {
                    out.extend(def.find(text, tokens));
                }
                // supplements share the key but are additive
                for sup in overrides.iter().filter(|e| e.key == format!("{}#supplement", t)) {
                    for mut m in sup.find(text, tokens) {
                        m.entity_type = t.clone();
                        out.push(m);
                    }
                }
            }
        }
        out
    }
}

/// Picks a non-overlapping subset of mentions: longest first, then priority,
/// then leftmost. Result is sorted by start offset.
pub fn select_non_overlapping(mut cands: Vec<Mention>) -> Vec<Mention> {
    cands.sort_by(|a, b| {
        (b.end - b.start)
            .cmp(&(a.end - a.start))
            .then(b.priority.cmp(&a.priority))
            .then(a.start.cmp(&b.start))
    });
    let mut chosen: Vec<Mention> = Vec::new();
    for c in cands {
        if !chosen.iter().any(|m| m.overlaps(&c)) {
            chosen.push(c);
        }
    }
    chosen.sort_by_key(|m| m.start);
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size_type() -> EntityTypeDef {
        EntityTypeDef::new(
            "size",
            "size",
            Kind::Map,
            vec![
                Entity { value: "large".into(), synonyms: vec!["large".into(), "big".into(), "extra large".into()] },
                Entity { value: "small".into(), synonyms: vec!["small".into(), "tiny".into()] },
            ],
            vec![],
            true,
        )
    }

    #[test]
    fn map_entities() {
        let t = size_type();
        let text = "an extra large and a tiny one";
        let ms = t.find(text, &tokenize(text));
        assert_eq!(ms.len(), 2);
        assert_eq!(ms[0].value, Value::from("large"));
        assert_eq!(ms[0].original, "extra large");
        assert_eq!(ms[1].value, Value::from("small"));
    }

    #[test]
    fn regexp_entities() {
        let t = EntityTypeDef::new(
            "order",
            "order-id",
            Kind::Regexp,
            vec![Entity { value: r"ORD-\d{4}".into(), synonyms: vec![] }],
            vec![],
            false,
        );
        let text = "status of ord-1234 please";
        let ms = t.find(text, &tokenize(text));
        assert_eq!(ms[0].original, "ord-1234");
    }

    #[test]
    fn selection() {
        let reg = EntityRegistry { custom: vec![size_type()] };
        let text = "two extra large";
        let toks = tokenize(text);
        let c = reg.candidates(text, &toks, &["size".into(), "sys.number".into()], &[]);
        let s = select_non_overlapping(c);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].entity_type, "sys.number");
        assert_eq!(s[0].value, Value::from(2));
    }
}

//! Intent classification with parameter extraction.
//!
//! Training phrases are compiled into token templates in which annotated
//! parts become typed slots. Queries are rewritten the same way (detected
//! entity mentions collapse into slots), and similarity combines an
//! IDF-weighted Dice overlap with a token-level edit-distance score.
//! Open entities (`@sys.any`, `@sys.person`, …) are extracted by aligning a
//! phrase's literal prefix/suffix around the slot.

use crate::entities::{select_non_overlapping, EntityRegistry, EntityTypeDef, Mention};
use crate::system;
use crate::text::{fuzzy_eq, is_stopword, levenshtein_seq, stem, tokenize, Token};
use cx_core::Value;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct ParamDef {
    pub id: String,
    /// Normalised entity type key (`sys.number` or custom key).
    pub entity_type: String,
    pub is_list: bool,
}

#[derive(Debug, Clone)]
pub struct PhrasePart {
    pub text: String,
    pub parameter_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IntentDef {
    pub key: String,
    pub display_name: String,
    pub phrases: Vec<Vec<PhrasePart>>,
    pub params: Vec<ParamDef>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParamValue {
    pub resolved: Value,
    pub original: String,
}

#[derive(Debug, Clone)]
pub struct IntentMatch {
    pub key: String,
    pub display_name: String,
    pub confidence: f64,
    pub params: Vec<(String, ParamValue)>,
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Word(String),
    Slot(String),
}

#[derive(Debug, Clone)]
struct PTok {
    tok: Tok,
    param: Option<String>,
}

#[derive(Debug, Clone)]
struct CompiledIntent {
    def: IntentDef,
    phrases: Vec<Vec<PTok>>,
    closed_types: Vec<String>,
}

#[derive(Debug, Clone)]
struct QTok {
    tok: Tok,
    start: usize,
    end: usize,
    mention: Option<usize>,
}

/// A trained intent classifier.
#[derive(Debug, Clone, Default)]
pub struct Classifier {
    intents: Vec<CompiledIntent>,
    weights: HashMap<String, f64>,
    vocab: Vec<String>,
    vocab_set: HashSet<String>,
    max_weight: f64,
    pub entities: EntityRegistry,
    all_types: Vec<String>,
}

impl Classifier {
    pub fn new(intents: Vec<IntentDef>, entities: EntityRegistry) -> Self {
        let mut compiled = Vec::new();
        let mut df: HashMap<String, usize> = HashMap::new();
        let mut all_types: Vec<String> = Vec::new();
        for def in intents {
            let mut phrases = Vec::new();
            let mut seen_in_intent: HashSet<String> = HashSet::new();
            for parts in &def.phrases {
                let mut toks = Vec::new();
                for part in parts {
                    let param = part.parameter_id.as_ref().and_then(|pid| def.params.iter().find(|p| &p.id == pid));
                    match param {
                        Some(p) => toks.push(PTok { tok: Tok::Slot(p.entity_type.clone()), param: Some(p.id.clone()) }),
                        None => {
                            for t in tokenize(&part.text) {
                                seen_in_intent.insert(t.stem.clone());
                                toks.push(PTok { tok: Tok::Word(t.stem), param: None });
                            }
                        }
                    }
                }
                if !toks.is_empty() {
                    phrases.push(toks);
                }
            }
            for w in seen_in_intent {
                *df.entry(w).or_insert(0) += 1;
            }
            let mut closed_types: Vec<String> = Vec::new();
            for p in &def.params {
                if !system::is_open(&p.entity_type) && !closed_types.contains(&p.entity_type) {
                    closed_types.push(p.entity_type.clone());
                }
                if !all_types.contains(&p.entity_type) {
                    all_types.push(p.entity_type.clone());
                }
            }
            compiled.push(CompiledIntent { def, phrases, closed_types });
        }
        let n = compiled.len().max(1) as f64;
        let mut weights = HashMap::new();
        let denom = (1.0 + n).ln();
        for (w, d) in &df {
            let idf = (1.0 + n / *d as f64).ln() / denom; // in (0, 1]
            let mut wt = 0.4 + 1.6 * idf;
            if is_stopword(w) {
                wt *= 0.25;
            }
            weights.insert(w.clone(), wt);
        }
        let max_weight = 2.0;
        let mut vocab: Vec<String> = df.keys().cloned().collect();
        vocab.sort();
        let vocab_set = vocab.iter().cloned().collect();
        Classifier { intents: compiled, weights, vocab, vocab_set, max_weight, entities, all_types }
    }

    fn weight(&self, t: &Tok) -> f64 {
        match t {
            Tok::Slot(_) => 1.5,
            Tok::Word(w) => match self.weights.get(w) {
                Some(x) => *x,
                None if is_stopword(w) => 0.3,
                None => self.max_weight,
            },
        }
    }

    fn correct(&self, stem_: &str) -> String {
        if self.vocab_set.contains(stem_) || stem_.chars().count() < 4 {
            return stem_.to_string();
        }
        for v in &self.vocab {
            if fuzzy_eq(stem_, v) {
                return v.clone();
            }
        }
        stem_.to_string()
    }

    /// Detects entity mentions for the given types.
    pub fn mentions(&self, text: &str, tokens: &[Token], types: &[String], overrides: &[EntityTypeDef]) -> Vec<Mention> {
        select_non_overlapping(self.entities.candidates(text, tokens, types, overrides))
    }

    fn query_seq(&self, tokens: &[Token], mentions: &[Mention]) -> Vec<QTok> {
        let mut out = Vec::new();
        let mut mi = 0;
        let mut i = 0;
        while i < tokens.len() {
            let t = &tokens[i];
            while mi < mentions.len() && mentions[mi].end <= t.start {
                mi += 1;
            }
            if mi < mentions.len() && mentions[mi].start < t.end && t.start < mentions[mi].end {
                let m = &mentions[mi];
                out.push(QTok { tok: Tok::Slot(m.entity_type.clone()), start: m.start.min(t.start), end: m.end, mention: Some(mi) });
                while i < tokens.len() && tokens[i].start < m.end {
                    i += 1;
                }
                mi += 1;
                continue;
            }
            out.push(QTok { tok: Tok::Word(self.correct(&t.stem)), start: t.start, end: t.end, mention: None });
            i += 1;
        }
        out
    }

    fn similarity(&self, q: &[Tok], p: &[Tok]) -> f64 {
        if q.is_empty() || p.is_empty() {
            return 0.0;
        }
        if q == p {
            return 1.0;
        }
        let qs: Vec<&Tok> = dedup(q);
        let ps: Vec<&Tok> = dedup(p);
        let wq: f64 = qs.iter().map(|t| self.weight(t)).sum();
        let wp: f64 = ps.iter().map(|t| self.weight(t)).sum();
        let common: f64 = qs.iter().filter(|t| ps.contains(t)).map(|t| self.weight(t)).sum();
        let dice = if wq + wp > 0.0 { 2.0 * common / (wq + wp) } else { 0.0 };
        let dist = levenshtein_seq(q, p, |a, b| a == b) as f64;
        let seq = 1.0 - dist / q.len().max(p.len()) as f64;
        (0.75 * dice + 0.25 * seq).clamp(0.0, 0.99)
    }

    /// Scores every intent against `text`. Results are sorted by confidence.
    pub fn classify(&self, text: &str, overrides: &[EntityTypeDef]) -> Vec<IntentMatch> {
        let tokens = tokenize(text);
        if tokens.is_empty() {
            return Vec::new();
        }
        let closed_all: Vec<String> = self.all_types.iter().filter(|t| !system::is_open(t)).cloned().collect();
        let all_cands = self.entities.candidates(text, &tokens, &closed_all, overrides);
        let mut results = Vec::new();
        for ci in &self.intents {
            let cands: Vec<Mention> = all_cands.iter().filter(|m| ci.closed_types.contains(&m.entity_type)).cloned().collect();
            let mentions = select_non_overlapping(cands);
            let qseq = self.query_seq(&tokens, &mentions);
            let qtoks: Vec<Tok> = qseq.iter().map(|q| q.tok.clone()).collect();
            let mut best: Option<(f64, usize, Option<(String, ParamValue)>)> = None;
            for (pi, phrase) in ci.phrases.iter().enumerate() {
                let ptoks: Vec<Tok> = phrase.iter().map(|p| p.tok.clone()).collect();
                let open_slots: Vec<usize> =
                    phrase.iter().enumerate().filter(|(_, p)| matches!(&p.tok, Tok::Slot(t) if system::is_open(t))).map(|(i, _)| i).collect();
                let mut score;
                let mut open_val = None;
                if open_slots.len() == 1 {
                    let k = open_slots[0];
                    let prefix = &ptoks[..k];
                    let suffix = &ptoks[k + 1..];
                    let fits = qtoks.len() > prefix.len() + suffix.len()
                        && qtoks[..prefix.len()] == *prefix
                        && qtoks[qtoks.len() - suffix.len()..] == *suffix;
                    if fits {
                        let mid = &qseq[prefix.len()..qseq.len() - suffix.len()];
                        let s = mid.first().unwrap().start;
                        let e = mid.last().unwrap().end;
                        let original = text[s..e].to_string();
                        let param = phrase[k].param.clone().unwrap_or_default();
                        let ty = match &phrase[k].tok {
                            Tok::Slot(t) => t.clone(),
                            _ => String::new(),
                        };
                        open_val = Some((param, ParamValue { resolved: system::open_value(&ty, &original), original }));
                        // literal context matched exactly: strong evidence
                        let literal = (prefix.len() + suffix.len()) as f64;
                        score = if literal == 0.0 { 0.5 } else { (0.85 + 0.03 * literal).min(0.98) };
                    } else {
                        score = self.similarity(&qtoks, &ptoks) * 0.85;
                    }
                } else {
                    score = self.similarity(&qtoks, &ptoks);
                }
                if score.is_nan() {
                    score = 0.0;
                }
                if best.as_ref().map_or(true, |b| score > b.0) {
                    best = Some((score, pi, open_val));
                }
            }
            let Some((score, pi, open_val)) = best else { continue };
            let params = self.assign_params(ci, &ci.phrases[pi], &qseq, &mentions, open_val);
            results.push(IntentMatch { key: ci.def.key.clone(), display_name: ci.def.display_name.clone(), confidence: score, params });
        }
        results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    fn assign_params(
        &self,
        ci: &CompiledIntent,
        phrase: &[PTok],
        qseq: &[QTok],
        mentions: &[Mention],
        open_val: Option<(String, ParamValue)>,
    ) -> Vec<(String, ParamValue)> {
        let mut out: Vec<(String, ParamValue)> = Vec::new();
        let mut used = vec![false; mentions.len()];
        let ordered: Vec<usize> = qseq.iter().filter_map(|q| q.mention).collect();
        // list params collect everything of their type
        for p in ci.def.params.iter().filter(|p| p.is_list && !system::is_open(&p.entity_type)) {
            let items: Vec<usize> = ordered.iter().copied().filter(|&m| !used[m] && mentions[m].entity_type == p.entity_type).collect();
            if !items.is_empty() {
                for &m in &items {
                    used[m] = true;
                }
                out.push((
                    p.id.clone(),
                    ParamValue {
                        resolved: Value::Array(items.iter().map(|&m| mentions[m].value.clone()).collect()),
                        original: items.iter().map(|&m| mentions[m].original.clone()).collect::<Vec<_>>().join(", "),
                    },
                ));
            }
        }
        // phrase slot order first, then any remaining params
        let mut order: Vec<&str> = phrase.iter().filter_map(|p| p.param.as_deref()).collect();
        for p in &ci.def.params {
            if !order.contains(&p.id.as_str()) {
                order.push(&p.id);
            }
        }
        for pid in order {
            if out.iter().any(|(k, _)| k == pid) {
                continue;
            }
            let Some(p) = ci.def.params.iter().find(|p| p.id == pid) else { continue };
            if system::is_open(&p.entity_type) {
                continue;
            }
            if let Some(&m) = ordered.iter().find(|&&m| !used[m] && mentions[m].entity_type == p.entity_type) {
                used[m] = true;
                out.push((p.id.clone(), ParamValue { resolved: mentions[m].value.clone(), original: mentions[m].original.clone() }));
            }
        }
        if let Some((pid, v)) = open_val {
            if !pid.is_empty() {
                let is_list = ci.def.params.iter().any(|p| p.id == pid && p.is_list);
                let v = if is_list { ParamValue { resolved: Value::Array(vec![v.resolved]), original: v.original } } else { v };
                out.push((pid, v));
            }
        }
        out
    }

    /// Extracts a value of `entity_type` from a whole utterance (used for
    /// form filling). Open types take the utterance minus filler words.
    pub fn extract(&self, text: &str, entity_type: &str, is_list: bool, overrides: &[EntityTypeDef]) -> Option<ParamValue> {
        let t = system::normalize_type_ref(entity_type);
        if system::is_open(&t) {
            let s = system::strip_fillers(text.trim());
            if s.trim().is_empty() {
                return None;
            }
            let v = system::open_value(&t, s);
            return Some(ParamValue { resolved: if is_list { Value::Array(vec![v]) } else { v }, original: s.trim().to_string() });
        }
        let tokens = tokenize(text);
        let ms = self.mentions(text, &tokens, &[t], overrides);
        if ms.is_empty() {
            return None;
        }
        if is_list {
            Some(ParamValue {
                resolved: Value::Array(ms.iter().map(|m| m.value.clone()).collect()),
                original: ms.iter().map(|m| m.original.clone()).collect::<Vec<_>>().join(", "),
            })
        } else {
            Some(ParamValue { resolved: ms[0].value.clone(), original: ms[0].original.clone() })
        }
    }

    pub fn intent_count(&self) -> usize {
        self.intents.len()
    }
}

fn dedup(v: &[Tok]) -> Vec<&Tok> {
    let mut out: Vec<&Tok> = Vec::new();
    for t in v {
        if !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// Convenience: stem a word list (used by tests and callers).
pub fn stems(text: &str) -> Vec<String> {
    tokenize(text).into_iter().map(|t| stem(&t.norm)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{Entity, Kind};

    fn phrase(parts: &[(&str, Option<&str>)]) -> Vec<PhrasePart> {
        parts.iter().map(|(t, p)| PhrasePart { text: t.to_string(), parameter_id: p.map(String::from) }).collect()
    }

    fn classifier() -> Classifier {
        let size = EntityTypeDef::new(
            "size",
            "size",
            Kind::Map,
            vec![
                Entity { value: "large".into(), synonyms: vec!["large".into(), "big".into()] },
                Entity { value: "small".into(), synonyms: vec!["small".into()] },
            ],
            vec![],
            true,
        );
        let intents = vec![
            IntentDef {
                key: "order".into(),
                display_name: "order.pizza".into(),
                phrases: vec![
                    phrase(&[("I want a ", None), ("large", Some("size")), (" pizza", None)]),
                    phrase(&[("order ", None), ("2", Some("count")), (" pizzas", None)]),
                    phrase(&[("can I get a pizza", None)]),
                ],
                params: vec![
                    ParamDef { id: "size".into(), entity_type: "size".into(), is_list: false },
                    ParamDef { id: "count".into(), entity_type: "sys.number".into(), is_list: false },
                ],
            },
            IntentDef {
                key: "hours".into(),
                display_name: "store.hours".into(),
                phrases: vec![phrase(&[("what time do you open", None)]), phrase(&[("when are you open", None)])],
                params: vec![],
            },
            IntentDef {
                key: "name".into(),
                display_name: "provide.name".into(),
                phrases: vec![phrase(&[("my name is ", None), ("John", Some("person"))])],
                params: vec![ParamDef { id: "person".into(), entity_type: "sys.person".into(), is_list: false }],
            },
        ];
        Classifier::new(intents, EntityRegistry { custom: vec![size] })
    }

    #[test]
    fn classify_with_params() {
        let c = classifier();
        let r = c.classify("i want a big pizza", &[]);
        assert_eq!(r[0].display_name, "order.pizza");
        assert!(r[0].confidence > 0.8, "{}", r[0].confidence);
        assert_eq!(r[0].params[0].0, "size");
        assert_eq!(r[0].params[0].1.resolved, Value::from("large"));

        let r = c.classify("please order three pizzas", &[]);
        assert_eq!(r[0].display_name, "order.pizza");
        assert!(r[0].params.iter().any(|(k, v)| k == "count" && v.resolved == Value::from(3)));

        let r = c.classify("when do you open?", &[]);
        assert_eq!(r[0].display_name, "store.hours");
    }

    #[test]
    fn open_slot() {
        let c = classifier();
        let r = c.classify("my name is Ada Lovelace", &[]);
        assert_eq!(r[0].display_name, "provide.name");
        assert_eq!(r[0].params[0].1.original, "Ada Lovelace");
    }

    #[test]
    fn unrelated_is_low() {
        let c = classifier();
        let r = c.classify("what's the weather in paris", &[]);
        assert!(r[0].confidence < 0.4, "{} {}", r[0].display_name, r[0].confidence);
    }

    #[test]
    fn typo_tolerance() {
        let c = classifier();
        let r = c.classify("wen are you opne", &[]);
        assert_eq!(r[0].display_name, "store.hours");
    }

    #[test]
    fn extract_for_forms() {
        let c = classifier();
        let v = c.extract("make it small", "size", false, &[]).unwrap();
        assert_eq!(v.resolved, Value::from("small"));
        let v = c.extract("I'm Grace", "@sys.person", false, &[]).unwrap();
        assert_eq!(v.resolved["name"], Value::from("Grace"));
        assert!(c.extract("no idea", "sys.number", false, &[]).is_none());
    }
}

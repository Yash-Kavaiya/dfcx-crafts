//! Document retrieval for data stores: passage chunking and Okapi BM25.
//!
//! Unstructured documents are split into passages (paragraph-aware, bounded
//! size); FAQ documents index their question (weighted) together with the
//! answer and return the answer.

use crate::text::{is_stopword, tokenize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Passage {
    /// Owning document resource name.
    pub document: String,
    pub title: String,
    pub uri: String,
    /// Text returned to the caller (passage or FAQ answer).
    pub text: String,
    /// For FAQ entries, the matched question.
    pub question: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Hit {
    pub passage: Passage,
    pub score: f64,
    /// Score normalised to 0..1 (relative to a perfect query match).
    pub confidence: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Index {
    passages: Vec<Passage>,
    terms: Vec<HashMap<String, f64>>,
    lengths: Vec<f64>,
    df: HashMap<String, usize>,
    avg_len: f64,
}

const K1: f64 = 1.2;
const B: f64 = 0.75;
const MAX_PASSAGE: usize = 700;

/// Words that carry no topical signal in questions, on top of the general
/// stopword list.
const QUERY_STOPWORDS: &[&str] = &[
    "what", "whats", "what's", "how", "when", "where", "why", "who", "which", "whom", "whose", "has", "have", "had",
    "tell", "know", "want", "need", "about", "much", "many", "there", "their", "they", "them", "if", "as", "by",
    "from", "not", "no", "yes", "but", "than", "then", "also", "very", "really", "should", "may", "might", "i'm",
    "i've", "i'd", "it's", "us", "he", "she", "his", "her",
];

fn terms(text: &str) -> Vec<String> {
    tokenize(text)
        .into_iter()
        .filter(|t| !is_stopword(&t.norm) && !QUERY_STOPWORDS.contains(&t.norm.as_str()))
        .map(|t| t.stem)
        .collect()
}

/// Splits text into passages of at most ~MAX_PASSAGE chars, preferring
/// paragraph and then sentence boundaries.
pub fn chunk(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        let t = cur.trim();
        if !t.is_empty() {
            out.push(t.to_string());
        }
        cur.clear();
    };
    for para in text.split("\n\n").flat_map(|p| p.split("\r\n\r\n")) {
        let para = para.trim();
        if para.is_empty() {
            continue;
        }
        if cur.len() + para.len() + 2 <= MAX_PASSAGE {
            if !cur.is_empty() {
                cur.push_str("\n\n");
            }
            cur.push_str(para);
            continue;
        }
        flush(&mut cur, &mut out);
        if para.len() <= MAX_PASSAGE {
            cur.push_str(para);
            continue;
        }
        // long paragraph: split by sentences
        let mut sentence = String::new();
        for ch in para.chars() {
            sentence.push(ch);
            if matches!(ch, '.' | '!' | '?') && sentence.len() > 20 {
                if cur.len() + sentence.len() > MAX_PASSAGE {
                    flush(&mut cur, &mut out);
                }
                cur.push_str(&sentence);
                sentence.clear();
            }
        }
        if cur.len() + sentence.len() > MAX_PASSAGE {
            flush(&mut cur, &mut out);
        }
        cur.push_str(&sentence);
    }
    flush(&mut cur, &mut out);
    out
}

impl Index {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.passages.len()
    }
    pub fn is_empty(&self) -> bool {
        self.passages.is_empty()
    }

    fn push(&mut self, p: Passage, indexed_text: &str) {
        let mut tf: HashMap<String, f64> = HashMap::new();
        let ts = terms(indexed_text);
        for t in &ts {
            *tf.entry(t.clone()).or_insert(0.0) += 1.0;
        }
        for t in tf.keys() {
            *self.df.entry(t.clone()).or_insert(0) += 1;
        }
        self.lengths.push(ts.len() as f64);
        self.terms.push(tf);
        self.passages.push(p);
        self.avg_len = self.lengths.iter().sum::<f64>() / self.lengths.len() as f64;
    }

    /// Adds an unstructured document, chunked into passages.
    pub fn add_document(&mut self, document: &str, title: &str, uri: &str, content: &str) {
        for c in chunk(content) {
            let indexed = format!("{} {}", title, c);
            self.push(Passage { document: document.into(), title: title.into(), uri: uri.into(), text: c, question: None }, &indexed);
        }
    }

    /// Adds an FAQ entry; the question is weighted higher than the answer.
    pub fn add_faq(&mut self, document: &str, question: &str, answer: &str, uri: &str) {
        let indexed = format!("{q} {q} {q} {a}", q = question, a = answer);
        self.push(
            Passage { document: document.into(), title: question.into(), uri: uri.into(), text: answer.into(), question: Some(question.into()) },
            &indexed,
        );
    }

    fn idf(&self, t: &str) -> f64 {
        let n = self.passages.len() as f64;
        let df = *self.df.get(t).unwrap_or(&0) as f64;
        ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
    }

    fn bm25(&self, i: usize, q: &[String]) -> f64 {
        let len = self.lengths[i];
        let norm = K1 * (1.0 - B + B * len / self.avg_len.max(1.0));
        q.iter()
            .map(|t| {
                let f = *self.terms[i].get(t).unwrap_or(&0.0);
                if f == 0.0 {
                    0.0
                } else {
                    self.idf(t) * f * (K1 + 1.0) / (f + norm)
                }
            })
            .sum()
    }

    /// Top-`k` passages for `query` (only those sharing at least one term).
    pub fn search(&self, query: &str, k: usize) -> Vec<Hit> {
        let mut q = terms(query);
        q.sort();
        q.dedup();
        if q.is_empty() || self.passages.is_empty() {
            return Vec::new();
        }
        // Terms that never occur in the corpus cannot discriminate between
        // passages, so confidence is computed over the known terms and only
        // mildly discounted by the share of unknown ones.
        let known: Vec<&String> = q.iter().filter(|t| self.df.contains_key(*t)).collect();
        if known.is_empty() {
            return Vec::new();
        }
        let known_share = known.len() as f64 / q.len() as f64;
        let ideal: f64 = known.iter().map(|t| self.idf(t).max(0.0)).sum::<f64>().max(1e-9);
        let idf_coverage = |i: usize| known.iter().filter(|t| self.terms[i].contains_key(**t)).map(|t| self.idf(t).max(0.0)).sum::<f64>() / ideal;
        let mut hits: Vec<Hit> = (0..self.passages.len())
            .filter_map(|i| {
                let s = self.bm25(i, &q);
                (s > 0.0).then(|| {
                    let c = ((s / ideal).min(1.0) * 0.4 + idf_coverage(i) * 0.6) * (0.6 + 0.4 * known_share);
                    Hit { passage: self.passages[i].clone(), score: s, confidence: c.min(1.0) }
                })
            })
            .collect();
        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(k);
        hits
    }
}

/// Parses a two-column FAQ CSV (`question,answer`, optional header row,
/// RFC 4180 quoting).
pub fn parse_faq_csv(text: &str) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            ',' => row.push(std::mem::take(&mut field)),
            '\n' | '\r' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    let mut out: Vec<(String, String)> = rows
        .into_iter()
        .filter(|r| r.len() >= 2 && !r[0].trim().is_empty() && !r[1].trim().is_empty())
        .map(|r| (r[0].trim().to_string(), r[1].trim().to_string()))
        .collect();
    if out.first().map_or(false, |(q, a)| q.eq_ignore_ascii_case("question") && a.eq_ignore_ascii_case("answer")) {
        out.remove(0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bm25_ranks_relevant_passage_first() {
        let mut ix = Index::new();
        ix.add_document("d1", "Returns", "http://x/returns", "You can return any item within 30 days of delivery for a full refund.\n\nRefunds are issued to the original payment method.");
        ix.add_document("d2", "Shipping", "http://x/ship", "Standard shipping takes 3-5 business days. Express shipping arrives next day.");
        ix.add_faq("d3", "Do you ship internationally?", "Yes, we ship to over 40 countries.", "");
        let h = ix.search("how many days do I have to return an item?", 3);
        assert_eq!(h[0].passage.document, "d1");
        assert!(h[0].confidence > 0.3);
        let h = ix.search("international shipping", 3);
        assert_eq!(h[0].passage.text, "Yes, we ship to over 40 countries.");
        assert!(ix.search("quantum chromodynamics", 3).is_empty());
        assert!(ix.search("what is it?", 3).is_empty(), "question words alone match nothing");
        // words unknown to the corpus do not sink an otherwise good match
        let h = ix.search("how long do refunds take", 3);
        assert_eq!(h[0].passage.document, "d1");
        assert!(h[0].confidence >= 0.35, "{}", h[0].confidence);
    }

    #[test]
    fn chunking_and_csv() {
        let long = "Sentence number one is here. ".repeat(60);
        let cs = chunk(&long);
        assert!(cs.len() > 1 && cs.iter().all(|c| c.len() <= MAX_PASSAGE + 40));
        let faq = parse_faq_csv("question,answer\n\"Hours?\",\"9-5, \"\"Mon-Fri\"\"\"\nParking?,Free\n");
        assert_eq!(faq, vec![("Hours?".into(), "9-5, \"Mon-Fri\"".into()), ("Parking?".into(), "Free".into())]);
    }
}

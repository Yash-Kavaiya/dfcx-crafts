//! Tokenisation, normalisation, stemming and string distance.

/// A token with its byte span in the original text.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub text: String,
    pub norm: String,
    pub stem: String,
    pub start: usize,
    pub end: usize,
}

/// Splits text into word tokens (alphanumerics, with inner apostrophes).
pub fn tokenize(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (b, c) = chars[i];
        if c.is_alphanumeric() {
            let start = b;
            let mut j = i + 1;
            while j < chars.len() {
                let cj = chars[j].1;
                if cj.is_alphanumeric() {
                    j += 1;
                } else if (cj == '\'' || cj == '’') && j + 1 < chars.len() && chars[j + 1].1.is_alphanumeric() {
                    j += 2;
                } else {
                    break;
                }
            }
            let end = if j < chars.len() { chars[j].0 } else { text.len() };
            let raw = &text[start..end];
            let norm = normalize_word(raw);
            out.push(Token { text: raw.to_string(), stem: stem(&norm), norm, start, end });
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

pub fn normalize_word(w: &str) -> String {
    w.to_lowercase().replace('’', "'")
}

/// Lowercases and collapses whitespace/punctuation.
pub fn normalize(text: &str) -> String {
    tokenize(text).into_iter().map(|t| t.norm).collect::<Vec<_>>().join(" ")
}

/// A deliberately small English stemmer, applied symmetrically to training
/// data and queries.
pub fn stem(w: &str) -> String {
    let mut s = w.to_string();
    if let Some(stripped) = s.strip_suffix("'s") {
        s = stripped.to_string();
    }
    let n = s.chars().count();
    if s.chars().any(|c| c.is_ascii_digit()) || n <= 3 {
        return s;
    }
    if n > 5 && s.ends_with("ing") {
        s.truncate(s.len() - 3);
    } else if s.ends_with("ies") && n > 4 {
        s.truncate(s.len() - 3);
        s.push('y');
    } else if (s.ends_with("ses") || s.ends_with("xes") || s.ends_with("ches") || s.ends_with("shes")) && n > 4 {
        s.truncate(s.len() - 2);
    } else if s.ends_with("ed") && n > 4 {
        s.truncate(s.len() - 2);
    } else if s.ends_with('s') && !s.ends_with("ss") && !s.ends_with("us") && !s.ends_with("is") {
        s.truncate(s.len() - 1);
    }
    s
}

pub const STOPWORDS: &[&str] = &[
    "a", "an", "the", "i", "me", "my", "we", "our", "you", "your", "it", "its", "is", "am", "are", "was", "were", "be",
    "to", "of", "in", "on", "at", "for", "with", "and", "or", "so", "that", "this", "please", "can", "could", "would",
    "will", "do", "does", "did", "just", "some", "any", "there", "here", "um", "uh", "like", "okay", "ok", "hey",
];

pub fn is_stopword(w: &str) -> bool {
    STOPWORDS.contains(&w)
}

/// Levenshtein distance over chars.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    levenshtein_seq(&a, &b, |x, y| x == y)
}

/// Generic edit distance over sequences.
pub fn levenshtein_seq<T>(a: &[T], b: &[T], eq: impl Fn(&T, &T) -> bool) -> usize {
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = if eq(&a[i - 1], &b[j - 1]) { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Whether two normalised words are "close enough" for fuzzy matching.
pub fn fuzzy_eq(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let la = a.chars().count();
    let lb = b.chars().count();
    let min = la.min(lb);
    if min < 4 || a.chars().any(|c| c.is_ascii_digit()) || b.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    let allowed = if min >= 8 { 2 } else { 1 };
    if la.abs_diff(lb) > allowed {
        return false;
    }
    levenshtein(a, b) <= allowed
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tokens() {
        let t = tokenize("I'd like 2 large pizzas, please!");
        let n: Vec<&str> = t.iter().map(|t| t.norm.as_str()).collect();
        assert_eq!(n, vec!["i'd", "like", "2", "large", "pizzas", "please"]);
        assert_eq!(t[4].stem, "pizza");
        assert_eq!(&"I'd like 2 large pizzas, please!"[t[3].start..t[3].end], "large");
    }
    #[test]
    fn stems() {
        assert_eq!(stem("ordering"), "order");
        assert_eq!(stem("flies"), "fly");
        assert_eq!(stem("boxes"), "box");
        assert_eq!(stem("booked"), "book");
        assert_eq!(stem("glass"), "glass");
    }
    #[test]
    fn fuzzy() {
        assert!(fuzzy_eq("pepperoni", "peperoni"));
        assert!(fuzzy_eq("large", "lagre") == false); // transposition = 2 edits for short word
        assert!(fuzzy_eq("medium", "meduim") == false);
        assert!(fuzzy_eq("medium", "medim"));
        assert!(!fuzzy_eq("cat", "car"));
    }
}

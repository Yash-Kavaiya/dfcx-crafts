//! System entities (`@sys.*`).

use crate::entities::Mention;
use crate::text::Token;
use cx_core::time::Date;
use cx_core::{obj, Value};
use cx_regex::Regex;
use std::sync::OnceLock;

/// Catalog of supported system entities: (name, description).
pub const SYSTEM_ENTITIES: &[(&str, &str)] = &[
    ("sys.any", "Any non-empty input"),
    ("sys.number", "Numbers, digits or words (\"42\", \"twenty one\")"),
    ("sys.number-integer", "Whole numbers"),
    ("sys.cardinal", "Cardinal numbers"),
    ("sys.ordinal", "Ordinal numbers (\"first\", \"3rd\")"),
    ("sys.number-sequence", "A sequence of digits, kept as text"),
    ("sys.date", "Dates (\"today\", \"next friday\", \"March 3\", \"2025-01-31\")"),
    ("sys.time", "Times (\"3pm\", \"15:30\", \"noon\")"),
    ("sys.email", "Email addresses"),
    ("sys.url", "URLs"),
    ("sys.phone-number", "Phone numbers"),
    ("sys.zip-code", "US ZIP codes"),
    ("sys.percentage", "Percentages (\"25%\", \"25 percent\")"),
    ("sys.unit-currency", "Money amounts (\"$20\", \"20 euros\")"),
    ("sys.color", "Basic colors"),
    ("sys.person", "A person's name (open)"),
    ("sys.given-name", "A first name (open)"),
    ("sys.last-name", "A last name (open)"),
    ("sys.geo-city", "City (open)"),
    ("sys.geo-country", "Country (open)"),
    ("sys.address", "Street address (open)"),
    ("sys.location", "Location (open)"),
];

/// Open system entities cannot be detected in isolation; they are filled
/// from the whole utterance (form filling) or by template alignment
/// (intent training phrases).
pub const OPEN_TYPES: &[&str] = &[
    "sys.any",
    "sys.person",
    "sys.given-name",
    "sys.last-name",
    "sys.geo-city",
    "sys.geo-country",
    "sys.address",
    "sys.location",
];

pub fn is_system(t: &str) -> bool {
    t.starts_with("sys.")
}

pub fn is_open(t: &str) -> bool {
    OPEN_TYPES.contains(&t)
}

/// Normalises an entity-type reference to a registry key: system entities
/// become `sys.xxx` regardless of how they are written
/// (`@sys.number`, `projects/-/locations/-/agents/-/entityTypes/sys.number`).
pub fn normalize_type_ref(r: &str) -> String {
    let id = cx_core::name::id_of(r);
    let id = id.trim_start_matches('@');
    if id.starts_with("sys.") {
        id.to_string()
    } else {
        r.trim_start_matches('@').to_string()
    }
}

/// Value for an open type given the raw utterance fragment.
pub fn open_value(t: &str, text: &str) -> Value {
    let text = text.trim().trim_end_matches(['.', '!', '?', ',']).trim();
    match t {
        "sys.person" => obj! {"name" => text},
        _ => Value::from(text),
    }
}

/// Filler prefixes stripped when an open entity takes a whole utterance.
pub fn strip_fillers(text: &str) -> &str {
    const FILLERS: &[&str] = &[
        "my name is ", "my name's ", "i am ", "i'm ", "it is ", "it's ", "call me ", "this is ", "i live in ", "i'm in ",
        "the answer is ", "it was ", "that is ", "that's ",
    ];
    let lower = text.to_lowercase();
    for f in FILLERS {
        if lower.starts_with(f) && text.is_char_boundary(f.len()) {
            return &text[f.len()..];
        }
    }
    text
}

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new_case_insensitive(pat).expect("valid builtin regex"))
}

static RE_NUMBER: OnceLock<Regex> = OnceLock::new();
static RE_EMAIL: OnceLock<Regex> = OnceLock::new();
static RE_URL: OnceLock<Regex> = OnceLock::new();
static RE_PHONE: OnceLock<Regex> = OnceLock::new();
static RE_ZIP: OnceLock<Regex> = OnceLock::new();
static RE_PERCENT: OnceLock<Regex> = OnceLock::new();
static RE_CURRENCY_PREFIX: OnceLock<Regex> = OnceLock::new();
static RE_CURRENCY_SUFFIX: OnceLock<Regex> = OnceLock::new();
static RE_ISO_DATE: OnceLock<Regex> = OnceLock::new();
static RE_US_DATE: OnceLock<Regex> = OnceLock::new();
static RE_TIME_AMPM: OnceLock<Regex> = OnceLock::new();
static RE_TIME_24: OnceLock<Regex> = OnceLock::new();
static RE_ORDINAL_NUM: OnceLock<Regex> = OnceLock::new();
static RE_DIGITS: OnceLock<Regex> = OnceLock::new();

const UNITS: &[(&str, f64)] = &[
    ("zero", 0.0), ("oh", 0.0), ("one", 1.0), ("two", 2.0), ("three", 3.0), ("four", 4.0), ("five", 5.0), ("six", 6.0),
    ("seven", 7.0), ("eight", 8.0), ("nine", 9.0), ("ten", 10.0), ("eleven", 11.0), ("twelve", 12.0),
    ("thirteen", 13.0), ("fourteen", 14.0), ("fifteen", 15.0), ("sixteen", 16.0), ("seventeen", 17.0),
    ("eighteen", 18.0), ("nineteen", 19.0), ("twenty", 20.0), ("thirty", 30.0), ("forty", 40.0), ("fifty", 50.0),
    ("sixty", 60.0), ("seventy", 70.0), ("eighty", 80.0), ("ninety", 90.0), ("dozen", 12.0), ("couple", 2.0),
];
const ORDINALS: &[(&str, f64)] = &[
    ("first", 1.0), ("second", 2.0), ("third", 3.0), ("fourth", 4.0), ("fifth", 5.0), ("sixth", 6.0),
    ("seventh", 7.0), ("eighth", 8.0), ("ninth", 9.0), ("tenth", 10.0), ("eleventh", 11.0), ("twelfth", 12.0),
    ("last", -1.0),
];
const MONTHS: &[&str] = &[
    "january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november",
    "december",
];
const WEEKDAYS: &[&str] = &["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];
const COLORS: &[&str] = &[
    "red", "green", "blue", "yellow", "orange", "purple", "pink", "black", "white", "gray", "grey", "brown", "silver",
    "gold", "violet", "cyan", "magenta", "beige", "navy", "maroon", "teal",
];

fn word_number(w: &str) -> Option<f64> {
    UNITS.iter().find(|(n, _)| *n == w).map(|(_, v)| *v)
}

fn parse_numeric(s: &str) -> Option<f64> {
    s.replace(',', "").parse::<f64>().ok()
}

/// Numbers written as words, e.g. "two hundred and five".
fn word_numbers(text: &str, tokens: &[Token]) -> Vec<(usize, usize, f64)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let mut j = i;
        let mut total = 0.0;
        let mut current = 0.0;
        let mut seen = false;
        let mut last_end = i;
        while j < tokens.len() {
            let w = tokens[j].norm.as_str();
            if let Some(v) = word_number(w) {
                if w == "couple" && !(j + 1 < tokens.len() && tokens[j + 1].norm == "of") {
                    // "couple" only counts in "a couple of"
                    if !seen {
                        break;
                    }
                }
                current += v;
                seen = true;
                j += 1;
                last_end = j;
                if w == "couple" {
                    break; // "of" is not part of the span
                }
            } else if w == "hundred" && (seen || current == 0.0) {
                current = if current == 0.0 { 100.0 } else { current * 100.0 };
                seen = true;
                j += 1;
                last_end = j;
            } else if (w == "thousand" || w == "million" || w == "billion") && (seen || current == 0.0) {
                let mult = match w {
                    "thousand" => 1e3,
                    "million" => 1e6,
                    _ => 1e9,
                };
                total += if current == 0.0 { mult } else { current * mult };
                current = 0.0;
                seen = true;
                j += 1;
                last_end = j;
            } else if w == "and" && seen && j + 1 < tokens.len() && word_number(&tokens[j + 1].norm).is_some() {
                j += 1;
            } else if w == "a" && !seen && j + 1 < tokens.len() && matches!(tokens[j + 1].norm.as_str(), "hundred" | "thousand" | "million" | "dozen") {
                j += 1;
                current = 0.0;
            } else {
                break;
            }
        }
        if seen {
            let start = tokens[i].start;
            let end = tokens[last_end - 1].end;
            let _ = text;
            out.push((start, end, total + current));
            i = last_end.max(i + 1);
        } else {
            i += 1;
        }
    }
    out
}

fn numbers(text: &str, tokens: &[Token]) -> Vec<(usize, usize, f64)> {
    let mut out: Vec<(usize, usize, f64)> = Vec::new();
    for m in re(&RE_NUMBER, r"-?\d+(?:,\d{3})*(?:\.\d+)?").find_iter(text) {
        // skip digits glued to letters ("3rd", "abc123") except plain numbers
        let before = text[..m.start].chars().last();
        let after = text[m.end..].chars().next();
        if before.map_or(false, |c| c.is_alphabetic()) || after.map_or(false, |c| c.is_alphabetic()) {
            continue;
        }
        if let Some(v) = parse_numeric(&text[m.start..m.end]) {
            out.push((m.start, m.end, v));
        }
    }
    out.extend(word_numbers(text, tokens));
    out
}

fn mk(t: &str, text: &str, s: usize, e: usize, v: Value) -> Mention {
    Mention::new(t, s, e, &text[s..e], v, 0)
}

fn date_value(d: Date) -> Value {
    obj! {"year" => d.year, "month" => d.month, "day" => d.day}
}

fn month_index(w: &str) -> Option<u32> {
    if w.len() < 3 {
        return None;
    }
    MONTHS.iter().position(|m| m.starts_with(w)).map(|i| i as u32 + 1)
}

fn day_number(w: &str) -> Option<u32> {
    let digits: String = w.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() {
        let rest = &w[digits.len()..];
        if rest.is_empty() || matches!(rest, "st" | "nd" | "rd" | "th") {
            return digits.parse().ok().filter(|d| (1..=31).contains(d));
        }
        return None;
    }
    ORDINALS.iter().find(|(n, _)| *n == w).map(|(_, v)| *v as u32).filter(|d| (1..=31).contains(d))
}

/// Date mentions relative to `today`.
pub fn dates(text: &str, tokens: &[Token], today: Date) -> Vec<Mention> {
    let t = "sys.date";
    let mut out = Vec::new();
    for m in re(&RE_ISO_DATE, r"\b\d{4}-\d{1,2}-\d{1,2}\b").find_iter(text) {
        let parts: Vec<i64> = text[m.start..m.end].split('-').filter_map(|p| p.parse().ok()).collect();
        let d = Date { year: parts[0], month: parts[1] as u32, day: parts[2] as u32 };
        if d.is_valid() {
            out.push(mk(t, text, m.start, m.end, date_value(d)));
        }
    }
    for m in re(&RE_US_DATE, r"\b\d{1,2}/\d{1,2}(?:/\d{2,4})?\b").find_iter(text) {
        let parts: Vec<i64> = text[m.start..m.end].split('/').filter_map(|p| p.parse().ok()).collect();
        let mut year = if parts.len() > 2 { parts[2] } else { today.year };
        if year < 100 {
            year += 2000;
        }
        let d = Date { year, month: parts[0] as u32, day: parts[1] as u32 };
        if d.is_valid() {
            out.push(mk(t, text, m.start, m.end, date_value(d)));
        }
    }
    let n = tokens.len();
    let mut i = 0;
    while i < n {
        let w = tokens[i].norm.as_str();
        let next = |k: usize| tokens.get(i + k).map(|t| t.norm.as_str()).unwrap_or("");
        let mut found: Option<(usize, Date)> = None; // (token count, date)
        match w {
            "today" | "tonight" => found = Some((1, today)),
            "tomorrow" => found = Some((1, today.add_days(1))),
            "yesterday" => found = Some((1, today.add_days(-1))),
            "day" if next(1) == "after" && next(2) == "tomorrow" => found = Some((3, today.add_days(2))),
            "day" if next(1) == "before" && next(2) == "yesterday" => found = Some((3, today.add_days(-2))),
            "next" if next(1) == "week" => found = Some((2, today.add_days(7))),
            "next" | "this" | "coming" | "on" if WEEKDAYS.contains(&next(1)) => {
                let wd = WEEKDAYS.iter().position(|d| *d == next(1)).unwrap() as i64;
                let mut delta = (wd - today.weekday() as i64).rem_euclid(7);
                if delta == 0 || w == "next" && delta < 1 {
                    delta = 7;
                }
                found = Some((2, today.add_days(delta)));
            }
            _ if WEEKDAYS.contains(&w) => {
                let wd = WEEKDAYS.iter().position(|d| *d == w).unwrap() as i64;
                let mut delta = (wd - today.weekday() as i64).rem_euclid(7);
                if delta == 0 {
                    delta = 7;
                }
                found = Some((1, today.add_days(delta)));
            }
            _ => {}
        }
        if found.is_none() {
            // "<month> <day>[,] [year]"
            if let Some(mo) = month_index(w).filter(|_| w.len() >= 3) {
                if let Some(day) = day_number(next(1)) {
                    let (year, used) = match next(2).parse::<i64>() {
                        Ok(y) if (1900..=2200).contains(&y) => (y, 3),
                        _ => (today.year, 2),
                    };
                    let d = Date { year, month: mo, day };
                    if d.is_valid() {
                        found = Some((used, d));
                    }
                }
            }
            // "<day> [of] <month> [year]"
            if found.is_none() {
                if let Some(day) = day_number(w) {
                    let (mo_tok, skip) = if next(1) == "of" { (next(2), 2) } else { (next(1), 1) };
                    if let Some(mo) = month_index(mo_tok).filter(|_| mo_tok.len() >= 3) {
                        let (year, extra) = match tokens.get(i + skip + 1).and_then(|t| t.norm.parse::<i64>().ok()) {
                            Some(y) if (1900..=2200).contains(&y) => (y, 1),
                            _ => (today.year, 0),
                        };
                        let d = Date { year, month: mo, day };
                        if d.is_valid() {
                            found = Some((skip + 1 + extra, d));
                        }
                    }
                }
            }
        }
        if let Some((cnt, d)) = found {
            let s = tokens[i].start;
            let e = tokens[i + cnt - 1].end;
            out.push(mk(t, text, s, e, date_value(d)));
            i += cnt;
        } else {
            i += 1;
        }
    }
    out
}

fn time_value(h: i64, m: i64) -> Value {
    obj! {"hours" => h, "minutes" => m, "seconds" => 0, "nanos" => 0}
}

pub fn times(text: &str, tokens: &[Token]) -> Vec<Mention> {
    let t = "sys.time";
    let mut out = Vec::new();
    let r = re(&RE_TIME_AMPM, r"\b(\d{1,2})(?::(\d{2}))?\s*(a\.?m\.?|p\.?m\.?)(?:\W|$)");
    let mut start = 0;
    while let Some(c) = r.captures_at(text, start) {
        let whole = c[0].unwrap();
        let h: i64 = c[1].map(|g| text[g.start..g.end].parse().unwrap_or(0)).unwrap_or(0);
        let mi: i64 = c[2].map(|g| text[g.start..g.end].parse().unwrap_or(0)).unwrap_or(0);
        let ampm = c[3].map(|g| text[g.start..g.end].to_lowercase()).unwrap_or_default();
        let end = c[3].unwrap().end;
        if (1..=12).contains(&h) && mi < 60 {
            let hh = match (ampm.starts_with('p'), h) {
                (true, 12) => 12,
                (true, h) => h + 12,
                (false, 12) => 0,
                (false, h) => h,
            };
            out.push(mk(t, text, whole.start, end, time_value(hh, mi)));
        }
        start = whole.end.max(start + 1);
        if start > text.len() {
            break;
        }
    }
    for m in re(&RE_TIME_24, r"\b([01]?\d|2[0-3]):([0-5]\d)\b").find_iter(text) {
        if out.iter().any(|o: &Mention| o.start <= m.start && m.end <= o.end) {
            continue;
        }
        let s = &text[m.start..m.end];
        let mut p = s.split(':');
        let h: i64 = p.next().unwrap().parse().unwrap_or(0);
        let mi: i64 = p.next().unwrap().parse().unwrap_or(0);
        out.push(mk(t, text, m.start, m.end, time_value(h, mi)));
    }
    for tok in tokens {
        match tok.norm.as_str() {
            "noon" | "midday" => out.push(mk(t, text, tok.start, tok.end, time_value(12, 0))),
            "midnight" => out.push(mk(t, text, tok.start, tok.end, time_value(0, 0))),
            _ => {}
        }
    }
    out
}

/// Finds mentions of system entity `t`.
pub fn find(t: &str, text: &str, tokens: &[Token]) -> Vec<Mention> {
    find_with_date(t, text, tokens, Date::today())
}

pub fn find_with_date(t: &str, text: &str, tokens: &[Token], today: Date) -> Vec<Mention> {
    match t {
        "sys.number" | "sys.cardinal" | "sys.number-integer" => numbers(text, tokens)
            .into_iter()
            .filter(|(_, _, v)| t != "sys.number-integer" || v.fract() == 0.0)
            .map(|(s, e, v)| mk(t, text, s, e, Value::from(v)))
            .collect(),
        "sys.ordinal" => {
            let mut out: Vec<Mention> = tokens
                .iter()
                .filter_map(|tok| ORDINALS.iter().find(|(n, _)| *n == tok.norm).map(|(_, v)| mk(t, text, tok.start, tok.end, Value::from(*v))))
                .collect();
            for m in re(&RE_ORDINAL_NUM, r"\b(\d+)(?:st|nd|rd|th)\b").find_iter(text) {
                let digits: String = text[m.start..m.end].chars().take_while(|c| c.is_ascii_digit()).collect();
                out.push(mk(t, text, m.start, m.end, Value::from(digits.parse::<f64>().unwrap_or(0.0))));
            }
            out
        }
        "sys.number-sequence" => re(&RE_DIGITS, r"\d+")
            .find_iter(text)
            .into_iter()
            .map(|m| mk(t, text, m.start, m.end, Value::from(&text[m.start..m.end])))
            .collect(),
        "sys.email" => regex_mentions(t, text, re(&RE_EMAIL, r"[\w.+-]+@[\w-]+(?:\.[\w-]+)+")),
        "sys.url" => regex_mentions(t, text, re(&RE_URL, r"(?:https?://|www\.)[^\s]+[^\s.,!?]")),
        "sys.phone-number" => regex_mentions(t, text, re(&RE_PHONE, r"\+?\(?\d[\d\s().-]{5,}\d")),
        "sys.zip-code" => regex_mentions(t, text, re(&RE_ZIP, r"\b\d{5}(?:-\d{4})?\b")),
        "sys.percentage" => re(&RE_PERCENT, r"(\d+(?:\.\d+)?)\s*(?:%|percent\b)")
            .find_iter(text)
            .into_iter()
            .map(|m| {
                let digits: String = text[m.start..m.end].chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                mk(t, text, m.start, m.end, Value::from(format!("{}%", digits)))
            })
            .collect(),
        "sys.unit-currency" => {
            let mut out = Vec::new();
            for m in re(&RE_CURRENCY_PREFIX, r"([$€£₹])\s?(\d+(?:,\d{3})*(?:\.\d+)?)").find_iter(text) {
                let s = &text[m.start..m.end];
                let sym = s.chars().next().unwrap();
                let amount = parse_numeric(s[sym.len_utf8()..].trim()).unwrap_or(0.0);
                out.push(mk(t, text, m.start, m.end, obj! {"amount" => amount, "currency" => currency_code(&sym.to_string())}));
            }
            let r = re(&RE_CURRENCY_SUFFIX, r"(\d+(?:,\d{3})*(?:\.\d+)?)\s*(dollars?|usd|bucks|euros?|eur|pounds?|gbp|rupees?|inr|yen|jpy)\b");
            let mut start = 0;
            while let Some(c) = r.captures_at(text, start) {
                let w = c[0].unwrap();
                let amount = c[1].map(|g| parse_numeric(&text[g.start..g.end]).unwrap_or(0.0)).unwrap_or(0.0);
                let unit = c[2].map(|g| text[g.start..g.end].to_lowercase()).unwrap_or_default();
                out.push(mk(t, text, w.start, w.end, obj! {"amount" => amount, "currency" => currency_code(&unit)}));
                start = w.end;
            }
            out
        }
        "sys.date" => dates(text, tokens, today),
        "sys.time" => times(text, tokens),
        "sys.color" => tokens
            .iter()
            .filter(|tok| COLORS.contains(&tok.norm.as_str()))
            .map(|tok| mk(t, text, tok.start, tok.end, Value::from(tok.norm.clone())))
            .collect(),
        _ => Vec::new(),
    }
}

fn currency_code(u: &str) -> &'static str {
    match u {
        "$" | "dollar" | "dollars" | "usd" | "bucks" => "USD",
        "€" | "euro" | "euros" | "eur" => "EUR",
        "£" | "pound" | "pounds" | "gbp" => "GBP",
        "₹" | "rupee" | "rupees" | "inr" => "INR",
        "yen" | "jpy" => "JPY",
        _ => "USD",
    }
}

fn regex_mentions(t: &str, text: &str, r: &Regex) -> Vec<Mention> {
    r.find_iter(text).into_iter().map(|m| mk(t, text, m.start, m.end, Value::from(text[m.start..m.end].trim()))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::tokenize;

    fn f(t: &str, text: &str) -> Vec<Value> {
        find_with_date(t, text, &tokenize(text), Date { year: 2024, month: 1, day: 1 }).into_iter().map(|m| m.value).collect()
    }

    #[test]
    fn numbers_work() {
        assert_eq!(f("sys.number", "I need 3 pizzas"), vec![Value::from(3)]);
        assert_eq!(f("sys.number", "twenty one tickets"), vec![Value::from(21)]);
        assert_eq!(f("sys.number", "two hundred and five"), vec![Value::from(205)]);
        assert_eq!(f("sys.number", "1,250.5 dollars"), vec![Value::from(1250.5)]);
        assert_eq!(f("sys.number", "a dozen eggs"), vec![Value::from(12)]);
        assert!(f("sys.number", "the 3rd please").is_empty());
        assert_eq!(f("sys.ordinal", "the 3rd one"), vec![Value::from(3)]);
    }

    #[test]
    fn dates_work() {
        // 2024-01-01 is a Monday
        assert_eq!(f("sys.date", "tomorrow please"), vec![obj! {"year"=>2024,"month"=>1,"day"=>2}]);
        assert_eq!(f("sys.date", "next friday"), vec![obj! {"year"=>2024,"month"=>1,"day"=>5}]);
        assert_eq!(f("sys.date", "on March 3rd 2025"), vec![obj! {"year"=>2025,"month"=>3,"day"=>3}]);
        assert_eq!(f("sys.date", "the 14th of feb"), vec![obj! {"year"=>2024,"month"=>2,"day"=>14}]);
        assert_eq!(f("sys.date", "2024-12-25"), vec![obj! {"year"=>2024,"month"=>12,"day"=>25}]);
    }

    #[test]
    fn misc() {
        assert_eq!(f("sys.time", "at 3:30 pm"), vec![obj! {"hours"=>15,"minutes"=>30,"seconds"=>0,"nanos"=>0}]);
        assert_eq!(f("sys.time", "at 18:05"), vec![obj! {"hours"=>18,"minutes"=>5,"seconds"=>0,"nanos"=>0}]);
        assert_eq!(f("sys.email", "it's bob@x.io."), vec![Value::from("bob@x.io")]);
        assert_eq!(f("sys.unit-currency", "costs $20"), vec![obj! {"amount"=>20,"currency"=>"USD"}]);
        assert_eq!(f("sys.unit-currency", "costs 20 euros"), vec![obj! {"amount"=>20,"currency"=>"EUR"}]);
        assert_eq!(f("sys.percentage", "about 15 percent"), vec![Value::from("15%")]);
        assert_eq!(f("sys.phone-number", "call +1 (555) 123-4567"), vec![Value::from("+1 (555) 123-4567")]);
        assert_eq!(normalize_type_ref("projects/-/locations/-/agents/-/entityTypes/sys.date"), "sys.date");
        assert_eq!(strip_fillers("My name is Ada"), "Ada");
    }
}

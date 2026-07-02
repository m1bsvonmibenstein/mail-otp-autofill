//! Verification-code detection. Keyword-anchored so it won't grab order numbers,
//! dates, or phone numbers. Kept in sync with the extension's otp-detect.js.

use regex::Regex;
use std::sync::OnceLock;

fn keywords() -> &'static Regex {
    static KW: OnceLock<Regex> = OnceLock::new();
    KW.get_or_init(|| {
        Regex::new(r"(?i)(one[\s-]?time|verification|verify|security|passcode|pass\s?code|access\s?code|confirmation|auth(?:entication)?|login|sign[\s-]?in|OTP|2FA|MFA|\bcode\b)").unwrap()
    })
}

fn token() -> &'static Regex {
    static TOK: OnceLock<Regex> = OnceLock::new();
    // Case-sensitive on purpose: alnum codes are uppercase; avoids matching words.
    TOK.get_or_init(|| Regex::new(r"\b([0-9]{3}[\s-]?[0-9]{3}|[0-9]{4,8}|[A-Z0-9]{6,8})\b").unwrap())
}

fn normalize(s: &str) -> String {
    s.chars().filter(|c| *c != ' ' && *c != '-' && *c != '\t').collect()
}

/// Byte-safe substring that snaps to char boundaries.
fn safe_slice(s: &str, start: usize, len: usize) -> &str {
    let mut a = start.min(s.len());
    while a < s.len() && !s.is_char_boundary(a) { a += 1; }
    let mut b = (a + len).min(s.len());
    while b < s.len() && !s.is_char_boundary(b) { b += 1; }
    &s[a..b]
}

pub fn find_code(text: &str) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    let kw = keywords();
    let tok = token();

    // Line-oriented: a keyword line, code on it or within the next two lines.
    let lines: Vec<&str> = text.split('\n').collect();
    for (i, line) in lines.iter().enumerate() {
        if !kw.is_match(line) {
            continue;
        }
        if let Some(c) = tok.captures(line) {
            return Some(normalize(&c[1]));
        }
        for j in 1..=2 {
            if let Some(l) = lines.get(i + j) {
                if let Some(c) = tok.captures(l) {
                    return Some(normalize(&c[1]));
                }
            }
        }
    }

    // Fallback: a token within ~60 chars after any keyword occurrence.
    for m in kw.find_iter(text) {
        let window = safe_slice(text, m.end(), 60);
        if let Some(c) = tok.captures(window) {
            return Some(normalize(&c[1]));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_common_codes() {
        assert_eq!(find_code("Your verification code is 624835").as_deref(), Some("624835"));
        assert_eq!(find_code("Enter this code:\n\n123 456").as_deref(), Some("123456"));
        assert_eq!(find_code("Your one-time passcode: 4821").as_deref(), Some("4821"));
    }

    #[test]
    fn ignores_unrelated_numbers() {
        assert_eq!(find_code("Order #10029 shipped on 2026-01-15"), None);
        assert_eq!(find_code("Call us at 5551234"), None);
    }
}

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

// --- magic sign-in links ----------------------------------------------------
// Ported from the extension's otp-detect.js: an https URL sitting near a
// login/verify keyword (proximity keeps footer/unsubscribe links out), plus an
// HTML-anchor variant for mail whose link only survives in an <a href>.

pub struct Link {
    pub url: String,
    pub host: String,
}

fn link_keywords() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)(magic\s?link|sign[\s-]?in|log[\s-]?in|verify|confirm(?:ation)?|activate|authenticat|one[\s-]?time|access\s+your|complete\s+your|finish\s+(?:signing|sign)|continue\s+to\s+(?:sign|log))").unwrap()
    })
}

fn link_deny() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)(unsubscribe|unsub\b|opt[\s-]?out|list-manage|list-unsubscribe|/preferences|email[\s-]?settings|manage[\s-]?(?:your[\s-]?)?preferences)").unwrap()
    })
}

fn url_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"https://[^\s<>"'`)\]]+"#).unwrap())
}

// <a ...href="https://...">text</a> — two quote alternatives since the regex
// crate has no backreferences.
fn anchor_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r#"(?is)<a\b[^>]*?href\s*=\s*(?:"(https://[^"]+)"|'(https://[^']+)')[^>]*?>(.*?)</a>"#).unwrap()
    })
}

fn trim_trailing_punct(s: &str) -> &str {
    s.trim_end_matches(|c| c == '.' || c == ',' || c == ';' || c == ':' || c == '!' || c == '?')
}

fn host_of(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let hostport = &rest[..end];
    let host = hostport.rsplit('@').next().unwrap_or(hostport);
    if host.is_empty() { None } else { Some(host.to_string()) }
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => { in_tag = false; out.push(' '); }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn decode_entities(s: &str) -> String {
    static NUM: OnceLock<Regex> = OnceLock::new();
    let num = NUM.get_or_init(|| Regex::new(r"(?i)&#(x?)([0-9a-f]+);").unwrap());
    let out = num.replace_all(s, |c: &regex::Captures| {
        let is_hex = !c[1].is_empty();
        let parsed = if is_hex {
            u32::from_str_radix(&c[2], 16)
        } else {
            c[2].parse::<u32>()
        };
        parsed.ok().and_then(char::from_u32).map(String::from).unwrap_or_else(|| c[0].to_string())
    });
    out.replace("&amp;", "&")
}

/// A magic-link URL from plaintext: https URL near a login/verify keyword.
pub fn find_link(text: &str) -> Option<Link> {
    if text.is_empty() {
        return None;
    }
    let deny = link_deny();
    let kw = link_keywords();
    for m in url_re().find_iter(text) {
        let url = trim_trailing_punct(m.as_str());
        if deny.is_match(url) {
            continue;
        }
        let host = match host_of(url) {
            Some(h) => h,
            None => continue,
        };
        let win_start = m.start().saturating_sub(160);
        let win_len = (m.start() - win_start) + url.len() + 40;
        let ctx = safe_slice(text, win_start, win_len);
        if kw.is_match(ctx) || kw.is_match(url) {
            return Some(Link { url: url.to_string(), host });
        }
    }
    None
}

/// A magic-link URL from an HTML body: the anchor whose text or href carries a
/// login/verify keyword. Used when plaintext conversion dropped the href.
pub fn find_link_in_html(html: &str) -> Option<Link> {
    if html.is_empty() {
        return None;
    }
    let deny = link_deny();
    let kw = link_keywords();
    for c in anchor_re().captures_iter(html) {
        let raw = c.get(1).or_else(|| c.get(2)).map(|m| m.as_str()).unwrap_or("");
        if raw.is_empty() {
            continue;
        }
        let url = decode_entities(raw);
        if deny.is_match(&url) {
            continue;
        }
        let host = match host_of(&url) {
            Some(h) => h,
            None => continue,
        };
        let text = strip_tags(c.get(3).map(|m| m.as_str()).unwrap_or(""));
        if kw.is_match(&text) || kw.is_match(&url) {
            return Some(Link { url, host });
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

    #[test]
    fn finds_plaintext_magic_link() {
        let l = find_link("Click to sign in:\nhttps://app.example.com/verify?t=abc123").unwrap();
        assert_eq!(l.host, "app.example.com");
        assert_eq!(l.url, "https://app.example.com/verify?t=abc123");
    }

    #[test]
    fn rejects_bare_and_denylisted_links() {
        assert!(find_link("https://example.com/home").is_none());
        assert!(find_link("To sign in visit https://n.com/email-settings/login").is_none());
        assert!(find_link("Sign in here: http://insecure.example.com/login").is_none());
    }

    #[test]
    fn finds_html_anchor_link() {
        let html = r#"<a href="https://www.anthropic.com"><img></a>
            <a href="https://claude.ai/magic?token=abc&amp;u=42">Sign in to Claude.ai</a>
            <a href="https://x/unsubscribe">Unsubscribe</a>"#;
        let l = find_link_in_html(html).unwrap();
        assert_eq!(l.host, "claude.ai");
        assert_eq!(l.url, "https://claude.ai/magic?token=abc&u=42");
    }

    #[test]
    fn html_ignores_decoy_only() {
        let html = r#"<a href="https://t/x">View in browser</a><a href="https://f/unsubscribe">Unsubscribe</a>"#;
        assert!(find_link_in_html(html).is_none());
    }
}

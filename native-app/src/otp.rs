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

// Words that, sitting just before a number, mark it as a phone / support line.
fn phone_words() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)(?:call|phone|tel|dial|contact|text\s+us|\bsms\b|fax|mobile|hotline|help\s?line|toll[\s.\-]?free|whats\s?app)\b").unwrap()
    })
}

// Separator-formatted phone numbers (bare 10+ digit runs can't leak a token
// because the token \b…\b boundaries fail on runs longer than 8).
fn phone_shape() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?:\+\d{1,3}[\s.\-]*)?\(?\d{3}\)?[\s.\-]+\d{3}[\s.\-]+\d{4}|1[\s.\-]?\d{3}[\s.\-]?\d{3}[\s.\-]?\d{4}").unwrap()
    })
}

/// True if the token at [start,end) in `ctx` is part of a phone number: a phone
/// word within ~25 chars before it, or overlap with a phone-shaped run.
fn is_phone(ctx: &str, start: usize, end: usize) -> bool {
    let ws = start.saturating_sub(25);
    let before = safe_slice(ctx, ws, start - ws);
    if phone_words().is_match(before) {
        return true;
    }
    phone_shape().find_iter(ctx).any(|pm| pm.start() < end && start < pm.end())
}

/// First token in `s` that is a plausible code: has at least one digit (so an
/// all-caps word can't match the alnum branch) and is not part of a phone
/// number. Iterates so a phone earlier in the text can't shadow a later code.
fn first_valid(s: &str) -> Option<String> {
    for m in token().find_iter(s) {
        let t = m.as_str();
        if !t.bytes().any(|b| b.is_ascii_digit()) {
            continue;
        }
        if is_phone(s, m.start(), m.end()) {
            continue;
        }
        return Some(normalize(t));
    }
    None
}

fn normalize(s: &str) -> String {
    s.chars().filter(|c| *c != ' ' && *c != '-' && *c != '\t').collect()
}

// URLs and email addresses are removed before code scanning: their embedded
// digits (UUIDs, tracking ids) and keywords (e.g. a "/verification/" path) are a
// major false-positive source. Links are still scanned separately by find_link.
fn strip_urls(text: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    let re = R.get_or_init(|| {
        Regex::new(r"(?i)https?://\S+|[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").unwrap()
    });
    re.replace_all(text, " ").into_owned()
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
    let cleaned = strip_urls(text);
    let text = cleaned.as_str();
    let kw = keywords();

    // Line-oriented: a keyword line, code on it or within the next two lines.
    let lines: Vec<&str> = text.split('\n').collect();
    for (i, line) in lines.iter().enumerate() {
        if !kw.is_match(line) {
            continue;
        }
        if let Some(c) = first_valid(line) {
            return Some(c);
        }
        for j in 1..=2 {
            if let Some(l) = lines.get(i + j) {
                if let Some(c) = first_valid(l) {
                    return Some(c);
                }
            }
        }
    }

    // Fallback: a token within ~60 chars after any keyword occurrence.
    for m in kw.find_iter(text) {
        let window = safe_slice(text, m.end(), 60);
        if let Some(c) = first_valid(window) {
            return Some(c);
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
        Regex::new(r"(?i)(magic\s?link|passwordless|sign[\s-]?in|log[\s-]?in|finish\s+(?:signing|sign)|continue\s+to\s+(?:sign|log)|one[\s-]?time\s+(?:sign|log|link))").unwrap()
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
    fn ignores_phone_numbers_near_keywords() {
        // Phone in a security alert must not be surfaced as a code.
        assert_eq!(find_code("For your security, if this wasn't you call 1-800-555-1234."), None);
        assert_eq!(find_code("We noticed a login. Call (555) 123-4567 to report it."), None);
        assert_eq!(find_code("Verify it was you - questions? Contact us at 555.867.5309."), None);
    }

    #[test]
    fn ignores_all_caps_words() {
        // The alnum branch must require a digit, so plain words don't match.
        assert_eq!(find_code("Login alert: your ACCOUNT was accessed"), None);
        assert_eq!(find_code("Security notice: WELCOME back"), None);
    }

    #[test]
    fn still_finds_alnum_codes() {
        assert_eq!(find_code("Your code is A1B2C3").as_deref(), Some("A1B2C3"));
    }

    #[test]
    fn ignores_numbers_inside_urls_and_emails() {
        // A verification URL whose path contains "verification" and a UUID must
        // not yield a code; nor should a copyright year with no nearby keyword.
        let body = "Please confirm your contact email address, mibsmibby@gmail.com, by clicking:\n\
            https://chrome.google.com/webstore/devconsole/1234d5a2-4429-43e0-9c8e-03da8b43481d/verification/AbiLaF8LSGQG\n\
            A verified email address is required.\n\
            © 2026 Google LLC, Mountain View, CA 94043 , USA";
        assert_eq!(find_code(body), None);
    }

    #[test]
    fn skips_phone_and_finds_real_code() {
        // A phone earlier on the line must not shadow the real code after it.
        let body = "Questions? call 800-555-1234. Verification code 908172";
        assert_eq!(find_code(body).as_deref(), Some("908172"));
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
    fn rejects_confirmation_and_verify_email_links() {
        // Account email-confirmation / "verify your email" links are not logins.
        assert!(find_link("Please confirm your contact email address by clicking:\nhttps://chrome.google.com/webstore/devconsole/abc/verification/xyz").is_none());
        assert!(find_link("Verify your email address: https://example.com/activate/token123").is_none());
        // A genuine passwordless login link still surfaces.
        assert_eq!(find_link("Your magic link to sign in:\nhttps://app.example.com/l/tok").unwrap().host, "app.example.com");
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

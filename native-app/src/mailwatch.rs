//! Per-account IMAP watcher. Uses IMAP IDLE to wait for new mail in real time,
//! then peeks new messages (BODY.PEEK, so they are NOT marked read), extracts a
//! verification code, and forwards it. Reconnects on error.

use crate::config::Account;
use crate::otp;
use std::sync::mpsc::Sender;
use std::time::Duration;

pub struct CodeEvent {
    pub code: String,
    pub account: String,
    pub from_name: String,
    pub from_email: String,
    pub subject: String,
}

pub fn watch<F: Fn(&str)>(account: Account, password: String, tx: Sender<CodeEvent>, log: F) {
    loop {
        if let Err(e) = run_once(&account, &password, &tx, &log) {
            log(&format!("[{}] error: {}", account.label, e));
        }
        std::thread::sleep(Duration::from_secs(15));
    }
}

fn run_once<F: Fn(&str)>(
    account: &Account,
    password: &str,
    tx: &Sender<CodeEvent>,
    log: &F,
) -> Result<(), Box<dyn std::error::Error>> {
    let tls = native_tls::TlsConnector::builder().build()?;
    let client = imap::connect((account.host.as_str(), account.port), account.host.as_str(), &tls)?;
    let mut session = client.login(&account.user, password).map_err(|(e, _)| e)?;
    log(&format!("[{}] connected", account.label));

    let mailbox = session.select("INBOX")?;
    // Baseline: only report mail that arrives after we start watching.
    let mut last_seen: u32 = mailbox.uid_next.map(|n| n.saturating_sub(1)).unwrap_or(0);

    loop {
        // Block until the server reports activity (new mail), re-issuing IDLE
        // periodically so the connection stays fresh.
        session.idle()?.wait_keepalive()?;

        let uids = session.uid_search(format!("UID {}:*", last_seen + 1))?;
        let mut fresh: Vec<u32> = uids.into_iter().filter(|&u| u > last_seen).collect();
        fresh.sort_unstable();

        for uid in fresh {
            let fetches = session.uid_fetch(uid.to_string(), "BODY.PEEK[]")?;
            for f in fetches.iter() {
                if let Some(body) = f.body() {
                    if let Some(ev) = extract(account, body) {
                        log(&format!("[{}] code in uid {}", account.label, uid));
                        let _ = tx.send(ev);
                    }
                }
            }
            last_seen = last_seen.max(uid);
        }
    }
}

/// One-shot connectivity check used by `otp-relay test` and the GUI.
pub fn check_connection(account: &Account, password: &str) -> Result<u32, Box<dyn std::error::Error>> {
    let tls = native_tls::TlsConnector::builder().build()?;
    let client = imap::connect((account.host.as_str(), account.port), account.host.as_str(), &tls)?;
    let mut session = client.login(&account.user, password).map_err(|(e, _)| e)?;
    let mb = session.select("INBOX")?;
    let _ = session.logout();
    Ok(mb.exists)
}

fn extract(account: &Account, raw: &[u8]) -> Option<CodeEvent> {
    let msg = mail_parser::MessageParser::default().parse(raw)?;
    let subject = msg.subject().unwrap_or("").to_string();
    let (from_name, from_email) = msg
        .from()
        .and_then(|a| a.first())
        .map(|addr| {
            (
                addr.name().unwrap_or("").to_string(),
                addr.address().unwrap_or("").to_string(),
            )
        })
        .unwrap_or_default();

    let mut hay = subject.clone();
    hay.push('\n');
    if let Some(t) = msg.body_text(0) {
        hay.push_str(&t);
    }
    if let Some(h) = msg.body_html(0) {
        hay.push('\n');
        hay.push_str(&strip_html(&h));
    }

    let code = otp::find_code(&hay)?;
    Some(CodeEvent {
        code,
        account: account.label.clone(),
        from_name,
        from_email,
        subject,
    })
}

fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

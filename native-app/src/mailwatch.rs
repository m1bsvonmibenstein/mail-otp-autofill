//! Per-account IMAP watcher. Uses IMAP IDLE to wait for new mail in real time,
//! then peeks new messages (BODY.PEEK, so they are NOT marked read), extracts a
//! verification code, and forwards it. Reconnects on error.

use crate::config::Account;
use crate::otp;
use std::sync::mpsc::Sender;
use std::time::Duration;

/// What a message yielded: a verification code, or a magic sign-in link.
pub enum Payload {
    Code(String),
    Link { url: String, host: String },
}

pub struct MailEvent {
    pub payload: Payload,
    pub account: String,
    pub from_name: String,
    pub from_email: String,
    pub subject: String,
}

pub fn watch<F: Fn(&str)>(account: Account, password: String, poll: Duration, tx: Sender<MailEvent>, log: F) {
    loop {
        if let Err(e) = run_once(&account, &password, poll, &tx, &log) {
            log(&format!("[{}] error: {}", account.label, e));
        }
        std::thread::sleep(Duration::from_secs(15));
    }
}

fn run_once<F: Fn(&str)>(
    account: &Account,
    password: &str,
    poll: Duration,
    tx: &Sender<MailEvent>,
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
        // Block until the server reports activity (new mail), or until the poll
        // interval elapses. New mail still wakes us instantly; the timeout is a
        // safety re-check in case IDLE silently stalls. Either wake re-scans.
        let _ = session.idle()?.wait_with_timeout(poll)?;

        let uids = session.uid_search(format!("UID {}:*", last_seen + 1))?;
        let mut fresh: Vec<u32> = uids.into_iter().filter(|&u| u > last_seen).collect();
        fresh.sort_unstable();

        for uid in fresh {
            let fetches = session.uid_fetch(uid.to_string(), "BODY.PEEK[]")?;
            for f in fetches.iter() {
                if let Some(body) = f.body() {
                    if let Some(ev) = extract(account, body) {
                        let kind = match ev.payload { Payload::Code(_) => "code", Payload::Link { .. } => "link" };
                        log(&format!("[{}] {} in uid {}", account.label, kind, uid));
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

fn extract(account: &Account, raw: &[u8]) -> Option<MailEvent> {
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

    let text_body = msg.body_text(0).map(|c| c.into_owned()).unwrap_or_default();
    let html_body = msg.body_html(0).map(|c| c.into_owned());

    // Code first, over subject + text + stripped HTML (matches previous behaviour).
    let mut hay = subject.clone();
    hay.push('\n');
    hay.push_str(&text_body);
    if let Some(h) = &html_body {
        hay.push('\n');
        hay.push_str(&strip_html(h));
    }

    let payload = if let Some(code) = otp::find_code(&hay) {
        Payload::Code(code)
    } else {
        // Magic link: plaintext (subject + text) first, HTML anchors as fallback
        // for mail whose URL only survives inside an <a href>.
        let mut link_hay = subject.clone();
        link_hay.push('\n');
        link_hay.push_str(&text_body);
        let link = otp::find_link(&link_hay)
            .or_else(|| html_body.as_deref().and_then(otp::find_link_in_html))?;
        Payload::Link { url: link.url, host: link.host }
    };

    Some(MailEvent {
        payload,
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

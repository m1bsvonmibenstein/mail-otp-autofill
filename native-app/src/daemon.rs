//! Background daemon: owns IMAP IDLE for every account 24/7, holds the latest
//! code briefly, and broadcasts codes to connected bridge clients over the local
//! socket. Also (best-effort) shows a desktop notification and optionally copies
//! the code to the clipboard, so codes work even with the browser closed.
//!
//! Single-instance: the local-socket name acts as the guard (a second daemon
//! fails to bind and exits). Console subsystem on purpose so `--console` shows
//! output; autostart runs it hidden via a launcher. Everything is file-logged to
//! %TEMP%/otp_relay.log because the process is otherwise invisible.

use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{GenericNamespaced, ListenerOptions, Stream};
use otp_relay::{config, ipc, log, mailwatch};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CODE_TTL: Duration = Duration::from_secs(90);

struct Held {
    frame: Vec<u8>,
    at: Instant,
}

type Clients = Arc<Mutex<Vec<Arc<Mutex<Stream>>>>>;
type Latest = Arc<Mutex<Option<Held>>>;

fn main() {
    let console = std::env::args().any(|a| a == "--console");
    log("daemon", "start");

    let name = match ipc::PIPE_NAME.to_ns_name::<GenericNamespaced>() {
        Ok(n) => n,
        Err(e) => {
            log("daemon", &format!("bad pipe name: {}", e));
            return;
        }
    };
    let listener = match ListenerOptions::new().name(name).create_sync() {
        Ok(l) => l,
        Err(e) => {
            log("daemon", &format!("bind failed (already running?): {}", e));
            if console {
                eprintln!("otp-relay-daemon: already running or pipe in use: {}", e);
            }
            return;
        }
    };
    if console {
        println!("otp-relay-daemon listening on {}", ipc::PIPE_NAME);
    }

    let cfg = config::load();
    let notify_on = cfg.notify;
    let auto_copy = cfg.auto_copy;

    let clients: Clients = Arc::new(Mutex::new(Vec::new()));
    let latest: Latest = Arc::new(Mutex::new(None));

    // Spawn one IMAP watcher per account; they feed codes into a channel.
    let (tx, rx) = std::sync::mpsc::channel::<mailwatch::MailEvent>();
    let mut watched = 0;
    for acct in cfg.accounts {
        match config::get_password(&acct.label) {
            Ok(pw) => {
                let txc = tx.clone();
                std::thread::spawn(move || mailwatch::watch(acct, pw, txc, |m| log("imap", m)));
                watched += 1;
            }
            Err(e) => log("daemon", &format!("no password for {}: {}", acct.label, e)),
        }
    }
    drop(tx);
    log("daemon", &format!("watching {} account(s)", watched));
    if console {
        println!("watching {} account(s)", watched);
    }

    // Code processor: store latest, broadcast, notify, clipboard.
    {
        let clients = clients.clone();
        let latest = latest.clone();
        std::thread::spawn(move || {
            for ev in rx {
                let meta = serde_json::json!({
                    "account": ev.account,
                    "subject": ev.subject,
                    "from": { "name": ev.from_name, "email": ev.from_email }
                });
                let (frame_json, log_line, clip) = match &ev.payload {
                    mailwatch::Payload::Code(code) => (
                        serde_json::json!({ "type": "code", "code": code, "meta": meta }),
                        format!("code {}", code),
                        code.clone(),
                    ),
                    mailwatch::Payload::Link { url, host } => (
                        serde_json::json!({ "type": "link", "url": url, "host": host, "meta": meta }),
                        format!("link {}", host),
                        url.clone(),
                    ),
                };
                let frame = frame_json.to_string().into_bytes();

                *latest.lock().unwrap() = Some(Held { frame: frame.clone(), at: Instant::now() });

                {
                    let mut g = clients.lock().unwrap();
                    g.retain(|c| {
                        let mut s = c.lock().unwrap();
                        ipc::write_frame(&mut *s, &frame).is_ok()
                    });
                    log("daemon", &format!("{} -> {} client(s)", log_line, g.len()));
                }

                if notify_on {
                    spawn_notifier(&ev);
                }
                if auto_copy {
                    if let Err(e) = set_clipboard(&clip) {
                        log("daemon", &format!("clipboard error: {}", e));
                    }
                }
            }
        });
    }

    // Accept clients; hand each a fresh held code on connect.
    for conn in listener.incoming() {
        match conn {
            Ok(mut stream) => {
                {
                    let mut l = latest.lock().unwrap();
                    match l.as_ref() {
                        Some(h) if h.at.elapsed() < CODE_TTL => {
                            let _ = ipc::write_frame(&mut stream, &h.frame);
                        }
                        Some(_) => *l = None, // expired
                        None => {}
                    }
                }
                clients.lock().unwrap().push(Arc::new(Mutex::new(stream)));
                log("daemon", "client connected");
            }
            Err(e) => log("daemon", &format!("accept error: {}", e)),
        }
    }
}

/// Spawn the custom notification popup (sibling exe) for a new code or link.
fn spawn_notifier(ev: &mailwatch::MailEvent) {
    let name = if cfg!(windows) { "otp-relay-notify.exe" } else { "otp-relay-notify" };
    let exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(name)));
    if let Some(exe) = exe {
        let mut cmd = std::process::Command::new(exe);
        match &ev.payload {
            mailwatch::Payload::Code(code) => { cmd.arg("--code").arg(code); }
            mailwatch::Payload::Link { url, host } => { cmd.arg("--link").arg(url).arg("--host").arg(host); }
        }
        let _ = cmd
            .arg("--from")
            .arg(&ev.from_name)
            .arg("--subject")
            .arg(&ev.subject)
            .spawn();
    }
}

fn set_clipboard(code: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut cb = arboard::Clipboard::new()?;
    cb.set_text(code.to_string())?;
    Ok(())
}

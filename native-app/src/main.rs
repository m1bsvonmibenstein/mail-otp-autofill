//! otp-relay - local native-messaging host for the Mail OTP Autofill extension.
//!
//! Default (launched by the browser): connect to each configured IMAP account,
//! watch via IDLE, and push verification codes to the extension over stdio.
//! Runs while the browser holds the native-messaging port open (which keeps the
//! extension's service worker - and therefore this process - alive).
//!
//! CLI (run from a terminal) manages accounts:
//!   otp-relay add --label mailcow --host mail.example.com --user you@example.com [--port 993]
//!   otp-relay list
//!   otp-relay remove --label mailcow
//!   otp-relay test         # connect to each account once and report

use otp_relay::{config, mailwatch};

use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

/// File logging only - stdout is reserved for the native-messaging protocol.
fn log(msg: &str) {
    let path = std::env::temp_dir().join("otp_relay.log");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{} pid={} {}", now_ms(), std::process::id(), msg);
    }
}

// --- native messaging framing (4-byte LE length prefix + JSON) --------------
fn read_message<R: Read>(r: &mut R) -> Option<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    if r.read_exact(&mut len_buf).is_err() {
        return None;
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 || len > 64 * 1024 * 1024 {
        return None;
    }
    let mut buf = vec![0u8; len];
    if r.read_exact(&mut buf).is_err() {
        return None;
    }
    Some(buf)
}

fn write_message(out: &Mutex<io::Stdout>, bytes: &[u8]) -> io::Result<()> {
    let len = bytes.len() as u32;
    let mut h = out.lock().unwrap();
    h.write_all(&len.to_le_bytes())?;
    h.write_all(bytes)?;
    h.flush()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("add") => cli_add(&args[1..]),
        Some("list") => cli_list(),
        Some("remove") => cli_remove(&args[1..]),
        Some("test") => cli_test(),
        Some("-h") | Some("--help") | Some("help") => print_help(),
        _ => run_host(),
    }
}

fn run_host() {
    log("host start");
    let cfg = config::load();
    let stdout = Arc::new(Mutex::new(io::stdout()));
    let (tx, rx) = mpsc::channel::<mailwatch::CodeEvent>();

    let _ = write_message(&stdout, br#"{"type":"hello"}"#);

    for acct in cfg.accounts {
        match config::get_password(&acct.label) {
            Ok(pw) => {
                let txc = tx.clone();
                let label = acct.label.clone();
                std::thread::spawn(move || {
                    mailwatch::watch(acct, pw, txc, |m| log(m));
                });
                log(&format!("watching {}", label));
            }
            Err(e) => log(&format!("no password for {}: {}", acct.label, e)),
        }
    }
    drop(tx); // watcher threads hold clones; rx stays open while any watcher lives

    // Forward codes to the browser.
    let stdout_w = stdout.clone();
    std::thread::spawn(move || {
        for ev in rx {
            let payload = serde_json::json!({
                "type": "code",
                "code": ev.code,
                "meta": {
                    "account": ev.account,
                    "subject": ev.subject,
                    "from": { "name": ev.from_name, "email": ev.from_email }
                }
            });
            let _ = write_message(&stdout_w, payload.to_string().as_bytes());
        }
    });

    // Read browser -> host messages until the port closes, then exit.
    let stdin = io::stdin();
    let mut lock = stdin.lock();
    loop {
        match read_message(&mut lock) {
            Some(_msg) => { /* e.g. {"type":"used"} - nothing to do host-side yet */ }
            None => {
                log("stdin closed / exit");
                std::process::exit(0);
            }
        }
    }
}

// --- CLI --------------------------------------------------------------------
fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == name {
            return it.next().map(String::as_str);
        }
    }
    None
}

fn cli_add(args: &[String]) {
    let label = match flag(args, "--label") {
        Some(v) => v.to_string(),
        None => return eprintln!("--label is required"),
    };
    let host = match flag(args, "--host") {
        Some(v) => v.to_string(),
        None => return eprintln!("--host is required"),
    };
    let user = match flag(args, "--user") {
        Some(v) => v.to_string(),
        None => return eprintln!("--user is required"),
    };
    let port: u16 = flag(args, "--port").and_then(|p| p.parse().ok()).unwrap_or(993);

    let password = match rpassword::prompt_password(format!("IMAP app password for {}: ", user)) {
        Ok(p) => p,
        Err(e) => return eprintln!("could not read password: {}", e),
    };
    if let Err(e) = config::set_password(&label, &password) {
        return eprintln!("failed to store password in keychain: {}", e);
    }

    let mut cfg = config::load();
    cfg.accounts.retain(|a| a.label != label);
    cfg.accounts.push(config::Account { label: label.clone(), host, port, user });
    match config::save(&cfg) {
        Ok(_) => println!("Added account '{}'. Password stored in the OS keychain.", label),
        Err(e) => eprintln!("failed to save config: {}", e),
    }
}

fn cli_list() {
    let cfg = config::load();
    if cfg.accounts.is_empty() {
        println!("No accounts. Add one with: otp-relay add --label <n> --host <h> --user <u>");
        return;
    }
    println!("Configured accounts ({}):", config::config_path().display());
    for a in &cfg.accounts {
        let has_pw = config::get_password(&a.label).is_ok();
        println!("  {}  {}@{}:{}  password:{}", a.label, a.user, a.host, a.port,
                 if has_pw { "stored" } else { "MISSING" });
    }
}

fn cli_remove(args: &[String]) {
    let label = match flag(args, "--label") {
        Some(v) => v.to_string(),
        None => return eprintln!("--label is required"),
    };
    let mut cfg = config::load();
    let before = cfg.accounts.len();
    cfg.accounts.retain(|a| a.label != label);
    if cfg.accounts.len() == before {
        return eprintln!("no account labelled '{}'", label);
    }
    let _ = config::delete_password(&label);
    match config::save(&cfg) {
        Ok(_) => println!("Removed '{}'.", label),
        Err(e) => eprintln!("failed to save config: {}", e),
    }
}

fn cli_test() {
    let cfg = config::load();
    if cfg.accounts.is_empty() {
        return println!("No accounts configured.");
    }
    for a in &cfg.accounts {
        print!("{} ({}@{}:{}) ... ", a.label, a.user, a.host, a.port);
        let _ = io::stdout().flush();
        let pw = match config::get_password(&a.label) {
            Ok(p) => p,
            Err(e) => { println!("no password: {}", e); continue; }
        };
        match mailwatch::check_connection(a, &pw) {
            Ok(n) => println!("OK ({} messages in INBOX)", n),
            Err(e) => println!("FAILED: {}", e),
        }
    }
}

fn print_help() {
    println!(
        "otp-relay - Mail OTP Autofill native host\n\n\
         Run with no arguments to act as the browser's native-messaging host.\n\n\
         Account management:\n\
         \x20 add --label <n> --host <h> --user <u> [--port 993]   add/update an account\n\
         \x20 list                                                 show accounts\n\
         \x20 remove --label <n>                                   remove an account\n\
         \x20 test                                                 connect to each account once"
    );
}

//! Native-messaging host — SPIKE build.
//!
//! Purpose right now: verify the MV3 service-worker lifecycle. The browser spawns
//! this process when the extension calls `connectNative`, and kills it when the
//! port closes (which happens when the SW is torn down after ~30s idle). This
//! build does nothing but log every spawn/exit with a timestamp so we can watch
//! whether the process is stable or thrashing.
//!
//! Log file: %TEMP%/otp_relay_spike.log (Windows) or $TMPDIR/otp_relay_spike.log.
//!
//! Once the spike answers "does it survive idle?", this becomes either the real
//! IMAP daemon (if stable) or a thin bridge to a standalone daemon (if not).

use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn log(msg: &str) {
    let path = std::env::temp_dir().join("otp_relay_spike.log");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{} pid={} {}", now_ms(), std::process::id(), msg);
    }
}

/// Native messaging framing: 4-byte little-endian length prefix, then JSON.
fn read_message() -> Option<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    if io::stdin().read_exact(&mut len_buf).is_err() {
        return None;
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 || len > 64 * 1024 * 1024 {
        return None;
    }
    let mut buf = vec![0u8; len];
    if io::stdin().read_exact(&mut buf).is_err() {
        return None;
    }
    Some(buf)
}

fn write_message(bytes: &[u8]) -> io::Result<()> {
    let len = bytes.len() as u32;
    let stdout = io::stdout();
    let mut h = stdout.lock();
    h.write_all(&len.to_le_bytes())?;
    h.write_all(bytes)?;
    h.flush()
}

fn main() {
    log("SPAWNED (spike build)");
    // Announce ourselves so the extension can confirm the channel is live.
    let _ = write_message(br#"{"type":"hello","stub":true}"#);
    log("sent hello");

    loop {
        match read_message() {
            Some(msg) => {
                log(&format!("recv: {}", String::from_utf8_lossy(&msg)));
                let _ = write_message(br#"{"type":"pong"}"#);
            }
            None => {
                log("stdin closed / EXIT");
                break;
            }
        }
    }
}

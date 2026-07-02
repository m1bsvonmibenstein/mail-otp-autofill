//! Shared library for the otp-relay host binary and the GUI account manager.

pub mod config;
pub mod ipc;
pub mod mailwatch;
pub mod otp;

use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

/// Append a line to %TEMP%/otp_relay.log, tagged with a component name.
pub fn log(component: &str, msg: &str) {
    let path = std::env::temp_dir().join("otp_relay.log");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        let _ = writeln!(f, "{} [{}] pid={} {}", ts, component, std::process::id(), msg);
    }
}

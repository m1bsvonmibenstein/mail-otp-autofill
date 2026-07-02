//! Local IPC between the background daemon and the browser-spawned bridge.
//! Uses the same 4-byte little-endian length-prefixed framing as native
//! messaging, so the bridge can relay frames straight through.

use std::io::{self, Read, Write};

/// Named-pipe / local-socket name (Windows: \\.\pipe\otp_relay.mibs).
pub const PIPE_NAME: &str = "otp_relay.mibs";

pub fn read_frame<R: Read>(r: &mut R) -> Option<Vec<u8>> {
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

pub fn write_frame<W: Write>(w: &mut W, bytes: &[u8]) -> io::Result<()> {
    let len = bytes.len() as u32;
    w.write_all(&len.to_le_bytes())?;
    w.write_all(bytes)?;
    w.flush()
}

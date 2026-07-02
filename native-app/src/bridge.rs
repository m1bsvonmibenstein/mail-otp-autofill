//! Native-messaging bridge. The browser spawns this via connectNative; it
//! connects to the background daemon over the local socket and relays code
//! frames to the extension (stdout). One-directional: it only reads stdin to
//! detect the browser closing the port, then exits.

use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{GenericNamespaced, Stream};
use otp_relay::{ipc, log};
use std::io::{self, Read};

fn main() {
    log("bridge", "start");

    let name = match ipc::PIPE_NAME.to_ns_name::<GenericNamespaced>() {
        Ok(n) => n,
        Err(e) => {
            log("bridge", &format!("bad pipe name: {}", e));
            return;
        }
    };

    let mut stream = match Stream::connect(name) {
        Ok(s) => s,
        Err(e) => {
            // Daemon not running. The extension's keepalive alarm will retry;
            // the installer keeps the daemon running / autostarted.
            log("bridge", &format!("daemon connect failed: {}", e));
            return;
        }
    };
    log("bridge", "connected to daemon");

    // Exit when the browser closes the port (stdin EOF).
    std::thread::spawn(|| {
        let mut stdin = io::stdin().lock();
        let mut buf = [0u8; 256];
        loop {
            match stdin.read(&mut buf) {
                Ok(0) => {
                    log("bridge", "stdin EOF / exit");
                    std::process::exit(0);
                }
                Ok(_) => { /* ignore browser -> host messages */ }
                Err(_) => std::process::exit(0),
            }
        }
    });

    // Relay daemon frames straight to the extension (identical framing).
    let stdout = io::stdout();
    loop {
        match ipc::read_frame(&mut stream) {
            Some(frame) => {
                let mut o = stdout.lock();
                if ipc::write_frame(&mut o, &frame).is_err() {
                    break;
                }
            }
            None => {
                log("bridge", "daemon pipe closed / exit");
                break;
            }
        }
    }
    std::process::exit(0);
}

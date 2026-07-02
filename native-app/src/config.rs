//! Account config + credential storage.
//!
//! Non-secret account info lives in a JSON file under the user's config dir.
//! App passwords live in the OS keychain (Windows Credential Manager / macOS
//! Keychain / Linux Secret Service) via the `keyring` crate — never on disk.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;

pub const KEYRING_SERVICE: &str = "com.mibs.otp_relay";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub label: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub user: String,
}

fn default_port() -> u16 {
    993
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub accounts: Vec<Account>,
}

pub fn config_path() -> PathBuf {
    let mut dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("otp-relay");
    dir.push("config.json");
    dir
}

pub fn load() -> Config {
    let path = config_path();
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub fn save(cfg: &Config) -> io::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_vec_pretty(cfg).map_err(io::Error::other)?;
    std::fs::write(&path, json)
}

pub fn get_password(label: &str) -> keyring::Result<String> {
    keyring::Entry::new(KEYRING_SERVICE, label)?.get_password()
}

pub fn set_password(label: &str, password: &str) -> keyring::Result<()> {
    keyring::Entry::new(KEYRING_SERVICE, label)?.set_password(password)
}

pub fn delete_password(label: &str) -> keyring::Result<()> {
    keyring::Entry::new(KEYRING_SERVICE, label)?.delete_password()
}

//! GUI account manager for the Mail OTP Autofill native host.
//! Add/remove IMAP mailboxes and test connectivity. App passwords are stored in
//! the OS keychain; account info in the shared config file.

#![cfg_attr(windows, windows_subsystem = "windows")] // no console window on Windows

use eframe::egui;
use otp_relay::config::{self, Account};
use otp_relay::mailwatch;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type Results = Arc<Mutex<HashMap<String, String>>>;

struct GuiApp {
    accounts: Vec<Account>,
    n_label: String,
    n_host: String,
    n_port: String,
    n_user: String,
    n_pass: String,
    status: String,
    notify: bool,
    auto_copy: bool,
    poll_secs: String,
    results: Results,
}

impl GuiApp {
    fn new() -> Self {
        let cfg = config::load();
        GuiApp {
            accounts: cfg.accounts,
            n_label: String::new(),
            n_host: String::new(),
            n_port: String::from("993"),
            n_user: String::new(),
            n_pass: String::new(),
            status: String::new(),
            notify: cfg.notify,
            auto_copy: cfg.auto_copy,
            poll_secs: cfg.poll_secs.to_string(),
            results: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn reload(&mut self) {
        self.accounts = config::load().accounts;
    }

    fn save_settings(&self) {
        let mut cfg = config::load();
        cfg.notify = self.notify;
        cfg.auto_copy = self.auto_copy;
        if let Ok(v) = self.poll_secs.trim().parse::<u64>() {
            cfg.poll_secs = v.clamp(config::MIN_POLL_SECS, config::MAX_POLL_SECS);
        }
        let _ = config::save(&cfg);
    }

    fn add_account(&mut self) {
        let label = self.n_label.trim().to_string();
        let host = self.n_host.trim().to_string();
        let user = self.n_user.trim().to_string();
        if label.is_empty() || host.is_empty() || user.is_empty() {
            self.status = "Label, host, and email/user are required.".into();
            return;
        }
        let port: u16 = self.n_port.trim().parse().unwrap_or(993);
        if let Err(e) = config::set_password(&label, &self.n_pass) {
            self.status = format!("Keychain error: {}", e);
            return;
        }
        let mut cfg = config::load();
        cfg.accounts.retain(|a| a.label != label);
        cfg.accounts.push(Account { label: label.clone(), host, port, user });
        if let Err(e) = config::save(&cfg) {
            self.status = format!("Save error: {}", e);
            return;
        }
        self.status = format!("Added '{}'. Restart the browser (or toggle the extension source) to apply.", label);
        self.n_label.clear();
        self.n_host.clear();
        self.n_port = "993".into();
        self.n_user.clear();
        self.n_pass.clear();
        self.reload();
    }

    fn reload_daemon(&mut self) {
        match restart_daemon() {
            Ok(_) => self.status = "Daemon reloaded - now watching the current accounts and settings.".into(),
            Err(e) => self.status = format!("Daemon reload failed: {}", e),
        }
    }

    fn remove_account(&mut self, label: &str) {
        let mut cfg = config::load();
        cfg.accounts.retain(|a| a.label != label);
        let _ = config::delete_password(label);
        let _ = config::save(&cfg);
        self.results.lock().unwrap().remove(label);
        self.status = format!("Removed '{}'.", label);
        self.reload();
    }

    fn start_test(&self, account: Account) {
        let results = self.results.clone();
        results.lock().unwrap().insert(account.label.clone(), "testing...".into());
        std::thread::spawn(move || {
            let msg = match config::get_password(&account.label) {
                Ok(pw) => match mailwatch::check_connection(&account, &pw) {
                    Ok(n) => format!("OK ({} messages in INBOX)", n),
                    Err(e) => format!("FAILED: {}", e),
                },
                Err(e) => format!("No password stored: {}", e),
            };
            results.lock().unwrap().insert(account.label, msg);
        });
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut to_remove: Option<String> = None;
        let mut to_test: Option<Account> = None;
        let mut do_reload = false;

        egui::CentralPanel::default().show(ctx, |ui| {
          egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.heading("Mail OTP Autofill - accounts");
            ui.label("IMAP mailboxes the native app watches. App passwords are kept in the OS keychain.");
            ui.add_space(6.0);
            ui.separator();

            ui.strong("Accounts");
            if self.accounts.is_empty() {
                ui.label("No accounts yet. Add one below.");
            }
            let results = self.results.lock().unwrap();
            for a in &self.accounts {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.monospace(format!("{}   {}@{}:{}", a.label, a.user, a.host, a.port));
                });
                ui.horizontal(|ui| {
                    if ui.button("Test").clicked() {
                        to_test = Some(a.clone());
                    }
                    if ui.button("Remove").clicked() {
                        to_remove = Some(a.label.clone());
                    }
                    if let Some(r) = results.get(&a.label) {
                        ui.label(r);
                    }
                });
            }
            drop(results);

            ui.add_space(8.0);
            ui.separator();
            ui.strong("Add / update account");
            ui.horizontal(|ui| {
                ui.label("Preset:");
                if ui.button("Gmail").clicked() {
                    self.n_host = "imap.gmail.com".into();
                    self.n_port = "993".into();
                }
                if ui.button("Outlook").clicked() {
                    self.n_host = "outlook.office365.com".into();
                    self.n_port = "993".into();
                }
                if ui.button("iCloud").clicked() {
                    self.n_host = "imap.mail.me.com".into();
                    self.n_port = "993".into();
                }
            });
            ui.small("Gmail / Outlook / iCloud need an app password (with 2-step verification enabled), not your normal password.");
            egui::Grid::new("add_grid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label("Label");
                ui.text_edit_singleline(&mut self.n_label);
                ui.end_row();
                ui.label("IMAP host");
                ui.text_edit_singleline(&mut self.n_host);
                ui.end_row();
                ui.label("Port");
                ui.text_edit_singleline(&mut self.n_port);
                ui.end_row();
                ui.label("Email / user");
                ui.text_edit_singleline(&mut self.n_user);
                ui.end_row();
                ui.label("App password");
                ui.add(egui::TextEdit::singleline(&mut self.n_pass).password(true));
                ui.end_row();
            });
            ui.add_space(6.0);
            if ui.button("Save account").clicked() {
                self.add_account();
            }

            ui.add_space(8.0);
            ui.separator();
            ui.strong("Settings");
            if ui.checkbox(&mut self.notify, "Desktop notification when a code arrives").changed() {
                self.save_settings();
            }
            if ui.checkbox(&mut self.auto_copy, "Auto-copy the code to the clipboard").changed() {
                self.save_settings();
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Re-check inbox every");
                if ui.add(egui::TextEdit::singleline(&mut self.poll_secs).desired_width(46.0)).changed() {
                    self.save_settings();
                }
                ui.label("seconds");
            });
            ui.small(format!(
                "Safety re-check only - new mail still arrives instantly via IMAP IDLE. Clamped to {}-{}s.",
                config::MIN_POLL_SECS, config::MAX_POLL_SECS
            ));
            ui.small("Applied by the background daemon; changes take effect on its next start.");
            ui.add_space(6.0);
            if ui.button("Reload daemon now").clicked() {
                do_reload = true;
            }
            ui.small("Restarts the background watcher so account and settings changes apply immediately.");

            if !self.status.is_empty() {
                ui.add_space(6.0);
                ui.separator();
                ui.label(&self.status);
            }
          });
        });

        if let Some(label) = to_remove {
            self.remove_account(&label);
        }
        if let Some(account) = to_test {
            self.start_test(account);
        }
        if do_reload {
            self.reload_daemon();
        }

        // Keep repainting while a test runs so results appear promptly.
        ctx.request_repaint_after(std::time::Duration::from_millis(400));
    }
}

/// Path to the daemon binary, expected next to this GUI exe (installed together).
fn daemon_exe() -> Option<std::path::PathBuf> {
    let name = if cfg!(windows) { "otp-relay-daemon.exe" } else { "otp-relay-daemon" };
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(name)))
}

#[cfg(windows)]
fn kill_daemon() {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "otp-relay-daemon.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

#[cfg(not(windows))]
fn kill_daemon() {
    let _ = std::process::Command::new("pkill").arg("-f").arg("otp-relay-daemon").output();
}

/// Kill any running daemon and start a fresh one from the install dir. A short
/// pause lets the OS release the single-instance socket before the new daemon
/// tries to bind it.
fn restart_daemon() -> Result<(), String> {
    let exe = daemon_exe().ok_or("could not resolve daemon path")?;
    if !exe.exists() {
        return Err(format!("daemon not found at {}", exe.display()));
    }
    kill_daemon();
    std::thread::sleep(std::time::Duration::from_millis(600));
    let mut cmd = std::process::Command::new(&exe);
    // The daemon is a console-subsystem binary; without this flag spawning it
    // directly pops a console window. (Autostart launches it hidden via VBS.)
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default().with_inner_size([560.0, 560.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!(
        "../../extension/icons/icon-128.png"
    )) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Mail OTP Autofill - Accounts",
        options,
        Box::new(|_cc| Ok(Box::new(GuiApp::new()) as Box<dyn eframe::App>)),
    )
}

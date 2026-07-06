//! Custom notification popup, spawned by the daemon when a code arrives.
//! A small borderless, always-on-top window in the bottom-right corner showing
//! the code + sender/subject, a Copy button and an X to close, with a short
//! chime. Auto-dismisses. Used instead of OS toasts (which need notifications
//! enabled + an AppUserModelID).

#![cfg_attr(windows, windows_subsystem = "windows")]

use eframe::egui;
use std::time::{Duration, Instant};

const AUTO_CLOSE: Duration = Duration::from_secs(12);
const WIN_W: f32 = 300.0;
const WIN_H: f32 = 128.0;

struct Note {
    code: String,
    link: String,
    host: String,
    from: String,
    subject: String,
    start: Instant,
    positioned: bool,
    styled: bool,
    copied: bool,
}

impl Note {
    fn is_link(&self) -> bool {
        !self.link.is_empty()
    }
}

/// Open a URL in the default browser without pulling in a dependency.
fn open_url(url: &str) {
    #[cfg(windows)]
    let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

const WINDOW_TITLE: &str = "otp-relay-notify";

impl eframe::App for Note {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Make it a tool window (no taskbar button, no Alt+Tab entry) once created.
        if !self.styled {
            make_tool_window(WINDOW_TITLE);
            self.styled = true;
        }
        // Move to bottom-right once the monitor size is known (clear the taskbar).
        if !self.positioned {
            if let Some(ms) = ctx.input(|i| i.viewport().monitor_size) {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                    (ms.x - WIN_W - 24.0).max(0.0),
                    (ms.y - WIN_H - 56.0).max(0.0),
                )));
                self.positioned = true;
            }
        }
        if self.start.elapsed() > AUTO_CLOSE {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        let is_link = self.is_link();
        let mut close = false;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                let title = if is_link { "SIGN-IN LINK" } else { "VERIFICATION CODE" };
                ui.label(egui::RichText::new(title).size(11.0).weak().strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(egui::RichText::new("\u{00D7}").size(18.0)).clicked() {
                        close = true;
                    }
                });
            });
            if is_link {
                ui.label(egui::RichText::new(&self.host).monospace().size(16.0).strong());
            } else {
                ui.label(egui::RichText::new(&self.code).monospace().size(30.0).strong());
            }
            let sub = if self.from.is_empty() {
                self.subject.clone()
            } else if self.subject.is_empty() {
                self.from.clone()
            } else {
                format!("{} - {}", self.from, self.subject)
            };
            if !sub.is_empty() {
                ui.label(egui::RichText::new(sub).weak());
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let label = if self.copied {
                    "Copied \u{2713}"
                } else if is_link {
                    "Copy link"
                } else {
                    "Copy code"
                };
                if ui.button(label).clicked() {
                    if let Ok(mut cb) = arboard::Clipboard::new() {
                        let _ = cb.set_text(if is_link { self.link.clone() } else { self.code.clone() });
                        self.copied = true;
                    }
                }
                if is_link && ui.button("Open").clicked() {
                    open_url(&self.link);
                    close = true;
                }
            });
        });

        if close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(150));
    }
}

fn arg(args: &[String], name: &str) -> String {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == name {
            return it.next().cloned().unwrap_or_default();
        }
    }
    String::new()
}

#[cfg(windows)]
fn play_chime() {
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
    const WAV: &[u8] = include_bytes!("../assets/chime.wav");
    unsafe {
        PlaySoundW(
            WAV.as_ptr() as *const u16,
            std::ptr::null_mut(),
            SND_ASYNC | SND_MEMORY | SND_NODEFAULT,
        );
    }
}

#[cfg(not(windows))]
fn play_chime() {}

#[cfg(windows)]
fn make_tool_window(title: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE,
        SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };
    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), wide.as_ptr());
        if !hwnd.is_null() {
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let new = (ex | WS_EX_TOOLWINDOW as isize) & !(WS_EX_APPWINDOW as isize);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE,
            );
        }
    }
}

#[cfg(not(windows))]
fn make_tool_window(_title: &str) {}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let note = Note {
        code: arg(&args, "--code"),
        link: arg(&args, "--link"),
        host: arg(&args, "--host"),
        from: arg(&args, "--from"),
        subject: arg(&args, "--subject"),
        start: Instant::now(),
        positioned: false,
        styled: false,
        copied: false,
    };

    play_chime();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WIN_W, WIN_H])
            .with_decorations(false)
            .with_resizable(false)
            .with_window_level(egui::WindowLevel::AlwaysOnTop),
        ..Default::default()
    };
    eframe::run_native(
        WINDOW_TITLE,
        options,
        Box::new(|_cc| Ok(Box::new(note) as Box<dyn eframe::App>)),
    )
}

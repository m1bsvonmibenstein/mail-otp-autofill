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
    from: String,
    subject: String,
    start: Instant,
    positioned: bool,
    copied: bool,
}

impl eframe::App for Note {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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

        let mut close = false;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("VERIFICATION CODE").size(11.0).weak().strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(egui::RichText::new("\u{2715}").size(13.0)).clicked() {
                        close = true;
                    }
                });
            });
            ui.label(egui::RichText::new(&self.code).monospace().size(30.0).strong());
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
                let label = if self.copied { "Copied \u{2713}" } else { "Copy code" };
                if ui.button(label).clicked() {
                    if let Ok(mut cb) = arboard::Clipboard::new() {
                        let _ = cb.set_text(self.code.clone());
                        self.copied = true;
                    }
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

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let note = Note {
        code: arg(&args, "--code"),
        from: arg(&args, "--from"),
        subject: arg(&args, "--subject"),
        start: Instant::now(),
        positioned: false,
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
        "otp-notify",
        options,
        Box::new(|_cc| Ok(Box::new(note) as Box<dyn eframe::App>)),
    )
}

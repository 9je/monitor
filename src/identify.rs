//! Identify overlay — spawned as a subprocess per monitor.
//!
//! Usage: monitor --identify INDEX NAME PHYS_X PHYS_Y
//!
//! PHYS_X/PHYS_Y are the monitor's top-left corner in raw X11 physical pixels
//! (as reported by xrandr). We convert to egui logical points at runtime using
//! ctx.pixels_per_point(), which is what ViewportCommand::OuterPosition expects.

use eframe::egui::{self, Color32, FontId, RichText, Rounding};
use std::time::{Duration, Instant};

// Fixed logical size in egui points — small corner box.
const BOX_W: f32 = 210.0;
const BOX_H: f32 = 160.0;
const DISPLAY_SECS: u64 = 4;

pub fn run(args: &[String]) -> eframe::Result {
    // args: ["monitor", "--identify", INDEX, NAME, PHYS_X, PHYS_Y]
    let index: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let name = args.get(3).cloned().unwrap_or_default();
    let phys_x: f32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let phys_y: f32 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(0.0);

    let title = format!("monitor-identify-{index}");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(&title)
            // Don't set position here — ViewportBuilder also multiplies by ppp
            // before passing to winit, and we don't know ppp at build time.
            // We force position via ViewportCommand in the first frame instead.
            .with_inner_size([BOX_W, BOX_H])
            .with_decorations(false)
            .with_resizable(false),
        ..Default::default()
    };

    eframe::run_native(
        &title,
        options,
        Box::new(move |_cc| {
            Ok(Box::new(IdentifyApp {
                index,
                name,
                phys_x,
                phys_y,
                deadline: Instant::now() + Duration::from_secs(DISPLAY_SECS),
                positioned: false,
            }))
        }),
    )
}

struct IdentifyApp {
    index: usize,
    name: String,
    /// Target position in raw X11 physical pixels (from xrandr).
    phys_x: f32,
    phys_y: f32,
    deadline: Instant,
    positioned: bool,
}

impl eframe::App for IdentifyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Force position every frame until the window settles.
        // ViewportCommand::OuterPosition takes egui *points* — divide physical
        // pixels by pixels_per_point to get the correct logical position.
        if !self.positioned {
            let ppp = ctx.pixels_per_point();
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                self.phys_x / ppp,
                self.phys_y / ppp,
            )));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(BOX_W, BOX_H)));
            self.positioned = true;
            ctx.request_repaint(); // ensure next frame fires immediately
        }

        // Auto-close
        let remaining = self.deadline.checked_duration_since(Instant::now());
        if remaining.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let secs_left = remaining.unwrap().as_secs() + 1;

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(12, 16, 30))
                    .stroke(egui::Stroke::new(3.0, Color32::from_rgb(80, 140, 255)))
                    .rounding(Rounding::same(8.0)),
            )
            .show(ctx, |ui: &mut egui::Ui| {
                ui.add_space(14.0);
                ui.vertical_centered(|ui| {
                    // Big number
                    ui.label(
                        RichText::new(self.index.to_string())
                            .font(FontId::proportional(72.0))
                            .color(Color32::WHITE)
                            .strong(),
                    );

                    ui.add_space(4.0);

                    // Port name
                    ui.label(
                        RichText::new(&self.name)
                            .font(FontId::proportional(14.0))
                            .color(Color32::from_rgb(130, 175, 255)),
                    );

                    ui.add_space(6.0);

                    // Countdown
                    ui.label(
                        RichText::new(format!("{}s  ·  Esc to close", secs_left))
                            .font(FontId::proportional(11.0))
                            .color(Color32::from_rgb(80, 90, 120)),
                    );
                });
            });

        // Dismiss on Escape
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

//! Identify overlay — spawned as a subprocess per monitor.
//!
//! Usage: monitor --identify INDEX NAME X Y W H
//!   INDEX  — 0-based monitor number shown as the big label
//!   NAME   — e.g. "DisplayPort-1"
//!   X Y    — top-left corner in global X11 coordinates
//!   W H    — monitor resolution in pixels

use eframe::egui::{self, Color32, FontId, RichText};
use std::time::{Duration, Instant};

const DISPLAY_SECS: u64 = 4;

pub fn run(args: &[String]) -> eframe::Result {
    // args: ["monitor", "--identify", INDEX, NAME, X, Y, W, H]
    let index: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let name = args.get(3).cloned().unwrap_or_default();
    let x: f32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let y: f32 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let w: f32 = args.get(6).and_then(|s| s.parse().ok()).unwrap_or(800.0);
    let h: f32 = args.get(7).and_then(|s| s.parse().ok()).unwrap_or(600.0);

    // Window title encodes index so xdotool can find it if needed
    let title = format!("monitor-identify-{index}");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(&title)
            .with_position([x, y])
            .with_inner_size([w, h])
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
                target_x: x,
                target_y: y,
                target_w: w,
                target_h: h,
                deadline: Instant::now() + Duration::from_secs(DISPLAY_SECS),
                frames: 0,
            }))
        }),
    )
}

struct IdentifyApp {
    index: usize,
    name: String,
    target_x: f32,
    target_y: f32,
    target_w: f32,
    target_h: f32,
    deadline: Instant,
    frames: u32,
}

impl eframe::App for IdentifyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Force position for the first several frames to override WM placement.
        // ViewportCommand::OuterPosition sends XMoveWindow directly to the X server.
        if self.frames < 5 {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                self.target_x,
                self.target_y,
            )));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                self.target_w,
                self.target_h,
            )));
            self.frames += 1;
        }

        // Close when time is up
        let remaining = self.deadline.checked_duration_since(Instant::now());
        if remaining.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let secs_left = remaining.unwrap().as_secs() + 1;

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::from_rgba_unmultiplied(10, 15, 30, 230)))
            .show(ctx, |ui: &mut egui::Ui| {
                let h = ui.available_height();

                // Big number — takes up the top ~60% of the window
                ui.add_space(h * 0.12);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new(self.index.to_string())
                            .font(FontId::proportional(h * 0.45))
                            .color(Color32::WHITE)
                            .strong(),
                    );
                });

                // Divider
                ui.add_space(8.0);
                ui.add(egui::Separator::default().horizontal().shrink(60.0));
                ui.add_space(12.0);

                // Monitor name
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new(&self.name)
                            .font(FontId::proportional(h * 0.065))
                            .color(Color32::from_rgb(140, 185, 255)),
                    );

                    ui.add_space(6.0);

                    // Countdown
                    ui.label(
                        RichText::new(format!("Closing in {secs_left}s — press Esc to dismiss"))
                            .font(FontId::proportional(h * 0.033))
                            .color(Color32::from_rgb(100, 110, 140)),
                    );
                });
            });

        // Close on Escape
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

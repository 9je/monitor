use crate::display::{self, Monitor, Rotation};
use eframe::egui::{self, Color32, FontId, Margin, RichText, Rounding, Stroke};
use std::time::{Duration, Instant};

const REFRESH_INTERVAL: Duration = Duration::from_secs(2);

pub struct App {
    monitors: Vec<Monitor>,
    last_refresh: Instant,
    status: Option<(String, bool)>,
    /// Live child processes for identify overlays. Cleaned up when they exit.
    identify_children: Vec<std::process::Child>,
}

impl App {
    pub fn new() -> Self {
        let monitors = display::get_monitors().unwrap_or_default();
        Self {
            monitors,
            last_refresh: Instant::now(),
            status: None,
            identify_children: Vec::new(),
        }
    }

    fn refresh(&mut self) {
        match display::get_monitors() {
            Ok(m) => {
                self.monitors = m;
                self.last_refresh = Instant::now();
            }
            Err(e) => self.status = Some((format!("Refresh failed: {e}"), true)),
        }
    }

    fn set_primary(&mut self, name: &str) {
        match display::set_primary(name) {
            Ok(()) => {
                self.status = Some((format!("Set {name} as primary"), false));
                self.refresh();
            }
            Err(e) => self.status = Some((format!("Error: {e}"), true)),
        }
    }

    fn identify_all(&mut self) {
        self.kill_identify();
        let children = display::spawn_identify_overlays(&self.monitors);
        self.identify_children = children;
        self.status = Some(("Showing overlays on all monitors — press Esc on any to close".into(), false));
    }

    fn identify_one(&mut self, index: usize, monitor: &Monitor) {
        if let Some(child) = display::spawn_identify_one(index, monitor) {
            self.identify_children.push(child);
        }
    }

    fn kill_identify(&mut self) {
        for child in &mut self.identify_children {
            let _ = child.kill();
        }
        self.identify_children.clear();
    }

    /// Reap children that have already exited naturally.
    fn reap_finished_children(&mut self) {
        self.identify_children
            .retain_mut(|c| c.try_wait().map(|s| s.is_none()).unwrap_or(true));
    }

    fn is_identifying(&self) -> bool {
        !self.identify_children.is_empty()
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.last_refresh.elapsed() >= REFRESH_INTERVAL {
            self.refresh();
        }

        // Clean up children that closed on their own (timed out or user pressed Esc)
        self.reap_finished_children();

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(22, 27, 40))
                    .inner_margin(Margin::same(16.0)),
            )
            .show(ctx, |ui| {
                // ── Header ──────────────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Monitor")
                            .font(FontId::proportional(28.0))
                            .color(Color32::from_rgb(130, 190, 255))
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut egui::Ui| {
                        if self.is_identifying() {
                            if ui.button("Dismiss").clicked() {
                                self.kill_identify();
                                self.status = None;
                            }
                        } else if ui.button("Identify All").clicked() {
                            self.identify_all();
                        }
                        ui.add_space(8.0);
                        if ui.button("Refresh").clicked() {
                            self.refresh();
                        }
                    });
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(8.0);

                // ── Status bar ───────────────────────────────────────────────
                if let Some((msg, is_error)) = &self.status {
                    let color = if *is_error {
                        Color32::from_rgb(255, 110, 110)
                    } else {
                        Color32::from_rgb(100, 220, 130)
                    };
                    ui.label(RichText::new(msg).color(color).small());
                    ui.add_space(4.0);
                }

                // ── Monitor cards ────────────────────────────────────────────
                let monitors: Vec<Monitor> = self.monitors.clone();
                if monitors.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new("No monitors detected.\nIs xrandr available?")
                                .color(Color32::from_rgb(180, 180, 180)),
                        );
                    });
                    return;
                }

                let mut pending: Option<CardAction> = None;

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, monitor) in monitors.iter().enumerate() {
                        let action = render_monitor_card(ui, i, monitor);
                        if action != CardAction::None {
                            pending = Some(action);
                        }
                        ui.add_space(10.0);
                    }
                });

                match pending {
                    Some(CardAction::SetPrimary(name)) => self.set_primary(&name),
                    Some(CardAction::Identify(idx, mon)) => self.identify_one(idx, &mon),
                    None | Some(CardAction::None) => {}
                }
            });

        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

// ── Card ────────────────────────────────────────────────────────────────────

enum CardAction {
    None,
    SetPrimary(String),
    Identify(usize, Monitor),
}

impl PartialEq for CardAction {
    fn eq(&self, other: &Self) -> bool {
        matches!((self, other), (CardAction::None, CardAction::None))
    }
}

fn render_monitor_card(ui: &mut egui::Ui, index: usize, monitor: &Monitor) -> CardAction {
    let is_primary = monitor.is_primary;
    let border_color = if is_primary {
        Color32::from_rgb(100, 160, 255)
    } else {
        Color32::from_rgb(55, 65, 85)
    };
    let bg_color = if is_primary {
        Color32::from_rgb(28, 38, 60)
    } else {
        Color32::from_rgb(30, 36, 50)
    };

    let mut action = CardAction::None;

    egui::Frame::none()
        .fill(bg_color)
        .rounding(Rounding::same(8.0))
        .stroke(Stroke::new(if is_primary { 2.0 } else { 1.0 }, border_color))
        .inner_margin(Margin::same(14.0))
        .show(ui, |ui: &mut egui::Ui| {
            ui.horizontal(|ui| {
                // ── Index badge on the left ──────────────────────────────────
                egui::Frame::none()
                    .fill(Color32::from_rgb(40, 50, 80))
                    .rounding(Rounding::same(6.0))
                    .inner_margin(Margin::symmetric(12.0, 6.0))
                    .show(ui, |ui: &mut egui::Ui| {
                        ui.label(
                            RichText::new(index.to_string())
                                .font(FontId::proportional(32.0))
                                .color(Color32::from_rgb(160, 200, 255))
                                .strong(),
                        );
                    });

                ui.add_space(10.0);

                // ── Info ─────────────────────────────────────────────────────
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&monitor.name)
                                .font(FontId::proportional(18.0))
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        if is_primary {
                            ui.add_space(6.0);
                            badge(ui, "PRIMARY", Color32::from_rgb(50, 100, 200));
                        }
                    });

                    ui.add_space(4.0);

                    let (lw, lh) = monitor.logical_resolution();
                    let res_str = if lw != monitor.resolution.0 || lh != monitor.resolution.1 {
                        format!(
                            "{}×{} (raw {}×{}) @ {:.0} Hz",
                            lw, lh, monitor.resolution.0, monitor.resolution.1, monitor.refresh_rate
                        )
                    } else {
                        format!("{}×{} @ {:.0} Hz", lw, lh, monitor.refresh_rate)
                    };
                    ui.label(RichText::new(res_str).color(Color32::from_rgb(180, 200, 230)));

                    if let Some(inches) = monitor.size_inches() {
                        let (w_mm, h_mm) = monitor.physical_mm.unwrap();
                        ui.label(
                            RichText::new(format!("{:.1}\" ({w_mm}mm × {h_mm}mm)", inches))
                                .color(Color32::from_rgb(140, 160, 190))
                                .small(),
                        );
                    }

                    let (px, py) = monitor.position;
                    let pos_str = if matches!(monitor.rotation, Rotation::Normal) {
                        format!("Position: ({px}, {py})")
                    } else {
                        format!("Position: ({px}, {py})  ·  Rotation: {}", monitor.rotation.label())
                    };
                    ui.label(
                        RichText::new(pos_str)
                            .color(Color32::from_rgb(120, 140, 170))
                            .small(),
                    );
                });

                // ── Buttons ──────────────────────────────────────────────────
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut egui::Ui| {
                    if !is_primary {
                        if ui
                            .add(
                                egui::Button::new(RichText::new("Set Primary").color(Color32::WHITE))
                                    .fill(Color32::from_rgb(40, 90, 170))
                                    .rounding(Rounding::same(6.0)),
                            )
                            .clicked()
                        {
                            action = CardAction::SetPrimary(monitor.name.clone());
                        }
                        ui.add_space(6.0);
                    }

                    if ui
                        .add(
                            egui::Button::new(RichText::new("Identify").color(Color32::WHITE))
                                .fill(Color32::from_rgb(45, 55, 80))
                                .rounding(Rounding::same(6.0)),
                        )
                        .clicked()
                    {
                        action = CardAction::Identify(index, monitor.clone());
                    }
                });
            });
        });

    action
}

fn badge(ui: &mut egui::Ui, label: &str, color: Color32) {
    egui::Frame::none()
        .fill(color)
        .rounding(Rounding::same(4.0))
        .inner_margin(Margin::symmetric(6.0, 2.0))
        .show(ui, |ui: &mut egui::Ui| {
            ui.label(
                RichText::new(label)
                    .font(FontId::proportional(11.0))
                    .color(Color32::WHITE)
                    .strong(),
            );
        });
}

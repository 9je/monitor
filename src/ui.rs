use crate::display::{self, Monitor, Rotation};
use eframe::egui::{
    self, Color32, FontId, Margin, RichText, Rounding, Stroke, ViewportBuilder, ViewportId,
};
use std::time::{Duration, Instant};

const REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const IDENTIFY_DURATION: Duration = Duration::from_secs(4);

pub struct App {
    monitors: Vec<Monitor>,
    last_refresh: Instant,
    status: Option<(String, bool)>, // (message, is_error)
    identify_viewports: Vec<(ViewportId, String)>,
    identify_until: Option<Instant>,
}

impl App {
    pub fn new() -> Self {
        let monitors = display::get_monitors().unwrap_or_default();
        Self {
            monitors,
            last_refresh: Instant::now(),
            status: None,
            identify_viewports: Vec::new(),
            identify_until: None,
        }
    }

    fn refresh(&mut self) {
        match display::get_monitors() {
            Ok(m) => {
                self.monitors = m;
                self.last_refresh = Instant::now();
            }
            Err(e) => {
                self.status = Some((format!("Refresh failed: {e}"), true));
            }
        }
    }

    fn set_primary(&mut self, name: &str) {
        match display::set_primary(name) {
            Ok(()) => {
                self.status = Some((format!("Set {name} as primary"), false));
                self.refresh();
            }
            Err(e) => {
                self.status = Some((format!("Error: {e}"), true));
            }
        }
    }

    fn identify_all(&mut self) {
        self.identify_viewports.clear();
        for m in &self.monitors {
            let id = ViewportId::from_hash_of(format!("identify_{}", m.name));
            self.identify_viewports.push((id, m.name.clone()));
        }
        self.identify_until = Some(Instant::now() + IDENTIFY_DURATION);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Auto-refresh
        if self.last_refresh.elapsed() >= REFRESH_INTERVAL {
            self.refresh();
        }

        // Expire identify overlays
        if let Some(until) = self.identify_until {
            if Instant::now() >= until {
                self.identify_viewports.clear();
                self.identify_until = None;
            } else {
                ctx.request_repaint_after(Duration::from_millis(200));
            }
        }

        // Render identify overlays as secondary viewports
        let viewports: Vec<(ViewportId, String, Monitor)> = self
            .identify_viewports
            .iter()
            .filter_map(|(id, name)| {
                self.monitors
                    .iter()
                    .find(|m| &m.name == name)
                    .map(|m| (*id, name.clone(), m.clone()))
            })
            .collect();

        for (id, name, monitor) in viewports {
            let (x, y) = monitor.position;
            let (w, h) = monitor.logical_resolution();
            let label = format_identify_label(&monitor);
            let seconds_left = self
                .identify_until
                .and_then(|u| u.checked_duration_since(Instant::now()))
                .map(|d| d.as_secs() + 1)
                .unwrap_or(0);

            ctx.show_viewport_deferred(
                id,
                ViewportBuilder::default()
                    .with_title(format!("Identify: {name}"))
                    .with_position([x as f32, y as f32])
                    .with_inner_size([w as f32, h as f32])
                    .with_decorations(false),
                move |ctx, _class| {
                    egui::CentralPanel::default()
                        .frame(
                            egui::Frame::none()
                                .fill(Color32::from_rgb(15, 20, 35)),
                        )
                        .show(ctx, |ui: &mut egui::Ui| {
                            ui.add_space(ui.available_height() * 0.25);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(&label)
                                        .font(FontId::proportional(72.0))
                                        .color(Color32::WHITE)
                                        .strong(),
                                );
                                ui.add_space(20.0);
                                ui.label(
                                    RichText::new(format!("Closing in {seconds_left}s"))
                                        .font(FontId::proportional(28.0))
                                        .color(Color32::from_rgb(150, 150, 180)),
                                );
                            });
                        });
                },
            );
        }

        // Main panel
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(22, 27, 40))
                    .inner_margin(Margin::same(16.0)),
            )
            .show(ctx, |ui| {
                // Header
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("monitour")
                            .font(FontId::proportional(28.0))
                            .color(Color32::from_rgb(130, 190, 255))
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Identify All").clicked() {
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

                // Status bar
                if let Some((msg, is_error)) = &self.status {
                    let color = if *is_error {
                        Color32::from_rgb(255, 100, 100)
                    } else {
                        Color32::from_rgb(100, 220, 130)
                    };
                    ui.label(RichText::new(msg).color(color).small());
                    ui.add_space(4.0);
                }

                // Monitor cards
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

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for monitor in &monitors {
                        render_monitor_card(ui, monitor, |name| self.set_primary(name));
                        ui.add_space(10.0);
                    }
                });
            });

        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

fn render_monitor_card(
    ui: &mut egui::Ui,
    monitor: &Monitor,
    mut on_set_primary: impl FnMut(&str),
) {
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

    egui::Frame::none()
        .fill(bg_color)
        .rounding(Rounding::same(8.0))
        .stroke(Stroke::new(if is_primary { 2.0 } else { 1.0 }, border_color))
        .inner_margin(Margin::same(14.0))
        .show(ui, |ui: &mut egui::Ui| {
            ui.horizontal(|ui| {
                // Left side: info
                ui.vertical(|ui| {
                    // Name + PRIMARY badge
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&monitor.name)
                                .font(FontId::proportional(20.0))
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        if is_primary {
                            ui.add_space(6.0);
                            egui::Frame::none()
                                .fill(Color32::from_rgb(50, 100, 200))
                                .rounding(Rounding::same(4.0))
                                .inner_margin(Margin::symmetric(6.0, 2.0))
                                .show(ui, |ui: &mut egui::Ui| {
                                    ui.label(
                                        RichText::new("PRIMARY")
                                            .font(FontId::proportional(11.0))
                                            .color(Color32::WHITE)
                                            .strong(),
                                    );
                                });
                        }
                    });

                    ui.add_space(6.0);

                    // Resolution + refresh rate
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

                    // Physical size
                    if let Some(inches) = monitor.size_inches() {
                        let (w_mm, h_mm) = monitor.physical_mm.unwrap();
                        ui.label(
                            RichText::new(format!("{:.1}\" ({w_mm}mm × {h_mm}mm)", inches))
                                .color(Color32::from_rgb(140, 160, 190))
                                .small(),
                        );
                    }

                    // Position + rotation
                    let (px, py) = monitor.position;
                    let pos_str = if matches!(monitor.rotation, Rotation::Normal) {
                        format!("Position: ({px}, {py})")
                    } else {
                        format!("Position: ({px}, {py})  |  Rotation: {}", monitor.rotation.label())
                    };
                    ui.label(
                        RichText::new(pos_str)
                            .color(Color32::from_rgb(120, 140, 170))
                            .small(),
                    );
                });

                // Right side: Set Primary button
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut egui::Ui| {
                    if !is_primary {
                        let name = monitor.name.clone();
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Set Primary").color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(40, 90, 170))
                                .rounding(Rounding::same(6.0)),
                            )
                            .clicked()
                        {
                            on_set_primary(&name);
                        }
                    }
                });
            });
        });
}

fn format_identify_label(monitor: &Monitor) -> String {
    let (lw, lh) = monitor.logical_resolution();
    format!(
        "{}\n{}×{}  {:.0} Hz",
        monitor.name, lw, lh, monitor.refresh_rate
    )
}

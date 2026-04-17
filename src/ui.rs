use crate::display::{self, Monitor, Rotation};
use eframe::egui::{self, Color32, FontId, Margin, RichText, Rounding, Stroke};
use std::time::{Duration, Instant};

const REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const IDENTIFY_DURATION: Duration = Duration::from_secs(3);

pub struct App {
    monitors: Vec<Monitor>,
    last_refresh: Instant,
    status: Option<(String, bool)>,           // (message, is_error)
    identifying: Option<(String, Instant)>,   // (monitor name, started_at)
}

impl App {
    pub fn new() -> Self {
        let monitors = display::get_monitors().unwrap_or_default();
        Self {
            monitors,
            last_refresh: Instant::now(),
            status: None,
            identifying: None,
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

    /// Dim all monitors except `target_name`. Spawns a thread to restore after IDENTIFY_DURATION.
    fn identify(&mut self, target_name: &str) {
        let monitors = self.monitors.clone();
        let target = target_name.to_string();

        match display::dim_others(&target, &monitors) {
            Ok(()) => {
                self.identifying = Some((target.clone(), Instant::now()));
                self.status = Some((
                    format!("Identifying {target} — look for the bright screen"),
                    false,
                ));

                // Restore in background after delay
                let restore_monitors = monitors.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(IDENTIFY_DURATION);
                    let _ = display::restore_brightness(&restore_monitors);
                });
            }
            Err(e) => self.status = Some((format!("Identify error: {e}"), true)),
        }
    }

    /// Cycle through each monitor (brighten one at a time) so user can see all ports.
    fn identify_all(&mut self) {
        let monitors = self.monitors.clone();
        self.status = Some(("Cycling through all monitors...".into(), false));

        std::thread::spawn(move || {
            for m in &monitors {
                let _ = display::dim_others(&m.name, &monitors);
                std::thread::sleep(Duration::from_millis(1600));
            }
            let _ = display::restore_brightness(&monitors);
        });
    }

    /// Cancel any active identify and restore brightness immediately.
    fn cancel_identify(&mut self) {
        self.identifying = None;
        let monitors = self.monitors.clone();
        std::thread::spawn(move || {
            let _ = display::restore_brightness(&monitors);
        });
        self.status = Some(("Restored brightness".into(), false));
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Auto-refresh monitor state
        if self.last_refresh.elapsed() >= REFRESH_INTERVAL {
            self.refresh();
        }

        // Expire identifying state (the restore already happened in the background thread)
        if let Some((_, started)) = &self.identifying {
            if started.elapsed() >= IDENTIFY_DURATION {
                self.identifying = None;
            } else {
                ctx.request_repaint_after(Duration::from_millis(250));
            }
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(22, 27, 40))
                    .inner_margin(Margin::same(16.0)),
            )
            .show(ctx, |ui| {
                // ── Header ──
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Monitor")
                            .font(FontId::proportional(28.0))
                            .color(Color32::from_rgb(130, 190, 255))
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut egui::Ui| {
                        if self.identifying.is_some() {
                            if ui.button("Cancel Identify").clicked() {
                                self.cancel_identify();
                            }
                        } else {
                            if ui.button("Identify All").clicked() {
                                self.identify_all();
                            }
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

                // ── Status bar ──
                if let Some((msg, is_error)) = &self.status {
                    let color = if *is_error {
                        Color32::from_rgb(255, 110, 110)
                    } else {
                        Color32::from_rgb(100, 220, 130)
                    };
                    ui.label(RichText::new(msg).color(color).small());
                    ui.add_space(4.0);
                }

                // ── Monitor cards ──
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

                let identifying_name: Option<String> = self.identifying.as_ref().map(|(n, _)| n.clone());

                let mut pending_action: Option<(CardAction, String)> = None;

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for monitor in &monitors {
                        let is_identifying = identifying_name.as_deref() == Some(&monitor.name);
                        let action = render_monitor_card(ui, monitor, is_identifying);
                        if action != CardAction::None {
                            pending_action = Some((action, monitor.name.clone()));
                        }
                        ui.add_space(10.0);
                    }
                });

                match pending_action {
                    Some((CardAction::SetPrimary, name)) => self.set_primary(&name),
                    Some((CardAction::Identify, name)) => self.identify(&name),
                    _ => {}
                }
            });

        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

#[derive(PartialEq)]
enum CardAction {
    None,
    SetPrimary,
    Identify,
}

fn render_monitor_card(ui: &mut egui::Ui, monitor: &Monitor, is_identifying: bool) -> CardAction {
    let is_primary = monitor.is_primary;
    let border_color = if is_identifying {
        Color32::from_rgb(255, 200, 50)
    } else if is_primary {
        Color32::from_rgb(100, 160, 255)
    } else {
        Color32::from_rgb(55, 65, 85)
    };
    let bg_color = if is_identifying {
        Color32::from_rgb(45, 40, 20)
    } else if is_primary {
        Color32::from_rgb(28, 38, 60)
    } else {
        Color32::from_rgb(30, 36, 50)
    };

    let mut action = CardAction::None;

    egui::Frame::none()
        .fill(bg_color)
        .rounding(Rounding::same(8.0))
        .stroke(Stroke::new(if is_primary || is_identifying { 2.0 } else { 1.0 }, border_color))
        .inner_margin(Margin::same(14.0))
        .show(ui, |ui: &mut egui::Ui| {
            ui.horizontal(|ui| {
                // ── Left: info ──
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&monitor.name)
                                .font(FontId::proportional(20.0))
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        if is_primary {
                            ui.add_space(6.0);
                            badge(ui, "PRIMARY", Color32::from_rgb(50, 100, 200));
                        }
                        if is_identifying {
                            ui.add_space(6.0);
                            badge(ui, "IDENTIFYING", Color32::from_rgb(160, 120, 0));
                        }
                    });

                    ui.add_space(6.0);

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

                // ── Right: buttons ──
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
                            action = CardAction::SetPrimary;
                        }
                        ui.add_space(6.0);
                    }

                    if !is_identifying {
                        if ui
                            .add(
                                egui::Button::new(RichText::new("Identify").color(Color32::WHITE))
                                    .fill(Color32::from_rgb(80, 60, 10))
                                    .rounding(Rounding::same(6.0)),
                            )
                            .clicked()
                        {
                            action = CardAction::Identify;
                        }
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

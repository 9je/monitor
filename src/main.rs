mod display;
mod identify;
mod ui;

use eframe::egui;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();

    // Subprocess mode: monitor --identify INDEX NAME X Y W H
    if args.get(1).map(|s| s.as_str()) == Some("--identify") {
        return identify::run(&args);
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Monitor")
            .with_inner_size([640.0, 500.0])
            .with_min_inner_size([480.0, 300.0])
            .with_icon(load_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "Monitor",
        options,
        Box::new(|_cc| Ok(Box::new(ui::App::new()))),
    )
}

fn load_icon() -> egui::IconData {
    let size = 32usize;
    let mut rgba = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let i = (y * size + x) * 4;
            let is_border = x < 3 || x >= size - 3 || y < 3 || y >= size - 3;
            let is_stand = (13..19).contains(&x) && (26..30).contains(&y);
            let is_base = (8..24).contains(&x) && y >= 29;
            if is_border || is_stand || is_base {
                rgba[i] = 80;
                rgba[i + 1] = 140;
                rgba[i + 2] = 255;
                rgba[i + 3] = 255;
            } else {
                rgba[i] = 22;
                rgba[i + 1] = 27;
                rgba[i + 2] = 40;
                rgba[i + 3] = 255;
            }
        }
    }
    egui::IconData { rgba, width: size as u32, height: size as u32 }
}

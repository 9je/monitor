use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub enum Rotation {
    Normal,
    Left,
    Right,
    Inverted,
}

impl Rotation {
    pub fn label(&self) -> &str {
        match self {
            Rotation::Normal => "Normal",
            Rotation::Left => "90° Left",
            Rotation::Right => "90° Right",
            Rotation::Inverted => "Inverted",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Monitor {
    pub name: String,
    pub is_primary: bool,
    pub resolution: (u32, u32),
    pub position: (i32, i32),
    pub rotation: Rotation,
    pub refresh_rate: f32,
    pub physical_mm: Option<(u32, u32)>,
}

impl Monitor {
    /// Physical diagonal size in inches.
    pub fn size_inches(&self) -> Option<f32> {
        self.physical_mm.map(|(w, h)| {
            let w_in = w as f32 / 25.4;
            let h_in = h as f32 / 25.4;
            (w_in * w_in + h_in * h_in).sqrt()
        })
    }

    /// Resolution accounting for rotation (portrait vs landscape).
    pub fn logical_resolution(&self) -> (u32, u32) {
        match self.rotation {
            Rotation::Left | Rotation::Right => (self.resolution.1, self.resolution.0),
            _ => self.resolution,
        }
    }
}

/// Returns true if we're running inside a Flatpak sandbox.
fn is_flatpak() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
}

/// Build an xrandr Command, transparently proxying via flatpak-spawn when sandboxed.
fn xrandr() -> Command {
    if is_flatpak() {
        let mut cmd = Command::new("flatpak-spawn");
        cmd.args(["--host", "xrandr"]);
        cmd
    } else {
        Command::new("xrandr")
    }
}

/// Parse `xrandr --query` output into connected monitors.
pub fn get_monitors() -> Result<Vec<Monitor>, String> {
    let output = xrandr()
        .arg("--query")
        .output()
        .map_err(|e| format!("Failed to run xrandr: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    parse_xrandr(&String::from_utf8_lossy(&output.stdout))
}

/// Set the primary monitor by output name.
pub fn set_primary(name: &str) -> Result<(), String> {
    let status = xrandr()
        .args(["--output", name, "--primary"])
        .status()
        .map_err(|e| format!("Failed to run xrandr: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("xrandr exited with status {status}"))
    }
}

/// Identify one monitor by dimming all others to 20% brightness.
/// Call `restore_brightness` after a delay to undo this.
pub fn dim_others(target_name: &str, all: &[Monitor]) -> Result<(), String> {
    for m in all {
        let brightness = if m.name == target_name { "1.0" } else { "0.2" };
        xrandr()
            .args(["--output", &m.name, "--brightness", brightness])
            .status()
            .map_err(|e| format!("xrandr failed for {}: {e}", m.name))?;
    }
    Ok(())
}

/// Restore all monitors to full brightness.
pub fn restore_brightness(monitors: &[Monitor]) -> Result<(), String> {
    for m in monitors {
        xrandr()
            .args(["--output", &m.name, "--brightness", "1.0"])
            .status()
            .map_err(|e| format!("xrandr failed for {}: {e}", m.name))?;
    }
    Ok(())
}

// ── xrandr parsing ────────────────────────────────────────────────────────────

fn parse_xrandr(text: &str) -> Result<Vec<Monitor>, String> {
    let mut monitors = Vec::new();
    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        if !line.contains(" connected ") {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        let name = parts[0].to_string();
        let is_primary = line.contains(" primary ");
        let (resolution, position, rotation) = parse_geometry_token(&parts);
        let physical_mm = parse_physical_mm(line);

        let mut best_refresh = 0.0f32;

        while let Some(peek) = lines.peek() {
            if !peek.starts_with(' ') && !peek.starts_with('\t') {
                break;
            }
            let mode_line = lines.next().unwrap();
            if let Some(rates) = parse_mode_rates(mode_line) {
                if mode_line.contains('*') {
                    for r in &rates {
                        if *r > best_refresh {
                            best_refresh = *r;
                        }
                    }
                }
            }
        }

        monitors.push(Monitor {
            name,
            is_primary,
            resolution,
            position,
            rotation,
            refresh_rate: best_refresh,
            physical_mm,
        });
    }

    Ok(monitors)
}

fn parse_geometry_token(parts: &[&str]) -> ((u32, u32), (i32, i32), Rotation) {
    let mut resolution = (0, 0);
    let mut position = (0, 0);
    let mut rotation = Rotation::Normal;

    for part in parts {
        if part.contains('x') && (part.contains('+') || part.contains('-')) {
            if let Some((res, pos)) = parse_geometry(part) {
                resolution = res;
                position = pos;
            }
        }
        match *part {
            "left" => rotation = Rotation::Left,
            "right" => rotation = Rotation::Right,
            "inverted" => rotation = Rotation::Inverted,
            "normal" => rotation = Rotation::Normal,
            _ => {}
        }
    }

    (resolution, position, rotation)
}

fn parse_geometry(s: &str) -> Option<((u32, u32), (i32, i32))> {
    let x_pos = s.find('x')?;
    let width: u32 = s[..x_pos].parse().ok()?;

    let rest = &s[x_pos + 1..];
    let sep = rest.find(|c| c == '+' || c == '-')?;
    let height: u32 = rest[..sep].parse().ok()?;

    let coords = &rest[sep..];
    let parts: Vec<&str> = coords
        .split(|c| c == '+' || c == '-')
        .filter(|s| !s.is_empty())
        .collect();
    let signs: Vec<char> = coords.chars().filter(|c| *c == '+' || *c == '-').collect();

    if parts.len() < 2 || signs.len() < 2 {
        return None;
    }

    let x: i32 = format!("{}{}", signs[0], parts[0]).parse().ok()?;
    let y: i32 = format!("{}{}", signs[1], parts[1]).parse().ok()?;

    Some(((width, height), (x, y)))
}

fn parse_physical_mm(line: &str) -> Option<(u32, u32)> {
    let mm_idx = line.find("mm x ")?;
    let before = &line[..mm_idx];
    let w: u32 = before.split_whitespace().last()?.parse().ok()?;
    let after = &line[mm_idx + 5..];
    let h: u32 = after
        .split_whitespace()
        .next()?
        .trim_end_matches("mm")
        .parse()
        .ok()?;
    Some((w, h))
}

fn parse_mode_rates(line: &str) -> Option<Vec<f32>> {
    let line = line.trim();
    if line.is_empty() || !line.contains('x') {
        return None;
    }
    let mut parts = line.split_whitespace();
    parts.next(); // skip WxH
    let rates: Vec<f32> = parts
        .filter_map(|r| {
            let cleaned: String = r.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
            cleaned.parse().ok()
        })
        .collect();
    if rates.is_empty() { None } else { Some(rates) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"Screen 0: minimum 320 x 200, current 5560 x 3169, maximum 16384 x 16384
DisplayPort-0 connected primary 1080x1920+4480+1249 left 527mm x 296mm
   1920x1080     60.00*+  50.00    59.94
   1280x720      60.00    50.00    59.94
DisplayPort-1 connected 2560x1440+1920+0 (normal left inverted right x axis y axis) 620mm x 370mm
   2560x1440    100.00*+  75.00    60.00
   1920x1080    100.00    60.00
DisplayPort-2 connected 2560x1440+1920+1440 (normal left inverted right x axis y axis) 597mm x 336mm
   2560x1440    240.00*+ 165.00    60.00
"#;

    #[test]
    fn test_parse_three_monitors() {
        let monitors = parse_xrandr(SAMPLE).unwrap();
        assert_eq!(monitors.len(), 3);

        let dp0 = &monitors[0];
        assert_eq!(dp0.name, "DisplayPort-0");
        assert!(dp0.is_primary);
        assert_eq!(dp0.resolution, (1080, 1920));
        assert_eq!(dp0.position, (4480, 1249));
        assert!(matches!(dp0.rotation, Rotation::Left));
        assert!((dp0.refresh_rate - 60.0).abs() < 0.1);
        assert_eq!(dp0.physical_mm, Some((527, 296)));

        let dp1 = &monitors[1];
        assert_eq!(dp1.name, "DisplayPort-1");
        assert!(!dp1.is_primary);
        assert_eq!(dp1.resolution, (2560, 1440));
        assert!((dp1.refresh_rate - 100.0).abs() < 0.1);

        let dp2 = &monitors[2];
        assert!((dp2.refresh_rate - 240.0).abs() < 0.1);
    }
}

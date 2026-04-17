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
    pub available_modes: Vec<(u32, u32, Vec<f32>)>, // (w, h, refresh_rates)
}

impl Monitor {
    /// Physical size in inches (diagonal)
    pub fn size_inches(&self) -> Option<f32> {
        self.physical_mm.map(|(w, h)| {
            let w_in = w as f32 / 25.4;
            let h_in = h as f32 / 25.4;
            (w_in * w_in + h_in * h_in).sqrt()
        })
    }

    /// Resolution accounting for rotation
    pub fn logical_resolution(&self) -> (u32, u32) {
        match self.rotation {
            Rotation::Left | Rotation::Right => (self.resolution.1, self.resolution.0),
            _ => self.resolution,
        }
    }
}

/// Parse `xrandr --query` output into a list of connected monitors.
pub fn get_monitors() -> Result<Vec<Monitor>, String> {
    let output = Command::new("xrandr")
        .arg("--query")
        .output()
        .map_err(|e| format!("Failed to run xrandr: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    parse_xrandr(&text)
}

/// Set the primary monitor by name.
pub fn set_primary(name: &str) -> Result<(), String> {
    let status = Command::new("xrandr")
        .args(["--output", name, "--primary"])
        .status()
        .map_err(|e| format!("Failed to run xrandr: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("xrandr exited with status {status}"))
    }
}

fn parse_xrandr(text: &str) -> Result<Vec<Monitor>, String> {
    let mut monitors = Vec::new();
    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        // Match connected output lines, e.g.:
        //   DisplayPort-0 connected primary 1080x1920+4480+1249 left 527mm x 296mm
        //   DisplayPort-1 connected 2560x1440+1920+0 (normal ...) 620mm x 370mm
        if !line.contains(" connected ") {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        let name = parts[0].to_string();

        let is_primary = line.contains(" primary ");

        // Find the geometry token: WxH+X+Y
        let (resolution, position, rotation) = parse_geometry_token(&parts);

        // Physical size: look for "NNNmm x NNNmm"
        let physical_mm = parse_physical_mm(line);

        // Now parse mode lines until next output line
        let mut best_refresh = 0.0f32;
        let mut available_modes = Vec::new();

        while let Some(peek) = lines.peek() {
            // Mode lines start with whitespace
            if !peek.starts_with(' ') && !peek.starts_with('\t') {
                break;
            }
            let mode_line = lines.next().unwrap();
            if let Some((w, h, rates)) = parse_mode_line(mode_line) {
                // The active refresh rate is marked with *
                if mode_line.contains('*') {
                    for r in &rates {
                        if *r > best_refresh {
                            best_refresh = *r;
                        }
                    }
                }
                available_modes.push((w, h, rates));
            }
        }

        // If we couldn't determine refresh from mode lines, leave 0.0
        monitors.push(Monitor {
            name,
            is_primary,
            resolution,
            position,
            rotation,
            refresh_rate: best_refresh,
            physical_mm,
            available_modes,
        });
    }

    Ok(monitors)
}

/// Parse geometry from xrandr words, returning (resolution, position, rotation).
fn parse_geometry_token(parts: &[&str]) -> ((u32, u32), (i32, i32), Rotation) {
    let mut resolution = (0, 0);
    let mut position = (0, 0);
    let mut rotation = Rotation::Normal;

    for part in parts {
        // Geometry: WxH+X+Y or WxH+X-Y etc.
        if part.contains('x') && (part.contains('+') || part.contains('-')) {
            if let Some((res, pos)) = parse_geometry(part) {
                resolution = res;
                position = pos;
            }
        }
        // Rotation keyword
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

/// Parse "WxH+X+Y" into ((w,h),(x,y)).
fn parse_geometry(s: &str) -> Option<((u32, u32), (i32, i32))> {
    // Split on 'x' to get width, then find +/- for height/x/y
    let x_pos = s.find('x')?;
    let width: u32 = s[..x_pos].parse().ok()?;

    // After 'x', find first + or -
    let rest = &s[x_pos + 1..];
    // height ends at first + or -
    let sep = rest.find(|c| c == '+' || c == '-')?;
    let height: u32 = rest[..sep].parse().ok()?;

    // X and Y with signs
    let coords = &rest[sep..];
    // coords looks like +4480+1249 or +1920-0
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

/// Parse "NNNmm x NNNmm" from a line.
fn parse_physical_mm(line: &str) -> Option<(u32, u32)> {
    // Look for pattern like "527mm x 296mm"
    let mm_idx = line.find("mm x ")?;
    // Walk back to find the number before "mm"
    let before = &line[..mm_idx];
    let w_str = before.split_whitespace().last()?;
    let w: u32 = w_str.parse().ok()?;

    let after = &line[mm_idx + 5..]; // skip "mm x "
    let h_str = after.split_whitespace().next()?;
    let h_str = h_str.trim_end_matches("mm");
    let h: u32 = h_str.parse().ok()?;

    Some((w, h))
}

/// Parse a mode line like "   1920x1080     60.00*+  50.00    59.94"
fn parse_mode_line(line: &str) -> Option<(u32, u32, Vec<f32>)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let mut parts = line.split_whitespace();
    let res_str = parts.next()?;

    let x_pos = res_str.find('x')?;
    let w: u32 = res_str[..x_pos].parse().ok()?;
    let h: u32 = res_str[x_pos + 1..].parse().ok()?;

    let rates: Vec<f32> = parts
        .filter_map(|r| {
            // Strip trailing markers (* + )
            let cleaned: String = r.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
            cleaned.parse().ok()
        })
        .collect();

    if rates.is_empty() {
        return None;
    }

    Some((w, h, rates))
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

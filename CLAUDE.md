# CLAUDE.md — Monitor

Guidelines for working in this codebase with Claude Code.

## Project overview

**Monitor** is a Rust/egui desktop app for Pop OS that lets you:
- See all connected monitors with port names, resolutions, refresh rates, and physical sizes
- Set the primary (default) monitor with one click
- Identify which physical screen is which port by dimming all other monitors (xrandr brightness)

Backend: `xrandr` (X11). Flatpak-aware: detects sandbox and proxies xrandr via `flatpak-spawn --host`.

## Architecture

```
src/
├── main.rs       Entry point. Sets up eframe NativeOptions and launches App.
├── display.rs    Pure data layer. Parses xrandr output, runs xrandr commands.
│                 No egui imports. All xrandr calls go through xrandr() helper
│                 which transparently uses flatpak-spawn when sandboxed.
└── ui.rs         All egui rendering. App struct owns monitor state.
                  Identify spawns background threads for timed brightness restore.

data/
├── io.github._9je.Monitor.desktop      App launcher entry
└── io.github._9je.Monitor.metainfo.xml AppStream metadata for Flatpak/Flathub

flatpak/
└── io.github._9je.Monitor.yml          Flatpak manifest

install.sh    Local install: builds release binary + installs .desktop file
```

**Key rule:** `display.rs` never imports `egui`/`eframe`. `ui.rs` never calls `Command` directly — always go through `display.rs`.

## Build & run

```bash
cargo build           # debug build
cargo run             # run (requires X11 session)
cargo test            # unit tests (no display needed)
cargo build --release # optimized binary
```

## After every code change — always do all three

```bash
bash install.sh       # rebuilds release binary + updates ~/.local/bin/monitor
git add -A && git commit -m "..." && git push
DISPLAY=:1 /home/admin/.local/bin/monitor &   # relaunch to verify
```

Never leave a session where code was changed but install.sh wasn't run and GitHub wasn't updated.

## Install to app launcher (local, no Flatpak)

```bash
bash install.sh
```

Copies the release binary to `~/.local/bin/monitor` and creates a `.desktop` file so Monitor appears in the GNOME app launcher.

## Flatpak build (local)

```bash
# Install tools
sudo apt install flatpak-builder
flatpak install flathub org.freedesktop.Platform//23.08 org.freedesktop.Sdk//23.08
flatpak install flathub org.freedesktop.Sdk.Extension.rust-stable//23.08

# Vendor cargo deps first (needed for offline Flatpak build)
cargo vendor vendor
mkdir -p .cargo
cat > .cargo/config.toml << 'EOF'
[source.crates-io]
replace-with = "vendored-sources"
[source.vendored-sources]
directory = "vendor"
EOF

# Build and install locally
flatpak-builder --install --user --force-clean build-flatpak flatpak/io.github._9je.Monitor.yml
```

## Identify feature

The identify feature uses `xrandr --brightness` to dim all monitors except the target to 20%.
After 3 seconds a background thread restores all to 100%. This approach is chosen over
overlay windows because it works regardless of window manager behavior or monitor layout.

## Adding Wayland support

1. Detect session: `std::env::var("WAYLAND_DISPLAY").is_ok()`
2. Add a `Backend` enum to `display.rs` with `Xrandr` and `GnomeRandr` variants
3. For GNOME Wayland: use `gdbus call --session --dest org.gnome.Mutter.DisplayConfig ...`
   or the `gnome-randr` CLI if available
4. `App::new()` selects backend; `ui.rs` stays unchanged

## Testing

- Unit tests for xrandr parsing live in `display.rs` under `#[cfg(test)]`
- Tests use inline sample strings — no real display required
- Add a test case when adding new xrandr parse logic
- Run `cargo test` before committing

## Code style

- `cargo fmt` before committing
- `cargo clippy -- -D warnings` before committing
- No `unwrap()` in `display.rs` — return `Result<_, String>`
- `ui.rs` shows errors in the status bar via `self.status = Some((msg, true))`
- Prefer descriptive error strings over panic

## egui/eframe notes (v0.29)

- `Frame::none()` not `Frame::new()` — the `new()` method doesn't exist in 0.29
- `Margin::same()`, `Rounding::same()` take `f32` — use `8.0` not `8`
- Closure params in `.show()` sometimes need explicit `|ui: &mut egui::Ui|` annotation

## Common xrandr commands this app replaces

```bash
xrandr --query                              # list monitors
xrandr --output DisplayPort-1 --primary    # set primary
xrandr --output DisplayPort-0 --brightness 0.2  # dim
```

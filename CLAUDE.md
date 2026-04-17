# CLAUDE.md — monitour

Guidelines for working in this codebase with Claude Code.

## Project overview

**monitour** is a Rust/egui desktop app for Pop OS that lets you:
- See all connected monitors with their port names, resolutions, refresh rates, and physical sizes
- Set the primary (default) monitor with one click
- Identify which physical screen corresponds to which port by opening a fullscreen overlay on each monitor

The backend is `xrandr` (X11). Wayland support via `gnome-randr`/DBus is a planned future addition.

## Architecture

```
src/
├── main.rs       Entry point. Sets up eframe NativeOptions and launches the app.
├── display.rs    Pure data layer. Parses xrandr output, runs xrandr commands.
│                 No egui imports here — only std and process::Command.
└── ui.rs         All egui rendering. Calls display.rs functions.
                  App struct owns monitor state and refresh logic.
```

**Key rule:** `display.rs` must never import `egui` or `eframe`. `ui.rs` must never shell out directly — always go through `display.rs` functions.

## Build & run

```bash
cargo build           # debug build
cargo run             # run the app (requires a running X11 session)
cargo test            # unit tests (no display required — tests parse xrandr text)
cargo build --release # optimized binary in target/release/monitour
```

## Adding a new backend (Wayland/gnome-randr)

1. Detect session type at startup: `std::env::var("WAYLAND_DISPLAY").is_ok()`
2. Add a `Backend` trait to `display.rs` with `get_monitors()` and `set_primary()` methods
3. Implement `XrandrBackend` and `GnomeRandrBackend` as separate structs
4. `App::new()` selects the right backend and stores it as `Box<dyn Backend>`
5. `ui.rs` stays unchanged — it only calls `display.rs` API

## Testing

- Unit tests for xrandr parsing live in `display.rs` under `#[cfg(test)]`
- Tests use inline sample strings, no real display required
- When adding new parse logic, add a corresponding test case with a real xrandr snippet
- Run `cargo test` before committing

## Code style

- Follow standard `rustfmt` formatting (`cargo fmt`)
- Lint with `cargo clippy -- -D warnings` before committing
- No `unwrap()` in `display.rs` — return `Result<_, String>` and propagate errors to `ui.rs`
- `ui.rs` may `unwrap_or_default()` on display results and show errors in the status bar
- Prefer descriptive error strings over panic

## egui / eframe version

Currently pinned to **egui 0.29 / eframe 0.29**. When upgrading:
- Check `Frame::none()` vs `Frame::new()` API (changed in 0.28 → 0.29)
- `Margin::same()` and `Rounding::same()` take `f32`, not integer literals
- Multi-viewport API (`show_viewport_deferred`) was introduced in 0.29

## Release / distribution

To install system-wide after a release build:
```bash
sudo cp target/release/monitour /usr/local/bin/
```

To add a desktop launcher, create `~/.local/share/applications/monitour.desktop`.

## Common xrandr commands this app replaces

```bash
# List monitors
xrandr --query

# Set primary
xrandr --output DisplayPort-1 --primary
```

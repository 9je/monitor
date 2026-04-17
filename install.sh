#!/usr/bin/env bash
# Install monitor locally so it appears in your app launcher.
# Run from the project root: bash install.sh

set -e

echo "Building release binary..."
cargo build --release

BINARY="$HOME/.local/bin/monitor"
DESKTOP="$HOME/.local/share/applications/io.github._9je.Monitor.desktop"

mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications"

cp target/release/monitor "$BINARY"
chmod +x "$BINARY"

cp data/io.github._9je.Monitor.desktop "$DESKTOP"
# Patch Exec path to full binary location in case ~/.local/bin isn't in PATH
sed -i "s|^Exec=monitor|Exec=$BINARY|" "$DESKTOP"

# Refresh app database
update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true

echo ""
echo "Done! 'Monitor' should now appear in your app launcher."
echo "Binary: $BINARY"
echo ""
echo "To uninstall:"
echo "  rm $BINARY $DESKTOP"

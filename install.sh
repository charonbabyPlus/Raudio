#!/usr/bin/env bash
# Install Raudio into the current user's ~/.local so it shows up in the app
# menu / taskbar with its name and icon. Run: ./install.sh
set -euo pipefail

APP_ID="com.raudio.Raudio"
PREFIX="${XDG_DATA_HOME:-$HOME/.local/share}"
BIN_DIR="$HOME/.local/bin"
HERE="$(cd "$(dirname "$0")" && pwd)"

echo "Building release binary…"
cargo build --release

echo "Installing binary…"
install -Dm755 "$HERE/target/release/raudio" "$BIN_DIR/raudio"

echo "Installing icon…"
# Install the icon at the standard hicolor sizes, named after the app id so the
# window/taskbar find it. Resize with ffmpeg when available, else copy as-is.
for size in 48 64 128 256 512; do
    dst="$PREFIX/icons/hicolor/${size}x${size}/apps/$APP_ID.png"
    mkdir -p "$(dirname "$dst")"
    if command -v ffmpeg >/dev/null 2>&1; then
        ffmpeg -y -loglevel error -i "$HERE/assets/icon.png" \
            -vf "scale=${size}:${size}:flags=lanczos" "$dst"
    else
        install -m644 "$HERE/assets/icon.png" "$dst"
    fi
done

echo "Installing desktop entry…"
mkdir -p "$PREFIX/applications"
cat > "$PREFIX/applications/$APP_ID.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Raudio
Comment=Music player with playlists and liked songs
Exec=$BIN_DIR/raudio
Icon=$APP_ID
Terminal=false
Categories=AudioVideo;Audio;Player;
StartupWMClass=$APP_ID
EOF

# Refresh caches (best-effort; ignore if the tools are missing).
gtk-update-icon-cache -f -t "$PREFIX/icons/hicolor" >/dev/null 2>&1 || true
update-desktop-database "$PREFIX/applications" >/dev/null 2>&1 || true

echo
echo "Done. Launch 'Raudio' from your app menu, or run: $BIN_DIR/raudio"
echo "(Make sure $BIN_DIR is on your PATH to run it by name.)"

#!/usr/bin/env bash
# Remove a Raudio installation done by install.sh. By default it keeps your
# library; pass --purge to also delete it. Run: ./uninstall.sh [--purge]
set -euo pipefail

APP_ID="com.raudio.Raudio"
PREFIX="${XDG_DATA_HOME:-$HOME/.local/share}"
BIN_DIR="$HOME/.local/bin"

echo "Removing binary, icons and desktop entry…"
rm -f "$BIN_DIR/raudio"
rm -f "$PREFIX/applications/$APP_ID.desktop"
for size in 48 64 128 256 512; do
    rm -f "$PREFIX/icons/hicolor/${size}x${size}/apps/$APP_ID.png"
done

if [[ "${1:-}" == "--purge" ]]; then
    rm -rf "$PREFIX/raudio"
    echo "Also removed your library and covers ($PREFIX/raudio)."
fi

# Refresh caches (best-effort).
gtk-update-icon-cache -f -t "$PREFIX/icons/hicolor" >/dev/null 2>&1 || true
update-desktop-database "$PREFIX/applications" >/dev/null 2>&1 || true

echo "Raudio uninstalled."
if [[ "${1:-}" != "--purge" ]]; then
    echo "Your library was kept — re-run with --purge to delete it too."
fi

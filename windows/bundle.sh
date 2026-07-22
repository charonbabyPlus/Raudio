#!/usr/bin/env bash
# Assemble a redistributable Windows folder for Raudio.
#
# Run inside the **MSYS2 MinGW64** shell, from the project root, after:
#   cargo build --release --no-default-features   # pure GTK4, no libadwaita
#
# Produces dist/raudio/ containing raudio.exe plus every DLL, GStreamer plugin,
# gdk-pixbuf loader and GLib schema it needs, so it runs on a machine without
# GTK installed. Bundling on Windows is fiddly — treat this as a solid starting
# point and add anything a first run reports as missing.
set -euo pipefail

MINGW="${MINGW_PREFIX:-/mingw64}"
BIN="target/release/raudio.exe"
OUT="dist/raudio"

[ -f "$BIN" ] || { echo "Build first: cargo build --release"; exit 1; }

rm -rf "$OUT"
mkdir -p "$OUT"
cp "$BIN" "$OUT/"

echo "Copying dependent DLLs…"
copy_deps() {
    ldd "$1" | awk '{print $3}' | grep -iF "$MINGW" | while read -r dll; do
        [ -f "$dll" ] || continue
        base=$(basename "$dll")
        [ -f "$OUT/$base" ] && continue
        cp "$dll" "$OUT/"
        copy_deps "$dll"
    done
}
copy_deps "$BIN"

echo "Copying GStreamer plugins (audio) and their DLLs…"
mkdir -p "$OUT/lib/gstreamer-1.0"
cp "$MINGW"/lib/gstreamer-1.0/*.dll "$OUT/lib/gstreamer-1.0/" 2>/dev/null || true
for p in "$OUT"/lib/gstreamer-1.0/*.dll; do [ -f "$p" ] && copy_deps "$p"; done

echo "Copying gdk-pixbuf loaders (image decoding for cover art)…"
cp -r "$MINGW/lib/gdk-pixbuf-2.0" "$OUT/lib/" 2>/dev/null || true

echo "Copying compiled GLib schemas…"
mkdir -p "$OUT/share/glib-2.0/schemas"
cp "$MINGW/share/glib-2.0/schemas/gschemas.compiled" \
   "$OUT/share/glib-2.0/schemas/" 2>/dev/null || true

echo
echo "Done -> $OUT/"
echo "Zip that folder to distribute. (Icons + stylesheet are already embedded"
echo "in the exe via GResource, so no separate assets are needed.)"

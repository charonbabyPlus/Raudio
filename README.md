# VIBECODE AI slop 

# Raudio

A small GTK4 / libadwaita music player written in Rust — playlists, single
tracks, liked songs, embedded cover art, and switchable colour themes.

![icon](icon.png)

## Features

- Scan a folder and read tags (title / artist / album / duration / cover) via `lofty`
- Playback through GStreamer (play / pause / seek / next / prev / volume)
- Shuffle and repeat (off / all / one)
- Liked songs and user playlists (with custom cover images)
- Live search, smooth progress bar, 7 colour themes
- Remembers your theme and volume between runs

## Dependencies

You need Rust (stable) and the GTK4 / libadwaita / GStreamer development
libraries, plus a C toolchain (for the bundled SQLite).

**Arch Linux:**

```sh
sudo pacman -S --needed rust base-devel pkgconf \
    gtk4 libadwaita \
    gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad
```

(`gst-plugins-good` / `-bad` provide MP3/AAC/FLAC/etc. decoders.)

**Debian / Ubuntu:**

```sh
sudo apt install build-essential pkg-config cargo \
    libgtk-4-dev libadwaita-1-dev \
    libgstreamer1.0-dev gstreamer1.0-plugins-base \
    gstreamer1.0-plugins-good gstreamer1.0-plugins-bad
```

**Fedora:**

```sh
sudo dnf install cargo gtk4-devel libadwaita-devel \
    gstreamer1-devel gstreamer1-plugins-base-devel \
    gstreamer1-plugins-good gstreamer1-plugins-bad-free
```

## Build & run

```sh
cargo run --release
```

## Install (menu entry + icon)

Installs the binary, icon, and a desktop entry under `~/.local` so "Raudio"
shows up in your application menu:

```sh
./install.sh
```

Then launch **Raudio** from your app menu, or run `~/.local/bin/raudio`
(ensure `~/.local/bin` is on your `PATH`).

# VIBECODE AI slop 

# Raudio

A small GTK4 music player written in Rust — playlists, single tracks, liked
songs, embedded cover art, and switchable colour themes. Runs on Linux and
Windows.

![icon](assets/icon.png)

## Features

- Scan a folder and read tags (title / artist / album / duration / cover) via `lofty`
- Playback through GStreamer (play / pause / seek / next / prev / volume)
- Shuffle and repeat (off / all / one)
- Liked songs and user playlists (with custom cover images)
- Live search, smooth progress bar, 7 colour themes
- Remembers your theme and volume between runs

## Dependencies

You need Rust (stable) and the GTK4 / libadwaita / GStreamer development
libraries, plus a C toolchain (for the bundled SQLite). On Linux libadwaita
gives the refined look; on Windows the app builds without it (pure GTK4).

**Arch Linux:**

```sh
sudo pacman -S --needed rust base-devel pkgconf \
    gtk4 libadwaita \
    gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad
```

(`gst-plugins-good` / `-bad` provide MP3/AAC/FLAC/etc. decoders.)

**Optional:** `yt-dlp` (+ `ffmpeg`) enables *Add music → Add from link…*, which
extracts a link's audio into your library. Only download content you have the
right to. On Arch: `sudo pacman -S --needed yt-dlp ffmpeg`.

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

## Uninstall

```sh
./uninstall.sh           # removes the app, keeps your library
./uninstall.sh --purge   # also deletes your library and covers
```

## Windows

On Windows the app builds **without libadwaita** (`--no-default-features`),
staying pure GTK4. Build via **MSYS2**.

1. Install [MSYS2](https://www.msys2.org/), open the **MinGW64** shell, and get
   the toolchain + libraries (no libadwaita needed):

   ```sh
   pacman -S --needed mingw-w64-x86_64-rust mingw-w64-x86_64-gtk4 \
       mingw-w64-x86_64-gstreamer mingw-w64-x86_64-gst-plugins-base \
       mingw-w64-x86_64-gst-plugins-good mingw-w64-x86_64-gst-plugins-bad \
       mingw-w64-x86_64-pkgconf
   ```

2. Build and run (pure GTK4):

   ```sh
   cargo run --release --no-default-features
   ```

3. Make a redistributable folder (bundles the exe + all DLLs, GStreamer
   plugins, image loaders and schemas — icons/CSS are already embedded):

   ```sh
   ./windows/bundle.sh      # writes dist/raudio/
   ```

   Zip `dist/raudio/` to share. `yt-dlp.exe` (for *Add from link*) is optional
   and can be dropped next to `raudio.exe`.

4. *(Optional)* Build a proper installer with
   [Inno Setup](https://jrsoftware.org/isinfo.php) — it wraps `dist/raudio/`
   into `dist/raudio-setup.exe` with Start-menu / desktop shortcuts:

   ```
   "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" windows\raudio.iss
   ```

The app icon is embedded into `raudio.exe` (via `winresource` at build time),
so Explorer, the taskbar and shortcuts show it.

## Project layout

```
src/         Rust sources (main, window, player, library, scanner, theme)
resources/   style.css + bundled symbolic icon pack, embedded via GResource
assets/      application icon
build.rs     compiles resources/ into the binary
```

The in-app symbolic icons are bundled from the **Adwaita** icon theme
(GNOME Project, CC BY-SA / GPL) so the UI looks the same regardless of the
system icon theme.

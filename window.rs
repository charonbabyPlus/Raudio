use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::MessageDialogExt;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use rusqlite::Connection;

use crate::library::{self, Track};
use crate::player::Player;
use crate::scanner;
use crate::theme;

/// How playback behaves when a track finishes.
#[derive(Clone, Copy, PartialEq)]
enum Repeat {
    Off,
    All,
    One,
}

/// Which collection the main track list is showing.
#[derive(Clone, Copy, PartialEq)]
enum View {
    All,
    Liked,
    Playlist(i64),
}

/// Handles the UI needs to reach after construction: playback, the database,
/// the track list and the now-playing labels. Cloning shares the same state,
/// so it can be moved into GTK signal closures freely.
#[derive(Clone)]
struct Ui {
    conn: Rc<Connection>,
    player: Player,
    tracks: Rc<RefCell<Vec<Track>>>,
    list: gtk::ListBox,
    /// Scroller wrapping `list`; kept so deletes can preserve scroll position.
    track_scroller: gtk::ScrolledWindow,
    np_cover: gtk::Box,
    np_title: gtk::Label,
    np_artist: gtk::Label,
    play_btn: gtk::Button,
    add_btn: gtk::Button,
    new_playlist_btn: gtk::Button,
    progress: gtk::Scale,
    time_pos: gtk::Label,
    time_dur: gtk::Label,
    hero_title: gtk::Label,
    hero_sub: gtk::Label,
    hero_art: gtk::Box,
    vol_scale: gtk::Scale,
    /// The sidebar list of "Liked Songs" + user playlists.
    sidebar_list: gtk::ListBox,
    /// Playlist ids matching sidebar rows 1.. (row 0 is Liked Songs).
    playlist_ids: Rc<RefCell<Vec<i64>>>,
    is_playing: Rc<Cell<bool>>,
    /// The tracks being played through — independent of the displayed list, so
    /// browsing another playlist doesn't hijack next/prev.
    queue: Rc<RefCell<Vec<Track>>>,
    /// Id of the currently playing track, for highlighting it in any view.
    playing_id: Rc<Cell<Option<i64>>>,
    /// Guards against the end-of-track handler firing twice (EOS + fallback).
    ended: Rc<Cell<bool>>,
    /// Interpolation anchor for smooth progress: a known position (seconds) and
    /// the wall-clock instant it was sampled at (None when paused).
    anchor_pos: Rc<Cell<f64>>,
    anchor_time: Rc<Cell<Option<std::time::Instant>>>,
    /// Index into `tracks` of the row currently loaded, if any.
    current: Rc<Cell<Option<usize>>>,
    /// Which collection is currently shown.
    view: Rc<Cell<View>>,
    /// The live search query filtering the visible rows.
    query: Rc<RefCell<String>>,
    /// Cache of decoded per-track cover textures, keyed by file path.
    cover_cache: Rc<RefCell<HashMap<String, Option<gdk::Texture>>>>,
    /// Pick the next track at random when advancing.
    shuffle: Rc<Cell<bool>>,
    /// What to do at the end of a track.
    repeat: Rc<Cell<Repeat>>,
}

/// Assemble the full window: sidebar | main content, with a now-playing bar
/// pinned across the bottom.
pub fn build(app: &adw::Application) -> adw::ApplicationWindow {
    let ui = Ui {
        conn: Rc::new(open_library()),
        player: Player::new(),
        tracks: Rc::new(RefCell::new(Vec::new())),
        list: gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(vec!["tracks"])
            .build(),
        track_scroller: gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build(),
        np_cover: {
            let c = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            c.append(&cover_widget(false, None));
            c
        },
        np_title: gtk::Label::builder().xalign(0.0).css_classes(vec!["track-title"]).build(),
        np_artist: gtk::Label::builder().xalign(0.0).css_classes(vec!["track-artist"]).build(),
        play_btn: gtk::Button::from_icon_name("media-playback-start-symbolic"),
        add_btn: nav_button("list-add-symbolic", "Add music", false),
        new_playlist_btn: {
            let b = gtk::Button::from_icon_name("list-add-symbolic");
            b.add_css_class("flat");
            b.add_css_class("add-playlist");
            b.set_tooltip_text(Some("New playlist"));
            b
        },
        progress: gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.001),
        time_pos: gtk::Label::builder().label("0:00").css_classes(vec!["time"]).build(),
        time_dur: gtk::Label::builder().label("0:00").css_classes(vec!["time"]).build(),
        hero_title: gtk::Label::builder().label("All Songs").xalign(0.0).css_classes(vec!["hero-title"]).build(),
        hero_sub: gtk::Label::builder().xalign(0.0).css_classes(vec!["hero-sub"]).build(),
        hero_art: {
            let holder = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            holder.set_size_request(180, 180);
            holder.set_halign(gtk::Align::Start);
            holder.set_valign(gtk::Align::Center);
            // Explicit false so a child's expand can't grow the holder past 180.
            holder.set_hexpand(false);
            holder.set_vexpand(false);
            holder.set_overflow(gtk::Overflow::Hidden);
            holder.add_css_class("hero-art");
            holder
        },
        vol_scale: gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.01),
        sidebar_list: gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(vec!["playlists"])
            .build(),
        playlist_ids: Rc::new(RefCell::new(Vec::new())),
        is_playing: Rc::new(Cell::new(false)),
        queue: Rc::new(RefCell::new(Vec::new())),
        playing_id: Rc::new(Cell::new(None)),
        ended: Rc::new(Cell::new(false)),
        anchor_pos: Rc::new(Cell::new(0.0)),
        anchor_time: Rc::new(Cell::new(None)),
        current: Rc::new(Cell::new(None)),
        view: Rc::new(Cell::new(View::All)),
        query: Rc::new(RefCell::new(String::new())),
        cover_cache: Rc::new(RefCell::new(HashMap::new())),
        shuffle: Rc::new(Cell::new(false)),
        repeat: Rc::new(Cell::new(Repeat::Off)),
    };
    ui.play_btn.add_css_class("play-mid");
    ui.track_scroller.set_child(Some(&ui.list));

    // Double-click / Enter on a row starts that track.
    ui.list.connect_row_activated(glib::clone!(
        #[strong] ui,
        move |_, row| ui.play_index(row.index() as usize)
    ));

    // Row visibility follows the search query. Row index maps 1:1 to `tracks`,
    // so filtering can look the track up by position.
    ui.list.set_filter_func(glib::clone!(
        #[strong] ui,
        move |row| {
            let query = ui.query.borrow().to_lowercase();
            if query.is_empty() {
                return true;
            }
            match ui.tracks.borrow().get(row.index() as usize) {
                Some(t) => {
                    t.title.to_lowercase().contains(&query)
                        || t.artist.to_lowercase().contains(&query)
                        || t.album.to_lowercase().contains(&query)
                }
                None => true,
            }
        }
    ));

    // A flat, full-width header bar gives the window a draggable top strip and
    // the standard KDE window controls, without breaking the card look below.
    let header = adw::HeaderBar::builder().css_classes(vec!["flat"]).build();

    // Search box lives in the title area and filters the visible rows live.
    let search = gtk::SearchEntry::builder().placeholder_text("Search").width_request(280).build();
    search.connect_search_changed(glib::clone!(
        #[strong] ui,
        move |entry| {
            *ui.query.borrow_mut() = entry.text().to_string();
            ui.list.invalidate_filter();
        }
    ));
    header.set_title_widget(Some(&search));

    // Theme picker: swatches only, no names.
    header.pack_end(&build_theme_menu(&ui));

    // Floating-card layout: a padded window background showing through 10px
    // gaps between the rail, the content, and the now-playing bar.
    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .vexpand(true)
        .build();
    root.set_margin_bottom(10);
    root.set_margin_start(10);
    root.set_margin_end(10);

    let sidebar = build_sidebar(&ui);
    sidebar.set_vexpand(true);
    sidebar.set_valign(gtk::Align::Fill);
    sidebar.set_size_request(200, -1);
    sidebar.set_margin_end(5);

    let content = build_content(&ui);
    content.set_hexpand(true);
    content.set_vexpand(true);
    content.set_valign(gtk::Align::Fill);
    content.set_size_request(360, -1);
    content.set_margin_start(5);

    // A draggable divider lets the user resize the sidebar. `shrink` off keeps
    // each panel above its minimum size.
    let top = gtk::Paned::builder()
        .orientation(gtk::Orientation::Horizontal)
        .start_child(&sidebar)
        .end_child(&content)
        .position(264)
        .resize_start_child(false)
        .shrink_start_child(false)
        .shrink_end_child(false)
        .vexpand(true)
        .build();

    root.append(&top);
    root.append(&build_now_playing(&ui));

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.append(&header);
    outer.append(&root);

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Raudio")
        .default_width(1180)
        .default_height(760)
        .content(&outer)
        .build();
    // Ties the window to the installed icon (named after the app id).
    gtk::Window::set_default_icon_name("com.raudio.Raudio");

    // Wire the folder picker and the playlist creator now that we have a
    // window to parent their dialogs on.
    wire_add_music(&ui, &window);
    wire_new_playlist(&ui, &window);

    // Decide what happens when the current track ends. Defer to an idle handler
    // so we don't re-drive the pipeline state from inside its own bus callback.
    ui.player.connect_eos(glib::clone!(
        #[strong] ui,
        move || {
            let ui = ui.clone();
            glib::idle_add_local_once(move || ui.on_track_end());
        }
    ));

    // Drive the progress bar from the frame clock so the knob glides smoothly
    // (~60 fps) instead of jumping on a coarse timer.
    ui.progress.add_tick_callback(glib::clone!(
        #[strong] ui,
        move |_, _| {
            ui.tick_progress();
            glib::ControlFlow::Continue
        }
    ));

    // Restore the persisted theme and volume.
    if let Some(i) = library::get_setting(&ui.conn, "theme").and_then(|v| v.parse::<usize>().ok()) {
        theme::set(i);
    }
    let vol = library::get_setting(&ui.conn, "volume")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.7);
    ui.vol_scale.set_value(vol);
    ui.player.set_volume(vol);

    ui.refresh_playlists();
    ui.refresh_tracks();
    window
}

impl Ui {
    /// Reload the track list for the current view, updating the hero header.
    /// Playback state is reset since row indices no longer line up.
    fn refresh_tracks(&self) {
        while let Some(row) = self.list.row_at_index(0) {
            self.list.remove(&row);
        }
        let view = self.view.get();
        let tracks = match view {
            View::All => library::all_tracks(&self.conn),
            View::Liked => library::liked_tracks(&self.conn),
            View::Playlist(id) => library::playlist_tracks(&self.conn, id),
        }
        .unwrap_or_default();

        for (i, track) in tracks.iter().enumerate() {
            self.list.append(&self.track_row(i + 1, track));
        }

        // Hero header reflects the active collection, incl. a custom cover.
        let (title, image) = match view {
            View::All => ("All Songs".to_owned(), None),
            View::Liked => ("Liked Songs".to_owned(), None),
            View::Playlist(id) => library::all_playlists(&self.conn)
                .unwrap_or_default()
                .into_iter()
                .find(|p| p.id == id)
                .map(|p| (p.name, p.image))
                .unwrap_or_else(|| ("Playlist".to_owned(), None)),
        };
        self.hero_title.set_label(&title);
        self.hero_sub.set_label(&plural_tracks(tracks.len() as i64));

        // Rebuild the hero cover. A custom image is cropped square (ContentFit
        // Cover) and clipped to rounded corners by the holder's overflow.
        while let Some(child) = self.hero_art.first_child() {
            self.hero_art.remove(&child);
        }
        match image.as_deref().and_then(|p| square_texture(p, 180)) {
            Some(tex) => {
                let pic = gtk::Picture::for_paintable(&tex);
                pic.set_size_request(180, 180);
                self.hero_art.append(&pic);
            }
            None => {
                // Heart only for Liked; a music glyph for All Songs and for
                // playlists that have no custom cover.
                let icon_name = match view {
                    View::Liked => "emblem-favorite-symbolic",
                    _ => "audio-x-generic-symbolic",
                };
                let icon = gtk::Image::from_icon_name(icon_name);
                icon.set_pixel_size(84);
                icon.set_hexpand(true);
                icon.set_vexpand(true);
                self.hero_art.append(&icon);
            }
        }

        *self.tracks.borrow_mut() = tracks;
        // Re-highlight the playing track if it happens to be in this view; the
        // queue and current index are left alone (playback keeps its own list).
        self.update_highlight();

        if self.tracks.borrow().is_empty() {
            self.np_title.set_label("No music yet");
            self.np_artist.set_label("Use “Add music” to scan a folder");
        }
    }

    /// Rebuild the sidebar: "Liked Songs" pinned first, then user playlists.
    fn refresh_playlists(&self) {
        while let Some(row) = self.sidebar_list.row_at_index(0) {
            self.sidebar_list.remove(&row);
        }
        self.sidebar_list.append(&playlist_row("Liked Songs", "Playlist", true, None));

        let playlists = library::all_playlists(&self.conn).unwrap_or_default();
        let mut ids = Vec::with_capacity(playlists.len());
        for p in &playlists {
            let sub = format!("Playlist · {}", plural_tracks(p.count));
            let row = playlist_row(&p.name, &sub, false, p.image.as_deref());

            // Right-click a playlist to delete it.
            let gesture = gtk::GestureClick::new();
            gesture.set_button(gdk::BUTTON_SECONDARY);
            let ui = self.clone();
            let pid = p.id;
            gesture.connect_pressed(glib::clone!(
                #[weak] row,
                move |_, _, x, y| ui.show_playlist_menu(&row, pid, x, y)
            ));
            row.add_controller(gesture);

            self.sidebar_list.append(&row);
            ids.push(p.id);
        }
        *self.playlist_ids.borrow_mut() = ids;
    }

    /// Right-click menu on a sidebar playlist row: set a cover image or delete.
    fn show_playlist_menu(&self, anchor: &impl IsA<gtk::Widget>, playlist_id: i64, x: f64, y: f64) {
        let popover = gtk::Popover::new();
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
        menu.set_margin_top(4);
        menu.set_margin_bottom(4);

        let window = anchor.root().and_downcast::<gtk::Window>();

        // Set image…
        let set_img = gtk::Button::builder()
            .label("Set image…")
            .css_classes(vec!["flat"])
            .build();
        let ui = self.clone();
        set_img.connect_clicked(glib::clone!(
            #[weak] popover,
            #[strong] window,
            move |_| {
                popover.popdown();
                let dialog = gtk::FileDialog::builder().title("Choose cover image").build();
                let filter = gtk::FileFilter::new();
                filter.add_mime_type("image/*");
                filter.set_name(Some("Images"));
                let filters = gio::ListStore::new::<gtk::FileFilter>();
                filters.append(&filter);
                dialog.set_filters(Some(&filters));

                dialog.open(window.as_ref(), gio::Cancellable::NONE, glib::clone!(
                    #[strong] ui,
                    move |result| {
                        if let Ok(file) = result {
                            if let Some(src) = file.path() {
                                // Copy into our own data dir so the cover keeps
                                // working even if the original is moved/deleted.
                                if let Some(dest) = copy_cover(playlist_id, &src) {
                                    let _ = library::set_playlist_image(&ui.conn, playlist_id, dest.to_str());
                                    ui.refresh_playlists();
                                    if ui.view.get() == View::Playlist(playlist_id) {
                                        ui.refresh_tracks();
                                    }
                                }
                            }
                        }
                    }
                ));
            }
        ));
        menu.append(&set_img);

        // Delete playlist
        let del = gtk::Button::builder()
            .label("Delete playlist")
            .css_classes(vec!["flat"])
            .build();
        let ui = self.clone();
        del.connect_clicked(glib::clone!(
            #[weak] popover,
            move |_| {
                let _ = library::delete_playlist(&ui.conn, playlist_id);
                if ui.view.get() == View::Playlist(playlist_id) {
                    ui.view.set(View::All);
                    ui.sidebar_list.unselect_all();
                    ui.refresh_tracks();
                }
                ui.refresh_playlists();
                popover.popdown();
            }
        ));
        menu.append(&del);

        popover.set_child(Some(&menu));
        popover.set_parent(anchor);
        popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        popover.connect_closed(|p| p.unparent());
        popover.popup();
    }

    /// Pop up a menu of playlists to add `track_id` to, anchored at the click.
    fn show_add_menu(&self, anchor: &impl IsA<gtk::Widget>, track_id: i64, x: f64, y: f64) {
        let popover = gtk::Popover::new();
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
        menu.set_margin_top(4);
        menu.set_margin_bottom(4);

        // When viewing a playlist, offer to remove the track from it.
        if let View::Playlist(pid) = self.view.get() {
            let remove = gtk::Button::builder()
                .label("Remove from this playlist")
                .css_classes(vec!["flat"])
                .build();
            let ui = self.clone();
            remove.connect_clicked(glib::clone!(
                #[weak] popover,
                move |_| {
                    let _ = library::remove_from_playlist(&ui.conn, pid, track_id);
                    ui.refresh_playlists();
                    ui.refresh_tracks();
                    popover.popdown();
                }
            ));
            menu.append(&remove);
            menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        }

        let header = gtk::Label::builder()
            .label("Add to playlist")
            .xalign(0.0)
            .css_classes(vec!["section-label"])
            .build();
        header.set_margin_start(8);
        header.set_margin_end(8);
        header.set_margin_bottom(4);
        menu.append(&header);

        let playlists = library::all_playlists(&self.conn).unwrap_or_default();
        if playlists.is_empty() {
            let empty = gtk::Label::builder().label("No playlists yet").xalign(0.0).build();
            empty.set_margin_start(8);
            empty.set_margin_end(8);
            menu.append(&empty);
        } else {
            for p in playlists {
                let item = gtk::Button::builder().label(&p.name).css_classes(vec!["flat"]).build();
                let ui = self.clone();
                item.connect_clicked(glib::clone!(
                    #[weak] popover,
                    move |_| {
                        let _ = library::add_to_playlist(&ui.conn, p.id, track_id);
                        // Refresh so the sidebar count updates immediately, and
                        // the list too if we're viewing that same playlist.
                        ui.refresh_playlists();
                        if ui.view.get() == View::Playlist(p.id) {
                            ui.refresh_tracks();
                        }
                        popover.popdown();
                    }
                ));
                menu.append(&item);
            }
        }

        // Delete the track from the library entirely.
        menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let delete = gtk::Button::builder()
            .label("Delete from library")
            .css_classes(vec!["flat"])
            .build();
        let ui = self.clone();
        delete.connect_clicked(glib::clone!(
            #[weak] popover,
            move |_| {
                // Preserve scroll position so deleting several tracks in a row
                // doesn't fling the list back to the top each time.
                let vadj = ui.track_scroller.vadjustment();
                let scroll = vadj.value();
                let _ = library::delete_track(&ui.conn, track_id);
                ui.refresh_playlists();
                ui.refresh_tracks();
                glib::idle_add_local_once(move || vadj.set_value(scroll));
                popover.popdown();
            }
        ));
        menu.append(&delete);

        popover.set_child(Some(&menu));
        popover.set_parent(anchor);
        popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        popover.connect_closed(|p| p.unparent());
        popover.popup();
    }

    /// Start playing from the currently displayed list: the whole list becomes
    /// the playback queue, so next/prev keep following it even if the user later
    /// browses a different playlist.
    fn play_index(&self, display_index: usize) {
        let tracks = self.tracks.borrow().clone();
        if display_index >= tracks.len() {
            return;
        }
        *self.queue.borrow_mut() = tracks;
        self.play_queue_index(display_index);
    }

    /// Load and start the track at `index` in the playback queue.
    fn play_queue_index(&self, index: usize) {
        let Some(track) = self.queue.borrow().get(index).cloned() else {
            return;
        };
        self.current.set(Some(index));
        self.playing_id.set(Some(track.id));
        self.update_highlight();

        let uri = gio::File::for_path(&track.path).uri();
        self.player.play_uri(&uri);
        self.is_playing.set(true);
        self.ended.set(false);
        self.set_anchor(0.0, true);
        self.play_btn.set_icon_name("media-playback-pause-symbolic");
        self.np_title.set_label(&track.title);
        self.np_artist.set_label(&track.artist);
        self.set_np_cover(&track.path);
    }

    /// Highlight the row of the playing track if it is in the current view.
    fn update_highlight(&self) {
        let playing = self.playing_id.get();
        for (i, t) in self.tracks.borrow().iter().enumerate() {
            if let Some(row) = self.list.row_at_index(i as i32) {
                if Some(t.id) == playing {
                    row.add_css_class("playing");
                } else {
                    row.remove_css_class("playing");
                }
            }
        }
    }

    /// Show the given track's embedded cover in the now-playing bar (or a
    /// music-note placeholder when it has none).
    fn set_np_cover(&self, path: &str) {
        while let Some(child) = self.np_cover.first_child() {
            self.np_cover.remove(&child);
        }
        let tex = self
            .cover_cache
            .borrow_mut()
            .entry(path.to_owned())
            .or_insert_with(|| track_cover(path))
            .clone();
        match tex {
            Some(tex) => self.np_cover.append(&texture_view(&tex, 40)),
            None => self.np_cover.append(&cover_widget(false, None)),
        }
    }

    /// Called when a track reaches its end. Honours repeat-one before advancing.
    /// Guarded so EOS and the position fallback can't both fire it.
    fn on_track_end(&self) {
        if self.ended.replace(true) {
            return;
        }
        if self.repeat.get() == Repeat::One {
            if let Some(i) = self.current.get() {
                self.play_queue_index(i);
            }
        } else {
            self.play_next();
        }
    }

    /// Advance within the playback queue. Shuffle picks at random; repeat-all
    /// wraps to the start; otherwise playback stops at the end of the queue.
    fn play_next(&self) {
        let len = self.queue.borrow().len();
        if len == 0 {
            return;
        }
        if self.shuffle.get() {
            self.play_queue_index(glib::random_int_range(0, len as i32) as usize);
            return;
        }
        let next = self.current.get().map_or(0, |i| i + 1);
        if next < len {
            self.play_queue_index(next);
        } else if self.repeat.get() == Repeat::All {
            self.play_queue_index(0);
        } else {
            self.is_playing.set(false);
            self.play_btn.set_icon_name("media-playback-start-symbolic");
        }
    }

    /// Go back one track in the queue (or restart it from the top).
    fn play_prev(&self) {
        let prev = match self.current.get() {
            Some(i) if i > 0 => i - 1,
            _ => 0,
        };
        self.play_queue_index(prev);
    }

    /// Move the progress bar and time labels to match the pipeline.
    /// Re-anchor interpolation to `pos` seconds; `running` starts the clock.
    fn set_anchor(&self, pos: f64, running: bool) {
        self.anchor_pos.set(pos);
        self.anchor_time
            .set(running.then(std::time::Instant::now));
    }

    fn tick_progress(&self) {
        let Some(dur) = self.player.duration() else {
            return;
        };
        let dur_s = dur.nseconds() as f64 / 1_000_000_000.0;
        if dur_s <= 0.0 {
            return;
        }

        let real = self.player.position().map(|p| p.nseconds() as f64 / 1_000_000_000.0);

        // Fallback end-of-track detection: some systems never deliver the
        // pipeline's EOS message, so advance once the real position reaches the
        // end. The `ended` guard keeps this from double-firing with EOS.
        if self.is_playing.get() {
            if let Some(r) = real {
                if r >= dur_s - 0.2 {
                    self.on_track_end();
                    return;
                }
            }
        }

        // Advance smoothly from the anchor using wall-clock time; the pipeline's
        // own position updates in coarse steps and would make the knob stutter.
        let interp = match self.anchor_time.get() {
            Some(t) => self.anchor_pos.get() + t.elapsed().as_secs_f64(),
            None => self.anchor_pos.get(),
        };

        // Re-sync if the pipeline has drifted from our estimate (e.g. a seek).
        let pos = match real {
            Some(real) => {
                if (real - interp).abs() > 0.35 {
                    self.set_anchor(real, self.is_playing.get());
                    real
                } else {
                    interp
                }
            }
            None => interp,
        }
        .min(dur_s);

        self.progress.set_value(pos / dur_s);
        self.time_pos.set_label(&fmt_duration(pos as i64));
        self.time_dur.set_label(&fmt_duration(dur_s as i64));
    }

    fn track_row(&self, index: usize, track: &Track) -> gtk::ListBoxRow {
        let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        row_box.set_margin_start(28);
        row_box.set_margin_end(28);
        row_box.set_margin_top(6);
        row_box.set_margin_bottom(6);

        let num = gtk::Label::builder()
            .label(index.to_string())
            .css_classes(vec!["track-num"])
            .width_request(20)
            .build();
        row_box.append(&num);

        // Cover art from the file's embedded picture (cached), or a placeholder.
        let tex = self
            .cover_cache
            .borrow_mut()
            .entry(track.path.clone())
            .or_insert_with(|| track_cover(&track.path))
            .clone();
        let cover: gtk::Widget = match tex {
            Some(t) => texture_view(&t, 40),
            None => cover_widget(false, None),
        };
        row_box.append(&cover);

        let text = gtk::Box::new(gtk::Orientation::Vertical, 1);
        text.set_hexpand(true);
        text.set_valign(gtk::Align::Center);
        text.append(&gtk::Label::builder().label(&track.title).xalign(0.0).css_classes(vec!["track-title"]).build());
        text.append(&gtk::Label::builder().label(&track.artist).xalign(0.0).css_classes(vec!["track-artist"]).build());
        row_box.append(&text);

        let album = gtk::Label::builder().label(&track.album).css_classes(vec!["track-album"]).build();
        row_box.append(&album);

        // Heart toggle as a glyph (♡ empty / ♥ filled) so a liked track is
        // always solidly coloured, independent of the icon theme. Persists the
        // flag straight to the database.
        let heart_label = gtk::Label::new(Some(if track.liked { "♥" } else { "♡" }));
        let heart = gtk::Button::builder()
            .child(&heart_label)
            .css_classes(vec!["flat", "heart"])
            .build();
        let liked = Rc::new(Cell::new(track.liked));
        if track.liked {
            heart.add_css_class("liked");
        }
        heart.connect_clicked(glib::clone!(
            #[strong] liked,
            #[weak] heart,
            #[weak] heart_label,
            #[strong(rename_to = conn)] self.conn,
            #[strong(rename_to = id)] track.id,
            move |_| {
                let now = !liked.get();
                liked.set(now);
                let _ = library::set_liked(&conn, id, now);
                heart_label.set_text(if now { "♥" } else { "♡" });
                if now { heart.add_css_class("liked"); } else { heart.remove_css_class("liked"); }
            }
        ));
        row_box.append(&heart);

        let dur = gtk::Label::builder()
            .label(fmt_duration(track.duration))
            .css_classes(vec!["track-dur"])
            .width_request(48)
            .build();
        row_box.append(&dur);

        // Right-click opens the "add to playlist" menu for this track.
        let gesture = gtk::GestureClick::new();
        gesture.set_button(gdk::BUTTON_SECONDARY);
        let ui = self.clone();
        let track_id = track.id;
        gesture.connect_pressed(glib::clone!(
            #[weak] row_box,
            move |_, _, x, y| ui.show_add_menu(&row_box, track_id, x, y)
        ));
        row_box.add_controller(gesture);

        let row = gtk::ListBoxRow::new();
        row.set_child(Some(&row_box));
        row
    }
}

/// Centre-crop a pixbuf to a square and scale it to exactly `size`px. Pre-scaling
/// caps the widget size (a raw `Picture` would grow to the image's natural
/// resolution) and gives every cover the same square shape.
fn square_from_pixbuf(full: &gtk::gdk_pixbuf::Pixbuf, size: i32) -> Option<gdk::Texture> {
    let (w, h) = (full.width(), full.height());
    let side = w.min(h).max(1);
    let square = full.new_subpixbuf((w - side) / 2, (h - side) / 2, side, side);
    let scaled = square.scale_simple(size, size, gtk::gdk_pixbuf::InterpType::Bilinear)?;
    Some(gdk::Texture::for_pixbuf(&scaled))
}

/// Texture resolution for the small (40px) covers. Kept well above the display
/// size so they stay sharp on HiDPI when shown through a sized `gtk::Image`.
const COVER_TEX: i32 = 160;

/// A square cover texture from an image file (used for playlist covers).
fn square_texture(path: &str, size: i32) -> Option<gdk::Texture> {
    let full = gtk::gdk_pixbuf::Pixbuf::from_file(path).ok()?;
    square_from_pixbuf(&full, size)
}

/// A square cover texture from a track's embedded album art, if present.
fn track_cover(path: &str) -> Option<gdk::Texture> {
    let bytes = scanner::read_cover(std::path::Path::new(path))?;
    let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from(&bytes));
    let pixbuf = gtk::gdk_pixbuf::Pixbuf::from_stream(&stream, gio::Cancellable::NONE).ok()?;
    square_from_pixbuf(&pixbuf, COVER_TEX)
}

/// Render a texture as a fixed `px`×`px` rounded cover. Uses `gtk::Image` with
/// `pixel_size` so it never grows past `px` (a raw `Picture` inherits the
/// texture's resolution) and stays crisp on HiDPI.
fn texture_view(tex: &gdk::Texture, px: i32) -> gtk::Widget {
    let img = gtk::Image::from_paintable(Some(tex));
    img.set_pixel_size(px);
    img.set_hexpand(true);
    img.set_vexpand(true);

    let holder = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    holder.set_size_request(px, px);
    holder.set_hexpand(false);
    holder.set_vexpand(false);
    holder.set_halign(gtk::Align::Center);
    holder.set_valign(gtk::Align::Center);
    holder.set_overflow(gtk::Overflow::Hidden);
    holder.add_css_class("cover-img");
    holder.append(&img);
    holder.upcast()
}

/// Write raw image bytes (e.g. an album's embedded art) into the covers dir and
/// return the stored path.
fn save_cover_bytes(playlist_id: i64, bytes: &[u8]) -> Option<std::path::PathBuf> {
    let mut dir = glib::user_data_dir();
    dir.push("raudio");
    dir.push("covers");
    std::fs::create_dir_all(&dir).ok()?;
    let dest = dir.join(format!("{playlist_id}.cover"));
    std::fs::write(&dest, bytes).ok()?;
    Some(dest)
}

/// Copy a chosen image into `~/.local/share/raudio/covers/` and return the
/// stored path. Named by playlist id so re-setting overwrites the old cover.
fn copy_cover(playlist_id: i64, src: &std::path::Path) -> Option<std::path::PathBuf> {
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("img");
    let mut dir = glib::user_data_dir();
    dir.push("raudio");
    dir.push("covers");
    std::fs::create_dir_all(&dir).ok()?;
    let dest = dir.join(format!("{playlist_id}.{ext}"));
    std::fs::copy(src, &dest).ok()?;
    Some(dest)
}

/// Open (creating if needed) the library database under the user data dir.
fn open_library() -> Connection {
    let mut dir = glib::user_data_dir();
    dir.push("raudio");
    let _ = std::fs::create_dir_all(&dir);
    dir.push("library.db");
    library::open(dir.to_str().expect("library path is valid UTF-8"))
        .expect("failed to open the library database")
}

/// A file-chooser filter that only shows audio files.
fn audio_filters() -> gio::ListStore {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Audio"));
    filter.add_mime_type("audio/*");
    for ext in ["mp3", "flac", "ogg", "opus", "m4a", "aac", "wav", "wma", "aiff"] {
        filter.add_suffix(ext);
    }
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    filters
}

/// Hook the "Add music" button up to a small menu: add files, add a folder, or
/// import a folder as an album playlist.
fn wire_add_music(ui: &Ui, window: &adw::ApplicationWindow) {
    ui.add_btn.connect_clicked(glib::clone!(
        #[strong] ui,
        #[weak] window,
        #[weak(rename_to = anchor)] ui.add_btn,
        move |_| {
            let popover = gtk::Popover::new();
            let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
            menu.set_margin_top(4);
            menu.set_margin_bottom(4);

            let make = |label: &str| {
                gtk::Button::builder().label(label).css_classes(vec!["flat"]).build()
            };

            // Add individual files.
            let files = make("Add files…");
            files.connect_clicked(glib::clone!(
                #[strong] ui, #[weak] window, #[weak] popover,
                move |_| {
                    popover.popdown();
                    let dialog = gtk::FileDialog::builder().title("Add music files").build();
                    dialog.set_filters(Some(&audio_filters()));
                    dialog.open_multiple(Some(&window), gio::Cancellable::NONE, glib::clone!(
                        #[strong] ui,
                        move |result| {
                            if let Ok(list) = result {
                                for i in 0..list.n_items() {
                                    if let Some(file) = list.item(i).and_downcast::<gio::File>() {
                                        if let Some(path) = file.path() {
                                            scanner::scan_file(&ui.conn, &path);
                                        }
                                    }
                                }
                                ui.refresh_tracks();
                            }
                        }
                    ));
                }
            ));
            menu.append(&files);

            // Add a whole folder.
            let folder = make("Add folder…");
            folder.connect_clicked(glib::clone!(
                #[strong] ui, #[weak] window, #[weak] popover,
                move |_| {
                    popover.popdown();
                    let dialog = gtk::FileDialog::builder().title("Add music folder").build();
                    dialog.select_folder(Some(&window), gio::Cancellable::NONE, glib::clone!(
                        #[strong] ui,
                        move |result| {
                            if let Ok(folder) = result {
                                if let Some(path) = folder.path() {
                                    scanner::scan_dir(&ui.conn, &path);
                                    ui.refresh_tracks();
                                }
                            }
                        }
                    ));
                }
            ));
            menu.append(&folder);

            // Import a folder as an album playlist.
            let album = make("Import album as playlist…");
            album.connect_clicked(glib::clone!(
                #[strong] ui, #[weak] window, #[weak] popover,
                move |_| {
                    popover.popdown();
                    let dialog = gtk::FileDialog::builder().title("Choose an album folder").build();
                    dialog.select_folder(Some(&window), gio::Cancellable::NONE, glib::clone!(
                        #[strong] ui,
                        move |result| {
                            if let Ok(folder) = result {
                                if let Some(path) = folder.path() {
                                    if let Some((name, ids, cover)) = scanner::import_album(&ui.conn, &path) {
                                        if let Ok(pid) = library::create_playlist(&ui.conn, &name) {
                                            for id in ids {
                                                let _ = library::add_to_playlist(&ui.conn, pid, id);
                                            }
                                            // Use the album's embedded art as the cover.
                                            if let Some(bytes) = cover {
                                                if let Some(dest) = save_cover_bytes(pid, &bytes) {
                                                    let _ = library::set_playlist_image(&ui.conn, pid, dest.to_str());
                                                }
                                            }
                                        }
                                        ui.refresh_playlists();
                                        ui.refresh_tracks();
                                    }
                                }
                            }
                        }
                    ));
                }
            ));
            menu.append(&album);

            popover.set_child(Some(&menu));
            popover.set_parent(&anchor);
            popover.connect_closed(|p| p.unparent());
            popover.popup();
        }
    ));
}

/// Hook the "+" button up to a name prompt that creates a playlist.
fn wire_new_playlist(ui: &Ui, window: &adw::ApplicationWindow) {
    ui.new_playlist_btn.connect_clicked(glib::clone!(
        #[strong] ui,
        #[weak] window,
        move |_| {
            let dialog = adw::MessageDialog::new(Some(&window), Some("New playlist"), None);
            dialog.add_response("cancel", "Cancel");
            dialog.add_response("create", "Create");
            dialog.set_response_appearance("create", adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some("create"));
            dialog.set_close_response("cancel");

            let entry = gtk::Entry::builder().placeholder_text("Playlist name").build();
            dialog.set_extra_child(Some(&entry));

            dialog.connect_response(None, glib::clone!(
                #[strong] ui,
                #[weak] entry,
                move |_, response| {
                    if response == "create" {
                        let name = entry.text();
                        let name = name.trim();
                        if !name.is_empty() {
                            let _ = library::create_playlist(&ui.conn, name);
                            ui.refresh_playlists();
                        }
                    }
                }
            ));
            dialog.present();
        }
    ));
}

// --- layout helpers -------------------------------------------------------

fn build_sidebar(ui: &Ui) -> gtk::Widget {
    // Inner spacing comes from CSS padding on `.rail`, not widget margins —
    // margins would shrink the card itself and leave it shorter than the
    // content panel next to it.
    let rail = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(14)
        .vexpand(true)
        .css_classes(vec!["rail"])
        .build();

    rail.append(&gtk::Label::builder().label("Raudio").xalign(0.0).css_classes(vec!["brand"]).build());

    let nav = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let home = nav_button("go-home-symbolic", "Home", true);
    home.connect_clicked(glib::clone!(
        #[strong] ui,
        move |_| {
            ui.sidebar_list.unselect_all();
            ui.view.set(View::All);
            ui.refresh_tracks();
        }
    ));
    nav.append(&home);
    nav.append(&ui.add_btn); // "Add music"
    rail.append(&nav);

    // Library header with a "+" to create a playlist.
    let lib_head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    lib_head.set_margin_top(6);
    lib_head.set_margin_start(10);
    lib_head.set_margin_end(4);
    let lib_label = gtk::Label::builder()
        .label("YOUR LIBRARY")
        .xalign(0.0)
        .hexpand(true)
        .css_classes(vec!["section-label"])
        .build();
    lib_head.append(&lib_label);
    lib_head.append(&ui.new_playlist_btn);
    rail.append(&lib_head);

    // Row 0 is Liked Songs; rows 1.. are user playlists (see `playlist_ids`).
    ui.sidebar_list.connect_row_selected(glib::clone!(
        #[strong] ui,
        move |_, row| {
            let Some(row) = row else { return };
            let idx = row.index();
            let view = if idx == 0 {
                View::Liked
            } else {
                match ui.playlist_ids.borrow().get((idx - 1) as usize) {
                    Some(&id) => View::Playlist(id),
                    None => return,
                }
            };
            ui.view.set(view);
            ui.refresh_tracks();
        }
    ));

    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&ui.sidebar_list)
        .build();
    rail.append(&scroller);

    rail.upcast()
}

/// A menu button showing the current accent, opening a grid of theme swatches.
fn build_theme_menu(ui: &Ui) -> gtk::MenuButton {
    let dot = gtk::Box::builder().css_classes(vec!["theme-dot"]).build();
    let btn = gtk::MenuButton::builder()
        .css_classes(vec!["flat"])
        .tooltip_text("Colour scheme")
        .build();
    btn.set_child(Some(&dot));

    let flow = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .max_children_per_line(4)
        .column_spacing(8)
        .row_spacing(8)
        .build();
    flow.set_margin_top(10);
    flow.set_margin_bottom(10);
    flow.set_margin_start(10);
    flow.set_margin_end(10);

    let popover = gtk::Popover::new();
    for i in 0..theme::COUNT {
        let sw = gtk::Button::builder()
            .css_classes(vec!["swatch", &format!("swatch-{i}")])
            .build();
        sw.connect_clicked(glib::clone!(
            #[weak] popover,
            #[strong(rename_to = conn)] ui.conn,
            move |_| {
                theme::set(i);
                let _ = library::set_setting(&conn, "theme", &i.to_string());
                popover.popdown();
            }
        ));
        flow.append(&sw);
    }
    popover.set_child(Some(&flow));
    btn.set_popover(Some(&popover));
    btn
}

fn nav_button(icon: &str, label: &str, active: bool) -> gtk::Button {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.append(&gtk::Image::from_icon_name(icon));
    content.append(&gtk::Label::new(Some(label)));
    let btn = gtk::Button::builder().child(&content).css_classes(vec!["flat", "nav"]).build();
    if active {
        btn.add_css_class("nav-active");
    }
    btn
}

/// A uniform 40×40 rounded cover: a custom image, the Liked badge, or a
/// neutral placeholder — all the same size so the sidebar lines up.
fn cover_widget(liked: bool, image: Option<&str>) -> gtk::Widget {
    if let Some(tex) = image.and_then(|p| square_texture(p, COVER_TEX)) {
        return texture_view(&tex, 40);
    }

    let icon = gtk::Image::from_icon_name(if liked {
        "emblem-favorite-symbolic"
    } else {
        "audio-x-generic-symbolic"
    });
    icon.set_pixel_size(20);
    // The icon expands to fill the frame so it centres; the frame's *explicit*
    // hexpand(false) stops that expansion leaking out and stretching the row.
    icon.set_hexpand(true);
    icon.set_vexpand(true);

    let frame = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    frame.set_size_request(40, 40);
    frame.set_hexpand(false);
    frame.set_vexpand(false);
    frame.set_halign(gtk::Align::Start);
    frame.set_valign(gtk::Align::Center);
    frame.add_css_class(if liked { "cover-liked" } else { "cover-box" });
    frame.append(&icon);
    frame.upcast()
}

fn playlist_row(name: &str, sub: &str, liked: bool, image: Option<&str>) -> gtk::ListBoxRow {
    let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row_box.append(&cover_widget(liked, image));

    let text = gtk::Box::new(gtk::Orientation::Vertical, 1);
    text.set_valign(gtk::Align::Center);
    text.append(&gtk::Label::builder().label(name).xalign(0.0).css_classes(vec!["pl-title"]).build());
    text.append(&gtk::Label::builder().label(sub).xalign(0.0).css_classes(vec!["pl-sub"]).build());
    row_box.append(&text);

    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&row_box));
    row
}

/// "1 track" / "N tracks" with correct singular.
fn plural_tracks(n: i64) -> String {
    if n == 1 {
        "1 track".to_owned()
    } else {
        format!("{n} tracks")
    }
}

fn build_content(ui: &Ui) -> gtk::Widget {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .vexpand(true)
        .css_classes(vec!["content"])
        .build();

    let hero = gtk::Box::new(gtk::Orientation::Horizontal, 24);
    hero.set_margin_top(24);
    hero.set_margin_bottom(20);
    hero.set_margin_start(28);
    hero.set_margin_end(28);
    hero.set_valign(gtk::Align::End);

    hero.append(&ui.hero_art);

    let info = gtk::Box::new(gtk::Orientation::Vertical, 8);
    info.set_valign(gtk::Align::End);
    info.append(&gtk::Label::builder().label("NOW VIEWING").xalign(0.0).css_classes(vec!["section-label"]).build());
    info.append(&ui.hero_title);
    info.append(&ui.hero_sub);
    hero.append(&info);
    content.append(&hero);

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    controls.set_margin_start(28);
    controls.set_margin_bottom(8);
    let play = gtk::Button::from_icon_name("media-playback-start-symbolic");
    play.add_css_class("play-lg");
    play.connect_clicked(glib::clone!(
        #[strong] ui,
        move |_| ui.play_index(0)
    ));
    controls.append(&play);
    content.append(&controls);
    content.append(&ui.track_scroller);

    content.upcast()
}

fn build_now_playing(ui: &Ui) -> gtk::Widget {
    let bar = gtk::CenterBox::builder().css_classes(vec!["now-bar"]).build();
    bar.set_margin_top(6);
    bar.set_margin_bottom(6);
    bar.set_margin_start(10);
    bar.set_margin_end(10);

    // Left: current track.
    let np = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    np.set_margin_start(12);
    np.append(&ui.np_cover);
    let np_text = gtk::Box::new(gtk::Orientation::Vertical, 1);
    np_text.set_valign(gtk::Align::Center);
    np_text.append(&ui.np_title);
    np_text.append(&ui.np_artist);
    np.append(&np_text);
    bar.set_start_widget(Some(&np));

    // Center: transport + progress.
    let center = gtk::Box::new(gtk::Orientation::Vertical, 6);
    center.set_valign(gtk::Align::Center);

    let transport = gtk::Box::new(gtk::Orientation::Horizontal, 18);
    transport.set_halign(gtk::Align::Center);
    transport.set_valign(gtk::Align::Center);
    ui.play_btn.set_valign(gtk::Align::Center);

    // Shuffle: toggles random advance, with an accent tint when active.
    let shuffle = flat_icon("media-playlist-shuffle-symbolic");
    shuffle.connect_clicked(glib::clone!(
        #[strong] ui,
        #[weak] shuffle,
        move |_| {
            let on = !ui.shuffle.get();
            ui.shuffle.set(on);
            if on { shuffle.add_css_class("active"); } else { shuffle.remove_css_class("active"); }
        }
    ));
    transport.append(&shuffle);

    let prev = flat_icon("media-skip-backward-symbolic");
    prev.connect_clicked(glib::clone!(#[strong] ui, move |_| ui.play_prev()));
    transport.append(&prev);

    ui.play_btn.connect_clicked(glib::clone!(
        #[strong] ui,
        move |_| {
            let now = !ui.is_playing.get();
            ui.is_playing.set(now);
            if now {
                ui.player.play();
                // Restart the interpolation clock from where we paused.
                ui.set_anchor(ui.anchor_pos.get(), true);
                ui.play_btn.set_icon_name("media-playback-pause-symbolic");
            } else {
                // Freeze at the current interpolated position.
                let frozen = match ui.anchor_time.get() {
                    Some(t) => ui.anchor_pos.get() + t.elapsed().as_secs_f64(),
                    None => ui.anchor_pos.get(),
                };
                ui.set_anchor(frozen, false);
                ui.player.pause();
                ui.play_btn.set_icon_name("media-playback-start-symbolic");
            }
        }
    ));
    transport.append(&ui.play_btn);

    let next = flat_icon("media-skip-forward-symbolic");
    next.connect_clicked(glib::clone!(#[strong] ui, move |_| ui.play_next()));
    transport.append(&next);

    // Repeat cycles off -> repeat-all -> repeat-one.
    let repeat = flat_icon("media-playlist-repeat-symbolic");
    repeat.connect_clicked(glib::clone!(
        #[strong] ui,
        #[weak] repeat,
        move |_| {
            let mode = match ui.repeat.get() {
                Repeat::Off => Repeat::All,
                Repeat::All => Repeat::One,
                Repeat::One => Repeat::Off,
            };
            ui.repeat.set(mode);
            match mode {
                Repeat::Off => {
                    repeat.remove_css_class("active");
                    repeat.set_icon_name("media-playlist-repeat-symbolic");
                }
                Repeat::All => {
                    repeat.add_css_class("active");
                    repeat.set_icon_name("media-playlist-repeat-symbolic");
                }
                Repeat::One => {
                    repeat.add_css_class("active");
                    repeat.set_icon_name("media-playlist-repeat-song-symbolic");
                }
            }
        }
    ));
    transport.append(&repeat);
    center.append(&transport);

    let progress = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    progress.append(&ui.time_pos);
    ui.progress.set_hexpand(true);
    ui.progress.set_draw_value(false);
    ui.progress.set_size_request(420, -1);
    // `change-value` only fires on user interaction, so the polling timer's
    // `set_value` calls won't feed back into a seek.
    ui.progress.connect_change_value(glib::clone!(
        #[strong] ui,
        move |_, _, value| {
            if let Some(dur) = ui.player.duration() {
                let target = value * dur.seconds() as f64;
                ui.player.seek(target);
                // Jump the interpolation anchor to the seek target immediately.
                ui.set_anchor(target, ui.is_playing.get());
            }
            glib::Propagation::Proceed
        }
    ));
    progress.append(&ui.progress);
    progress.append(&ui.time_dur);
    center.append(&progress);
    bar.set_center_widget(Some(&center));

    // Right: volume.
    let vol = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    vol.set_valign(gtk::Align::Center);
    vol.set_margin_end(12);
    vol.append(&gtk::Image::from_icon_name("audio-volume-high-symbolic"));
    ui.vol_scale.set_value(0.7);
    ui.vol_scale.set_draw_value(false);
    ui.vol_scale.set_size_request(110, -1);
    // Apply volume live, but debounce the database write so dragging stays
    // smooth (a synchronous SQLite write per pixel would stutter the slider).
    let vol_save: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    ui.vol_scale.connect_value_changed(glib::clone!(
        #[strong] ui,
        #[strong] vol_save,
        move |s| {
            let value = s.value();
            ui.player.set_volume(value);
            if let Some(id) = vol_save.borrow_mut().take() {
                id.remove();
            }
            let id = glib::timeout_add_local_once(
                std::time::Duration::from_millis(400),
                glib::clone!(
                    #[strong] ui,
                    #[strong] vol_save,
                    move || {
                        let _ = library::set_setting(&ui.conn, "volume", &value.to_string());
                        *vol_save.borrow_mut() = None;
                    }
                ),
            );
            *vol_save.borrow_mut() = Some(id);
        }
    ));
    vol.append(&ui.vol_scale);
    bar.set_end_widget(Some(&vol));

    bar.upcast()
}

fn flat_icon(icon: &str) -> gtk::Button {
    let btn = gtk::Button::from_icon_name(icon);
    btn.add_css_class("flat");
    btn.add_css_class("transport");
    btn.set_valign(gtk::Align::Center);
    btn
}

/// Format a duration in seconds as `m:ss`.
fn fmt_duration(secs: i64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

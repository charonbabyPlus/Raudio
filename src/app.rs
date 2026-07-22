use gtk::gdk;
use gtk::prelude::*;

use crate::{theme, window};

/// Install the shared stylesheet provider and apply the default theme. Also
/// points the icon theme at our bundled icon pack so symbolic icons render the
/// same on every system (KDE, Windows, …), not just where Adwaita is installed.
pub fn load_css() {
    let display = gdk::Display::default().expect("no default display");
    gtk::IconTheme::for_display(&display).add_resource_path("/dev/raudio/icons");
    theme::install(&display);
}

/// Build and present the main window for an activation.
pub fn build_ui(app: &adw::Application) {
    let win = window::build(app);
    win.present();
}

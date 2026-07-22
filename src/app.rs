use gtk::gdk;
use gtk::prelude::*;

use crate::{theme, window};

/// Install the shared stylesheet provider and apply the default theme. Also
/// points the icon theme at our bundled icon pack so symbolic icons render the
/// same on every system (KDE, Windows, …), not just where Adwaita is installed.
pub fn load_css() {
    let display = gdk::Display::default().expect("no default display");
    gtk::IconTheme::for_display(&display).add_resource_path("/dev/raudio/icons");

    // Linux: libadwaita registers its refined stylesheet globally (styles our
    // plain GTK widgets). Windows / --no-default-features: force GTK's own
    // Adwaita so we don't inherit the system GTK theme (e.g. KDE's Breeze).
    #[cfg(feature = "adwaita")]
    {
        let _ = adw::init();
    }
    #[cfg(not(feature = "adwaita"))]
    {
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_theme_name(Some("Adwaita"));
        }
    }

    theme::install(&display);
}

/// Build and present the main window for an activation.
pub fn build_ui(app: &gtk::Application) {
    let win = window::build(app);
    win.present();
}

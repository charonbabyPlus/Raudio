use gtk::gdk;
use gtk::prelude::*;

use crate::{theme, window};

/// Install the shared stylesheet provider and apply the default theme.
pub fn load_css() {
    let display = gdk::Display::default().expect("no default display");
    theme::install(&display);
}

/// Build and present the main window for an activation.
pub fn build_ui(app: &adw::Application) {
    let win = window::build(app);
    win.present();
}

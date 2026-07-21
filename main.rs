mod app;
mod library;
mod player;
mod scanner;
mod theme;
mod window;

use gtk::glib;
use gtk::prelude::*;

/// Reverse-DNS application id. Must match the installed `.desktop` file name and
/// the icon name so the desktop/taskbar shows "Raudio" and its icon.
const APP_ID: &str = "com.raudio.Raudio";

fn main() -> glib::ExitCode {
    // GStreamer must be initialised before any pipeline element is created.
    gstreamer::init().expect("failed to initialise GStreamer");

    // Human-readable name shown by the shell where an app name is needed.
    glib::set_application_name("Raudio");

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .build();

    // Load the stylesheet once the display is available.
    app.connect_startup(|_| app::load_css());
    app.connect_activate(app::build_ui);

    app.run()
}

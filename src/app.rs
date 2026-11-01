//! Application entry point.

use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;

use crate::ui;

/// Launch the GTK application and return its exit code.
pub fn run() -> gtk::glib::ExitCode {
    let app = adw::Application::builder()
        .application_id("io.github.lumalens.LumaLens")
        .build();

    app.connect_activate(|app| {
        let window = ui::window::new(app);
        window.present();
    });

    app.run()
}

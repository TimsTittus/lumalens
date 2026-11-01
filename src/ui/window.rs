//! Main application window.

use libadwaita as adw;

/// Marker for the main window (reserved for future state).
pub struct Window;

/// Build and return the primary application window.
pub fn new(app: &adw::Application) -> adw::ApplicationWindow {
    adw::ApplicationWindow::builder()
        .application(app)
        .title("LumaLens")
        .default_width(1280)
        .default_height(720)
        .build()
}

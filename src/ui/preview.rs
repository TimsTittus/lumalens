//! Camera preview widget.

use gtk4 as gtk;

/// Wraps a [`gtk::Picture`] to display the live camera feed.
pub struct PreviewWidget {
    #[allow(dead_code)]
    picture: gtk::Picture,
}

impl PreviewWidget {
    pub fn new() -> Self {
        Self {
            picture: gtk::Picture::new(),
        }
    }
}

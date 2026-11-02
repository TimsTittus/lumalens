//! Camera preview widget.

use gtk4 as gtk;
use gtk4::gdk;
use gtk4::prelude::*;

/// Wraps a [`gtk::Picture`] to display the live camera feed.
pub struct PreviewWidget {
    picture: gtk::Picture,
}

impl Default for PreviewWidget {
    fn default() -> Self {
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Contain);
        picture.set_size_request(640, 360);
        Self { picture }
    }
}

impl PreviewWidget {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn widget(&self) -> &gtk::Picture {
        &self.picture
    }

    pub fn set_paintable(&self, paintable: &gdk::Paintable) {
        self.picture.set_paintable(Some(paintable));
    }
}

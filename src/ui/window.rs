//! Main application window.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4 as gtk;
use gtk4::prelude::*;
use libadwaita as adw;
use tracing::error;

use crate::engine::{camera, pipeline::Pipeline};
use crate::ui::preview::PreviewWidget;

/// Build and return the primary application window.
pub fn new(app: &adw::Application) -> adw::ApplicationWindow {
    let header = adw::HeaderBar::new();

    let preview = PreviewWidget::new();
    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(preview.widget()));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("LumaLens")
        .default_width(1280)
        .default_height(720)
        .content(&toolbar_view)
        .build();

    let pipeline_cell: Rc<RefCell<Option<Pipeline>>> = Rc::new(RefCell::new(None));

    let cell = Rc::clone(&pipeline_cell);
    let toolbar_view_ref = toolbar_view.clone();
    window.connect_realize(move |_win| {
        match start_preview(&preview) {
            Ok(pipe) => {
                cell.replace(Some(pipe));
            }
            Err(msg) => {
                show_error(&toolbar_view_ref, &msg);
            }
        }
    });

    let cell = Rc::clone(&pipeline_cell);
    window.connect_close_request(move |_win| {
        if let Some(ref pipe) = *cell.borrow() {
            if let Err(e) = pipe.stop() {
                error!("failed to stop pipeline: {e}");
            }
        }
        gtk::glib::Propagation::Proceed
    });

    window
}

fn start_preview(preview: &PreviewWidget) -> Result<Pipeline, String> {
    let cameras = camera::discover().map_err(|e| format!("Camera discovery failed: {e}"))?;
    let cam = cameras.first().ok_or("No cameras found".to_string())?;

    let pipeline =
        Pipeline::build(&cam.device_path).map_err(|e| format!("Pipeline build failed: {e}"))?;

    preview.set_paintable(&pipeline.paintable());
    pipeline.start().map_err(|e| format!("Pipeline start failed: {e}"))?;

    Ok(pipeline)
}

fn show_error(toolbar_view: &adw::ToolbarView, message: &str) {
    error!("{message}");
    let status = adw::StatusPage::builder()
        .icon_name("camera-disabled-symbolic")
        .title("No Camera Available")
        .description(message)
        .vexpand(true)
        .build();
    toolbar_view.set_content(Some(&status));
}

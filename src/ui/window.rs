//! Main application window.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gstreamer as gst;
use gtk4 as gtk;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use tracing::{error, info};

use crate::engine::commands::EngineCommand;
use crate::engine::events::EngineEvent;
use crate::engine::runtime;

use super::preview::PreviewWidget;

/// Build and return the primary application window.
pub fn new(app: &adw::Application) -> adw::ApplicationWindow {
    let header = adw::HeaderBar::new();
    let preview = PreviewWidget::new();

    // Create the sink on the main thread so its !Send Paintable stays here.
    let sink = gst::ElementFactory::make("gtk4paintablesink")
        .name("sink")
        .build()
        .expect("gtk4paintablesink not available — is gst-plugin-gtk4 installed?");
    let paintable = sink.property::<gtk4::gdk::Paintable>("paintable");
    preview.set_paintable(&paintable);

    let (engine, event_rx) = runtime::spawn(sink);
    let engine = Rc::new(RefCell::new(Some(engine)));

    let toast_overlay = adw::ToastOverlay::new();
    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(preview.widget()));
    toast_overlay.set_child(Some(&toolbar_view));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("LumaLens")
        .default_width(1280)
        .default_height(720)
        .content(&toast_overlay)
        .build();

    let toast = toast_overlay.clone();
    glib::timeout_add_local(Duration::from_millis(16), move || {
        while let Ok(event) = event_rx.try_recv() {
            match event {
                EngineEvent::CamerasDiscovered(cameras) => {
                    info!("discovered {} camera(s)", cameras.len());
                    // TODO: populate camera dropdown
                }
                EngineEvent::Started => {
                    info!("engine started");
                }
                EngineEvent::Stopped => {
                    info!("engine stopped");
                }
                EngineEvent::Error(msg) => {
                    error!("{msg}");
                    toast.add_toast(adw::Toast::new(&msg));
                }
            }
        }
        glib::ControlFlow::Continue
    });

    let eng = Rc::clone(&engine);
    window.connect_realize(move |_| {
        if let Some(ref e) = *eng.borrow() {
            e.send(EngineCommand::Start);
        }
    });

    let eng = Rc::clone(&engine);
    window.connect_close_request(move |_| {
        if let Some(e) = eng.borrow_mut().take() {
            e.shutdown();
        }
        glib::Propagation::Proceed
    });

    window
}

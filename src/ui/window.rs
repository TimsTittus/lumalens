//! Main application window.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gstreamer as gst;
use gtk4 as gtk;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use tracing::{error, info};

use crate::engine::commands::EngineCommand;
use crate::engine::events::EngineEvent;
use crate::engine::runtime;

use super::controls::Controls;
use super::preview::PreviewWidget;

/// Build and return the primary application window.
pub fn new(app: &adw::Application) -> adw::ApplicationWindow {
    let preview = PreviewWidget::new();

    let sink = gst::ElementFactory::make("gtk4paintablesink")
        .name("sink")
        .build()
        .expect("gtk4paintablesink not available — is gst-plugin-gtk4 installed?");
    let paintable = sink.property::<gtk4::gdk::Paintable>("paintable");
    preview.set_paintable(&paintable);

    let (engine, event_rx) = runtime::spawn(sink);
    let engine = Rc::new(RefCell::new(Some(engine)));

    let controls = Controls::new();
    let camera_row = controls.camera_row.clone();
    let brightness_scale = controls.brightness.clone();

    // --- sidebar -----------------------------------------------------------

    let sidebar_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sidebar_box.set_margin_top(12);
    sidebar_box.set_margin_bottom(12);
    sidebar_box.set_margin_start(12);
    sidebar_box.set_margin_end(12);
    sidebar_box.append(&controls.root);

    let sidebar = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&sidebar_box)
        .build();

    // --- content ------------------------------------------------------------

    let sidebar_btn = gtk::ToggleButton::builder()
        .icon_name("sidebar-show-symbolic")
        .active(true)
        .tooltip_text("Toggle sidebar")
        .build();

    let header = adw::HeaderBar::new();
    header.pack_start(&sidebar_btn);

    let toast_overlay = adw::ToastOverlay::new();
    toast_overlay.set_child(Some(preview.widget()));

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&toast_overlay));

    // --- split view ---------------------------------------------------------

    let split_view = adw::OverlaySplitView::builder()
        .content(&toolbar_view)
        .sidebar(&sidebar)
        .show_sidebar(true)
        .min_sidebar_width(280.0)
        .max_sidebar_width(360.0)
        .build();

    sidebar_btn
        .bind_property("active", &split_view, "show-sidebar")
        .bidirectional()
        .sync_create()
        .build();

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("LumaLens")
        .default_width(1280)
        .default_height(720)
        .content(&split_view)
        .build();

    // --- camera dropdown signal ---------------------------------------------

    let device_paths: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let suppress_camera_signal = Rc::new(Cell::new(false));

    let eng = Rc::clone(&engine);
    let paths = Rc::clone(&device_paths);
    let suppress = Rc::clone(&suppress_camera_signal);
    camera_row.connect_selected_notify(move |row| {
        if suppress.get() {
            return;
        }
        let idx = row.selected();
        if idx == gtk::INVALID_LIST_POSITION {
            return;
        }
        let paths = paths.borrow();
        if let Some(path) = paths.get(idx as usize) {
            if let Some(ref e) = *eng.borrow() {
                e.send(EngineCommand::SelectCamera(path.clone()));
            }
        }
    });

    // --- brightness debounce ------------------------------------------------

    let debounce_id: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    let eng = Rc::clone(&engine);
    let debounce = Rc::clone(&debounce_id);
    brightness_scale.connect_value_changed(move |scale| {
        let value = scale.value() as f32;

        if let Some(id) = debounce.borrow_mut().take() {
            id.remove();
        }

        let eng_inner = Rc::clone(&eng);
        let debounce_inner = Rc::clone(&debounce);
        let id = glib::timeout_add_local_once(Duration::from_millis(60), move || {
            debounce_inner.borrow_mut().take();
            if let Some(ref e) = *eng_inner.borrow() {
                e.send(EngineCommand::SetBrightness(value));
            }
        });
        debounce.borrow_mut().replace(id);
    });

    // --- event polling ------------------------------------------------------

    let toast = toast_overlay.clone();
    let paths = Rc::clone(&device_paths);
    let suppress = Rc::clone(&suppress_camera_signal);
    glib::timeout_add_local(Duration::from_millis(16), move || {
        while let Ok(event) = event_rx.try_recv() {
            match event {
                EngineEvent::CamerasDiscovered(cameras) => {
                    info!("discovered {} camera(s)", cameras.len());
                    paths.replace(
                        cameras.iter().map(|c| c.device_path.clone()).collect(),
                    );
                    let names: Vec<&str> =
                        cameras.iter().map(|c| c.name.as_str()).collect();
                    suppress.set(true);
                    camera_row.set_model(Some(&gtk::StringList::new(&names)));
                    suppress.set(false);
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

    // --- lifecycle ----------------------------------------------------------

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

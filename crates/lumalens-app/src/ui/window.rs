//! Main application window.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gstreamer as gst;
use gtk4 as gtk;
use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use tracing::{info, warn};

use lumalens_common::FlipMethod;
use lumalens_engine::commands::EngineCommand;
use lumalens_engine::events::EngineEvent;
use lumalens_engine::runtime;

use super::controls::Controls;
use super::preview::PreviewWidget;

/// Build and return the primary application window.
pub fn new(app: &adw::Application) -> adw::ApplicationWindow {
    let preview = PreviewWidget::new();

    let sink = gst::ElementFactory::make("gtk4paintablesink")
        .name("sink")
        .build()
        .expect("gtk4paintablesink not available — is gst-plugin-gtk4 installed?");

    // `Pipeline::paintable()` returns `glib::Object` to keep `lumalens-engine`
    // free of any `gdk` / `gtk4` compile-time dependency.  We perform the
    // downcast here, in the app crate where `gdk` is already an explicit dep.
    let paintable_obj: gst::glib::Object = sink.property("paintable");
    let paintable = paintable_obj
        .downcast::<gdk::Paintable>()
        .expect("gtk4paintablesink paintable must implement gdk::Paintable");
    preview.set_paintable(&paintable);

    let (engine, event_rx) = runtime::spawn(sink);
    let engine = Rc::new(RefCell::new(Some(engine)));

    let controls = Controls::new();
    let camera_row = controls.camera_row.clone();
    let brightness_scale = controls.brightness.clone();
    let contrast_scale = controls.contrast.clone();
    let saturation_scale = controls.saturation.clone();
    let flip_dropdown = controls.flip_dropdown.clone();

    // --- sidebar -----------------------------------------------------------

    let sidebar_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
    sidebar_box.set_margin_top(12);
    sidebar_box.set_margin_bottom(12);
    sidebar_box.set_margin_start(12);
    sidebar_box.set_margin_end(12);
    sidebar_box.append(&controls.root);
    sidebar_box.append(&controls.output_group);

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

    let status_page = adw::StatusPage::builder()
        .icon_name("camera-disabled-symbolic")
        .title("No Cameras")
        .description("Connect a camera to get started")
        .build();

    let content_stack = gtk::Stack::new();
    content_stack.add_named(preview.widget(), Some("preview"));
    content_stack.add_named(&status_page, Some("no-cameras"));
    content_stack.set_visible_child_name("preview");

    let toast_overlay = adw::ToastOverlay::new();
    toast_overlay.set_child(Some(&content_stack));

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

    // --- contrast debounce ---------------------------------------------------

    let contrast_debounce_id: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    let eng = Rc::clone(&engine);
    let debounce = Rc::clone(&contrast_debounce_id);
    contrast_scale.connect_value_changed(move |scale| {
        let value = scale.value() as f32;

        if let Some(id) = debounce.borrow_mut().take() {
            id.remove();
        }

        let eng_inner = Rc::clone(&eng);
        let debounce_inner = Rc::clone(&debounce);
        let id = glib::timeout_add_local_once(Duration::from_millis(60), move || {
            debounce_inner.borrow_mut().take();
            if let Some(ref e) = *eng_inner.borrow() {
                e.send(EngineCommand::SetContrast(value));
            }
        });
        debounce.borrow_mut().replace(id);
    });

    // --- saturation debounce -------------------------------------------------

    let saturation_debounce_id: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    let eng = Rc::clone(&engine);
    let debounce = Rc::clone(&saturation_debounce_id);
    saturation_scale.connect_value_changed(move |scale| {
        let value = scale.value() as f32;

        if let Some(id) = debounce.borrow_mut().take() {
            id.remove();
        }

        let eng_inner = Rc::clone(&eng);
        let debounce_inner = Rc::clone(&debounce);
        let id = glib::timeout_add_local_once(Duration::from_millis(60), move || {
            debounce_inner.borrow_mut().take();
            if let Some(ref e) = *eng_inner.borrow() {
                e.send(EngineCommand::SetSaturation(value));
            }
        });
        debounce.borrow_mut().replace(id);
    });

    // --- flip dropdown -------------------------------------------------------

    let eng = Rc::clone(&engine);
    flip_dropdown.connect_selected_notify(move |row| {
        let idx = row.selected() as usize;
        if let Some(&method) = FlipMethod::ALL.get(idx) {
            if let Some(ref e) = *eng.borrow() {
                e.send(EngineCommand::SetFlip(method));
            }
        }
    });

    // --- virtual camera switch -----------------------------------------------

    let eng = Rc::clone(&engine);
    let vcam_toast = toast_overlay.clone();
    controls
        .virtual_cam_switch
        .connect_state_set(move |_switch, state| {
            if state {
                match lumalens_engine::virtual_cam::find_device() {
                    Some(_) => {
                        if let Some(ref e) = *eng.borrow() {
                            e.send(EngineCommand::EnableVirtualCamera(true));
                        }
                        glib::Propagation::Proceed
                    }
                    None => {
                        warn!("virtual camera toggle: device not found");
                        let msg = lumalens_engine::virtual_cam::module_install_instructions();
                        let t = adw::Toast::new(msg);
                        t.set_timeout(5);
                        vcam_toast.add_toast(t);
                        glib::Propagation::Stop
                    }
                }
            } else {
                if let Some(ref e) = *eng.borrow() {
                    e.send(EngineCommand::EnableVirtualCamera(false));
                }
                glib::Propagation::Proceed
            }
        });

    // --- event polling ------------------------------------------------------

    let toast = toast_overlay.clone();
    let paths = Rc::clone(&device_paths);
    let suppress = Rc::clone(&suppress_camera_signal);
    let stack = content_stack;
    glib::timeout_add_local(Duration::from_millis(16), move || {
        while let Ok(event) = event_rx.try_recv() {
            match event {
                EngineEvent::CamerasDiscovered(cameras) => {
                    info!("discovered {} camera(s)", cameras.len());

                    let old_selected = {
                        let p = paths.borrow();
                        let idx = camera_row.selected();
                        if idx != gtk::INVALID_LIST_POSITION {
                            p.get(idx as usize).cloned()
                        } else {
                            None
                        }
                    };

                    paths.replace(cameras.iter().map(|c| c.device_path.clone()).collect());
                    let names: Vec<&str> = cameras.iter().map(|c| c.name.as_str()).collect();
                    suppress.set(true);
                    camera_row.set_model(Some(&gtk::StringList::new(&names)));

                    if cameras.is_empty() {
                        stack.set_visible_child_name("no-cameras");
                    } else {
                        stack.set_visible_child_name("preview");
                        let idx = old_selected
                            .as_ref()
                            .and_then(|old| cameras.iter().position(|c| c.device_path == *old))
                            .unwrap_or(0);
                        camera_row.set_selected(idx as u32);
                    }

                    suppress.set(false);
                }
                EngineEvent::Started => {
                    info!("engine started");
                }
                EngineEvent::Stopped => {
                    info!("engine stopped");
                }
                EngineEvent::Error(msg) => {
                    warn!("{msg}");
                    let t = adw::Toast::new(&msg);
                    t.set_timeout(5);
                    toast.add_toast(t);
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

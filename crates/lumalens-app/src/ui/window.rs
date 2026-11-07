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

use lumalens_common::{CameraInfo, FlipMethod, Resolution};
use lumalens_config::ImageSettings;
use lumalens_engine::commands::EngineCommand;
use lumalens_engine::events::EngineEvent;
use lumalens_engine::runtime;

use super::controls::Controls;
use super::diagnostics::DiagnosticsPanel;
use super::effects_page;
use super::preview::PreviewWidget;

/// Build and return the primary application window.
pub fn new(app: &adw::Application) -> adw::ApplicationWindow {
    // --- config & presets ----------------------------------------------------

    lumalens_config::preset::ensure_builtin_presets();
    let config = Rc::new(RefCell::new(lumalens_config::load()));
    let presets: Rc<RefCell<Vec<lumalens_config::preset::Preset>>> =
        Rc::new(RefCell::new(lumalens_config::preset::list_presets()));

    // --- pipeline ------------------------------------------------------------

    let preview = PreviewWidget::new();

    let sink = gst::ElementFactory::make("gtk4paintablesink")
        .name("sink")
        .build()
        .expect("gtk4paintablesink not available — is gst-plugin-gtk4 installed?");

    let paintable_obj: gst::glib::Object = sink.property("paintable");
    let paintable = paintable_obj
        .downcast::<gdk::Paintable>()
        .expect("gtk4paintablesink paintable must implement gdk::Paintable");
    preview.set_paintable(&paintable);

    let blur_enabled = {
        #[cfg(feature = "ai")]
        {
            config.borrow().ai.background_blur
        }
        #[cfg(not(feature = "ai"))]
        {
            false
        }
    };
    let (engine, event_rx) = runtime::spawn(sink, config.borrow().effects.order.clone(), blur_enabled);
    let engine = Rc::new(RefCell::new(Some(engine)));

    let controls = Controls::new();

    // --- apply config to initial UI state ------------------------------------

    {
        let cfg = config.borrow();
        controls.brightness.set_value(cfg.image.brightness);
        controls.contrast.set_value(cfg.image.contrast);
        controls.saturation.set_value(cfg.image.saturation);
        let flip_idx = FlipMethod::ALL
            .iter()
            .position(|&m| m == cfg.image.flip)
            .unwrap_or(0);
        controls.flip_dropdown.set_selected(flip_idx as u32);
        #[cfg(feature = "ai")]
        controls.blur_switch.set_active(cfg.ai.background_blur);
    }

    // --- populate preset dropdown --------------------------------------------

    fn refresh_preset_dropdown(
        dropdown: &adw::ComboRow,
        presets: &Rc<RefCell<Vec<lumalens_config::preset::Preset>>>,
    ) {
        let list = presets.borrow();
        let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
        dropdown.set_model(Some(&gtk::StringList::new(&names)));
    }

    refresh_preset_dropdown(&controls.preset_dropdown, &presets);

    // --- config save debounce ------------------------------------------------

    let save_debounce: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    let schedule_save = {
        let config = Rc::clone(&config);
        let debounce = Rc::clone(&save_debounce);
        move || {
            if let Some(id) = debounce.borrow_mut().take() {
                id.remove();
            }
            let cfg = Rc::clone(&config);
            let deb = Rc::clone(&debounce);
            let id = glib::timeout_add_local_once(Duration::from_millis(500), move || {
                deb.borrow_mut().take();
                if let Err(e) = lumalens_config::save(&cfg.borrow()) {
                    warn!("failed to save config: {e}");
                }
            });
            debounce.borrow_mut().replace(id);
        }
    };

    let camera_row = controls.camera_row.clone();
    let resolution_dropdown = controls.resolution_dropdown.clone();
    let brightness_scale = controls.brightness.clone();
    let contrast_scale = controls.contrast.clone();
    let saturation_scale = controls.saturation.clone();
    let flip_dropdown = controls.flip_dropdown.clone();

    let available_resolutions: Rc<RefCell<Vec<Resolution>>> = Rc::new(RefCell::new(Vec::new()));
    let suppress_resolution_signal = Rc::new(Cell::new(false));

    // --- sidebar -------------------------------------------------------------

    let sidebar_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
    sidebar_box.set_margin_top(12);
    sidebar_box.set_margin_bottom(12);
    sidebar_box.set_margin_start(12);
    sidebar_box.set_margin_end(12);
    sidebar_box.append(&controls.root);
    sidebar_box.append(&controls.preset_group);
    sidebar_box.append(&controls.output_group);

    #[cfg(feature = "ai")]
    sidebar_box.append(&controls.ai_group);

    let effects_nav_row = adw::ActionRow::builder()
        .title("Effects Order")
        .activatable(true)
        .build();
    effects_nav_row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    let effects_nav_group = adw::PreferencesGroup::new();
    effects_nav_group.add(&effects_nav_row);
    sidebar_box.append(&effects_nav_group);

    let sidebar_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&sidebar_box)
        .build();

    let controls_page = adw::NavigationPage::builder()
        .title("Controls")
        .tag("controls")
        .child(&sidebar_scroll)
        .build();

    let effects_page = effects_page::build(
        Rc::clone(&engine),
        Rc::clone(&config),
        schedule_save.clone(),
    );

    let nav_view = adw::NavigationView::new();
    nav_view.add(&controls_page);

    {
        let nv = nav_view.clone();
        let ep = effects_page.clone();
        effects_nav_row.connect_activated(move |_| {
            nv.push(&ep);
        });
    }

    // --- content -------------------------------------------------------------

    let sidebar_btn = gtk::ToggleButton::builder()
        .icon_name("sidebar-show-symbolic")
        .active(true)
        .tooltip_text("Toggle sidebar")
        .build();

    let diag_btn = gtk::Button::builder()
        .icon_name("utilities-system-monitor-symbolic")
        .tooltip_text("Diagnostics (Ctrl+Shift+D)")
        .build();

    let header = adw::HeaderBar::new();
    header.pack_start(&sidebar_btn);
    header.pack_end(&diag_btn);

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

    // --- split view ----------------------------------------------------------

    let split_view = adw::OverlaySplitView::builder()
        .content(&toolbar_view)
        .sidebar(&nav_view)
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

    // --- diagnostics panel (Ctrl+Alt+D) ------------------------------------

    let diag = Rc::new(DiagnosticsPanel::new(&window));

    {
        let d = Rc::clone(&diag);
        diag_btn.connect_clicked(move |_| {
            d.toggle();
        });
    }

    {
        let d = Rc::clone(&diag);
        let controller = gtk::EventControllerKey::new();
        controller.connect_key_pressed(move |_ctrl, key, _code, modifiers| {
            // Ctrl+Shift+D — Ctrl+Alt+D conflicts with GNOME "Show Desktop"
            let want = gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK;
            if key == gdk::Key::D && modifiers.contains(want) {
                d.toggle();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(controller);
    }

    // --- camera dropdown signal ----------------------------------------------

    let device_paths: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let discovered_cameras: Rc<RefCell<Vec<CameraInfo>>> = Rc::new(RefCell::new(Vec::new()));
    let suppress_camera_signal = Rc::new(Cell::new(false));

    let eng = Rc::clone(&engine);
    let paths = Rc::clone(&device_paths);
    let cams = Rc::clone(&discovered_cameras);
    let suppress = Rc::clone(&suppress_camera_signal);
    let suppress_res_cam = Rc::clone(&suppress_resolution_signal);
    let res_dd_cam = controls.resolution_dropdown.clone();
    let avail_res_cam = Rc::clone(&available_resolutions);
    let cfg = Rc::clone(&config);
    let sched = schedule_save.clone();
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
            cfg.borrow_mut().camera.selected_id = path.clone();
            sched();

            let cameras = cams.borrow();
            if let Some(cam) = cameras.get(idx as usize) {
                let supported = &cam.supported_resolutions;
                suppress_res_cam.set(true);
                let labels: Vec<String> = supported.iter().map(|r| r.label()).collect();
                let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
                res_dd_cam.set_model(Some(&gtk::StringList::new(&label_refs)));
                *avail_res_cam.borrow_mut() = supported.clone();
                if !supported.is_empty() {
                    res_dd_cam.set_selected(0);
                }
                suppress_res_cam.set(false);
            }
        }
    });

    // --- resolution dropdown signal ------------------------------------------

    let eng = Rc::clone(&engine);
    let resolutions = Rc::clone(&available_resolutions);
    let suppress = Rc::clone(&suppress_resolution_signal);
    let cfg = Rc::clone(&config);
    let sched = schedule_save.clone();
    resolution_dropdown.connect_selected_notify(move |row| {
        if suppress.get() {
            return;
        }
        let idx = row.selected();
        if idx == gtk::INVALID_LIST_POSITION {
            return;
        }
        let res = resolutions.borrow();
        if let Some(r) = res.get(idx as usize) {
            if let Some(ref e) = *eng.borrow() {
                e.send(EngineCommand::SetResolution {
                    width: r.width,
                    height: r.height,
                    fps: r.fps,
                });
            }
            let mut c = cfg.borrow_mut();
            c.camera.resolution_width = r.width;
            c.camera.resolution_height = r.height;
            c.camera.fps = r.fps;
            sched();
        }
    });

    // --- brightness debounce -------------------------------------------------

    let debounce_id: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    let eng = Rc::clone(&engine);
    let debounce = Rc::clone(&debounce_id);
    let cfg = Rc::clone(&config);
    let sched = schedule_save.clone();
    brightness_scale.connect_value_changed(move |scale| {
        let value = scale.value() as f32;

        if let Some(id) = debounce.borrow_mut().take() {
            id.remove();
        }

        cfg.borrow_mut().image.brightness = scale.value();
        sched();

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
    let cfg = Rc::clone(&config);
    let sched = schedule_save.clone();
    contrast_scale.connect_value_changed(move |scale| {
        let value = scale.value() as f32;

        if let Some(id) = debounce.borrow_mut().take() {
            id.remove();
        }

        cfg.borrow_mut().image.contrast = scale.value();
        sched();

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
    let cfg = Rc::clone(&config);
    let sched = schedule_save.clone();
    saturation_scale.connect_value_changed(move |scale| {
        let value = scale.value() as f32;

        if let Some(id) = debounce.borrow_mut().take() {
            id.remove();
        }

        cfg.borrow_mut().image.saturation = scale.value();
        sched();

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
    let cfg = Rc::clone(&config);
    let sched = schedule_save.clone();
    flip_dropdown.connect_selected_notify(move |row| {
        let idx = row.selected() as usize;
        if let Some(&method) = FlipMethod::ALL.get(idx) {
            if let Some(ref e) = *eng.borrow() {
                e.send(EngineCommand::SetFlip(method));
            }
            cfg.borrow_mut().image.flip = method;
            sched();
        }
    });

    // --- virtual camera switch -----------------------------------------------

    let eng = Rc::clone(&engine);
    let vcam_toast = toast_overlay.clone();
    let cfg = Rc::clone(&config);
    let sched = schedule_save.clone();
    controls
        .virtual_cam_switch
        .connect_state_set(move |_switch, state| {
            cfg.borrow_mut().virtual_camera.enabled = state;
            sched();

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

    // --- background blur switch (AI feature) --------------------------------

    #[cfg(feature = "ai")]
    {
        let eng = Rc::clone(&engine);
        let cfg = Rc::clone(&config);
        let sched = schedule_save.clone();

        controls
            .blur_switch
            .connect_state_set(move |_switch, state| {
                cfg.borrow_mut().ai.background_blur = state;
                sched();
                if let Some(ref e) = *eng.borrow() {
                    e.send(EngineCommand::EnableBackgroundBlur(state));
                }
                glib::Propagation::Proceed
            });
    }

    // --- preset: Apply -------------------------------------------------------

    let preset_dd = controls.preset_dropdown.clone();
    let presets_ref = Rc::clone(&presets);
    let eng = Rc::clone(&engine);
    let cfg = Rc::clone(&config);
    let sched = schedule_save.clone();
    let br = controls.brightness.clone();
    let co = controls.contrast.clone();
    let sa = controls.saturation.clone();
    let fl = controls.flip_dropdown.clone();
    controls.apply_preset_btn.connect_clicked(move |_| {
        let idx = preset_dd.selected();
        if idx == gtk::INVALID_LIST_POSITION {
            return;
        }
        let list = presets_ref.borrow();
        let Some(preset) = list.get(idx as usize) else {
            return;
        };

        apply_image_settings(&preset.image, &br, &co, &sa, &fl, &eng, &cfg, &sched);
    });

    // --- preset: Save As -----------------------------------------------------

    let preset_dd = controls.preset_dropdown.clone();
    let presets_ref = Rc::clone(&presets);
    let cfg = Rc::clone(&config);
    let win = window.clone();
    let save_toast = toast_overlay.clone();
    controls.save_preset_btn.connect_clicked(move |_| {
        let dialog = adw::MessageDialog::new(
            Some(&win),
            Some("Save Preset"),
            Some("Enter a name for the preset:"),
        );
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("save", "Save");
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("save"));
        dialog.set_close_response("cancel");

        let entry = gtk::Entry::builder()
            .placeholder_text("Preset name")
            .activates_default(true)
            .build();
        dialog.set_extra_child(Some(&entry));

        let cfg_inner = Rc::clone(&cfg);
        let presets_inner = Rc::clone(&presets_ref);
        let dd = preset_dd.clone();
        let toast_inner = save_toast.clone();
        dialog.connect_response(None, move |dialog, response| {
            if response == "save" {
                let name = entry.text().trim().to_string();
                if name.is_empty() {
                    return;
                }
                let image = cfg_inner.borrow().image.clone();
                match lumalens_config::preset::save_preset(&name, &image) {
                    Ok(()) => {
                        *presets_inner.borrow_mut() = lumalens_config::preset::list_presets();
                        refresh_preset_dropdown(&dd, &presets_inner);
                        let t = adw::Toast::new(&format!("Preset \"{name}\" saved"));
                        t.set_timeout(3);
                        toast_inner.add_toast(t);
                    }
                    Err(e) => {
                        warn!("failed to save preset: {e}");
                        let t = adw::Toast::new(&format!("Failed to save preset: {e}"));
                        t.set_timeout(5);
                        toast_inner.add_toast(t);
                    }
                }
            }
            dialog.close();
        });

        dialog.present();
    });

    // --- preset: Delete ------------------------------------------------------

    let preset_dd = controls.preset_dropdown.clone();
    let presets_ref = Rc::clone(&presets);
    let delete_toast = toast_overlay.clone();
    let win2 = window.clone();
    controls.delete_preset_btn.connect_clicked(move |_| {
        let idx = preset_dd.selected();
        if idx == gtk::INVALID_LIST_POSITION {
            return;
        }
        let preset_name = {
            let list = presets_ref.borrow();
            match list.get(idx as usize) {
                Some(p) => p.name.clone(),
                None => return,
            }
        };

        let dialog = adw::MessageDialog::new(
            Some(&win2),
            Some("Delete Preset"),
            Some(&format!("Delete the preset \"{}\"?", preset_name)),
        );
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("delete", "Delete");
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");

        let presets_inner = Rc::clone(&presets_ref);
        let dd = preset_dd.clone();
        let toast_inner = delete_toast.clone();
        dialog.connect_response(None, move |dialog, response| {
            if response == "delete" {
                match lumalens_config::preset::delete_preset(&preset_name) {
                    Ok(()) => {
                        *presets_inner.borrow_mut() = lumalens_config::preset::list_presets();
                        refresh_preset_dropdown(&dd, &presets_inner);
                        let t = adw::Toast::new(&format!("Preset \"{preset_name}\" deleted"));
                        t.set_timeout(3);
                        toast_inner.add_toast(t);
                    }
                    Err(e) => {
                        warn!("failed to delete preset: {e}");
                        let t = adw::Toast::new(&format!("Failed to delete preset: {e}"));
                        t.set_timeout(5);
                        toast_inner.add_toast(t);
                    }
                }
            }
            dialog.close();
        });

        dialog.present();
    });

    // --- event polling -------------------------------------------------------

    let toast = toast_overlay.clone();
    let paths = Rc::clone(&device_paths);
    let cams_event = Rc::clone(&discovered_cameras);
    let suppress = Rc::clone(&suppress_camera_signal);
    let suppress_res = Rc::clone(&suppress_resolution_signal);
    let res_dropdown = controls.resolution_dropdown.clone();
    let avail_res = Rc::clone(&available_resolutions);
    let stack = content_stack;
    let eng_for_start = Rc::clone(&engine);
    let br_for_start = controls.brightness.clone();
    let co_for_start = controls.contrast.clone();
    let sa_for_start = controls.saturation.clone();
    let fl_for_start = controls.flip_dropdown.clone();
    let res_dd_for_start = controls.resolution_dropdown.clone();
    let avail_res_for_start = Rc::clone(&available_resolutions);
    let cfg_for_start = Rc::clone(&config);
    let diag = Rc::clone(&diag);
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
                    *cams_event.borrow_mut() = cameras.clone();
                    let names: Vec<&str> = cameras.iter().map(|c| c.name.as_str()).collect();
                    suppress.set(true);
                    camera_row.set_model(Some(&gtk::StringList::new(&names)));

                    if cameras.is_empty() {
                        stack.set_visible_child_name("no-cameras");
                        suppress_res.set(true);
                        avail_res.borrow_mut().clear();
                        res_dropdown.set_model(Some(&gtk::StringList::new(&[] as &[&str])));
                        suppress_res.set(false);
                    } else {
                        stack.set_visible_child_name("preview");

                        let saved_id = &cfg_for_start.borrow().camera.selected_id;
                        let idx = if !saved_id.is_empty() {
                            cameras.iter().position(|c| c.device_path == *saved_id)
                        } else {
                            None
                        }
                        .or_else(|| {
                            old_selected
                                .as_ref()
                                .and_then(|old| cameras.iter().position(|c| c.device_path == *old))
                        })
                        .unwrap_or(0);
                        camera_row.set_selected(idx as u32);

                        let supported = &cameras[idx].supported_resolutions;
                        suppress_res.set(true);
                        let labels: Vec<String> = supported.iter().map(|r| r.label()).collect();
                        let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
                        res_dropdown.set_model(Some(&gtk::StringList::new(&label_refs)));
                        *avail_res.borrow_mut() = supported.clone();

                        let cfg = cfg_for_start.borrow();
                        let saved_res = Resolution {
                            width: cfg.camera.resolution_width,
                            height: cfg.camera.resolution_height,
                            fps: cfg.camera.fps,
                        };
                        let res_idx = supported.iter().position(|r| *r == saved_res).unwrap_or(0);
                        res_dropdown.set_selected(res_idx as u32);
                        suppress_res.set(false);
                    }

                    suppress.set(false);
                }
                EngineEvent::Started => {
                    info!("engine started");
                    if let Some(ref e) = *eng_for_start.borrow() {
                        let res_idx = res_dd_for_start.selected();
                        if res_idx != gtk::INVALID_LIST_POSITION {
                            let resolutions = avail_res_for_start.borrow();
                            if let Some(r) = resolutions.get(res_idx as usize) {
                                e.send(EngineCommand::SetResolution {
                                    width: r.width,
                                    height: r.height,
                                    fps: r.fps,
                                });
                            }
                        }
                        e.send(EngineCommand::SetBrightness(br_for_start.value() as f32));
                        e.send(EngineCommand::SetContrast(co_for_start.value() as f32));
                        e.send(EngineCommand::SetSaturation(sa_for_start.value() as f32));
                        if let Some(&method) = FlipMethod::ALL.get(fl_for_start.selected() as usize)
                        {
                            e.send(EngineCommand::SetFlip(method));
                        }
                    }
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
                EngineEvent::MetricsUpdated(m) => {
                    diag.update(&m);
                }
            }
        }
        glib::ControlFlow::Continue
    });

    // --- lifecycle ------------------------------------------------------------

    let eng = Rc::clone(&engine);
    window.connect_realize(move |_| {
        if let Some(ref e) = *eng.borrow() {
            e.send(EngineCommand::Start);
        }
    });

    let eng = Rc::clone(&engine);
    let cfg = Rc::clone(&config);
    let save_deb = Rc::clone(&save_debounce);
    window.connect_close_request(move |_| {
        // Flush any pending config save
        if let Some(id) = save_deb.borrow_mut().take() {
            id.remove();
        }
        if let Err(e) = lumalens_config::save(&cfg.borrow()) {
            warn!("failed to save config on exit: {e}");
        }

        if let Some(e) = eng.borrow_mut().take() {
            e.shutdown();
        }
        glib::Propagation::Proceed
    });

    window
}

/// Apply an `ImageSettings` preset to the UI controls and engine.
#[allow(clippy::too_many_arguments)]
fn apply_image_settings(
    image: &ImageSettings,
    brightness: &gtk::Scale,
    contrast: &gtk::Scale,
    saturation: &gtk::Scale,
    flip_dropdown: &adw::ComboRow,
    engine: &Rc<RefCell<Option<lumalens_engine::runtime::Engine>>>,
    config: &Rc<RefCell<lumalens_config::Config>>,
    schedule_save: &impl Fn(),
) {
    brightness.set_value(image.brightness);
    contrast.set_value(image.contrast);
    saturation.set_value(image.saturation);

    let flip_idx = FlipMethod::ALL
        .iter()
        .position(|&m| m == image.flip)
        .unwrap_or(0);
    flip_dropdown.set_selected(flip_idx as u32);

    {
        let mut cfg = config.borrow_mut();
        cfg.image = image.clone();
    }
    schedule_save();

    if let Some(ref e) = *engine.borrow() {
        e.send(EngineCommand::SetBrightness(image.brightness as f32));
        e.send(EngineCommand::SetContrast(image.contrast as f32));
        e.send(EngineCommand::SetSaturation(image.saturation as f32));
        e.send(EngineCommand::SetFlip(image.flip));
    }
}

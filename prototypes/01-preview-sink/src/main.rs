use gstreamer as gst;
use gstreamer::prelude::*;
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use tracing::{error, info, warn};

use std::sync::mpsc;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .init();

    gst::init().expect("failed to init gstreamer");
    gtk::init().expect("failed to init gtk4");

    let app = adw::Application::builder()
        .application_id("dev.lumalens.preview-sink")
        .build();

    app.connect_activate(build_ui);

    {
        let app = app.clone();
        let (tx, rx) = mpsc::channel();
        ctrlc::set_handler(move || { let _ = tx.send(()); }).ok();
        gtk::glib::idle_add_local(move || {
            if rx.try_recv().is_ok() {
                info!("Ctrl+C received, quitting");
                app.quit();
            }
            gtk::glib::ControlFlow::Continue
        });
    }

    app.run();
}

fn build_ui(app: &adw::Application) {
    let pipeline = gst::parse::launch(
        "v4l2src device=/dev/video0 ! videoconvert ! gtk4paintablesink name=sink",
    )
    .expect("failed to create pipeline")
    .downcast::<gst::Pipeline>()
    .expect("not a pipeline");

    let sink = pipeline
        .by_name("sink")
        .expect("sink element not found");

    let paintable = sink
        .property::<gtk::gdk::Paintable>("paintable");

    let picture = gtk::Picture::for_paintable(&paintable);
    picture.set_halign(gtk::Align::Fill);
    picture.set_valign(gtk::Align::Fill);
    picture.set_hexpand(true);
    picture.set_vexpand(true);

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("LumaLens – Preview Sink")
        .default_width(1280)
        .default_height(720)
        .content(&picture)
        .build();

    let bus = pipeline.bus().expect("pipeline has no bus");
    let pipeline_weak = pipeline.downgrade();

    let _bus_watch = bus.add_watch_local(move |_, msg| {
        use gst::MessageView;
        match msg.view() {
            MessageView::StateChanged(s) => {
                if s.src().map(|e| e == pipeline_weak.upgrade().unwrap().upcast_ref::<gst::Object>())
                    .unwrap_or(false)
                {
                    info!(
                        "pipeline state: {:?} -> {:?}",
                        s.old(),
                        s.current()
                    );
                }
            }
            MessageView::Error(e) => {
                error!(
                    "pipeline error: {} (debug: {:?})",
                    e.error(),
                    e.debug()
                );
            }
            MessageView::Warning(w) => {
                warn!(
                    "pipeline warning: {} (debug: {:?})",
                    w.error(),
                    w.debug()
                );
            }
            MessageView::Eos(_) => {
                info!("end of stream");
            }
            _ => {}
        }
        gst::glib::ControlFlow::Continue
    })
    .expect("failed to add bus watch");

    pipeline
        .set_state(gst::State::Playing)
        .expect("failed to start pipeline");

    let pipeline_for_close = pipeline.clone();
    window.connect_close_request(move |_| {
        info!("window closed, tearing down pipeline");
        pipeline_for_close
            .set_state(gst::State::Null)
            .expect("failed to stop pipeline");
        gtk::glib::Propagation::Proceed
    });

    window.present();

    let pipeline_for_shutdown = pipeline.clone();
    app.connect_shutdown(move |_| {
        info!("application shutdown, ensuring pipeline is Null");
        let _ = pipeline_for_shutdown.set_state(gst::State::Null);
    });
}


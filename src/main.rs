use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    gstreamer::init().expect("failed to initialise GStreamer");
    gtk::init().expect("failed to initialise GTK4");

    let app = adw::Application::builder()
        .application_id("io.github.lumalens.LumaLens")
        .build();

    app.connect_activate(|app| {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("LumaLens")
            .default_width(1280)
            .default_height(720)
            .build();

        window.present();
    });

    app.run();
}

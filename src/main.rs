#![allow(unused)]

mod app;
mod config;
mod engine;
mod prelude;
mod ui;

fn main() -> gtk4::glib::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    gstreamer::init().expect("failed to initialise GStreamer");
    gtk4::init().expect("failed to initialise GTK4");

    app::run()
}

use gstreamer as gst;
use gstreamer::prelude::*;
use tracing::{error, info, warn};

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .init();

    gst::init().expect("failed to init gstreamer");

    if !std::path::Path::new("/dev/video10").exists() {
        eprintln!("ERROR: /dev/video10 does not exist.");
        eprintln!("Load v4l2loopback first:");
        eprintln!("  sudo modprobe v4l2loopback exclusive_caps=1 video_nr=10 card_label=\"CamForge Test\"");
        std::process::exit(1);
    }

    let pipeline = gst::parse::launch(concat!(
        "v4l2src device=/dev/video0 ! ",
        "image/jpeg,width=1280,height=720,framerate=30/1 ! ",
        "jpegdec ! ",
        "videoconvert ! ",
        "video/x-raw,format=YUY2,width=1280,height=720,framerate=30/1 ! ",
        "tee name=t ",
        "t. ! queue ! fakesink ",
        "t. ! queue ! ",
        "filesink location=/dev/video10 sync=false buffer-mode=unbuffered",
    ))
    .expect("failed to create pipeline")
    .downcast::<gst::Pipeline>()
    .expect("not a pipeline");

    pipeline
        .set_state(gst::State::Playing)
        .expect("failed to start pipeline");

    info!("pipeline is Playing, waiting for caps to negotiate...");
    thread::sleep(Duration::from_secs(1));

    print_tee_caps(&pipeline);

    {
        let device = "/dev/video10";
        info!("querying v4l2 output device after 2s settle...");
        thread::sleep(Duration::from_secs(1));
        match Command::new("v4l2-ctl")
            .args(["--device", device, "--all"])
            .output()
        {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                info!("v4l2-ctl --device={device} --all:\n{stdout}");
                if !stderr.is_empty() {
                    warn!("v4l2-ctl stderr: {stderr}");
                }
            }
            Err(e) => error!("failed to run v4l2-ctl: {e}"),
        }
    }

    let running = Arc::new(AtomicBool::new(true));
    {
        let running = running.clone();
        ctrlc::set_handler(move || {
            info!("Ctrl+C received");
            running.store(false, Ordering::SeqCst);
        })
        .expect("failed to set Ctrl+C handler");
    }

    let bus = pipeline.bus().expect("pipeline has no bus");
    while running.load(Ordering::SeqCst) {
        if let Some(msg) = bus.timed_pop(gst::ClockTime::from_mseconds(250)) {
            use gst::MessageView;
            match msg.view() {
                MessageView::Error(e) => {
                    error!("pipeline error: {} (debug: {:?})", e.error(), e.debug());
                    break;
                }
                MessageView::Warning(w) => {
                    warn!("pipeline warning: {} (debug: {:?})", w.error(), w.debug());
                }
                MessageView::Eos(_) => {
                    info!("end of stream");
                    break;
                }
                _ => {}
            }
        }
    }

    info!("shutting down pipeline");
    pipeline
        .set_state(gst::State::Null)
        .expect("failed to stop pipeline");
    info!("pipeline stopped, exiting");
}

fn print_tee_caps(pipeline: &gst::Pipeline) {
    let tee = match pipeline.by_name("t") {
        Some(t) => t,
        None => {
            warn!("tee element 't' not found");
            return;
        }
    };

    for pad in tee.src_pads() {
        let peer = pad.peer();
        let pad_name = pad.name();
        let peer_name = peer
            .as_ref()
            .map(|p| format!("{}:{}", p.parent_element().map_or("?".into(), |e| e.name().to_string()), p.name()))
            .unwrap_or_else(|| "(unlinked)".into());

        let caps = pad.current_caps();
        info!(
            "tee pad {pad_name} -> {peer_name}: caps = {}",
            caps.map_or("(not negotiated)".into(), |c| c.to_string())
        );
    }
}

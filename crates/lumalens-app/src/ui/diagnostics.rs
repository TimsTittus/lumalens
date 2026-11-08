//! Floating diagnostics panel (Ctrl+Alt+D).

use std::cell::RefCell;
use std::rc::Rc;

use gtk4 as gtk;
use gtk4::gdk;
use gtk4::prelude::*;
use libadwaita as adw;

use lumalens_engine::events::Metrics;

pub struct DiagnosticsPanel {
    pub window: adw::Window,
    input_fps: gtk::Label,
    output_fps: gtk::Label,
    dropped_frames: gtk::Label,
    processing_ms: gtk::Label,
    queue_preview: gtk::Label,
    queue_virtual: gtk::Label,
    ai_inference: gtk::Label,
    ai_mask_age: gtk::Label,
    last_metrics: Rc<RefCell<Metrics>>,
}

impl DiagnosticsPanel {
    pub fn new(parent: &adw::ApplicationWindow) -> Self {
        let grid = gtk::Grid::builder()
            .row_spacing(4)
            .column_spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        let labels = [
            "Input FPS",
            "Output FPS",
            "Dropped Frames",
            "Processing (ms)",
            "Queue: Preview",
            "Queue: Virtual Cam",
            "AI Inference (ms)",
            "AI Mask Age (ms)",
        ];

        let mut value_labels = Vec::new();
        for (i, name) in labels.iter().enumerate() {
            let name_label = gtk::Label::builder()
                .label(*name)
                .halign(gtk::Align::Start)
                .css_classes(["dim-label"])
                .build();
            let value_label = gtk::Label::builder()
                .label("—")
                .halign(gtk::Align::End)
                .hexpand(true)
                .css_classes(["monospace"])
                .build();
            grid.attach(&name_label, 0, i as i32, 1, 1);
            grid.attach(&value_label, 1, i as i32, 1, 1);
            value_labels.push(value_label);
        }

        let copy_btn = gtk::Button::builder()
            .label("Copy Diagnostic Bundle")
            .css_classes(["suggested-action"])
            .margin_top(12)
            .build();

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&grid);
        content.append(&copy_btn);

        let header = adw::HeaderBar::builder()
            .show_end_title_buttons(true)
            .build();

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&content));

        let window = adw::Window::builder()
            .title("Diagnostics")
            .default_width(320)
            .default_height(300)
            .resizable(false)
            .modal(false)
            .transient_for(parent)
            .content(&toolbar)
            .hide_on_close(true)
            .build();

        let last_metrics: Rc<RefCell<Metrics>> = Rc::new(RefCell::new(Metrics::default()));

        // Copy button: build a diagnostic bundle string
        {
            let metrics = Rc::clone(&last_metrics);
            let win = window.clone();
            copy_btn.connect_clicked(move |_| {
                let bundle = build_diagnostic_bundle(&metrics.borrow());
                if let Some(display) = gdk::Display::default() {
                    let clipboard = display.clipboard();
                    clipboard.set_text(&bundle);
                }

                let toast = adw::Toast::new("Diagnostic bundle copied");
                toast.set_timeout(2);
                if let Some(overlay) = find_toast_overlay(&win) {
                    overlay.add_toast(toast);
                }
            });
        }

        Self {
            window,
            input_fps: value_labels[0].clone(),
            output_fps: value_labels[1].clone(),
            dropped_frames: value_labels[2].clone(),
            processing_ms: value_labels[3].clone(),
            queue_preview: value_labels[4].clone(),
            queue_virtual: value_labels[5].clone(),
            ai_inference: value_labels[6].clone(),
            ai_mask_age: value_labels[7].clone(),
            last_metrics,
        }
    }

    pub fn update(&self, m: &Metrics) {
        self.input_fps.set_label(&format!("{:.1}", m.input_fps));
        self.output_fps.set_label(&format!("{:.1}", m.output_fps));
        self.dropped_frames
            .set_label(&format!("{}", m.dropped_frames));
        self.processing_ms
            .set_label(&format!("{:.2}", m.processing_ms));
        self.queue_preview
            .set_label(&format!("{}", m.queue_depth_preview));
        self.queue_virtual
            .set_label(&format!("{}", m.queue_depth_virtual));
        self.ai_inference.set_label(
            &m.ai_inference_ms
                .map(|v| format!("{v:.1}"))
                .unwrap_or_else(|| "—".into()),
        );
        self.ai_mask_age.set_label(
            &m.ai_mask_age_ms
                .map(|v| format!("{v}"))
                .unwrap_or_else(|| "—".into()),
        );
        *self.last_metrics.borrow_mut() = m.clone();
    }

    pub fn toggle(&self) {
        if self.window.is_visible() {
            self.window.set_visible(false);
        } else {
            self.window.present();
        }
    }
}

fn find_toast_overlay(_win: &adw::Window) -> Option<adw::ToastOverlay> {
    // The diagnostics window is simple; no toast overlay embedded.
    // Just skip the toast in the diagnostics window itself.
    None
}

fn build_diagnostic_bundle(metrics: &Metrics) -> String {
    let mut out = String::new();

    out.push_str("=== LumaLens Diagnostic Bundle ===\n\n");

    // Metrics
    out.push_str("--- Metrics ---\n");
    out.push_str(&format!("Input FPS:         {:.1}\n", metrics.input_fps));
    out.push_str(&format!("Output FPS:        {:.1}\n", metrics.output_fps));
    out.push_str(&format!("Dropped Frames:    {}\n", metrics.dropped_frames));
    out.push_str(&format!(
        "Processing (ms):   {:.2}\n",
        metrics.processing_ms
    ));
    out.push_str(&format!(
        "Queue Preview:     {}\n",
        metrics.queue_depth_preview
    ));
    out.push_str(&format!(
        "Queue Virtual Cam: {}\n",
        metrics.queue_depth_virtual
    ));
    out.push_str(&format!(
        "AI Inference (ms): {}\n",
        metrics
            .ai_inference_ms
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "N/A".into())
    ));
    out.push_str(&format!(
        "AI Mask Age (ms):  {}\n",
        metrics
            .ai_mask_age_ms
            .map(|v| format!("{v}"))
            .unwrap_or_else(|| "N/A".into())
    ));

    // GStreamer version
    let gst_ver = gstreamer::version();
    out.push_str(&format!(
        "\n--- GStreamer ---\n{}.{}.{}.{}\n",
        gst_ver.0, gst_ver.1, gst_ver.2, gst_ver.3
    ));

    // GTK version
    out.push_str(&format!(
        "\n--- GTK ---\n{}.{}.{}\n",
        gtk::major_version(),
        gtk::minor_version(),
        gtk::micro_version()
    ));

    // Libadwaita version
    out.push_str(&format!(
        "\n--- Libadwaita ---\n{}.{}.{}\n",
        adw::major_version(),
        adw::minor_version(),
        adw::micro_version()
    ));

    // Distro info
    out.push_str("\n--- System ---\n");
    if let Ok(release) = std::fs::read_to_string("/etc/os-release") {
        for line in release.lines() {
            if line.starts_with("PRETTY_NAME=")
                || line.starts_with("NAME=")
                || line.starts_with("VERSION=")
            {
                out.push_str(line);
                out.push('\n');
            }
        }
    } else {
        out.push_str("(could not read /etc/os-release)\n");
    }

    // Last 100 log lines from the RUST_LOG ring buffer are not easily
    // available without a custom tracing subscriber. Include a note.
    out.push_str("\n--- Logs ---\n");
    out.push_str("(attach stderr output or run with RUST_LOG=debug)\n");

    out
}

//! Camera control widgets.

use gtk4 as gtk;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use lumalens_common::FlipMethod;

pub struct Controls {
    pub root: adw::PreferencesGroup,
    pub output_group: adw::PreferencesGroup,
    pub camera_row: adw::ComboRow,
    pub brightness: gtk::Scale,
    pub contrast: gtk::Scale,
    pub saturation: gtk::Scale,
    pub flip_dropdown: adw::ComboRow,
    pub virtual_cam_switch: gtk::Switch,
}

impl Controls {
    pub fn new() -> Self {
        let camera_row = adw::ComboRow::builder()
            .title("Camera")
            .model(&gtk::StringList::new(&[]))
            .build();

        let brightness = gtk::Scale::builder()
            .orientation(gtk::Orientation::Horizontal)
            .hexpand(true)
            .draw_value(true)
            .build();
        brightness.set_range(-1.0, 1.0);
        brightness.set_increments(0.05, 0.1);
        brightness.set_value(0.0);

        let brightness_row = adw::ActionRow::builder().title("Brightness").build();
        brightness_row.add_suffix(&brightness);

        let contrast = gtk::Scale::builder()
            .orientation(gtk::Orientation::Horizontal)
            .hexpand(true)
            .draw_value(true)
            .build();
        contrast.set_range(0.0, 2.0);
        contrast.set_increments(0.05, 0.1);
        contrast.set_value(1.0);

        let contrast_row = adw::ActionRow::builder().title("Contrast").build();
        contrast_row.add_suffix(&contrast);

        let saturation = gtk::Scale::builder()
            .orientation(gtk::Orientation::Horizontal)
            .hexpand(true)
            .draw_value(true)
            .build();
        saturation.set_range(0.0, 2.0);
        saturation.set_increments(0.05, 0.1);
        saturation.set_value(1.0);

        let saturation_row = adw::ActionRow::builder().title("Saturation").build();
        saturation_row.add_suffix(&saturation);

        let flip_labels: Vec<&str> = FlipMethod::ALL.iter().map(|m| m.label()).collect();
        let flip_dropdown = adw::ComboRow::builder()
            .title("Mirror / Rotate")
            .model(&gtk::StringList::new(&flip_labels))
            .selected(0)
            .build();

        let root = adw::PreferencesGroup::builder().title("Camera").build();
        root.add(&camera_row);
        root.add(&brightness_row);
        root.add(&contrast_row);
        root.add(&saturation_row);
        root.add(&flip_dropdown);

        let virtual_cam_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();

        let vcam_row = adw::ActionRow::builder().title("Virtual Camera").build();
        vcam_row.add_suffix(&virtual_cam_switch);
        vcam_row.set_activatable_widget(Some(&virtual_cam_switch));

        let output_group = adw::PreferencesGroup::builder().title("Output").build();
        output_group.add(&vcam_row);

        Self {
            root,
            output_group,
            camera_row,
            brightness,
            contrast,
            saturation,
            flip_dropdown,
            virtual_cam_switch,
        }
    }
}

//! Camera control widgets.

use gtk4 as gtk;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use lumalens_common::FlipMethod;

pub struct Controls {
    pub root: adw::PreferencesGroup,
    pub output_group: adw::PreferencesGroup,
    pub preset_group: adw::PreferencesGroup,
    pub camera_row: adw::ComboRow,
    pub resolution_dropdown: adw::ComboRow,
    pub brightness: gtk::Scale,
    pub contrast: gtk::Scale,
    pub saturation: gtk::Scale,
    pub flip_dropdown: adw::ComboRow,
    pub virtual_cam_switch: gtk::Switch,
    pub preset_dropdown: adw::ComboRow,
    pub apply_preset_btn: gtk::Button,
    pub save_preset_btn: gtk::Button,
    pub delete_preset_btn: gtk::Button,
}

impl Controls {
    pub fn new() -> Self {
        let camera_row = adw::ComboRow::builder()
            .title("Camera")
            .model(&gtk::StringList::new(&[]))
            .build();

        let resolution_dropdown = adw::ComboRow::builder()
            .title("Resolution")
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
        root.add(&resolution_dropdown);
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

        // --- presets ---

        let preset_dropdown = adw::ComboRow::builder()
            .title("Preset")
            .model(&gtk::StringList::new(&[]))
            .build();

        let apply_preset_btn = gtk::Button::builder()
            .label("Apply")
            .css_classes(["suggested-action"])
            .build();

        let save_preset_btn = gtk::Button::builder().label("Save As…").build();

        let delete_preset_btn = gtk::Button::builder()
            .label("Delete")
            .css_classes(["destructive-action"])
            .build();

        let btn_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .halign(gtk::Align::Center)
            .margin_top(6)
            .margin_bottom(6)
            .build();
        btn_box.append(&apply_preset_btn);
        btn_box.append(&save_preset_btn);
        btn_box.append(&delete_preset_btn);

        let preset_group = adw::PreferencesGroup::builder().title("Presets").build();
        preset_group.add(&preset_dropdown);
        preset_group.add(&btn_box);

        Self {
            root,
            output_group,
            preset_group,
            camera_row,
            resolution_dropdown,
            brightness,
            contrast,
            saturation,
            flip_dropdown,
            virtual_cam_switch,
            preset_dropdown,
            apply_preset_btn,
            save_preset_btn,
            delete_preset_btn,
        }
    }
}

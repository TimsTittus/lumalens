//! Effects reorder page — lists effects grouped by stage with drag reordering.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4 as gtk;
use gtk4::gdk;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use lumalens_effects::Stage;
use lumalens_engine::commands::EngineCommand;
use lumalens_engine::runtime::Engine;

struct EffectMeta {
    id: &'static str,
    name: &'static str,
    stage: Stage,
}

const DEFAULT_EFFECTS: &[EffectMeta] = &[
    EffectMeta {
        id: "brightness",
        name: "Brightness",
        stage: Stage::Correction,
    },
    EffectMeta {
        id: "contrast",
        name: "Contrast",
        stage: Stage::Correction,
    },
    EffectMeta {
        id: "saturation",
        name: "Saturation",
        stage: Stage::Correction,
    },
    EffectMeta {
        id: "flip",
        name: "Flip",
        stage: Stage::Transform,
    },
    EffectMeta {
        id: "rotate",
        name: "Rotate",
        stage: Stage::Transform,
    },
];

/// Build the effects reorder navigation page.
pub fn build(
    engine: Rc<RefCell<Option<Engine>>>,
    config: Rc<RefCell<lumalens_config::Config>>,
    schedule_save: impl Fn() + 'static,
) -> adw::NavigationPage {
    let schedule_save: Rc<dyn Fn()> = Rc::new(schedule_save);

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let header = adw::HeaderBar::new();
    outer.append(&header);

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);

    let saved_order = config.borrow().effects.order.clone();

    for &stage in &Stage::EFFECT_STAGES {
        let mut effects: Vec<(&'static str, &'static str)> = DEFAULT_EFFECTS
            .iter()
            .filter(|e| e.stage == stage)
            .map(|e| (e.id, e.name))
            .collect();

        if effects.is_empty() {
            continue;
        }

        if let Some(saved) = saved_order.get(stage.as_str()) {
            effects.sort_by_key(|(id, _)| {
                saved
                    .iter()
                    .position(|s| s.as_str() == *id)
                    .unwrap_or(usize::MAX)
            });
        }

        let group = build_stage_group(stage, effects, &engine, &config, &schedule_save);
        content.append(&group);
    }

    scroll.set_child(Some(&content));
    outer.append(&scroll);

    adw::NavigationPage::builder()
        .title("Effects")
        .tag("effects")
        .child(&outer)
        .build()
}

fn build_stage_group(
    stage: Stage,
    effects: Vec<(&'static str, &'static str)>,
    engine: &Rc<RefCell<Option<Engine>>>,
    config: &Rc<RefCell<lumalens_config::Config>>,
    schedule_save: &Rc<dyn Fn()>,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(stage.label())
        .build();

    let listbox = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .build();
    listbox.add_css_class("boxed-list");

    let stage_order: Rc<RefCell<Vec<(&'static str, &'static str)>>> =
        Rc::new(RefCell::new(effects));

    {
        let order = stage_order.borrow();
        for &(_id, name) in order.iter() {
            let row = build_effect_row(
                name,
                stage,
                &stage_order,
                &listbox,
                engine,
                config,
                schedule_save,
            );
            listbox.append(&row);
        }
    }

    group.add(&listbox);
    group
}

fn build_effect_row(
    name: &str,
    stage: Stage,
    stage_order: &Rc<RefCell<Vec<(&'static str, &'static str)>>>,
    listbox: &gtk::ListBox,
    engine: &Rc<RefCell<Option<Engine>>>,
    config: &Rc<RefCell<lumalens_config::Config>>,
    schedule_save: &Rc<dyn Fn()>,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(name).build();

    let handle = gtk::Image::from_icon_name("list-drag-handle-symbolic");
    handle.add_css_class("dim-label");
    row.add_prefix(&handle);

    // --- DragSource on the drag handle ---
    let drag = gtk::DragSource::new();
    drag.set_actions(gdk::DragAction::MOVE);

    let row_for_drag = row.clone();
    drag.connect_prepare(move |_, _, _| {
        let idx = row_for_drag.index();
        if idx < 0 {
            return None;
        }
        Some(gdk::ContentProvider::for_value(&(idx as u32).to_value()))
    });

    handle.add_controller(drag);

    // --- DropTarget on the row ---
    let drop = gtk::DropTarget::new(u32::static_type(), gdk::DragAction::MOVE);

    let target_row = row.clone();
    let lb = listbox.clone();
    let ord = Rc::clone(stage_order);
    let eng = Rc::clone(engine);
    let cfg = Rc::clone(config);
    let sched = Rc::clone(schedule_save);

    drop.connect_drop(move |_, value, _, _| {
        let Ok(from) = value.get::<u32>() else {
            return false;
        };
        let from = from as usize;
        let to = target_row.index() as usize;

        if from == to {
            return false;
        }

        {
            let mut order = ord.borrow_mut();
            if from >= order.len() || to >= order.len() {
                return false;
            }
            order.swap(from, to);
        }

        if let Some(ref e) = *eng.borrow() {
            e.send(EngineCommand::ReorderEffect { stage, from, to });
        }

        {
            let mut c = cfg.borrow_mut();
            let order = ord.borrow();
            let ids: Vec<String> = order.iter().map(|(id, _)| id.to_string()).collect();
            c.effects.order.insert(stage.as_str().to_string(), ids);
        }
        sched();

        if let (Some(from_row), Some(to_row)) = (
            lb.row_at_index(from as i32)
                .and_then(|r| r.downcast::<adw::ActionRow>().ok()),
            lb.row_at_index(to as i32)
                .and_then(|r| r.downcast::<adw::ActionRow>().ok()),
        ) {
            let ft = from_row.title();
            let tt = to_row.title();
            from_row.set_title(&tt);
            to_row.set_title(&ft);
        }

        true
    });

    row.add_controller(drop);

    row
}

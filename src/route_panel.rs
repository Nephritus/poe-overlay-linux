//! The overlay panel showing the Steps for the Character's current Area.

use gtk::prelude::*;

use crate::poe1::route::RouteProgress;

pub const CSS: &str = "
.route-area { font-weight: bold; color: rgb(230, 190, 110); }
.route-step { margin-top: 2px; }
.route-sub-step { font-size: smaller; color: rgb(190, 190, 200); margin-left: 12px; }
.route-note { font-size: smaller; color: rgb(230, 120, 100); }
";

const WIDTH: i32 = 340;

pub struct RoutePanel {
    root: gtk::Box,
    area: gtk::Label,
    steps: gtk::Box,
    note: gtk::Label,
}

impl RoutePanel {
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 4);
        root.add_css_class("panel");
        root.set_size_request(WIDTH, -1);
        let area = label("", "route-area");
        let steps = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let note = label("", "route-note");
        note.set_visible(false);
        root.append(&area);
        root.append(&steps);
        root.append(&note);
        Self { root, area, steps, note }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn show(&self, progress: &RouteProgress) {
        self.area.set_label(progress.area_name());
        while let Some(child) = self.steps.first_child() {
            self.steps.remove(&child);
        }
        for step in progress.steps() {
            self.steps.append(&label(&step.text, "route-step"));
            for sub in &step.sub_steps {
                self.steps.append(&label(sub, "route-sub-step"));
            }
        }
    }

    /// A problem that stops the Route from following the game.
    pub fn set_note(&self, text: &str) {
        self.note.set_label(text);
        self.note.set_visible(true);
    }
}

fn label(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::builder().label(text).xalign(0.0).wrap(true).max_width_chars(1).build();
    label.add_css_class(class);
    label
}

//! The overlay panel showing the Steps for the Character's current Area.

use gtk::prelude::*;

use crate::poe1::route::RouteProgress;

pub const CSS: &str = "
.route-area { font-weight: bold; color: rgb(230, 190, 110); }
.route-character { font-size: smaller; color: rgb(160, 160, 170); }
.route-nav { min-width: 0; min-height: 0; padding: 0 6px; }
.route-step { margin-top: 2px; }
.route-sub-step { font-size: smaller; color: rgb(190, 190, 200); margin-left: 12px; }
.route-note { font-size: smaller; color: rgb(230, 120, 100); }
.route-off { font-size: smaller; font-style: italic; color: rgb(160, 160, 170); }
";

const WIDTH: i32 = 340;

/// Widgets are reference-counted, so clones share the same panel.
#[derive(Clone)]
pub struct RoutePanel {
    root: gtk::Box,
    area: gtk::Label,
    character: gtk::Label,
    previous: gtk::Button,
    next: gtk::Button,
    off_route: gtk::Label,
    steps: gtk::Box,
    note: gtk::Label,
}

impl RoutePanel {
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 4);
        root.add_css_class("panel");
        root.set_size_request(WIDTH, -1);

        let area = label("", "route-area");
        area.set_hexpand(true);
        let previous = nav_button("◀", "Previous Area visit");
        let next = nav_button("▶", "Next Area visit");
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        header.append(&area);
        header.append(&previous);
        header.append(&next);

        let character = label("", "route-character");
        let off_route = label("", "route-off");
        let steps = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let note = label("", "route-note");
        note.set_visible(false);
        root.append(&header);
        root.append(&character);
        root.append(&off_route);
        root.append(&steps);
        root.append(&note);
        Self { root, area, character, previous, next, off_route, steps, note }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn on_previous(&self, f: impl Fn() + 'static) {
        self.previous.connect_clicked(move |_| f());
    }

    pub fn on_next(&self, f: impl Fn() + 'static) {
        self.next.connect_clicked(move |_| f());
    }

    /// Shows `progress` for `character` (`None`: not identified yet).
    pub fn show(&self, progress: &RouteProgress, character: Option<&str>) {
        self.area.set_label(&format!("Act {} · {}", progress.act(), progress.area_name()));
        self.character.set_label(match character {
            Some(name) => name,
            None => "Character not known yet: progress is saved after its next level-up",
        });
        self.off_route.set_visible(progress.is_off_route());
        self.off_route.set_label(&match progress.character_area_name() {
            Some(name) if progress.is_off_route() => format!("You're in {name}, which the Route doesn't visit."),
            _ => "You're off the Route.".into(),
        });
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

    /// Adds a problem the player should know about (e.g. the Route can't
    /// follow the game); notes stack.
    pub fn set_note(&self, text: &str) {
        let label = match self.note.label().as_str() {
            "" => text.to_owned(),
            existing => format!("{existing}\n{text}"),
        };
        self.note.set_label(&label);
        self.note.set_visible(true);
    }
}

fn label(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::builder().label(text).xalign(0.0).wrap(true).max_width_chars(1).build();
    label.add_css_class(class);
    label
}

/// A pointer-only button: the overlay never takes keyboard focus.
fn nav_button(text: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::builder().label(text).tooltip_text(tooltip).focusable(false).focus_on_click(false).build();
    button.add_css_class("route-nav");
    button.add_css_class("flat");
    button
}

//! The Overlay: a full-screen, see-through layer-shell window over the game
//! that holds Panels. It never takes keyboard focus, shows only while the
//! game is focused (on the game's output), and only shown Panels catch
//! clicks; everything else passes through to the game.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::gdk;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::focus::Visibility;

/// Styling shared by every Panel; Panels add their own via [`OverlaySurface::new`].
const BASE_CSS: &str = "
window.poe-overlay { background: transparent; }
.panel {
  background: rgba(20, 20, 30, 0.8);
  border: 1px solid rgb(204, 153, 51);
  border-radius: 8px;
  padding: 12px;
  color: white;
}
.panel-title { font-weight: bold; color: rgb(230, 190, 110); }
.panel-button { min-width: 0; min-height: 0; padding: 0 6px; }
.panel-name { margin-right: 12px; }
.panel-number { font-feature-settings: 'tnum'; margin-left: 12px; }
.panel-head, .panel-status { font-size: smaller; color: rgb(160, 160, 170); }
.panel-error { font-size: smaller; color: rgb(230, 120, 100); }
";

/// How long a Panel stays shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind {
    /// Shown whenever the Overlay is (the Route Panel).
    Always,
    /// Shown by an Action; closes when the game loses focus.
    Toggled,
}

struct Panel {
    widget: gtk::Widget,
    kind: PanelKind,
}

/// Widgets and the window are reference-counted; clones share the Overlay.
#[derive(Clone)]
pub struct OverlaySurface {
    window: gtk::ApplicationWindow,
    fixed: gtk::Fixed,
    panels: Rc<RefCell<Vec<Panel>>>,
}

impl OverlaySurface {
    /// Creates the (hidden) Overlay window; `css` is each Panel's styling.
    pub fn new(app: &gtk::Application, css: &[&str]) -> Self {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&css.iter().fold(String::from(BASE_CSS), |all, panel| all + panel));
        gtk::style_context_add_provider_for_display(
            &gdk::Display::default().expect("GDK display"),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let window = gtk::ApplicationWindow::new(app);
        window.add_css_class("poe-overlay");
        window.set_decorated(false);
        window.init_layer_shell();
        window.set_namespace(Some("poe-overlay"));
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::None);
        window.set_exclusive_zone(-1);
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            window.set_anchor(edge, true);
        }
        let fixed = gtk::Fixed::new();
        window.set_child(Some(&fixed));

        let surface = Self { window, fixed, panels: Rc::default() };
        surface.track_click_area();
        surface
    }

    /// Places a Panel at `(x, y)` logical pixels from the output's top left.
    /// `Toggled` Panels start hidden.
    pub fn add_panel(&self, widget: &impl IsA<gtk::Widget>, x: f64, y: f64, kind: PanelKind) {
        let widget = widget.as_ref().clone();
        widget.set_visible(kind == PanelKind::Always);
        self.fixed.put(&widget, x, y);
        self.panels.borrow_mut().push(Panel { widget, kind });
    }

    /// Shows or hides a `Toggled` Panel; returns whether it is shown now.
    /// See [`OverlaySurface::show`].
    pub fn toggle(&self, widget: &impl IsA<gtk::Widget>) -> bool {
        if widget.as_ref().is_visible() {
            self.hide(widget);
            false
        } else {
            self.show(widget)
        }
    }

    /// Shows a `Toggled` Panel in place of any other shown one (they share
    /// the space next to the Route Panel); returns whether it is shown.
    /// Ignored while the game isn't focused, so a Panel never pops up later
    /// out of nowhere.
    pub fn show(&self, widget: &impl IsA<gtk::Widget>) -> bool {
        let widget = widget.as_ref();
        if !self.window.is_visible() {
            return false;
        }
        for panel in self.panels.borrow().iter().filter(|p| p.kind == PanelKind::Toggled && p.widget != *widget) {
            panel.widget.set_visible(false);
        }
        widget.set_visible(true);
        true
    }

    pub fn hide(&self, widget: &impl IsA<gtk::Widget>) {
        widget.as_ref().set_visible(false);
    }

    /// Moves a Panel to `(x, y)` logical pixels from the output's top left.
    pub fn move_panel(&self, widget: &impl IsA<gtk::Widget>, x: f64, y: f64) {
        self.fixed.move_(widget, x, y);
    }

    /// The output's width in logical pixels (0 before it is first shown).
    pub fn width(&self) -> i32 {
        self.window.width()
    }

    /// Whether the game is focused (the Overlay shows only then).
    pub fn game_focused(&self) -> bool {
        self.window.is_visible()
    }

    /// Follows the game window's focus: shows the Overlay on the game's
    /// output, or hides it and closes `Toggled` Panels.
    pub fn follow_game(&self, visibility: &Visibility) {
        let Visibility::Shown { output } = visibility else {
            self.window.set_visible(false);
            for panel in self.panels.borrow().iter().filter(|p| p.kind == PanelKind::Toggled) {
                panel.widget.set_visible(false);
            }
            return;
        };
        let monitor = output.as_deref().and_then(monitor_named);
        if self.window.is_visible() && self.window.monitor() == monitor {
            return;
        }
        // A layer surface only moves to another output when remapped.
        self.window.set_visible(false);
        self.window.set_monitor(monitor.as_ref());
        self.window.present();
    }

    /// Keeps the surface's input region equal to the shown Panels' bounds,
    /// re-checked after every repaint since Panels resize with their content
    /// and showing or hiding one repaints the window.
    fn track_click_area(&self) {
        let surface = self.clone();
        self.window.connect_realize(move |window| {
            let gdk_surface = window.surface().expect("realized window has a surface");
            // Reused buffers: this runs on every frame.
            let applied = RefCell::new(None::<Vec<cairo::RectangleInt>>);
            let next = RefCell::new(Vec::new());
            let surface = surface.clone();
            let update = move || {
                let mut next = next.borrow_mut();
                surface.shown_panel_rects(&mut next);
                let mut applied = applied.borrow_mut();
                if applied.as_deref() != Some(&next[..]) {
                    gdk_surface.set_input_region(Some(&cairo::Region::create_rectangles(&next)));
                    match applied.as_mut() {
                        Some(applied) => std::mem::swap(applied, &mut next),
                        None => *applied = Some(next.clone()),
                    }
                }
            };
            update();
            window.frame_clock().expect("realized window has a frame clock").connect_after_paint(move |_| update());
        });
    }

    fn shown_panel_rects(&self, rects: &mut Vec<cairo::RectangleInt>) {
        rects.clear();
        let panels = self.panels.borrow();
        let shown = panels.iter().filter(|panel| panel.widget.is_visible());
        rects.extend(shown.filter_map(|panel| panel.widget.compute_bounds(&self.window)).map(|b| {
            cairo::RectangleInt::new(
                b.x().floor() as i32,
                b.y().floor() as i32,
                b.width().ceil() as i32,
                b.height().ceil() as i32,
            )
        }));
    }
}

fn monitor_named(connector: &str) -> Option<gdk::Monitor> {
    gdk::Display::default()?
        .monitors()
        .iter::<gdk::Monitor>()
        .flatten()
        .find(|m| m.connector().as_deref() == Some(connector))
}

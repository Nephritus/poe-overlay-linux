//! The Trade Requests Panel: one card per Trade Request read from Client.txt,
//! newest on top, at the top centre of the screen. Cards stay until closed;
//! nothing in the log removes them.

use gtk::prelude::*;

use crate::overlay_surface::{OverlaySurface, PanelKind};
use crate::poe1::trade_request::{Direction, TradeRequest};

pub const CSS: &str = "
.trade-card { padding: 8px 10px; }
.trade-card.outgoing { border-color: rgb(110, 160, 230); }
.trade-card-who { font-weight: bold; }
.trade-card.incoming .trade-card-who { color: rgb(230, 190, 110); }
.trade-card.outgoing .trade-card-who { color: rgb(140, 185, 240); }
.trade-card-price { font-weight: bold; }
";

/// Cards kept: a sixth pushes out the oldest.
const MAX_CARDS: usize = 5;
const WIDTH: i32 = 320;
const TOP: f64 = 60.0;

#[derive(Clone)]
pub struct TradePanel {
    root: gtk::Box,
    overlay: OverlaySurface,
}

impl TradePanel {
    /// Adds the (empty, hidden) Panel to `overlay`.
    pub fn new(overlay: &OverlaySurface) -> TradePanel {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.set_size_request(WIDTH, -1);
        overlay.add_panel(&root, 0.0, TOP, PanelKind::Always);
        root.set_visible(false);
        TradePanel { root, overlay: overlay.clone() }
    }

    /// Puts a card for `request` on top of the stack.
    pub fn add(&self, request: &TradeRequest) {
        let (card, close) = card(request);
        let (panel, closed) = (self.root.downgrade(), card.downgrade());
        close.connect_clicked(move |_| {
            if let (Some(panel), Some(card)) = (panel.upgrade(), closed.upgrade()) {
                panel.remove(&card);
                panel.set_visible(panel.first_child().is_some());
            }
        });
        self.root.prepend(&card);
        let mut child = self.root.first_child();
        for _ in 0..MAX_CARDS {
            child = child.and_then(|card| card.next_sibling());
        }
        while let Some(oldest) = child {
            child = oldest.next_sibling();
            self.root.remove(&oldest);
        }
        // Centred on the output, once its width is known.
        let width = self.overlay.width();
        if width > 0 {
            self.overlay.move_panel(&self.root, f64::from((width - WIDTH) / 2), TOP);
        }
        self.root.set_visible(true);
    }
}

/// A card: who, what, for how much, where it lies, and the buyer's note;
/// returns its ✕ button too.
fn card(request: &TradeRequest) -> (gtk::Box, gtk::Button) {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let side = match request.direction {
        Direction::Incoming => "incoming",
        Direction::Outgoing => "outgoing",
    };
    for class in ["panel", "trade-card", side] {
        card.add_css_class(class);
    }
    let who = match request.direction {
        Direction::Incoming => format!("{} wants to buy", request.character),
        Direction::Outgoing => format!("You asked {}", request.character),
    };
    let who = label(&who, "trade-card-who");
    who.set_hexpand(true);
    // Pointer-only: clicking it leaves the keyboard with the game.
    let close = gtk::Button::builder().label("✕").tooltip_text("Close").focusable(false).focus_on_click(false).build();
    close.add_css_class("panel-button");
    close.add_css_class("flat");
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    header.append(&who);
    header.append(&close);
    card.append(&header);
    card.append(&label(&request.item, "panel-name"));
    if let Some(price) = &request.price {
        card.append(&label(price, "trade-card-price"));
    }
    if let Some(stash) = &request.stash {
        card.append(&label(&format!("Tab \"{}\" · left {}, top {}", stash.tab, stash.left, stash.top), "panel-status"));
    }
    if let Some(note) = &request.note {
        card.append(&label(&format!("“{note}”"), "panel-status"));
    }
    (card, close)
}

fn label(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::builder().label(text).xalign(0.0).wrap(true).max_width_chars(1).build();
    label.add_css_class(class);
    label
}

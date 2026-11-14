//! The Price Check Panel: what the item under the pointer is worth on
//! poe.ninja. Opened by the `price-check` Action, on the half of the screen
//! away from the pointer.

use std::cell::{Cell, RefCell};
use std::fmt::Write as _;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gtk::prelude::*;

use crate::item_copier::ItemCopier;
use crate::overlay_surface::OverlaySurface;
use crate::panel_prices::{PriceTable, Prices, ask};
use crate::poe1::item::{self, Item, ItemKind};
use crate::poe1::item_copy::CopyError;
use crate::poe1::prices::ItemPrices;

const WIDTH: i32 = 320;

/// From the middle of the output, when the Panel goes on the right half.
const RIGHT_MARGIN: f64 = 40.0;

/// Widgets are reference-counted, so clones share the same Panel.
#[derive(Clone)]
pub struct PricePanel {
    root: gtk::Box,
    title: gtk::Label,
    table: Rc<RefCell<PriceTable>>,
    status: gtk::Label,
    /// Where the Panel goes on the left half (where it was added).
    left: (f64, f64),
    /// The copied text the Panel shows, to tell a second press on the same
    /// item (close) from one on another item (check it).
    text: Rc<RefCell<String>>,
    /// Counts presses, so a slow copy or answer for an earlier one is dropped.
    check: Rc<Cell<u64>>,
}

impl PricePanel {
    /// `left`: where it goes on the left half, as passed to
    /// [`OverlaySurface::add_panel`].
    pub fn new(left: (f64, f64)) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.add_css_class("panel");
        root.set_size_request(WIDTH, -1);
        let title = gtk::Label::builder().xalign(0.0).wrap(true).max_width_chars(1).hexpand(true).build();
        title.add_css_class("panel-title");
        // Pointer-only: the Overlay never takes keyboard focus.
        let close = gtk::Button::builder().label("✕").tooltip_text("Close").focusable(false).focus_on_click(false).build();
        close.add_css_class("panel-button");
        close.add_css_class("flat");
        let panel = root.clone();
        close.connect_clicked(move |_| panel.set_visible(false));
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        header.append(&title);
        header.append(&close);
        let table = PriceTable::new();
        let status = gtk::Label::builder().xalign(0.0).wrap(true).max_width_chars(1).build();
        root.append(&header);
        root.append(table.widget());
        root.append(&status);
        let table = Rc::new(RefCell::new(table));
        Self { root, title, table, status, left, text: Rc::default(), check: Rc::default() }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Shows `title` over an empty table.
    fn start(&self, title: &str) {
        self.title.set_label(title);
        self.table.borrow_mut().clear();
    }

    fn show_error(&self, title: &str, message: &str) {
        self.start(title);
        self.set_status(message, true);
    }

    fn show_prices(&self, item: &Item, prices: &ItemPrices) {
        self.start(&item.name);
        let mut table = self.table.borrow_mut();
        let mut name = String::new();
        for price in &prices.prices {
            name.clear();
            name.push_str(price.label.as_deref().unwrap_or("Each"));
            if let Some(listings) = price.listings {
                let _ = write!(name, " ({listings} listed)");
            }
            table.row(&name, price.chaos, price.divine);
        }
        if let ([price], stack @ 2..) = (&prices.prices[..], prices.stack) {
            let stack_size = f64::from(stack);
            table.row(&format!("Stack of {stack}"), price.chaos * stack_size, price.divine * stack_size);
        }
        let note = match &item.kind {
            ItemKind::UnidentifiedUnique => "Unidentified: it is one of these uniques. ",
            _ if prices.approximate => "No exact match; the closest poe.ninja lists. ",
            _ => "",
        };
        self.set_status(&format!("{note}poe.ninja · {}", prices.league), false);
    }

    fn set_status(&self, text: &str, error: bool) {
        self.status.set_label(text);
        self.status.set_css_classes(&[if error { "panel-error" } else { "panel-status" }]);
    }
}

/// One press of the Price Check hotkey. A press while the Panel is open
/// closes it at once; the copy then decides whether it opens again: not for
/// the item it showed or when nothing is hovered, yes for another item.
pub async fn price_check(copier: ItemCopier, overlay: OverlaySurface, panel: PricePanel, prices: Prices) {
    // The copy keys go to whichever window has focus: never to another program.
    if !overlay.game_focused() {
        return;
    }
    let was_open = panel.root.is_visible();
    overlay.hide(panel.widget());
    let check = panel.check.get() + 1;
    panel.check.set(check);
    let copied = copier.copy().await;
    // A later press took over.
    if panel.check.get() != check {
        return;
    }
    let text = match copied.text {
        Ok(text) => text,
        Err(CopyError::NothingCopied) if was_open => return,
        Err(err) => {
            panel.text.borrow_mut().clear();
            place(&overlay, &panel, copied.left_half);
            panel.show_error("Price Check", &err.to_string());
            overlay.show(panel.widget());
            return;
        }
    };
    if was_open && *panel.text.borrow() == text {
        return;
    }
    place(&overlay, &panel, copied.left_half);
    let parsed = item::parse(&text);
    panel.text.replace(text);
    let item = match parsed {
        Ok(item) => item,
        Err(err) => {
            panel.show_error("Price Check", &err.to_string());
            overlay.show(panel.widget());
            return;
        }
    };
    panel.start(&item.name);
    panel.set_status("Checking poe.ninja…", false);
    if !overlay.show(panel.widget()) {
        return;
    }
    let item = Arc::new(item);
    let lookup = item.clone();
    let result = ask(&prices, move |source| source.item_prices(&lookup, Instant::now())).await;
    if panel.check.get() != check {
        return;
    }
    match result {
        Ok(prices) => panel.show_prices(&item, &prices),
        Err(message) => panel.show_error(&item.name, &message),
    }
}

/// Away from the pointer: on the right half if the pointer is on the left
/// half, else (or if unknown) on the left.
fn place(overlay: &OverlaySurface, panel: &PricePanel, pointer_on_left: Option<bool>) {
    let (x, y) = match pointer_on_left {
        Some(true) => (f64::from(overlay.width()) / 2.0 + RIGHT_MARGIN, panel.left.1),
        _ => panel.left,
    };
    overlay.move_panel(panel.widget(), x, y);
}

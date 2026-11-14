//! The Currency Rates Panel: the most traded currencies of the league,
//! priced in chaos and divine. Toggled by the `currency-rates` Action.

use gtk::prelude::*;

use crate::poe1::prices::CurrencyRates;

pub const CSS: &str = "
.rates-title { font-weight: bold; color: rgb(230, 190, 110); }
.rates-name { margin-right: 12px; }
.rates-price { font-feature-settings: 'tnum'; margin-left: 12px; }
.rates-head { font-size: smaller; color: rgb(160, 160, 170); }
.rates-status { font-size: smaller; color: rgb(160, 160, 170); }
.rates-error { font-size: smaller; color: rgb(230, 120, 100); }
";

#[derive(Clone)]
pub struct CurrencyPanel {
    root: gtk::Box,
    title: gtk::Label,
    grid: gtk::Grid,
    status: gtk::Label,
}

impl CurrencyPanel {
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.add_css_class("panel");
        let title = label("Currency Rates", "rates-title");
        let grid = gtk::Grid::new();
        let status = label("", "rates-status");
        root.append(&title);
        root.append(&grid);
        root.append(&status);
        Self { root, title, grid, status }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn show_loading(&self) {
        self.set_status("Loading prices from poe.ninja…", false);
    }

    pub fn show_rates(&self, rates: &CurrencyRates) {
        self.title.set_label(&format!("Currency Rates · {}", rates.league));
        while let Some(child) = self.grid.first_child() {
            self.grid.remove(&child);
        }
        self.grid.attach(&label("", "rates-head"), 0, 0, 1, 1);
        self.grid.attach(&price_label("chaos", "rates-head"), 1, 0, 1, 1);
        self.grid.attach(&price_label("divine", "rates-head"), 2, 0, 1, 1);
        for (row, rate) in (1..).zip(&rates.rates) {
            self.grid.attach(&label(&rate.name, "rates-name"), 0, row, 1, 1);
            self.grid.attach(&price_label(&format_price(rate.chaos), "rates-price"), 1, row, 1, 1);
            self.grid.attach(&price_label(&format_price(rate.divine), "rates-price"), 2, row, 1, 1);
        }
        self.set_status("Prices from poe.ninja", false);
    }

    /// Shows a problem; prices already shown stay, marked by the message.
    pub fn show_error(&self, message: &str) {
        self.set_status(message, true);
    }

    fn set_status(&self, text: &str, error: bool) {
        self.status.set_label(text);
        self.status.set_css_classes(&[if error { "rates-error" } else { "rates-status" }]);
    }
}

fn label(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::builder().label(text).xalign(0.0).build();
    label.add_css_class(class);
    label
}

fn price_label(text: &str, class: &str) -> gtk::Label {
    let label = label(text, class);
    label.set_xalign(1.0);
    label
}

/// Three significant digits, enough to compare currencies at a glance.
fn format_price(value: f64) -> String {
    match value {
        v if v >= 100.0 => format!("{v:.0}"),
        v if v >= 10.0 => format!("{v:.1}"),
        v if v >= 1.0 => format!("{v:.2}"),
        v if v >= 0.001 => format!("{v:.3}"),
        _ => "<0.001".into(),
    }
}

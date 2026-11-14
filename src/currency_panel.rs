//! The Currency Rates Panel: the most traded currencies of the league,
//! priced in chaos and divine. Toggled by the `currency-rates` Action.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use gtk::prelude::*;

use crate::http::Http;
use crate::poe1::prices::{CurrencyRates, PriceSource};

/// Prices shared between the GTK thread and fetch threads.
pub type Prices = Arc<Mutex<PriceSource<Http>>>;

/// Shows the current Currency Rates: from cache at once, otherwise fetched
/// off the GTK thread (poe.ninja may take seconds).
pub async fn refresh(panel: CurrencyPanel, prices: Prices) {
    let cached = prices.try_lock().ok().and_then(|source| source.cached_currency_rates(Instant::now()));
    let result = match cached {
        Some(result) => result,
        None => {
            panel.show_loading();
            let (tx, rx) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let mut source = prices.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                let _ = tx.send_blocking(source.currency_rates(Instant::now()));
            });
            rx.recv().await.unwrap_or_else(|_| Err("Couldn't load prices.".into()))
        }
    };
    match result {
        Ok(rates) => panel.show_rates(rates),
        Err(message) => panel.show_error(&message),
    }
}

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
    /// The rates on screen, to skip rebuilding the rows for the same ones.
    shown: Rc<RefCell<Option<Arc<CurrencyRates>>>>,
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
        Self { root, title, grid, status, shown: Rc::default() }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn show_loading(&self) {
        self.set_status("Loading prices from poe.ninja…", false);
    }

    pub fn show_rates(&self, rates: Arc<CurrencyRates>) {
        self.set_status("Prices from poe.ninja", false);
        let mut shown = self.shown.borrow_mut();
        if shown.as_ref().is_some_and(|shown| Arc::ptr_eq(shown, &rates)) {
            return;
        }
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
        *shown = Some(rates);
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

/// Three significant digits (at most six decimals), enough to compare
/// currencies at a glance: 93.4, 1.74, 0.213, 0.00228.
fn format_price(value: f64) -> String {
    if value <= 0.0 {
        return "0".into();
    }
    let decimals = (2 - value.log10().floor() as i32).clamp(0, 6) as usize;
    format!("{value:.decimals$}")
}

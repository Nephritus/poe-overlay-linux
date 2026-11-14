//! The Currency Rates Panel: the most traded currencies of the league,
//! priced in chaos and divine. Toggled by the `currency-rates` Action.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gtk::prelude::*;

use crate::panel_prices::{PriceTable, Prices, ask};
use crate::poe1::prices::CurrencyRates;

/// Shows the current Currency Rates: from cache at once, otherwise fetched.
pub async fn refresh(panel: CurrencyPanel, prices: Prices) {
    let cached = prices.try_lock().ok().and_then(|source| source.cached_currency_rates(Instant::now()));
    let result = match cached {
        Some(result) => result,
        None => {
            panel.show_loading();
            ask(&prices, |source| source.currency_rates(Instant::now())).await
        }
    };
    match result {
        Ok(rates) => panel.show_rates(rates),
        Err(message) => panel.show_error(&message),
    }
}

#[derive(Clone)]
pub struct CurrencyPanel {
    root: gtk::Box,
    title: gtk::Label,
    table: Rc<RefCell<PriceTable>>,
    status: gtk::Label,
    /// The rates on screen, to skip rebuilding the rows for the same ones.
    shown: Rc<RefCell<Option<Arc<CurrencyRates>>>>,
}

impl CurrencyPanel {
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.add_css_class("panel");
        let title = label("Currency Rates", "panel-title");
        let table = PriceTable::new();
        let status = label("", "panel-status");
        root.append(&title);
        root.append(table.widget());
        root.append(&status);
        Self { root, title, table: Rc::new(RefCell::new(table)), status, shown: Rc::default() }
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
        let mut table = self.table.borrow_mut();
        table.clear();
        for rate in &rates.rates {
            table.row(&rate.name, rate.chaos, rate.divine);
        }
        *shown = Some(rates);
    }

    /// Shows a problem; prices already shown stay, marked by the message.
    pub fn show_error(&self, message: &str) {
        self.set_status(message, true);
    }

    fn set_status(&self, text: &str, error: bool) {
        self.status.set_label(text);
        self.status.set_css_classes(&[if error { "panel-error" } else { "panel-status" }]);
    }
}

fn label(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::builder().label(text).xalign(0.0).build();
    label.add_css_class(class);
    label
}

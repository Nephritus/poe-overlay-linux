//! What the price Panels share: the price source, asked off the GTK thread,
//! and tables of prices in chaos and divine.

use std::sync::{Arc, Mutex};

use gtk::prelude::*;

use crate::http::Http;
use crate::poe1::prices::PriceSource;

/// Prices shared between the GTK thread and fetch threads.
pub type Prices = Arc<Mutex<PriceSource<Http>>>;

/// Runs `question` on the price source on a thread of its own: poe.ninja may
/// take seconds to answer.
pub async fn ask<T: Send + 'static>(
    prices: &Prices,
    question: impl FnOnce(&mut PriceSource<Http>) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let (tx, rx) = async_channel::bounded(1);
    let prices = prices.clone();
    std::thread::spawn(move || {
        let mut source = prices.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let _ = tx.send_blocking(question(&mut source));
    });
    rx.recv().await.unwrap_or_else(|_| Err("Couldn't load prices.".into()))
}

/// Rows of a name and its price in chaos and divine, under a header.
pub struct PriceTable {
    grid: gtk::Grid,
    rows: i32,
}

impl PriceTable {
    pub fn new() -> PriceTable {
        PriceTable { grid: gtk::Grid::new(), rows: 0 }
    }

    pub fn widget(&self) -> &gtk::Grid {
        &self.grid
    }

    /// Empties the table, leaving the header.
    pub fn clear(&mut self) {
        while let Some(child) = self.grid.first_child() {
            self.grid.remove(&child);
        }
        self.rows = 0;
        self.attach("", "chaos", "divine", "panel-head");
    }

    pub fn row(&mut self, name: &str, chaos: f64, divine: f64) {
        self.attach(name, &format_price(chaos), &format_price(divine), "panel-number");
    }

    fn attach(&mut self, name: &str, chaos: &str, divine: &str, class: &str) {
        let name_class = if self.rows == 0 { "panel-head" } else { "panel-name" };
        self.grid.attach(&cell(name, name_class, 0.0), 0, self.rows, 1, 1);
        self.grid.attach(&cell(chaos, class, 1.0), 1, self.rows, 1, 1);
        self.grid.attach(&cell(divine, class, 1.0), 2, self.rows, 1, 1);
        self.rows += 1;
    }
}

fn cell(text: &str, class: &str, xalign: f32) -> gtk::Label {
    let label = gtk::Label::builder().label(text).xalign(xalign).build();
    label.add_css_class(class);
    label
}

/// Three significant digits (at most six decimals), enough to compare
/// prices at a glance: 93.4, 1.74, 0.213, 0.00228.
fn format_price(value: f64) -> String {
    if value <= 0.0 {
        return "0".into();
    }
    let decimals = (2 - value.log10().floor() as i32).clamp(0, 6) as usize;
    format!("{value:.decimals$}")
}

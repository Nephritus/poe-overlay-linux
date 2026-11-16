//! The Price Check Panel: what the item under the pointer is worth on
//! poe.ninja, or a trade site search for items poe.ninja doesn't price.
//! Opened by the `price-check` Action, on the half of the screen away from
//! the pointer.

use std::cell::RefCell;
use std::fmt::Write as _;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use gtk::prelude::*;

use crate::item_copier::ItemCopier;
use crate::overlay_surface::OverlaySurface;
use crate::panel_prices::{PriceTable, Prices, Trades, ask, clear};
use crate::poe1::item::{self, Item, ItemKind};
use crate::poe1::item_copy::CopyError;
use crate::poe1::prices::ItemPrices;
use crate::poe1::trade::{TradeData, TradeQuery};
use crate::poe1::trade_search::{Listings, TradeError, age};

const WIDTH: i32 = 320;

/// For a trade site search: the search on the left, Listings on the right,
/// about twice `WIDTH` in all.
const CRITERIA_WIDTH: i32 = 380;
const RESULTS_WIDTH: i32 = 220;

/// From the middle of the output, when the Panel goes on the right half.
const RIGHT_MARGIN: f64 = 40.0;

/// A trade site search and the league it searches.
struct LeagueSearch {
    query: TradeQuery<'static>,
    league: Arc<str>,
}

/// Widgets are reference-counted, so clones share the same Panel.
#[derive(Clone)]
pub struct PricePanel {
    root: gtk::Box,
    title: gtk::Label,
    table: Rc<RefCell<PriceTable>>,
    status: gtk::Label,
    /// The trade site search, for items poe.ninja doesn't price.
    search: gtk::Grid,
    /// One row per stat searched: a checkbox with its wording, its minimum.
    filters: gtk::Grid,
    checks: Rc<RefCell<Vec<gtk::CheckButton>>>,
    search_button: gtk::Button,
    /// Modifiers the search leaves out.
    left_out: gtk::Label,
    /// How many the search found, or why it found none.
    found: gtk::Label,
    /// The cheapest Listings the trade site has.
    listings: gtk::Grid,
    /// The search shown; `None` while it is being sent.
    shown: Rc<RefCell<Option<LeagueSearch>>>,
    trades: Trades,
    /// Where the Panel goes on the left half (where it was added).
    left: (f64, f64),
    /// The copied text the Panel shows, to tell a second press on the same
    /// item (close) from one on another item (check it).
    text: Rc<RefCell<String>>,
    /// Counts presses and Searches, so a slow copy or answer for an earlier
    /// one is dropped; shared with search threads, which skip a stale one.
    presses: Arc<AtomicU64>,
}

impl PricePanel {
    /// `left`: where it goes on the left half, as passed to
    /// [`OverlaySurface::add_panel`].
    pub fn new(left: (f64, f64), trades: Trades) -> Self {
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
        // Left: what the search asks for.
        let filters = gtk::Grid::builder().column_spacing(8).row_spacing(2).build();
        let search_button = gtk::Button::builder().label("Search").focusable(false).focus_on_click(false).halign(gtk::Align::Start).build();
        let left_out = gtk::Label::builder().xalign(0.0).wrap(true).max_width_chars(1).build();
        left_out.add_css_class("panel-status");
        let criteria = gtk::Box::new(gtk::Orientation::Vertical, 6);
        criteria.set_size_request(CRITERIA_WIDTH, -1);
        criteria.append(&filters);
        criteria.append(&search_button);
        criteria.append(&left_out);
        // Right: what it finds.
        let found = gtk::Label::builder().xalign(0.0).wrap(true).max_width_chars(1).build();
        let listings = gtk::Grid::builder().column_spacing(12).build();
        let results = gtk::Box::new(gtk::Orientation::Vertical, 6);
        results.set_size_request(RESULTS_WIDTH, -1);
        results.append(&found);
        results.append(&listings);
        // A Grid, not a horizontal Box: a Box measures its height with each
        // column at its narrowest, which wraps every stat to one word a line.
        let search = gtk::Grid::builder().column_spacing(16).build();
        search.attach(&criteria, 0, 0, 1, 1);
        search.attach(&results, 1, 0, 1, 1);
        root.append(&header);
        root.append(table.widget());
        root.append(&status);
        root.append(&search);
        let table = Rc::new(RefCell::new(table));
        let panel = Self {
            root,
            title,
            table,
            status,
            search,
            filters,
            checks: Rc::default(),
            search_button: search_button.clone(),
            left_out,
            found,
            listings,
            shown: Rc::default(),
            trades,
            left,
            text: Rc::default(),
            presses: Arc::default(),
        };
        let searcher = panel.clone();
        search_button.connect_clicked(move |_| searcher.search_again());
        panel
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Shows `title` alone, narrow: no prices or search yet.
    fn start(&self, title: &str) {
        self.title.set_label(title);
        self.table.borrow_mut().clear();
        self.table.borrow().widget().set_visible(false);
        self.status.set_visible(true);
        self.search.set_visible(false);
    }

    fn show_error(&self, title: &str, message: &str) {
        self.start(title);
        self.set_status(message, true);
    }

    fn show_prices(&self, item: &Item, prices: &ItemPrices) {
        self.start(&item.name);
        let mut table = self.table.borrow_mut();
        table.widget().set_visible(true);
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

    /// For items poe.ninja doesn't price: the Panel goes wide, with the
    /// search's stats (each switchable) and the modifiers it leaves out on
    /// the left, its Listings on the right.
    fn show_search(&self, query: &TradeQuery) {
        // The last item's search, until this one's first answer.
        self.shown.take();
        self.status.set_visible(false);
        self.search.set_visible(true);
        clear(&self.filters);
        let mut checks = self.checks.borrow_mut();
        checks.clear();
        for (row, filter) in (0..).zip(&query.filters) {
            let text = gtk::Label::builder().label(filter.text).xalign(0.0).wrap(true).max_width_chars(1).hexpand(true).build();
            // Pointer-only: the Overlay never takes keyboard focus.
            let checkbox = gtk::CheckButton::builder().active(filter.enabled).child(&text).hexpand(true).focusable(false).focus_on_click(false).build();
            let min = gtk::Label::builder().label(filter.min.map(|min| min.to_string()).unwrap_or_default()).xalign(1.0).build();
            min.add_css_class("panel-number");
            self.filters.attach(&checkbox, 0, row, 1, 1);
            self.filters.attach(&min, 1, row, 1, 1);
            checks.push(checkbox);
        }
        let mut left_out = String::new();
        for (heading, modifiers) in [("the trade site has no such stat", &query.excluded), ("implicit, crafted or enchant", &query.skipped)] {
            if !modifiers.is_empty() {
                let _ = write!(left_out, "{}Not searched ({heading}):\n{}", if left_out.is_empty() { "" } else { "\n" }, modifiers.join("\n"));
            }
        }
        self.left_out.set_visible(!left_out.is_empty());
        self.left_out.set_label(&left_out);
    }

    /// Runs `search` and shows what it finds, unless a later press or Search
    /// took over. The Search button waits for the answer; the last search's
    /// Listings go, so an error never sits above them.
    async fn search(self, check: u64, search: LeagueSearch) {
        self.search_button.set_sensitive(false);
        self.listings.set_visible(false);
        self.set_found("Searching the trade site for ones like it…", false);
        let presses = self.presses.clone();
        // The search goes to the search thread and comes back with the
        // answer; lost only if that thread panics (the next press recovers).
        let answer = ask(&self.trades, move |source| {
            // A search still waiting for the lock when the player has moved
            // on would only spend the rate limit.
            let found = if presses.load(Ordering::Relaxed) == check {
                source.listings(&search.query, &search.league, Instant::now())
            } else {
                Err(TradeError::Failed(String::new()))
            };
            Ok::<_, TradeError>((search, found))
        })
        .await;
        if !self.is_current(check) {
            return;
        }
        let found = answer.and_then(|(search, found)| {
            self.shown.replace(Some(search));
            self.search_button.set_sensitive(true);
            found
        });
        match found {
            Ok(found) => self.show_listings(&found),
            Err(err) => self.set_found(&err.to_string(), true),
        }
    }

    /// The Search button: the shown search again, with the stats switched
    /// on and off as the player left them.
    fn search_again(&self) {
        let Some(mut search) = self.shown.take() else { return };
        for (filter, checkbox) in search.query.filters.iter_mut().zip(self.checks.borrow().iter()) {
            filter.enabled = checkbox.is_active();
        }
        let check = self.presses.fetch_add(1, Ordering::Relaxed) + 1;
        gtk::glib::spawn_future_local(self.clone().search(check, search));
    }

    /// Whether `check` is still the latest press or Search.
    fn is_current(&self, check: u64) -> bool {
        self.presses.load(Ordering::Relaxed) == check
    }

    /// The cheapest listings: price, how long ago it was listed, and whether
    /// the seller is away.
    fn show_listings(&self, found: &Listings) {
        clear(&self.listings);
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs() as i64);
        for (row, listing) in (0..).zip(&found.cheapest) {
            let price = listing.price.as_ref().map_or_else(|| "no price".to_owned(), |(amount, currency)| format!("{amount} {currency}"));
            let listed = listing.listed.map(|listed| age(now - listed)).unwrap_or_default();
            let cells = [(price, "panel-name", 0.0), (listed, "panel-status", 1.0), ((if listing.afk { "AFK" } else { "" }).to_owned(), "panel-status", 0.0)];
            for (column, (text, class, xalign)) in (0..).zip(cells) {
                let cell = gtk::Label::builder().label(&text).xalign(xalign).build();
                cell.add_css_class(class);
                self.listings.attach(&cell, column, row, 1, 1);
            }
        }
        self.listings.set_visible(!found.cheapest.is_empty());
        let status = match found.total {
            0 => "No online listings like it: switch off some stats and search again.".to_owned(),
            total => format!("Trade site · {total} listed online, cheapest first"),
        };
        self.set_found(&status, false);
    }

    fn set_status(&self, text: &str, error: bool) {
        set_label(&self.status, text, error);
    }

    fn set_found(&self, text: &str, error: bool) {
        set_label(&self.found, text, error);
    }
}

fn set_label(label: &gtk::Label, text: &str, error: bool) {
    label.set_label(text);
    label.set_css_classes(&[if error { "panel-error" } else { "panel-status" }]);
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
    let check = panel.presses.fetch_add(1, Ordering::Relaxed) + 1;
    let copied = copier.copy().await;
    // A later press took over.
    if !panel.is_current(check) {
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
    let searched = matches!(item.kind, ItemKind::Other { .. });
    panel.set_status(if searched { "Building a trade site search…" } else { "Checking poe.ninja…" }, false);
    if !overlay.show(panel.widget()) {
        return;
    }
    let item = Arc::new(item);
    let lookup = item.clone();
    if searched {
        // The trade data is read on first use; the league may need poe.ninja.
        let search = ask(&prices, move |source| {
            let league = source.league(Instant::now())?;
            let query = TradeQuery::new(&lookup, TradeData::bundled()).expect("an item searched on the trade site");
            Ok::<_, String>(LeagueSearch { query, league })
        })
        .await;
        if !panel.is_current(check) {
            return;
        }
        let search = match search {
            Ok(search) => search,
            Err(message) => return panel.show_error(&item.name, &message),
        };
        panel.show_search(&search.query);
        panel.search(check, search).await;
        return;
    }
    let result = ask(&prices, move |source| source.item_prices(&lookup, Instant::now())).await;
    if !panel.is_current(check) {
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

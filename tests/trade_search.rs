//! Searching the trade site through a fake HTTP client. The search and fetch
//! answers were recorded from the trade site on 2026-11-16 (Assassin's Mitts
//! with 65+ total life) and trimmed to the fields read.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use poe_overlay::poe1::item::parse;
use poe_overlay::poe1::trade::{ListingKind, TradeData, TradeQuery};
use poe_overlay::poe1::login::Login;
use poe_overlay::poe1::trade_search::{TradeError, TradeHttp, TradeResponse, TradeSource, age};

const SEARCH: &str = include_str!("data/trade-search-mitts.json");
const FETCH: &str = include_str!("data/trade-fetch-mitts.json");
const FETCH_BOTH_KINDS: &str = include_str!("data/trade-fetch-available.json");

fn response(status: u16, headers: &[(&str, &str)], body: &str) -> TradeResponse {
    TradeResponse { status, headers: headers.iter().map(|(name, value)| ((*name).to_owned(), (*value).to_owned())).collect(), body: body.to_owned() }
}

fn search_ok(body: &str) -> TradeResponse {
    let headers = [("X-Rate-Limit-Rules", "Ip"), ("X-Rate-Limit-Ip", "5:10:60,15:60:300"), ("X-Rate-Limit-Ip-State", "1:10:0,1:60:0")];
    response(200, &headers, body)
}

fn fetch_ok() -> TradeResponse {
    let headers = [("X-Rate-Limit-Rules", "Ip"), ("X-Rate-Limit-Ip", "12:4:10,16:12:300"), ("X-Rate-Limit-Ip-State", "1:4:0,1:12:0")];
    response(200, &headers, FETCH)
}

/// Answers searches with `search` and fetches with `fetch` (the recorded
/// listings unless changed); logs every request as (url, body).
struct FakeTrade {
    search: TradeResponse,
    fetch: RefCell<TradeResponse>,
    requests: RefCell<Vec<(String, Option<String>)>>,
}

impl FakeTrade {
    fn new(search: TradeResponse) -> FakeTrade {
        FakeTrade { search, fetch: RefCell::new(fetch_ok()), requests: RefCell::default() }
    }

    fn sent(&self) -> usize {
        self.requests.borrow().len()
    }
}

impl TradeHttp for &FakeTrade {
    fn send(&self, url: &str, body: Option<&str>, _login: Option<&Login>) -> Result<TradeResponse, String> {
        self.requests.borrow_mut().push((url.to_owned(), body.map(str::to_owned)));
        Ok(if url.contains("/api/trade/search/") { self.search.clone() } else { self.fetch.borrow().clone() })
    }
}

fn gloves() -> TradeQuery<'static> {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/items/rare-gloves-death-nails.txt")).unwrap();
    TradeQuery::new(&parse(&text).unwrap(), TradeData::bundled()).unwrap()
}

#[test]
fn search_finds_the_cheapest_listings_with_price_time_and_seller() {
    let trade = FakeTrade::new(search_ok(SEARCH));
    let query = gloves();

    let found = TradeSource::new(&trade).listings(&query, "Covens", Instant::now()).unwrap();

    assert_eq!((found.total, found.cheapest.len()), (79, 10));
    let first = &found.cheapest[0];
    assert_eq!(first.price, Some((1.0, "chaos".to_owned())));
    assert_eq!(first.listed, Some(1_794_741_914)); // 2026-11-15T11:25:14Z
    assert!(!first.afk);
    assert!(found.cheapest[3].afk);

    let requests = trade.requests.borrow();
    assert_eq!(requests[0], ("https://www.pathofexile.com/api/trade/search/Covens".to_owned(), Some(query.to_json())));
    let (fetch, body) = &requests[1];
    assert!(fetch.starts_with("https://www.pathofexile.com/api/trade/fetch/52f17ee4e3a498b475b5668f69f9e9c35fba49d9861e973b0778c57012ae2a86,"), "{fetch}");
    assert!(fetch.ends_with("?query=H4sIAAAAAAAAA31PQQrCQAz8iuTipQgKeujNB_gCkbLYFALbbN1khVLyd1OLUBHMJckkM5NMIBq0CNQTpEEpMdSQOBIjWAU6DujAWSSIEG9lcyFVgepNc9Z1-uwEbh3uKCrmZUCtw4NgadNuSY0mDbGJ1KHvPkMsOBv35K6no1n1h4QRe-S5yijk9nz_FdkfzG52W92x3Nes-lWZQyYdv37nxIXp4Zo2xwug7fuDIAEAAA"));
    assert_eq!((fetch.matches(',').count(), body), (9, &None), "ten listings in one fetch");
}

#[test]
fn listing_times_are_read_across_months_and_leap_days() {
    let trade = FakeTrade::new(search_ok(SEARCH));
    trade.fetch.borrow_mut().body = FETCH.replacen("2026-11-15T11:25:14Z", "2028-02-29T23:59:59Z", 1);

    let found = TradeSource::new(&trade).listings(&gloves(), "Covens", Instant::now()).unwrap();

    assert_eq!(found.cheapest[0].listed, Some(1_835_481_599));
}

#[test]
fn instant_buyout_listings_are_told_from_in_person_ones_and_are_never_afk() {
    // Recorded 2026-11-16 with both kinds: instant-buyout listings carry the
    // gold fee. The first seller is made online and away here.
    let mut fetched: serde_json::Value = serde_json::from_str(FETCH_BOTH_KINDS).unwrap();
    fetched["result"][0]["listing"]["account"]["online"] = serde_json::json!({ "league": "Covens", "status": "afk" });
    let trade = FakeTrade::new(search_ok(SEARCH));
    trade.fetch.borrow_mut().body = fetched.to_string();

    let found = TradeSource::new(&trade).listings(&gloves(), "Covens", Instant::now()).unwrap();

    let kinds: Vec<ListingKind> = found.cheapest.iter().map(|listing| listing.kind).collect();
    let (instant, in_person) = (ListingKind::InstantBuyout, ListingKind::InPerson);
    assert_eq!(kinds, [instant, instant, in_person, instant, instant, instant, instant, instant, instant, instant]);
    assert!(!found.cheapest[0].afk, "an instant buyout needs no one at the keyboard");
}

#[test]
fn search_answered_but_fetch_held_back_is_not_searched_again() {
    let trade = FakeTrade::new(search_ok(SEARCH));
    *trade.fetch.borrow_mut() = response(429, &[("Retry-After", "5")], "");
    let mut source = TradeSource::new(&trade);
    let start = Instant::now();
    assert!(matches!(source.listings(&gloves(), "Covens", start), Err(TradeError::Limited { .. })));

    *trade.fetch.borrow_mut() = fetch_ok();
    let found = source.listings(&gloves(), "Covens", start + Duration::from_secs(6)).unwrap();

    assert_eq!(found.cheapest.len(), 10);
    let searches = trade.requests.borrow().iter().filter(|(url, _)| url.contains("/search/")).count();
    assert_eq!((searches, trade.sent()), (1, 3));
}

#[test]
fn search_without_results_fetches_nothing() {
    let trade = FakeTrade::new(search_ok(r#"{"id":"abc","complexity":4,"result":[],"total":0}"#));

    let found = TradeSource::new(&trade).listings(&gloves(), "Covens", Instant::now()).unwrap();

    assert_eq!((found.total, found.cheapest.len(), trade.sent()), (0, 0, 1));
}

#[test]
fn same_search_within_five_minutes_is_answered_from_the_last_one() {
    let trade = FakeTrade::new(search_ok(SEARCH));
    let mut source = TradeSource::new(&trade);
    let start = Instant::now();
    source.listings(&gloves(), "Covens", start).unwrap();

    source.listings(&gloves(), "Covens", start + Duration::from_secs(299)).unwrap();
    assert_eq!(trade.sent(), 2);

    source.listings(&gloves(), "Covens", start + Duration::from_secs(300)).unwrap();
    assert_eq!(trade.sent(), 4);
}

#[test]
fn edited_search_is_sent_instead_of_answered_from_the_last_one() {
    let trade = FakeTrade::new(search_ok(SEARCH));
    let mut source = TradeSource::new(&trade);
    let start = Instant::now();
    let mut query = gloves();
    source.listings(&query, "Covens", start).unwrap();

    query.filters[0].enabled = false;
    source.listings(&query, "Covens", start + Duration::from_secs(10)).unwrap();

    assert_eq!(trade.sent(), 4);
    let body = trade.requests.borrow()[2].1.clone().unwrap();
    assert!(body.contains("pseudo_total_elemental_resistance") && !body.contains("pseudo_total_life"), "{body}");
}

#[test]
fn rate_limited_search_waits_without_sending_and_says_how_long() {
    let headers = [("X-Rate-Limit-Rules", "Ip"), ("X-Rate-Limit-Ip", "5:10:60"), ("X-Rate-Limit-Ip-State", "6:10:60"), ("Retry-After", "60")];
    let trade = FakeTrade::new(response(429, &headers, r#"{"error":{"code":3,"message":"Rate limit exceeded"}}"#));
    let mut source = TradeSource::new(&trade);
    let start = Instant::now();

    let first = source.listings(&gloves(), "Covens", start).unwrap_err();
    assert!(matches!(first, TradeError::Limited { wait } if wait.as_secs() == 60), "{first:?}");

    // Waits count from the answer, a few microseconds after `start` here.
    let later = source.listings(&gloves(), "Covens", start + Duration::from_secs(45)).unwrap_err();
    assert!(matches!(later, TradeError::Limited { wait } if wait.as_secs() == 15), "{later:?}");
    assert_eq!(trade.sent(), 1);
    assert!(later.to_string().contains("15 s"), "{later}");
}

#[test]
fn rate_limited_answer_without_a_stated_wait_holds_searches_a_minute() {
    let trade = FakeTrade::new(response(429, &[], ""));
    let mut source = TradeSource::new(&trade);
    let start = Instant::now();
    source.listings(&gloves(), "Covens", start).unwrap_err();

    let later = source.listings(&gloves(), "Covens", start + Duration::from_secs(59)).unwrap_err();

    assert!(matches!(later, TradeError::Limited { .. }), "{later:?}");
    assert_eq!(trade.sent(), 1);
}

#[test]
fn refused_search_is_not_retried_for_minutes() {
    let trade = FakeTrade::new(response(403, &[], "<html>Forbidden</html>"));
    let mut source = TradeSource::new(&trade);
    let start = Instant::now();

    let err = source.listings(&gloves(), "Covens", start).unwrap_err();
    assert!(matches!(err, TradeError::Refused { .. }) && err.to_string().contains("403"), "{err}");

    // Repeated 403s count against the address on GGG's side.
    assert!(matches!(source.listings(&gloves(), "Covens", start + Duration::from_secs(299)), Err(TradeError::Refused { .. })));
    assert_eq!(trade.sent(), 1);
}

#[test]
fn other_answers_are_errors_with_the_site_message() {
    let trade = FakeTrade::new(response(400, &[], r#"{"error":{"code":2,"message":"Invalid query"}}"#));

    let err = TradeSource::new(&trade).listings(&gloves(), "Covens", Instant::now()).unwrap_err();

    assert!(err.to_string().contains("Invalid query"), "{err}");
}

#[test]
fn listing_age_is_rounded_to_its_unit() {
    for (seconds, text) in [(30, "just now"), (125, "2 min"), (3 * 3600 + 59 * 60, "3 h"), (2 * 86_400 + 5, "2 d")] {
        assert_eq!(age(seconds), text, "{seconds} s");
    }
}

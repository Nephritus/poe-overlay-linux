//! Contacting sellers from fetched Listings: the whisper of an in-person
//! Listing, and Travel to Hideout for an instant buyout, through a fake
//! trade site. The fetch answer was recorded from the trade site on
//! 2026-11-17 (Tabula Rasa in Covens: two in-person Listings, two instant
//! buyouts fetched logged in, one fetched logged out), trimmed to the fields
//! read, with sellers renamed and the hideout tokens replaced by tokens of
//! the same shape that expire at `TOKEN_EXPIRES`.

use std::cell::RefCell;
use std::time::{Duration, Instant, SystemTime};

use poe_overlay::browsers::Cookie;
use poe_overlay::poe1::item::parse;
use poe_overlay::poe1::login::{Found, Login};
use poe_overlay::poe1::trade::{ListingKind, TradeData, TradeQuery};
use poe_overlay::poe1::trade_search::{Listings, TradeError, TradeHttp, TradeResponse, TradeSource, Travel};

const FETCH: &str = include_str!("data/trade-fetch-contact.json");
/// When the recorded hideout tokens expire (2026-11-17 18:01:05 UTC).
const TOKEN_EXPIRES: i64 = 1_794_927_665;
const BEFORE_EXPIRY: i64 = TOKEN_EXPIRES - 60;
const TRAVEL_URL: &str = "https://www.pathofexile.com/api/trade/whisper";

/// A request as sent: url, body, and whether it carried a login.
type Sent = (String, Option<String>, bool);

/// Answers searches and fetches with the recording, and travel requests
/// with `travel` in turn; logs every request.
struct FakeTrade {
    travel: RefCell<Vec<TradeResponse>>,
    sent: RefCell<Vec<Sent>>,
}

impl FakeTrade {
    fn answering(travel: Vec<TradeResponse>) -> FakeTrade {
        FakeTrade { travel: RefCell::new(travel), sent: RefCell::default() }
    }

    fn travels_sent(&self) -> Vec<Sent> {
        self.sent.borrow().iter().filter(|(url, _, _)| url == TRAVEL_URL).cloned().collect()
    }
}

impl TradeHttp for &FakeTrade {
    fn send(&self, url: &str, body: Option<&str>, login: Option<&Login>) -> Result<TradeResponse, String> {
        self.sent.borrow_mut().push((url.to_owned(), body.map(str::to_owned), login.is_some()));
        let body = if url.contains("/search/") {
            r#"{"id":"q","result":["a","b","c","d","e"],"total":5}"#.to_owned()
        } else if url.contains("/fetch/") {
            FETCH.to_owned()
        } else {
            return Ok(self.travel.borrow_mut().remove(0));
        };
        Ok(TradeResponse { status: 200, headers: Vec::new(), body })
    }
}

fn answer(status: u16, headers: &[(&str, &str)], body: &str) -> TradeResponse {
    TradeResponse { status, headers: headers.iter().map(|(name, value)| ((*name).to_owned(), (*value).to_owned())).collect(), body: body.to_owned() }
}

fn gloves() -> TradeQuery<'static> {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/items/rare-gloves-death-nails.txt")).unwrap();
    TradeQuery::new(&parse(&text).unwrap(), TradeData::bundled()).unwrap()
}

fn logged_in(source: &mut TradeSource<&FakeTrade>) {
    source.log_in(Box::new(|| {
        let now = SystemTime::now();
        let session = Cookie { name: "POESESSID".to_owned(), value: "session".to_owned(), expires: Some(now + Duration::from_secs(3600)), created: now };
        vec![Found { browser: "Firefox", default: true, user_agent: None, cookies: vec![session] }]
    }));
}

fn listings(source: &mut TradeSource<&FakeTrade>, now: Instant) -> std::sync::Arc<Listings> {
    source.listings(&gloves(), "Covens", now).unwrap()
}

#[test]
fn in_person_listings_carry_the_whisper_and_instant_buyouts_the_way_to_travel() {
    let trade = FakeTrade::answering(Vec::new());
    let mut source = TradeSource::new(&trade);
    let found = listings(&mut source, Instant::now());
    let kinds: Vec<_> = found.cheapest.iter().map(|listing| (listing.kind, listing.whisper.is_some(), listing.hideout.is_some())).collect();
    assert_eq!(
        kinds,
        [
            (ListingKind::InPerson, true, false),
            (ListingKind::InPerson, true, false),
            (ListingKind::InstantBuyout, false, true),
            (ListingKind::InstantBuyout, false, true),
            // Fetched logged out: no way to travel, and no whisper either
            // (an instant buyout needs none).
            (ListingKind::InstantBuyout, false, false),
        ]
    );
    assert_eq!(
        found.cheapest[0].whisper.as_deref(),
        Some("@Seller_One Hi, I would like to buy your Tabula Rasa Simple Robe listed for 2 chaos in Covens (stash tab \"~price 2 chaos\"; position: left 16, top 3)")
    );
}

#[test]
fn the_same_search_after_logging_in_or_out_fetches_the_listings_again() {
    let trade = FakeTrade::answering(Vec::new());
    let mut source = TradeSource::new(&trade);
    let now = Instant::now();
    listings(&mut source, now);
    let fetches = || trade.sent.borrow().iter().filter(|(url, _, _)| url.contains("/fetch/")).map(|(_, _, login)| *login).collect::<Vec<_>>();
    listings(&mut source, now + Duration::from_secs(10));
    assert_eq!(fetches(), [false], "reused while nothing changed");

    // Fetched logged out, a Listing has no way to travel: Log in, then Search.
    logged_in(&mut source);
    listings(&mut source, now + Duration::from_secs(20));
    source.log_out();
    listings(&mut source, now + Duration::from_secs(30));
    assert_eq!(fetches(), [false, true, false]);
}

#[test]
fn listings_fetched_before_the_browser_held_a_login_are_fetched_again_once_it_does() {
    let trade = FakeTrade::answering(Vec::new());
    let mut source = TradeSource::new(&trade);
    let session = std::sync::Arc::new(std::sync::Mutex::new(false));
    let held = session.clone();
    source.log_in(Box::new(move || {
        let now = SystemTime::now();
        let cookies = if *held.lock().unwrap() { vec![Cookie { name: "POESESSID".to_owned(), value: "session".to_owned(), expires: Some(now + Duration::from_secs(3600)), created: now }] } else { Vec::new() };
        vec![Found { browser: "Firefox", default: true, user_agent: None, cookies }]
    }));
    let now = Instant::now();
    listings(&mut source, now);

    // The row said Log in; the player logs in on the site, then Searches.
    *session.lock().unwrap() = true;
    listings(&mut source, now + Duration::from_secs(61));
    let fetches: Vec<bool> = trade.sent.borrow().iter().filter(|(url, _, _)| url.contains("/fetch/")).map(|(_, _, login)| *login).collect();
    assert_eq!(fetches, [false, true]);
}

#[test]
fn an_expired_listing_is_fetched_again_by_the_same_search() {
    let trade = FakeTrade::answering(Vec::new());
    let mut source = TradeSource::new(&trade);
    logged_in(&mut source);
    let now = Instant::now();
    let found = listings(&mut source, now);
    // Our clock may run ahead of the site's: expired here, while the
    // search is still reused.
    assert_eq!(source.travel(found.cheapest[2].hideout.as_ref().unwrap(), false, now, TOKEN_EXPIRES), Err(TradeError::Expired));
    listings(&mut source, now + Duration::from_secs(10));
    assert_eq!(trade.sent.borrow().iter().filter(|(url, _, _)| url.contains("/fetch/")).count(), 2);
}

#[test]
fn travel_sends_the_listings_token_with_the_login_once_per_call() {
    let trade = FakeTrade::answering(vec![answer(200, &[], r#"{"success":true}"#)]);
    let mut source = TradeSource::new(&trade);
    logged_in(&mut source);
    let now = Instant::now();
    let found = listings(&mut source, now);
    let token = found.cheapest[2].hideout.as_ref().unwrap();

    assert_eq!(source.travel(token, false, now, BEFORE_EXPIRY), Ok(Travel::Teleporting));

    let sent = trade.travels_sent();
    assert_eq!(sent.len(), 1);
    let body: serde_json::Value = serde_json::from_str(sent[0].1.as_deref().unwrap()).unwrap();
    let recorded: serde_json::Value = serde_json::from_str(FETCH).unwrap();
    assert_eq!(body, serde_json::json!({ "token": recorded["result"][2]["listing"]["hideout_token"] }));
    assert!(sent[0].2, "carries the login");
}

#[test]
fn an_item_in_demand_is_reported_and_the_next_call_travels_anyway() {
    let trade = FakeTrade::answering(vec![answer(200, &[], r#"{"success":false}"#), answer(200, &[], r#"{"success":true}"#)]);
    let mut source = TradeSource::new(&trade);
    logged_in(&mut source);
    let now = Instant::now();
    let found = listings(&mut source, now);
    let token = found.cheapest[3].hideout.as_ref().unwrap();

    assert_eq!(source.travel(token, false, now, BEFORE_EXPIRY), Ok(Travel::InDemand));
    assert_eq!(source.travel(token, true, now + Duration::from_secs(5), BEFORE_EXPIRY), Ok(Travel::Teleporting));

    let bodies: Vec<serde_json::Value> = trade.travels_sent().iter().map(|(_, body, _)| serde_json::from_str(body.as_deref().unwrap()).unwrap()).collect();
    assert_eq!(bodies[0].get("continue"), None);
    assert_eq!(bodies[1]["continue"], true);
}

#[test]
fn nothing_is_sent_logged_out_or_with_an_expired_token() {
    let trade = FakeTrade::answering(Vec::new());
    let mut source = TradeSource::new(&trade);
    let now = Instant::now();
    logged_in(&mut source);
    let found = listings(&mut source, now);
    let token = found.cheapest[2].hideout.as_ref().unwrap();

    assert_eq!(source.travel(token, false, now, TOKEN_EXPIRES), Err(TradeError::Expired));
    source.log_out();
    assert_eq!(source.travel(token, false, now, BEFORE_EXPIRY), Err(TradeError::LoggedOut));
    assert!(trade.travels_sent().is_empty());
}

#[test]
fn a_refused_travel_says_the_trade_sites_reason() {
    let refusals = vec![
        answer(400, &[], r#"{"error":{"code":1,"message":"Resource not found"}}"#),
        answer(200, &[], r#"{"error":{"code":6,"message":"You must be in the same league"}}"#),
        answer(500, &[], "<html>"),
        answer(403, &[], "<html>"),
    ];
    let trade = FakeTrade::answering(refusals);
    let mut source = TradeSource::new(&trade);
    logged_in(&mut source);
    let now = Instant::now();
    let found = listings(&mut source, now);
    let token = found.cheapest[2].hideout.as_ref().unwrap();

    let reasons: Vec<String> = (0..4).map(|wait| source.travel(token, false, now + Duration::from_secs(wait * 10), BEFORE_EXPIRY).unwrap_err().to_string()).collect();
    assert!(reasons[0].contains("Resource not found"), "{}", reasons[0]);
    assert!(reasons[1].contains("You must be in the same league"), "{}", reasons[1]);
    assert!(reasons[2].contains("HTTP 500"), "{}", reasons[2]);
    assert!(reasons[3].contains("HTTP 403") && !reasons[3].contains("search"), "{}", reasons[3]);
    // A refused travel doesn't hold back searches.
    let searched = trade.sent.borrow().len();
    source.listings(&gloves(), "Standard", now + Duration::from_secs(50)).unwrap();
    assert!(trade.sent.borrow().len() > searched);
}

#[test]
fn travel_keeps_the_rate_limits_the_site_states_for_it() {
    let limited = [("X-Rate-Limit-Rules", "Account"), ("X-Rate-Limit-Account", "1:10:60"), ("X-Rate-Limit-Account-State", "1:10:0")];
    let trade = FakeTrade::answering(vec![answer(200, &limited, r#"{"success":false}"#), answer(200, &[], r#"{"success":true}"#)]);
    let mut source = TradeSource::new(&trade);
    logged_in(&mut source);
    let now = Instant::now();
    let found = listings(&mut source, now);
    let token = found.cheapest[2].hideout.as_ref().unwrap();

    assert_eq!(source.travel(token, false, now, BEFORE_EXPIRY), Ok(Travel::InDemand));
    assert!(matches!(source.travel(token, true, now + Duration::from_secs(2), BEFORE_EXPIRY), Err(TradeError::Limited { .. })));
    assert_eq!(trade.travels_sent().len(), 1);
    // Searches have limits of their own.
    source.listings(&gloves(), "Standard", now + Duration::from_secs(2)).unwrap();
    assert_eq!(source.travel(token, true, now + Duration::from_secs(11), BEFORE_EXPIRY), Ok(Travel::Teleporting));
}

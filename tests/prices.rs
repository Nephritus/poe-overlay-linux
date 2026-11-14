//! Currency Rates from poe.ninja, through a fake fetcher and a fake clock.
//! Responses were recorded from poe.ninja on 2026-11-14.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use poe_overlay::poe1::prices::{Fetch, PriceSource};

const LEAGUES: &str = include_str!("data/ninja-leagues.json");
const COVENS_CURRENCY: &str = include_str!("data/ninja-exchange-currency-covens.json");

/// Answers from recorded responses and logs every requested URL.
#[derive(Default)]
struct FakeNinja {
    requests: RefCell<Vec<String>>,
    offline: bool,
}

impl Fetch for &FakeNinja {
    fn get(&self, url: &str) -> Result<String, String> {
        self.requests.borrow_mut().push(url.to_owned());
        if self.offline {
            return Err("connection refused".into());
        }
        if url.ends_with("/economy/leagues") {
            Ok(LEAGUES.into())
        } else if url.contains("exchange/current/overview") && url.contains("type=Currency") {
            Ok(COVENS_CURRENCY.into())
        } else {
            Err(format!("unexpected URL {url}"))
        }
    }
}

impl FakeNinja {
    fn currency_requests(&self) -> Vec<String> {
        self.requests.borrow().iter().filter(|u| u.contains("exchange")).cloned().collect()
    }
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1e-4
}

#[test]
fn currency_rates_are_the_most_traded_currencies_in_chaos_and_divine() {
    let ninja = FakeNinja::default();
    let mut source = PriceSource::new(&ninja, None);

    let rates = source.currency_rates(Instant::now()).unwrap();

    let names: Vec<&str> = rates.rates.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Divine Orb",
            "Orb of Regret",
            "Orb of Fusing",
            "Stacked Deck",
            "Instilling Orb",
            "Orb of Scouring",
            "Glassblower's Bauble",
            "Tainted Orb of Fusing",
            "Vaal Orb",
            "Lesser Eldritch Ichor",
            "Orb of Alteration",
            "Gemcutter's Prism",
        ]
    );
    let divine = &rates.rates[0];
    assert!(close(divine.chaos, 93.39), "{divine:?}");
    let tainted = &rates.rates[7];
    assert!(close(tainted.chaos, 25.38) && close(tainted.divine, 25.38 * 0.01071), "{tainted:?}");
}

#[test]
fn default_league_is_the_first_one_poe_ninja_lists() {
    let ninja = FakeNinja::default();
    let mut source = PriceSource::new(&ninja, None);

    let rates = source.currency_rates(Instant::now()).unwrap();

    assert_eq!(rates.league, "Covens");
    assert!(ninja.currency_requests()[0].contains("league=Covens"));
}

#[test]
fn configured_league_is_used_without_asking_for_the_list() {
    let ninja = FakeNinja::default();
    let mut source = PriceSource::new(&ninja, Some("Hardcore Covens".into()));

    let rates = source.currency_rates(Instant::now()).unwrap();

    assert_eq!(rates.league, "Hardcore Covens");
    let requests = ninja.requests.borrow();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert!(requests[0].contains("league=Hardcore%20Covens"), "{requests:?}");
}

#[test]
fn poe_ninja_is_asked_at_most_once_per_fifteen_minutes() {
    let ninja = FakeNinja::default();
    let mut source = PriceSource::new(&ninja, None);
    let start = Instant::now();

    source.currency_rates(start).unwrap();
    source.currency_rates(start + Duration::from_secs(60)).unwrap();
    source.currency_rates(start + Duration::from_secs(14 * 60 + 59)).unwrap();
    assert_eq!(ninja.requests.borrow().len(), 2, "one league list, one currency overview");

    source.currency_rates(start + Duration::from_secs(15 * 60)).unwrap();
    assert_eq!(ninja.currency_requests().len(), 2);
}

#[test]
fn network_failure_is_a_readable_error() {
    let ninja = FakeNinja { offline: true, ..FakeNinja::default() };
    let mut source = PriceSource::new(&ninja, None);

    let err = source.currency_rates(Instant::now()).unwrap_err();

    assert!(err.contains("poe.ninja") && err.contains("connection refused"), "{err}");
}

#[test]
fn league_poe_ninja_has_no_prices_for_is_named_in_the_error() {
    struct UnknownLeague;
    impl Fetch for UnknownLeague {
        fn get(&self, _: &str) -> Result<String, String> {
            // Recorded: poe.ninja's answer for a league name it doesn't know.
            Ok(r#"{"core":{"items":[],"rates":{},"primary":"chaos","secondary":"divine"},"lines":[],"items":[]}"#.into())
        }
    }
    let mut source = PriceSource::new(UnknownLeague, Some("Covnes".into()));

    let err = source.currency_rates(Instant::now()).unwrap_err();

    assert!(err.contains("Covnes") && err.contains("[prices] league"), "{err}");
}

#[test]
fn failed_fetch_is_retried_after_a_minute_not_on_every_open() {
    let ninja = FakeNinja { offline: true, ..FakeNinja::default() };
    let mut source = PriceSource::new(&ninja, Some("Covens".into()));
    let start = Instant::now();

    let first = source.currency_rates(start).unwrap_err();
    let again = source.currency_rates(start + Duration::from_secs(59)).unwrap_err();
    assert_eq!(ninja.requests.borrow().len(), 1);
    assert_eq!(again, first);

    let _ = source.currency_rates(start + Duration::from_secs(60));
    assert_eq!(ninja.requests.borrow().len(), 2);
}

#[test]
fn default_league_without_prices_does_not_blame_the_config() {
    struct NoPrices;
    impl Fetch for NoPrices {
        fn get(&self, url: &str) -> Result<String, String> {
            if url.ends_with("/leagues") {
                return Ok(LEAGUES.into());
            }
            Ok(r#"{"core":{"items":[],"rates":{},"primary":"chaos","secondary":"divine"},"lines":[],"items":[]}"#.into())
        }
    }
    let mut source = PriceSource::new(NoPrices, None);

    let err = source.currency_rates(Instant::now()).unwrap_err();

    assert!(err.contains("Covens") && !err.contains("config"), "{err}");
}

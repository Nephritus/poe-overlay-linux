//! The trade site's rate limits, as its response headers state them
//! (format from https://www.pathofexile.com/developer/docs#ratelimits).

use std::time::{Duration, Instant};

use poe_overlay::poe1::trade_search::RateLimits;

/// Response headers as (name, value) pairs; lookups ignore case.
fn headers<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<&'a str> {
    move |name| pairs.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| *value)
}

/// The trade search's limits as recorded on 2026-11-16, with `state`.
fn search_headers(state: &str) -> Vec<(&'static str, String)> {
    vec![
        ("x-rate-limit-policy", "trade-search-request-limit".into()),
        ("x-rate-limit-rules", "Ip".into()),
        ("x-rate-limit-ip", "5:10:60,15:60:300,30:300:1800,600:21600:3600".into()),
        ("x-rate-limit-ip-state", state.into()),
    ]
}

fn update(limits: &mut RateLimits, pairs: &[(&'static str, String)], now: Instant) {
    let pairs: Vec<(&str, &str)> = pairs.iter().map(|(name, value)| (*name, value.as_str())).collect();
    limits.update(headers(&pairs), now);
}

const S: Duration = Duration::from_secs(1);

#[test]
fn first_request_may_go_out_before_any_limit_is_known() {
    assert_eq!(RateLimits::default().wait_until(Instant::now()), None);
}

#[test]
fn requests_are_counted_against_each_rule() {
    let start = Instant::now();
    let mut limits = RateLimits::default();
    for i in 0..5 {
        let now = start + i * S;
        assert_eq!(limits.wait_until(now), None, "request {i}");
        limits.sent(now);
        update(&mut limits, &search_headers(&format!("{}:10:0,{}:60:0,{}:300:0,{}:21600:0", i + 1, i + 1, i + 1, i + 1)), now);
    }

    // The fifth request filled the 10-second window opened by the first.
    assert_eq!(limits.wait_until(start + 5 * S), Some(start + 10 * S));
    assert_eq!(limits.wait_until(start + 10 * S), None);
}

#[test]
fn longer_rules_hold_requests_too() {
    let start = Instant::now();
    let mut limits = RateLimits::default();
    update(&mut limits, &search_headers("1:10:0,1:60:0,1:300:0,1:21600:0"), start);
    // 15 requests, three every 12 seconds: never 5 in 10 seconds.
    for i in 0..15 {
        let now = start + (i / 3) * 12 * S + (i % 3) * S;
        assert_eq!(limits.wait_until(now), None, "request {i}");
        limits.sent(now);
    }

    assert_eq!(limits.wait_until(start + 55 * S), Some(start + 60 * S));
}

#[test]
fn restriction_the_server_reports_blocks_until_it_ends() {
    let now = Instant::now();
    let mut limits = RateLimits::default();

    update(&mut limits, &search_headers("6:10:60,6:60:0,6:300:0,6:21600:0"), now);

    assert_eq!(limits.wait_until(now + 59 * S), Some(now + 60 * S));
    assert_eq!(limits.wait_until(now + 60 * S), None);
}

#[test]
fn rule_the_server_counts_as_full_blocks_for_its_period() {
    // Other tools on the same address count too.
    let now = Instant::now();
    let mut limits = RateLimits::default();

    update(&mut limits, &search_headers("5:10:0,5:60:0,5:300:0,5:21600:0"), now);

    assert_eq!(limits.wait_until(now), Some(now + 10 * S));
}

#[test]
fn retry_after_blocks_for_that_long() {
    let now = Instant::now();
    let mut limits = RateLimits::default();
    let mut pairs = search_headers("1:10:0,1:60:0,1:300:0,1:21600:0");
    pairs.push(("Retry-After", "42".into()));

    update(&mut limits, &pairs, now);

    assert_eq!(limits.wait_until(now), Some(now + 42 * S));
}

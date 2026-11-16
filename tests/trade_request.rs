//! Trade Requests read from Client.txt whisper lines. The messages follow the
//! trade site's own whisper templates, fetched on 2026-11-16 (one listing and
//! one bulk currency exchange); character names are made up.

use poe_overlay::poe1::client_log::{LogEvent, parse_line};
use poe_overlay::poe1::trade_request::{Direction, StashSpot, TradeRequest};

const PREFIX: &str = "2026/11/16 20:01:02 1234567 cffb0719 [INFO Client 1234] ";

fn request(message: &str) -> Option<TradeRequest> {
    match parse_line(&format!("{PREFIX}{message}")) {
        Some(LogEvent::TradeRequest(request)) => Some(request),
        Some(other) => panic!("not a Trade Request: {other:?}"),
        None => None,
    }
}

fn spot(tab: &str, left: u32, top: u32) -> Option<StashSpot> {
    Some(StashSpot { tab: tab.to_owned(), left, top })
}

#[test]
fn buyer_whispering_for_a_listing_is_an_incoming_request_with_item_price_and_stash_spot() {
    let found = request(r#"@From Tyrael_Buys: Hi, I would like to buy your Gale Fist Assassin's Mitts listed for 1 chaos in Covens (stash tab "~price 1 chaos"; position: left 1, top 8)"#);

    assert_eq!(
        found,
        Some(TradeRequest {
            direction: Direction::Incoming,
            character: "Tyrael_Buys".to_owned(),
            item: "Gale Fist Assassin's Mitts".to_owned(),
            price: Some("1 chaos".to_owned()),
            stash: spot("~price 1 chaos", 1, 8),
            note: None,
        })
    );
}

#[test]
fn guild_tag_is_not_part_of_the_buyers_name() {
    let found = request(r#"@From <ZANA> Kirac_Fan: Hi, I would like to buy your Entropy Hold Assassin's Mitts listed for 2 divine in Hardcore Covens (stash tab "sell; 2 div"; position: left 12, top 3)"#).unwrap();

    assert_eq!(found.character, "Kirac_Fan");
    assert_eq!((found.item.as_str(), found.price.as_deref()), ("Entropy Hold Assassin's Mitts", Some("2 divine")));
    assert_eq!(found.stash, spot("sell; 2 div", 12, 3), "a tab name with a semicolon");
}

#[test]
fn players_own_whisper_to_a_seller_is_an_outgoing_request() {
    let found = request(r#"@To Seller_Of_Mitts: Hi, I would like to buy your Entropy Hold Assassin's Mitts listed for 1 chaos in Covens (stash tab "~b/o 1 chaos"; position: left 3, top 6)"#).unwrap();

    assert_eq!((found.direction, found.character.as_str()), (Direction::Outgoing, "Seller_Of_Mitts"));
}

#[test]
fn bulk_currency_request_has_no_stash_spot() {
    let found = request("@From Orb_Hoarder: Hi, I'd like to buy your 1 Divine Orb for my 100 Chaos Orb in Covens").unwrap();

    assert_eq!((found.item.as_str(), found.price.as_deref(), &found.stash), ("1 Divine Orb", Some("100 Chaos Orb"), &None));
}

#[test]
fn bulk_request_with_a_full_stop_and_a_note_keeps_the_note() {
    let found = request("@From Orb_Hoarder: Hi, I'd like to buy your 3 Divine Orb for my 400 Chaos Orb in Covens. Can do 390?").unwrap();

    assert_eq!((found.price.as_deref(), found.note.as_deref()), (Some("400 Chaos Orb"), Some("Can do 390?")));
}

#[test]
fn words_after_the_stash_spot_are_the_buyers_note() {
    let found = request(r#"@From Tyrael_Buys: Hi, I would like to buy your Gale Fist Assassin's Mitts listed for 1 chaos in Covens (stash tab "~price 1 chaos"; position: left 1, top 8) would you take 1 alch?"#).unwrap();

    assert_eq!(found.note.as_deref(), Some("would you take 1 alch?"));
    assert_eq!(found.stash, spot("~price 1 chaos", 1, 8));
}

#[test]
fn listing_without_a_price_has_none() {
    let found = request(r#"@From Tyrael_Buys: Hi, I would like to buy your Glyph Nails Assassin's Mitts in Covens (stash tab "dump"; position: left 10, top 7)"#).unwrap();

    assert_eq!((found.item.as_str(), found.price), ("Glyph Nails Assassin's Mitts", None));
}

#[test]
fn plain_whispers_and_other_chat_are_not_trade_requests() {
    assert_eq!(request("@From Tyrael_Buys: are you still selling those gloves?"), None);
    assert_eq!(request("#Tyrael_Buys: Hi, I would like to buy your Gale Fist Assassin's Mitts listed for 1 chaos in Covens"), None);
    // Whispers in a buyer's own language (English client only).
    assert_eq!(request("@From Ivan_Pokupatel: Здравствуйте, хочу купить у вас Разумные ладони Перчатки убийцы за 1 chaos в лиге Covens (секция \"~price 1 chaos\"; позиция: 3 столбец, 3 ряд)"), None);
}

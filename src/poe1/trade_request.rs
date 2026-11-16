//! Trade Requests: whispers in the trade site's templates, to the player
//! from a buyer or from the player to a seller (English client only).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeRequest {
    pub direction: Direction,
    /// The other character: the buyer of an incoming request, the seller of
    /// an outgoing one.
    pub character: String,
    /// As the whisper names it (`Gale Fist Assassin's Mitts`, `3 Divine Orb`).
    pub item: String,
    /// `None` for a listing without a price.
    pub price: Option<String>,
    /// Where the item lies; `None` for bulk currency.
    pub stash: Option<StashSpot>,
    /// Anything the buyer wrote after the template.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// From a buyer to the player.
    Incoming,
    /// From the player to a seller.
    Outgoing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashSpot {
    pub tab: String,
    pub left: u32,
    pub top: u32,
}

/// The Trade Request in a chat message (`@From <TAG> Name: Hi, I would like
/// to buy your …`), if it is one.
pub fn parse(message: &str) -> Option<TradeRequest> {
    let (direction, rest) = match (message.strip_prefix("@From "), message.strip_prefix("@To ")) {
        (Some(rest), _) => (Direction::Incoming, rest),
        (_, Some(rest)) => (Direction::Outgoing, rest),
        _ => return None,
    };
    // Guild tags come before the name; names have no spaces or colons.
    let rest = match rest.strip_prefix('<') {
        Some(tagged) => tagged.split_once("> ")?.1,
        None => rest,
    };
    let (character, body) = rest.split_once(": ")?;
    let request = |item: &str, price: Option<&str>, stash, note: &str| TradeRequest {
        direction,
        character: character.to_owned(),
        item: item.to_owned(),
        price: price.map(str::to_owned),
        stash,
        note: Some(note.trim()).filter(|note| !note.is_empty()).map(str::to_owned),
    };
    if let Some(rest) = body.strip_prefix("Hi, I would like to buy your ") {
        // `ITEM listed for PRICE in LEAGUE (stash tab "TAB"; position: left L, top T)NOTE`
        let (wanted, spot) = rest.split_once(" (stash tab \"")?;
        let (tab, spot) = spot.rsplit_once("\"; position: left ")?;
        let (left, spot) = spot.split_once(", top ")?;
        let (top, note) = spot.split_once(')')?;
        let (item_and_price, _league) = wanted.rsplit_once(" in ")?;
        let (item, price) = match item_and_price.split_once(" listed for ") {
            Some((item, price)) => (item, Some(price)),
            None => (item_and_price, None),
        };
        let stash = StashSpot { tab: tab.to_owned(), left: left.parse().ok()?, top: top.parse().ok()? };
        return Some(request(item, price, Some(stash), note));
    }
    // `ITEM for my PRICE in LEAGUE`, once with a full stop and a note.
    let rest = body.strip_prefix("Hi, I'd like to buy your ")?;
    let (item, rest) = rest.split_once(" for my ")?;
    let (price, league) = rest.rsplit_once(" in ")?;
    let note = league.split_once('.').map_or("", |(_, note)| note);
    Some(request(item, Some(price), None, note))
}

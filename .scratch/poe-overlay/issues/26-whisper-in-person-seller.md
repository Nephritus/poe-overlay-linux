# 26: Whisper an in-person seller

**What to build:** Each in-person Listing row in the Price Check Panel gets **Whisper**. One click puts the trade message the trade site provides for that Listing into game chat and sends it, the same way Trade Request actions send their one chat command (clipboard paste, previous clipboard restored). One click sends one message; nothing is sent without a click. The whisper is the player's own outgoing Trade Request, so it shows as an outgoing card once those exist.

**Blocked by:** 21 (Sale type filter and listing kinds), 12 (Trade Request actions for incoming requests)

**Status:** ready-for-human

- [x] Clicking Whisper sends that Listing's trade message to the seller in game
- [x] The player's previous clipboard content is restored afterwards
- [x] One click sends one message; instant-buyout rows have no such button

## Comments

### Implemented (agent, 2026-11-17)

- **Message:** the fetch's `listing.whisper` (`@Seller Hi, I would like to buy…`), kept only for in-person Listings; sent like a Trade Request button's line (paste, clipboard put back) through the shared `ChatSender::send_to_game`, which also gives the keys back from a Price Check box first. Direct Whisper (the site's server-side whisper, login needed) was offered and not chosen.
- **Language:** the site words the whisper in the seller's language (German and Russian seen). Those reach the seller, but the game's `@To` line isn't English, so no outgoing card appears for them (Trade Request parsing is English only).
- **Tests:** whisper text on in-person Listings only, in `tests/trade_travel.rs`; the sending itself is the tested chat sequence (`tests/chat.rs`).
- **Checked live** (stand-in game whose terminal pastes on Ctrl+V): Whisper on the first in-person row sent exactly `\r`, the site's whisper (German, to that seller), `\r`; the clipboard afterwards held what it held before (the copied item).

**Needs a check (human):**

| Check | Pass when | Result |
|---|---|---|
| In game | Whisper on an in-person row whispers the seller; an English one shows an outgoing card | |

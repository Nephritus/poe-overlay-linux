# 13: Outgoing Trade Request cards with actions

**What to build:** The player's own outgoing Trade Requests (their whispers to sellers) appear as cards with Visit hideout, Trade, Thank and Leave party buttons, following the same stacking, ✕ and one-command-per-click rules as incoming cards, and visually distinct from incoming ones.

**Blocked by:** 12 (Trade Request actions for incoming requests)

**Status:** ready-for-human

- [x] An outgoing trade whisper produces a card distinct from incoming cards
- [x] Each button sends its single command for that seller
- [x] Stacking and ✕ behaviour match incoming cards

## Comments

### Implemented (agent, 2026-11-16, one commit with ticket 12)

- Outgoing cards came with ticket 11 (blue border and name, "You asked NAME"); this adds Hideout `/hideout NAME`, Trade `/tradewith NAME`, Thank `@NAME Thanks, enjoy!` and Leave `/leave` (Sidekick's default; no character name needed, the user's choice). Same stack, ✕ and one-line-per-click sending as incoming cards.
- **Tests:** the commands are in `each_card_button_sends_one_command_for_the_character_on_the_card` (`tests/chat.rs`).
- **Checked live:** an outgoing card on top of an incoming one; Hideout and Leave each sent exactly `\r`, the command, `\r`.

**Needs a check (human):**

| Check | Pass when | Result |
|---|---|---|
| Visit and leave | Hideout takes you to the seller's hideout; Leave leaves the party | |

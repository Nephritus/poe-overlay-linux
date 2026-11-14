# 11: Incoming Trade Request cards

**What to build:** Incoming Trade Requests (trade-site whispers from buyers in Client.txt) appear as cards showing buyer, item, price, and stash tab and position. Cards stack newest on top, at most about five; older ones are pushed out. Each stays until closed with ✕. Plain whispers that aren't Trade Requests do not create cards. Nothing is removed automatically on log events.

**Blocked by:** 03 (Route follows Area changes)

**Status:** ready-for-agent

- [ ] A trade-site whisper line produces a card with all four fields
- [ ] A non-trade whisper produces no card
- [ ] A sixth request pushes out the oldest card
- [ ] ✕ closes a card and the game keeps focus
- [ ] Parsing is covered by tests with real whisper lines, including guild tags and bulk currency requests

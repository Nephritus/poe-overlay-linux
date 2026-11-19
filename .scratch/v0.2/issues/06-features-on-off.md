# 06: Turn Features on and off

**What to build:** Each Feature's section (Route, Currency Rates, Price Check, trade-site search, Trade Requests) starts with an on/off switch, in the Menu and the Settings window. Off hides the Feature's Panel and lets go of its Hotkey at once, so the game gets the key. While off, the Route still follows Areas, so it's in the right place when turned back on, and Trade Requests drops whispers for good. With the trade-site search off, checking a rare, magic or normal item says "Trade-site search is off" with a link to the Menu's Price Check section, and sends nothing to the trade site. README and GLOSSARY (Feature) follow.

**Blocked by:** 05 (Record Hotkeys, applied live)

**Status:** ready-for-agent

- [ ] Each switch writes config.toml and takes effect without a restart
- [ ] Currency Rates off: its Panel hides, Alt+R reaches the game
- [ ] Route off, three Area changes, Route on: it shows the current Area's Steps
- [ ] Trade Requests off: a buyer's whisper makes no card, and none appears after turning it back on
- [ ] Search off: a rare shows the note and no trade-site request is made
- [ ] Price Check off: Alt+D reaches the game and no item is copied

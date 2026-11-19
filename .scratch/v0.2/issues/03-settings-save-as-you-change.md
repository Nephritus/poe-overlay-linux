# 03: Settings save as you change them

**What to build:** The Settings window has no Save button: changing the league, Client.txt or the trade login writes config.toml at once, keeping the player's comments, and a running Overlay applies it within a second. The window's settings become one form that the Menu (04) will show too, so each later setting is added once and appears in both. A Client.txt path that can't be read is still reported and not written.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Changing each setting writes config.toml with no Save button; comments and other settings in the file stay
- [ ] A running Overlay applies the change without a restart
- [ ] An unreadable Client.txt path is reported in the window and not written
- [ ] A config.toml with a syntax error isn't overwritten; the window says why

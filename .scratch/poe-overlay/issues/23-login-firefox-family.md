# 23: Log in with a Firefox-family browser

**What to build:** The Settings window gets **Log in**. It opens the default browser on the pathofexile.com login page; the player logs in there as usual, and the Overlay picks the login up by itself, with no copying, by reading the pathofexile.com cookies (the session, the short-lived trade token, and Cloudflare's clearance) from the browser's cookie file. Supported: Firefox, LibreWolf, Zen, Floorp and Waterfox, including Flatpak installs; the default browser's profiles are read first. Settings shows "Logged in" and a **Log out** that makes the Overlay forget the login (the browser stays logged in). Trade site requests carry those cookies and the User-Agent of the browser they came from, since Cloudflare's clearance only holds for it. When the trade token expires the Overlay reads the browser again on its own; whether the trade site renews the token in its answers is checked and recorded in this ticket. README says what is read, from where, and that only pathofexile.com cookies are kept.

Found on 2026-11-16 in the user's Firefox: the session cookie lasts 30 days, the trade token 1 day, Cloudflare's clearance 1 year; all three are written to disk.

**Blocked by:** None (can start immediately)

**Status:** ready-for-human

- [x] Log in opens the default browser; after logging in there, Settings shows "Logged in" without further steps
- [x] Trade searches send the login and the browser's User-Agent
- [x] An expired trade token is replaced without the player doing anything while the browser stays logged in
- [x] Log out stops the Overlay sending the login
- [x] A browser that is open (cookie file locked) can still be read
- [x] No cookie other than pathofexile.com's is kept or logged

## Comments

### Implemented (agent, 2026-11-17)

- **Token renewal (checked 2026-11-16):** one logged-in search with the user's Firefox cookies and Firefox 159's User-Agent answered 200 with `X-Rate-Limit-Rules: Account,Ip` (anonymous searches get `Ip`) and **no `Set-Cookie`**: the trade site doesn't renew POETOKEN in its answers. So the Overlay reads the browser again when the token expires, and at most once a minute while it has none; the browser gets a new token when the player opens the site there.
- **Browsers (`browsers`, generic):** Firefox (`.config/mozilla/firefox` since Firefox 147, the user's case; `.mozilla/firefox`, Snap, Flatpak), LibreWolf, Zen, Floorp, Waterfox (dot folders and Flatpak; `.config/librewolf/librewolf` and `.config/zen` are guesses following Firefox's move, unverified). A profile is a folder with `cookies.sqlite`. The default browser is gio's https handler, matched by desktop ID. The cookie file and its `-wal` are copied into a 0700 folder in `$XDG_RUNTIME_DIR`, read with SQL limited to the domain and its subdomains, and deleted. Expiry is read in ms (Firefox 159) or s (older).
- **User-Agent:** the profile's `general.useragent.override` if set; else Firefox's UA for the Gecko version from the install's `platform.ini` (forks: Zen 1.17 is Gecko 145), else `LastVersion` in `compatibility.ini` when it's ≥ 100; else the Overlay's usual Chrome UA.
- **Login (`poe1::login`):** only POESESSID, POETOKEN and cf_clearance, unexpired, newest per name; no session → no login. Default browser's login wins, then the newest session. Kept in memory only; `Login` has no `Debug`. `TradeHttp::send` takes the login; `TradeSite` adds `Cookie` and the browser's `User-Agent`.
- **Settings:** a Trade site row with Log in / Log out, written at once as `[trade] login = true` (the Overlay follows config.toml within a second). Log in uses a login a browser already holds (user's choice); else it opens the default browser on `https://www.pathofexile.com/login` and looks every 3 s until "Logged in with Firefox". Unreadable profiles are listed under the status.
- **Tests:** `tests/browser_cookies.rs` (4: only the site's cookies with expiry, older seconds format, a locked file with cookies only in the WAL, missing file named), `tests/browser_profiles.rs` (5: native/XDG/Flatpak folders for all five, default browser first, UA from the version that last ran the profile, a fork's UA from its Gecko version, UA override), `tests/trade_login.rs` (7: login and UA sent on search and fetch, nothing sent or read without login or after Log out, expired token read again, expired cookies left out, re-read once a minute without login, without token, default browser then newest). Each fails when the code it covers is broken (mutation-checked).
- **Checked live:**
  - Settings with a stand-in default browser (a `firefox.desktop` that logs its URL) and an empty home: Log in opened `https://www.pathofexile.com/login`, wrote `[trade] login = true`; a Firefox profile with a session appearing 4 s later turned the status into "Logged in with Firefox." without a click. Log out removed the key: "Logged out. Your browser stays logged in on pathofexile.com."
  - Overlay with the user's real Firefox profile (Firefox closed) and `login = true`: the first Price Check (Death Nails) read `prefs.js`, `compatibility.ini`, `cookies.sqlite` and its `-wal` (seen with inotify) through a private copy that was gone afterwards, and the search answered (1 listed). After Log out in config.toml, the next Price Check (Invasion Track, 22 listed) didn't touch the profile.
  - Headers checked against a local server: `cookie: POESESSID=…; POETOKEN=…` and only the browser's `user-agent`; without a login, the usual Chrome one.
- **After review (inline; the review agents hit an API rate limit):** both cookie readers share `Copy::read` (copy, open, map errors once) instead of repeating it; `Login`'s expiry and token flag are private to `poe1::login`; the Settings counter that stops an older login search is `login_shown` (it counts every shown state, not clicks). Not changed: if the Overlay is killed mid-read, the private copy stays in `$XDG_RUNTIME_DIR` (user-only, cleared at logout) until then; the Overlay reads browsers on the search thread, so a slow keyring delays that search.

**Needs a check (human):**

| Check | Pass when | Result |
|---|---|---|
| Real login | With Firefox open and logged in on pathofexile.com, Log in shows "Logged in with Firefox" at once | |
| Fresh login | Logged out of pathofexile.com in Firefox, Log in opens the login page; after logging in there, Settings shows "Logged in with Firefox" within a few seconds | |
| Next day | A day after the last visit, open the trade site in Firefox once; Price Checks keep working without Settings | |
| Fork | LibreWolf, Zen, Floorp or Waterfox (any one you use) is found the same way | |

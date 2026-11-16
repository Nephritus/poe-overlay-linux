# 23: Log in with a Firefox-family browser

**What to build:** The Settings window gets **Log in**. It opens the default browser on the pathofexile.com login page; the player logs in there as usual, and the Overlay picks the login up by itself, with no copying, by reading the pathofexile.com cookies (the session, the short-lived trade token, and Cloudflare's clearance) from the browser's cookie file. Supported: Firefox, LibreWolf, Zen, Floorp and Waterfox, including Flatpak installs; the default browser's profiles are read first. Settings shows "Logged in" and a **Log out** that makes the Overlay forget the login (the browser stays logged in). Trade site requests carry those cookies and the User-Agent of the browser they came from, since Cloudflare's clearance only holds for it. When the trade token expires the Overlay reads the browser again on its own; whether the trade site renews the token in its answers is checked and recorded in this ticket. README says what is read, from where, and that only pathofexile.com cookies are kept.

Found on 2026-11-16 in the user's Firefox: the session cookie lasts 30 days, the trade token 1 day, Cloudflare's clearance 1 year; all three are written to disk.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Log in opens the default browser; after logging in there, Settings shows "Logged in" without further steps
- [ ] Trade searches send the login and the browser's User-Agent
- [ ] An expired trade token is replaced without the player doing anything while the browser stays logged in
- [ ] Log out stops the Overlay sending the login
- [ ] A browser that is open (cookie file locked) can still be read
- [ ] No cookie other than pathofexile.com's is kept or logged

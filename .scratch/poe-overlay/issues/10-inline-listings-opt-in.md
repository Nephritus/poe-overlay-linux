# 10: Opt-in inline listings in Price Check

**What to build:** A settings toggle (off by default) lets the overlay fetch live listings from the trade site and show them inside the Price Check panel. Requests use a browser User-Agent (Cloudflare rejects identifying ones) and respect the trade site's rate-limit headers. On a 403 or rate limit the panel falls back to the "Open on trade site" button and says why. The settings window explains the trade endpoints are undocumented and tolerated, not officially permitted.

**Blocked by:** 08 (Price Check trade site search), 09 (Settings window)

**Status:** ready-for-agent

- [ ] With the toggle off, the overlay makes no trade site requests
- [ ] With it on, Price Checking a rare shows cheapest listings in the panel
- [ ] Rate-limit headers are honoured: no request is sent while a limit is active
- [ ] A 403 shows the fallback button with an explanation
- [ ] Settings window shows the risk notice next to the toggle

# 08: Price Check for everything else: open search on the trade site

**What to build:** For items poe.ninja can't price (rares, magic items, bases, etc.), the Price Check panel offers "Open on trade site", which opens the user's browser on the official trade site with a search prefilled from the item's base and mods. The overlay itself does not call the trade site: stat IDs come from a bundled snapshot (the live stats endpoint is behind Cloudflare without a browser User-Agent). Mods the snapshot can't map are listed in the panel as excluded rather than silently dropped.

**Blocked by:** 07 (Price Check for poe.ninja-priced items)

**Status:** ready-for-agent

- [ ] Price Checking a rare shows the button; clicking it opens the default browser on a trade search matching the item's base and mods
- [ ] No request to the trade site is made by the overlay
- [ ] Unmapped mods are visibly listed as excluded
- [ ] Query building is covered by tests using real rare item samples

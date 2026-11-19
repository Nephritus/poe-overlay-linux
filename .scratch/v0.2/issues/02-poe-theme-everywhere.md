# 02: PoE-inspired theme everywhere

**What to build:** Every Panel (Route, Currency Rates, Price Check, Trade Requests) and the Settings window use the theme approved in 01. Cinzel and Alegreya ship inside the program, so the packages, the AppImage and `cargo install` all show them without anything installed on the system. Their OFL licenses ship with every install form and appear in `poe-overlay credits`.

**Blocked by:** 01 (Prototype the PoE-inspired look)

**Status:** ready-for-agent

- [ ] Each Panel and the Settings window match the approved prototype
- [ ] The fonts show on a system without them installed (checked in the packaging smoke containers or a clean user)
- [ ] `poe-overlay credits` and the packages carry the fonts' OFL licenses
- [ ] Buttons, dropdowns and boxes keep their focus, hover and error looks (e.g. a refused Price Check bound)

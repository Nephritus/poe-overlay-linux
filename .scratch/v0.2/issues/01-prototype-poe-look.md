# 01: Prototype the PoE-inspired look

**What to build:** A throwaway GTK program under `prototypes/` that shows the Menu (section column, a Feature switch, a Hotkey box, a slider), the Route Panel and the wide Price Check Panel with Listings. It uses the PoE-inspired theme: dark brown/black, gold trim, Cinzel headings, Alegreya text, our own borders and colours, no GGG art. It's drawn with the real GTK CSS on a layer-shell surface over a screenshot or the running game, so it shows only what GTK can draw. The player approves the look or asks for changes; the approved CSS and colour values are recorded in this ticket for 02.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] The prototype runs and shows the three Panels in the theme, at UI scale 1 and over a busy game background
- [ ] Text stays readable at the Panels' smallest sizes (Listing rows, Sub-steps)
- [ ] The player approved the look; the decision-rich CSS (palette, borders, font sizes) is recorded under Comments
- [ ] Nothing from the prototype is used by the program as code

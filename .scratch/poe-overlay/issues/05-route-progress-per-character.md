# 05: Route progress per Character with Previous/Next

**What to build:** Route progress belongs to a Character. The Character's name is learned from Client.txt level-up lines; progress is saved per Character and restored after the overlay restarts. Clickable Previous and Next buttons on the Route panel correct the position when auto-advance gets it wrong (e.g. a backtrack). Buttons work without the overlay taking keyboard focus.

**Blocked by:** 03 (Route follows Area changes)

**Status:** ready-for-agent

- [ ] Previous/Next move the current Step and the game keeps keyboard focus
- [ ] Progress for two different Characters is tracked independently
- [ ] Restarting the overlay restores the last Step for the active Character
- [ ] Before any Character is identified, the Route still works and attaches to the Character once identified

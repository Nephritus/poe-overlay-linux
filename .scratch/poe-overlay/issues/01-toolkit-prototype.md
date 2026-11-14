# 01: Toolkit prototype: GTK4 vs iced click-through panel over PoE

**What to build:** Build two throwaway prototypes, one with gtk4-rs + gtk4-layer-shell and one with iced + iced_exwlshell, each showing a transparent layer-shell panel with one button over a running Path of Exile on niri. The point is to prove the core guarantee this project exists for (the overlay never steals focus from the game and never blocks clicks outside its panels) and to let the user pick the toolkit they prefer working in. If both pass and preference is tied, choose GTK4.

Awakened PoE Trade failed on this exact setup: overlay not on top, focus stealing/flicker, dead hotkeys, broken copy, and a black screen after Alt+Tab (absent when it isn't running). Sourced facts behind these decisions are in this feature's `research.md`. Vocabulary per `GLOSSARY.md`.

**Blocked by:** None (can start immediately)

**Status:** ready-for-human

- [ ] Each prototype shows a semi-transparent panel with a button on the overlay layer above PoE (borderless windowed, Proton/Xwayland) on niri
- [ ] Clicking the button registers in the prototype while the PoE window keeps keyboard focus (confirmed via the compositor's focused-window query and by game keys still working)
- [ ] Clicks outside the panel reach the game
- [ ] Alt+Tab away and back shows the game normally: no black screen, no flicker
- [ ] A short comparison note (code feel, look, dependencies, AppImage implications) is appended to this ticket
- [ ] The user's toolkit choice is recorded in this ticket; prototype code is deleted or kept out of the main source tree

## Comments

### Prototype built (agent, 2026-11-14)

The prototype code is on the throwaway branch `prototype/01-toolkit` (commits 4d77f4f, 3e88d75), under `prototypes/01-toolkit/`. Run it with `./run.sh iced` or `./run.sh gtk4`; the in-game checklist (A–F) is in that folder's README. Both prototypes draw the same thing: a full-screen transparent surface on the Overlay layer with keyboard interactivity `none`, where only a 360×170 panel accepts clicks. The panel shows a click counter and niri's focused window.

**Smoke check on niri 26.04, without the game:** both surfaces appear on the Overlay layer with keyboard interactivity `none`, and the focused window (id 128) is unchanged after they start.

| | iced 0.14 + iced_exwlshell 0.21.4 | gtk4-rs 0.11 + gtk4-layer-shell 0.8 (lib 1.3.0) |
|---|---|---|
| Release binary | 21 MB | 720 KB |
| Crates in dependency tree | 337 | 79 |
| Shared libraries linked | 5 (libc, libm, libgcc) | 114 (GTK, pango, cairo, glib, wayland, gtk4-layer-shell, …) |
| Cold release build | ~1m13s | ~1 min |
| Prototype code (non-blank lines) | 122 | 128 |
| Build friction | None | `CssProvider::load_from_string` needs the `v4_12` feature; input region takes `Option<&Region>` |

**Code feel**
- **iced:** Elm style, with state, a `Message` enum, `update` and `view`. Layer settings go in one struct. The input region is set by returning a `SetInputRegion` message. Watch out: sent at startup, before the surface is registered, iced_exwlshell drops it with an error nothing displays, so the prototype sends it on the first tick instead. That kind of silent sharp edge counts against iced. A polling timer is a subscription. Events from the log, clipboard and socket would map naturally onto messages.
- **GTK4:** builds widgets step by step, shares state through `Rc<RefCell<…>>` cloned into each signal handler, and styles with CSS. The input region is set on the GDK surface at realize and again on every layout.

**AppImage implications**
- **iced:** links only libc, so an AppImage is trivial. wgpu loads the system's Vulkan or GL drivers at runtime, and those must not be bundled.
- **GTK4:** the AppImage must bundle the GTK4 stack and gtk4-layer-shell, or require them on the host. gtk4-layer-shell must load before libwayland-client. AUR, .deb and .rpm can simply declare the packages as dependencies.

**Look:** your call after running both.

**In-game results:** pending (human).

| Check | iced | GTK4 |
|---|---|---|
| A Panel visible, game visible around it | | |
| B Click keeps focus on PoE | | |
| C Game keys work after clicking | | |
| D Click-through outside the panel | | |
| E Alt+Tab, no black screen or flicker | | |
| F No flicker or FPS drop | | |

**Toolkit choice:** pending (human).

# PoE1 Wayland overlay — design research (as of 2026-11-14)

Legend: **[src]** = verified by reading source/docs listed; **[INFERENCE]** = not directly verified; **[REPORTED]** = user/third-party report.

Source checkouts used (shallow clones, 2026-11-14): niri `73a0444` (2026-11-12), xwayland-satellite (2026-11-05), awakened-poe-trade (2026-09-25), exile-leveling (2026-11-12), Exile-UI, klayveR/poe-log-monitor (2020-08-31), ValveSoftware/steam-devices.

---

## 1. Global shortcuts on Wayland

| Compositor | GlobalShortcuts portal | Notes |
|---|---|---|
| **niri** | **Yes, niri-git only** (merged ~2026-11-07, not in a tagged release yet) via xdg-desktop-portal-gnome | PR [#4744](https://github.com/niri-wm/niri/pull/4744). niri implements `org.gnome.Shell` `GrabAccelerator(s)`/`UngrabAccelerator` (`src/dbus/gnome_shell.rs` in niri main); xdp-gnome shows the UI. **Keys are forwarded to the focused window (pass-through)**, not swallowed (PR description). Requires `gnome-control-center` installed ([Important-Software.md](https://github.com/niri-wm/niri/blob/main/docs/wiki/Important-Software.md)). Not triggered on lock screen (GNOME behaviour tested in PR). Earlier attempts: [#3587](https://github.com/niri-wm/niri/pull/3587), [#1165 hyprland-global-shortcuts-v1](https://github.com/niri-wm/niri/pull/1165), [discussion #2775](https://github.com/niri-wm/niri/discussions/2775). |
| **GNOME** | Yes since GNOME 48 (xdp-gnome) | [TWIG #189](https://thisweek.gnome.org/posts/2025/02/twig-189), [GNOME 48 notes](https://release.gnome.org/48); xdp-gnome 46.x lacked it ([report](https://github.com/pingdotgg/t3code/issues/12589)). |
| **KDE Plasma** | Yes (xdg-desktop-portal-kde → kglobalaccel) | [globalshortcuts.cpp](https://invent.kde.org/plasma/xdg-desktop-portal-kde/-/blob/master/src/globalshortcuts.cpp) |
| **Hyprland** | Yes (xdph, via `hyprland-global-shortcuts-v1`) | [GlobalShortcuts.cpp](https://github.com/hyprwm/xdg-desktop-portal-hyprland/blob/master/src/portals/GlobalShortcuts.cpp), [wiki](https://wiki.hypr.land/Hypr-Ecosystem/xdg-desktop-portal-hyprland). Hyprland ignores app-provided default triggers; user binds via `global` dispatcher ([dec05eba analysis](https://dec05eba.com/2024/03/29/wayland-global-hotkeys-shortcut-is-mostly-useless)). |
| **sway / xdpw** | **No** | [xdpw #240](https://github.com/emersion/xdg-desktop-portal-wlr/issues/240) (open), [sway #8062](https://github.com/swaywm/sway/issues/8062). |

Portal spec: [org.freedesktop.portal.GlobalShortcuts](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html).

**niri native binds** ([Configuration:-Key-Bindings.md](https://github.com/niri-wm/niri/blob/main/docs/wiki/Configuration:-Key-Bindings.md)) [src]:
- `Key { spawn "prog" "arg"; }` (no shell) and `spawn-sh` (shell). Works as a universal fallback: bind → spawn `overlayctl price-check` (IPC to daemon).
- `allow-when-locked=true` exists only for spawn binds (lock screen).
- `allow-inhibiting=false` makes a bind ignore keyboard-shortcuts-inhibit (games/Xwayland may inhibit; `toggle-keyboard-shortcuts-inhibit` escape hatch).
- `repeat=false`, `cooldown-ms`, press/release actions.
- **Bound keys are swallowed**: niri records them in `suppressed_keys` and returns `InterceptOnly`, so the focused window never sees press or release (`src/input/mod.rs`, ~L640–730) [src]. Consequence: a niri bind on `Ctrl+D` will NOT reach PoE; use a key PoE doesn't need, or use the portal path (pass-through).
- Hyprland equivalent of pass-through: `bind = ..., pass, class:...` ([APT #1713](https://github.com/SnosMe/awakened-poe-trade/issues/1713)).

## 2. Clipboard without focus (data-control)

| Compositor | ext-data-control-v1 | wlr-data-control |
|---|---|---|
| niri | yes | yes |
| Hyprland | yes | yes |
| sway 1.11 (wlroots 0.19) | yes | yes |
| COSMIC | yes | yes |
| KWin (Plasma 6.x) | yes | **no** |
| Mutter (GNOME) | **no** | **no** |

Sources: niri source contains `ext_data_control_state` and `wlr_data_control_state` [src]; matrix from [Clipmer support table (Aug 2026)](https://clipmer.app/wayland-clipboard-support) [third-party]; [mutter #3941](https://gitlab.gnome.org/GNOME/mutter/-/work_items/3941) (open); [sway 1.11 release](https://github.com/swaywm/sway/releases/tag/1.11). wl-clipboard learned ext-data-control only in 2.3.0 (Mar 2026) per Clipmer. KDE bug about offer ordering for Xwayland SAVE_TARGETS: [kde-bugs 527831](https://mail-archive.com/kde-bugs-dist%40kde.org/msg1224316.html).

**X11 → Wayland bridging with xwayland-satellite**: satellite contains a selection bridge (`src/xstate/selection.rs`, `src/server/selection.rs`, interns `CLIPBOARD`) and lists *Primary selection* as optional protocol ([README](https://github.com/Supreeeme/xwayland-satellite#wayland-protocols)) [src]. So text copied by an X11 client is exposed as a Wayland selection owned by satellite, which data-control clients should see. [INFERENCE: not runtime-tested here with PoE.] The niri Xwayland page's caveat "rootful Xwayland doesn't share clipboard" applies only to rootful Xwayland, not satellite ([Xwayland.md](https://github.com/niri-wm/niri/blob/main/docs/wiki/Xwayland.md)). Alternative avoiding Wayland altogether: read CLIPBOARD as an X11 client on `$DISPLAY` (same X server as the game).

Policy constraint (GGG): clipboard/screen reads only once per manual key press ([developer docs](https://www.pathofexile.com/developer/docs)).

## 3. Window focus tracking

- **niri**: implements `zwlr_foreign_toplevel_manager_v1` and sets `State::Activated` for the focused window (`src/protocols/foreign_toplevel.rs` ~L699–704; comment notes clients treat activated as keyboard focus) [src]. Also `ext_foreign_toplevel_list_v1` (no state). IPC: `niri msg --json event-stream` emits `WindowsChanged`, `WindowFocusChanged { id }`, windows carry `is_focused` ([niri-ipc/src/lib.rs](https://github.com/niri-wm/niri/blob/main/niri-ipc/src/lib.rs), [IPC.md](https://github.com/niri-wm/niri/blob/main/docs/wiki/IPC.md)) [src]. Xwayland windows via satellite appear as normal Wayland toplevels (app_id/title from X11 class/title [INFERENCE]).
- **Hyprland**: socket2 events `activewindow>>class,title`, `activewindowv2>>address` ([IPC wiki](https://wiki.hypr.land/IPC/)) [not re-fetched]; also wlr-foreign-toplevel.
- **sway**: i3 IPC `subscribe ["window"]` → `change: "focus"` ([sway-ipc(7)](https://man.archlinux.org/man/sway-ipc.7)) [not re-fetched]; wlroots provides wlr-foreign-toplevel.
- **KDE**: no wlr-foreign-toplevel ([KDE bug 502647](https://bugs.kde.org/show_bug.cgi?id=502647)); uses `org_kde_plasma_window_management` ([xml](https://github.com/KDE/plasma-wayland-protocols/blob/master/src/protocols/plasma-window-management.xml)) or KWin scripting / D-Bus.
- **GNOME**: neither; extension only [INFERENCE].
- Cross-compositor fallback for an X11 game: query `_NET_ACTIVE_WINDOW` on the Xwayland X server — reliability under satellite (which acts as X WM) [unverified].

## 4. Layer-shell support

- **niri**: wlr-layer-shell incl. on-demand focus (`layer_shell_on_demand_focus`) [src]; docs: [Layer-Shell-Components.md](https://github.com/niri-wm/niri/blob/main/docs/wiki/Layer%E2%80%90Shell-Components.md), [Layer rules](https://github.com/niri-wm/niri/blob/main/docs/wiki/Configuration:-Layer-Rules.md) (recommends mapping with `none` then switching to `on-demand`). Bottom/background layers only get *exclusive* focus when workspace has no windows.
- **Click on a `keyboard_interactivity=none` layer surface in niri**: `focus_layer_surface_if_on_demand()` only grants focus when interactivity is `OnDemand`; otherwise it just clears any existing on-demand layer focus. The window focus is untouched → toplevel (game) keeps keyboard focus (`src/niri.rs` ~L7131–7153) [src]. Pointer input still goes to the layer surface where its input region covers.
- **Hyprland, sway, COSMIC**: implement wlr-layer-shell (reference implementations in wlroots; [protocol page](https://wayland.app/protocols/wlr-layer-shell-unstable-v1) lists compositors) [not individually re-verified].
- **KDE Plasma 6**: supported; Qt clients via [LayerShellQt](https://invent.kde.org/plasma/layer-shell-qt) (Arch `layer-shell-qt` 6.8.2 [pkg](https://archlinux.org/packages/extra/x86_64/layer-shell-qt)).
- **GNOME**: not supported ([mutter #973](https://gitlab.gnome.org/GNOME/mutter/-/work_items/973)).
- Click-through = `wl_surface.set_input_region` (core protocol, compositor-agnostic). `keyboard_interactivity` none/exclusive/on_demand (on_demand since protocol v4) — [protocol](https://wayland.app/protocols/wlr-layer-shell-unstable-v1).

## 5. Input injection

- **uinput** (kernel virtual keyboard): compositor-agnostic, goes through libinput → focused window, including Xwayland. Caveats: emits keycodes, so text depends on the active XKB layout (send keycodes for Ctrl/Enter, but use clipboard paste for chat text); must release modifiers the user still holds (hotkey Ctrl may still be down); new device appears to compositor (keymap/device-add latency on first use) [INFERENCE, common practice]. GNOME/Mutter reportedly drops uinput keys in some setups ([openwhispr #956](https://github.com/OpenWhispr/openwhispr/issues/956)) [REPORTED].
- **udev/ACL**: Steam's rule `KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", OPTIONS+="static_node=uinput"` ([60-steam-input.rules](https://github.com/ValveSoftware/steam-devices/blob/master/60-steam-input.rules)) [src] — grants the active-seat user via systemd-logind uaccess. Arch package `steam-devices`/`game-devices-udev` [unverified names].
- **niri** also exposes `zwp_virtual_keyboard_v1` (Wayland virtual keyboard; requires uploading a keymap) [src: `virtual_keyboard_*` in niri].
- **XTEST into Xwayland**: X11 XTEST is delivered within the X server. Since Xwayland 23.2, if built with the EI portal, XTEST is routed via libei/RemoteDesktop portal (user permission prompt) ([Hutterer, 2026-07](http://who-t.blogspot.com/2026/07/libei-integrations-in-xdg-remotedesktop.html); Fedora mutter change: [discussion](https://discussion.fedoraproject.org/t/todays-update-of-mutter-breaks-simulating-key-presses-with-xdotool/98216)). Under xwayland-satellite (rootless Xwayland spawned by satellite) behaviour is **[unverified]**; satellite source has no XTEST handling (`grep xtest` empty) [src].
- **winewayland**: still opt-in (`PROTON_ENABLE_WAYLAND=1` in GE/CachyOS forks), not default as of late 2026 ([steam-for-linux #13341](https://github.com/ValveSoftware/steam-for-linux/issues/13341), [proton-cachyos #163](https://github.com/CachyOS/proton-cachyos/issues/163), [GE-Proton 11-6](https://www.gamingonlinux.com/2026/08/ge-proton-11-6-released-with-updates-for-wayland-ps5-controllers-and-more-upgrades)). If enabled, the game is a native Wayland client → XTEST and X11 clipboard/active-window approaches stop working; uinput still works. → **uinput is the robust choice.**
- GGG macro rules: manual invocation, one action per press ([developer docs](https://www.pathofexile.com/developer/docs)).

## 6. Official PoE1 trade API

Endpoints used by Awakened PoE Trade [src: `renderer/src/web/...`]:
- `GET https://www.pathofexile.com/api/leagues?type=main&realm=pc` (Leagues.ts)
- `POST /api/trade/search/{league}` → `GET /api/trade/fetch/{ids,≤10}?query={id}` (pathofexile-trade.ts L773/806)
- `POST /api/trade/exchange/{league}` (pathofexile-bulk.ts)
- Stats/items: `/api/trade/data/stats`, `/api/trade/data/items`, `/api/trade/data/static`, `/api/trade/data/leagues` — used by trade site; APT bundles pre-generated data instead [INFERENCE, not located in APT tree].
- Rate limiting: APT parses `X-Rate-Limit-Rules`, `X-Rate-Limit-{rule}`, `X-Rate-Limit-{rule}-State` and starts with conservative 1 req/5 s limiters (`trade/common.ts`) [src].
- POESESSID: APT sends session cookies (`useSessionCookies: true`) from its built-in browser login; not required for public search ([APT #691](https://github.com/SnosMe/awakened-poe-trade/issues/691), maintainer: login possible in builtin browser) [INFERENCE that anonymous works today].
- User-Agent: APT uses Electron default UA (`main/src/proxy.ts`).

GGG policy ([developer docs](https://www.pathofexile.com/developer/docs), [authorization](https://www.pathofexile.com/developer/docs/authorization)):
- **Trade search/fetch is NOT part of the documented API reference** (reference only lists e.g. `/api/currency-exchange`). Docs: "It is against our Terms of Use (section 7i) to reverse-engineer endpoints outside of this documentation." Overlay tools using trade endpoints rely on tolerance, not permission. ⚠ legal/policy risk.
- Standalone executables "not encouraged, but permitted"; must use **public OAuth client** (PKCE, localhost redirect, 10 h access / 7 d refresh, shared rate limits, cannot use `service:*`).
- Scopes: `account:profile|leagues|stashes|characters|league_accounts|item_filter`; services (confidential only): `service:leagues`, `service:leagues:ladder`, `service:pvp_matches[:ladder]`, `service:psapi`, `service:cxapi` (Currency Exchange also public).
- Required UA: `User-Agent: OAuth {clientId}/{version} (contact: {contact}) ...`.
- Required notice: "This product isn't affiliated with or endorsed by Grinding Gear Games in any way."
- Rate-limit headers: `X-Rate-Limit-Policy`, `-Rules`, `-{rule}`, `-{rule}-State`, `Retry-After`; too many 4xx → restriction.
- **App registration currently closed**: "We are currently unable to process new applications."
- Overlay rules: screen/clipboard/log reads only with user awareness; screen/clipboard once per key press; macros one action per press.
- **Cloudflare**: GGG uses Cloudflare bot protection; APT documents "Failed to load leagues" → solve CAPTCHA in built-in browser every ~15 min ([APT doc](https://github.com/SnosMe/awakened-poe-trade/blob/master/docs/failed-load-leagues.md)); APT strips `Partitioned` from Cloudflare cookies (proxy.ts) [src]; related issues [#1630](https://github.com/SnosMe/awakened-poe-trade/issues/1630) (2025-08, ERR_CONNECTION_RESET), [#1555](https://github.com/SnosMe/awakened-poe-trade/issues/1555) (QUIC errors, updated 2026-08). Trade-site rate-limit complaints: [forum 3801062](https://www.pathofexile.com/forum/view-thread/3801062), [3802598](https://www.pathofexile.com/forum/view-thread/3802598).

## 7. poe.ninja API

Official doc page: [poe.ninja/docs/api](https://poe.ninja/docs/api) [src]:
- Supported public surface = economy endpoints only; builds/profiles/character/PoB APIs **not allowed**.
- `GET /poe1/api/economy/leagues`
- `GET /poe1/api/economy/exchange/current/overview?league=&type=` (Currency, Fragment, Scarab, Essence, Oil, DivinationCard, …)
- `GET /poe1/api/economy/stash/current/item/overview?league=&type=` (UniqueWeapon, SkillGem, Map, BaseType, …)
- `GET /poe1/api/economy/stash/current/currency/overview?league=&type=Currency|Fragment`
- No versioning/SLA; ~5 min HTTP cache (ETag), PoE1 data refresh ~15 min; send descriptive UA with contact; **desktop apps should proxy via own backend** (recommendation); misbehaving clients blocked. No explicit attribution clause found on the page.
- APT currently uses an undocumented `poe.ninja/poe1/api/economy/current/dense/overviews?league=&language=en` (Prices.ts) [src].
- Historical dumps: [poe.ninja/poe1/data](https://poe.ninja/poe1/data).

## 8. Leveling route data

- **HeartofPhos/exile-leveling** — **MIT** ([repo](https://github.com/HeartofPhos/exile-leveling), LICENSE © 2025 HeartofPhos) [src]. Data in `common/data`:
  - `routes/act-1.txt … act-10.txt`: custom DSL, e.g. `➞ {enter|1_1_town} #Lioneye's Watch`, `{kill|Hillock}`, `{quest|a1q1}`, `{waypoint|1_1_2}`, `{waypoint_get}`, `{logout}`, `{dir|270}`, `{quest_text|...}`, `#section`, `#sub`, `#ifdef LEAGUE_START`/`#endif`.
  - `json/areas.json` keyed by internal area ID (`1_1_1` → name, act, level, has_waypoint, is_town_area, parent_town_area_id, connection_ids, crafting_recipes) — IDs match Client.txt `Generating level N area "ID"`.
  - `json/quests.json`, `gems.json`, `characters.json`, `kill-waypoints.json`, `gem-colours.json`, `vaal-gem-lookup.json`, `awakened-gem-lookup.json`.
  - Seeded from game `.dat` via [HeartofPhos/exile-export](https://github.com/HeartofPhos/exile-export) and GGG [skilltree-export](https://github.com/grindinggear/skilltree-export) (README/seeding sources).
  - Reuse in OSS: permitted under MIT with copyright notice; game-derived strings remain GGG IP (GGG forbids commercial use of IP) [INFERENCE].
- **Lailloken/Exile-UI** — **MIT** © 2022 Lailloken ([repo](https://github.com/Lailloken/Exile-UI)) [src]; AHK, leveling tracker consumes exile-leveling exports ([discussion #88](https://github.com/Lailloken/Exile-UI/discussions/88)).
- **PoE Leveling Guide (JusKillmeQ)** — license **not verified** (repo not checked).
- **RePoE / poe-dat-viewer / poewiki**: not fetched in this pass [unverified]. poewiki content is typically CC BY-NC-SA [unverified].
- Older area name data: klayveR/poe-log-monitor `resource/areas.json` (MIT, 2018) — keyed by display name, stale.

## 9. Client.txt line formats (PoE1)

From klayveR/poe-log-monitor `resource/events.json` (MIT, last updated 2020) [src] and Exile-UI `modules/client log.ahk` + `data/english/client.txt` (current) [src]. Prefix: `YYYY/MM/DD HH:MM:SS <ticks> <hex> [INFO Client <pid>] `.

- Area generated: `... [DEBUG Client N] Generating level 12 area "1_1_4_1" with seed 123` — Exile-UI parses `Generating level `, `area "`→ID, `with seed ` (L369–383).
- Area entered: `: You have entered The Coast.` regex `\[INFO Client [0-9]*] : You have entered (.*)\.`
- Level up: `: Name (Class) is now level 12` → `: (.*) \((.*)\) is now level ([0-9]+)`
- Whispers: `@From <Guild> Name: msg` / `@To Name: msg` → `@(To|From) (?:<(\S+)> )?(\S+): (.+)`
- Trade: `: Trade accepted.` / `: Trade cancelled.` → `: Trade (cancelled|accepted)`
- AFK/DND: `: AFK mode is now ON. Autoreply "..."` / `... is now OFF.`
- Joined/left: `: (\S+) has joined the area.` / `: (\S+) has left the area.`
- Other: deaths `has been slain`, `/played`, `/age`, `Connecting to instance server at`, `/whois` → `is a level # in the`, `You have killed # monsters`, hideout `Spawning discoverable Hideout`, NPC lines `Einhar, Beastmaster:` etc.
- Localised clients change text (e.g. German `Ihr habt # betreten`) — Exile-UI ships per-language strings.
- [unverified] whether 3.27 changed the bracket prefix; APT watcher just tails the file (`main/src/host-files/GameLogWatcher.ts`, polls every 450 ms, starts at EOF). Default Linux Steam path: `~/.local/share/Steam/steamapps/common/Path of Exile/logs/Client.txt` (APT).

## 10. Toolkit options

- **gtk4-layer-shell** (C lib, [wmww/gtk4-layer-shell](https://github.com/wmww/gtk4-layer-shell), [docs](https://wmww.github.io/gtk4-layer-shell/gtk4-layer-shell-GTK4-Layer-Shell.html)); Arch package `gtk4-layer-shell` [name unverified this pass]; Rust crate [`gtk4-layer-shell`](https://docs.rs/gtk4-layer-shell). Mature (used by many bars). Click-through via `gdk_surface_set_input_region` (works per [issue #61](https://github.com/wmww/gtk4-layer-shell/issues/61)); transparency via CSS/RGBA. Keyboard mode none/exclusive/on_demand.
- **LayerShellQt** (KDE, Qt6; [repo](https://invent.kde.org/plasma/layer-shell-qt)) — production (Plasma uses it). Input region via `QWindow::setMask` [INFERENCE].
- **Quickshell** (QML) — `QsWindow.mask: Region` for input masks ([docs](https://quickshell.org/docs/v0.1.0/types/Quickshell/QsWindow)); transparent `color`. Popular for shells; v0.x.
- **iced_layershell** v0.19.1 ([crates.io](https://crates.io/crates/iced_layershell), [docs](https://docs.rs/iced_layershell/latest/iced_layershell/)) — active but lower doc coverage (~29%); input-region support [unverified].
- **smithay-client-toolkit** — low-level `shell::wlr_layer` module; full control (`wl_surface.set_input_region`, buffer alpha) but you bring your own renderer [not re-fetched].
- All rely on compositor layer-shell → no GNOME.

## 11. Black screen on alt-tab / with overlay

- No report found specifically for PoE + niri/xwayland-satellite alt-tab black screen [searched; none found].
- Awakened PoE Trade on Wayland: 3.27.101 "full black screen covering PoE monitor"; fix = force X11 (`XDG_SESSION_TYPE=x11`) ([#1655](https://github.com/SnosMe/awakened-poe-trade/issues/1655)); PSA [#1647](https://github.com/SnosMe/awakened-poe-trade/issues/1647) (run via XWayland); transparent Electron overlay rendered with black background under XWayland ([#1854](https://github.com/SnosMe/awakened-poe-trade/issues/1854)); focus not returned to game on Hyprland ([#1752](https://github.com/SnosMe/awakened-poe-trade/issues/1752)); old multi-monitor black overlay ([#185](https://github.com/SnosMe/awakened-poe-trade/issues/185)). Cause pattern: a fullscreen transparent X11/Electron window losing alpha (compositing/ARGB visual) → black → a layer-shell overlay with real alpha and small input region avoids this [INFERENCE].
- Windows-side: "Alt tabbing causes black-screen for 3 seconds" in windowed fullscreen ([forum 3792214](https://www.pathofexile.com/forum/view-thread/3792214)).

# 24: Log in with a Chromium-family browser

**What to build:** The login from Firefox-family browsers also works when the player logs in with Chrome, Chromium, Brave, Edge, Vivaldi or Opera, including Flatpak installs. These browsers encrypt cookies; the Overlay decrypts the pathofexile.com ones with the key the browser keeps in the system keyring (GNOME Keyring or another Secret Service, and KWallet), or the fixed key used when no keyring is available. When more than one browser holds a login, the default browser's wins, then the newest. A locked keyring or a missing key shows in Settings as a reason, not as "not logged in".

**Blocked by:** 23 (Log in with a Firefox-family browser)

**Status:** ready-for-agent

- [ ] Logging in with Chrome under GNOME Keyring shows "Logged in" without further steps
- [ ] The same works for at least one other Chromium-family browser
- [ ] With logins in two browsers, the default browser's login is used
- [ ] A locked or missing keyring is named in Settings

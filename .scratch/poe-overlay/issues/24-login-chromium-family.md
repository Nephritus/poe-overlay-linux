# 24: Log in with a Chromium-family browser

**What to build:** The login from Firefox-family browsers also works when the player logs in with Chrome, Chromium, Brave, Edge, Vivaldi or Opera, including Flatpak installs. These browsers encrypt cookies; the Overlay decrypts the pathofexile.com ones with the key the browser keeps in the system keyring (GNOME Keyring or another Secret Service, and KWallet), or the fixed key used when no keyring is available. When more than one browser holds a login, the default browser's wins, then the newest. A locked keyring or a missing key shows in Settings as a reason, not as "not logged in".

**Blocked by:** 23 (Log in with a Firefox-family browser)

**Status:** ready-for-human

- [ ] Logging in with Chrome under GNOME Keyring shows "Logged in" without further steps (checked with a Chrome-format cookie file and the real "Chrome Safe Storage" key; no Chrome installed here)
- [ ] The same works for at least one other Chromium-family browser (human check)
- [x] With logins in two browsers, the default browser's login is used
- [x] A locked or missing keyring is named in Settings

## Comments

### Implemented (agent, 2026-11-17)

- **Browsers:** Chrome, Chromium (incl. Snap), Brave, Edge, Vivaldi, Opera, native and Flatpak; profiles are the folders with `Cookies` or `Network/Cookies`. User-Agent from the browser's `Last Version` (Chrome's reduced UA, `Chrome/156.0.0.0`; Edge adds `Edg/…`); Opera's and Vivaldi's versions aren't Chromium's, so their logins go with the Overlay's usual UA.
- **Decryption:** `v10` with the fixed key (PBKDF2-SHA1 of "peanuts"), `v11` with the keyring's password, then the empty password (Chromium's when the keyring gave none); AES-128-CBC, IV of spaces. Since schema 24 the value starts with SHA-256 of its host, which is checked and dropped. The keyring is asked only when a pathofexile.com cookie is `v11`, once per read. Keyring names as yt-dlp uses them on Linux: "Chrome", "Chromium", "Brave"; Edge and Opera "Chromium", Vivaldi "Chrome".
- **Keyring (`keyring::SystemKeyring`, gio D-Bus, no new crate):** Secret Service `SearchItems`, item found by its label "NAME Safe Storage"; found only among locked items → *locked*; a `plain` session reads the secret. Falls back to KWallet (kwalletd6, then 5) only if it's already running and the wallet open, so nothing prompts; unverified here (no KWallet set up). Unlock is never requested (user's choice): Settings names the reason, e.g. "Brave's cookies need the key "Brave Safe Storage", which isn't in your keyring."
- **New dependencies:** aes, cbc, pbkdf2 (hmac), sha1, sha2 (RustCrypto).
- **Tests:** `tests/browser_cookies.rs` +4 (v10, v11 and empty-password v11 decrypted with the host hash, an unencrypted value kept, another site's cookie never decrypted; schema 23 without hash and no keyring asked for `v10`; a wrong key is an error; locked and missing keyring named). The encrypted values were made with `openssl`, not the code under test. `tests/browser_profiles.rs` +2 (all six browsers' folders and both cookie places, default browser first; Chrome and Edge UA, none for Opera). Mutation-checked; one survivor: dropping the host-hash comparison (the padding and UTF-8 checks still reject the fixtures' wrong keys).
- **Checked live:**
  - A Chrome profile under a temporary home whose `POESESSID` was encrypted (`v11`) with the real "Chrome Safe Storage" password from GNOME Keyring: the Overlay's reader fetched the key over D-Bus and decrypted it (19 characters, right length), UA `Chrome/156.0.0.0`.
  - Same home plus a newer Firefox login: default browser Chrome → Chrome's login; default Firefox → Firefox's; neither default → the newest (Firefox).
  - A Brave profile whose key isn't in the keyring: Settings, after Log in, showed in red "Brave's cookies need the key "Brave Safe Storage", which isn't in your keyring." under the waiting line, and opened the login page.

**Needs a check (human):**

| Check | Pass when | Result |
|---|---|---|
| Chrome | Logged in on pathofexile.com in Chrome (GNOME Keyring), Log in shows "Logged in with Chrome" and searches work | |
| Another | The same with Brave, Chromium, Edge, Vivaldi or Opera | |
| Locked keyring | With the keyring locked, Settings names it instead of "Logged in" | |
| KWallet | On KDE with KWallet (no Secret Service), an open wallet gives the login | |

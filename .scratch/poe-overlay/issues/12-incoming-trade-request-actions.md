# 12: Trade Request actions for incoming requests

**What to build:** Incoming Trade Request cards get Invite, Trade, Thank, Kick and Reply "sold" buttons. Each click sends exactly one chat command to the game: save the clipboard, put the command on it, send Enter, Ctrl+V, Enter via uinput, then restore the clipboard. Pasting avoids keyboard-layout problems. One chat command per click and no automatic game actions, per GGG's one-action-per-input rule.

**Blocked by:** 07 (Price Check for poe.ninja-priced items), 11 (Incoming Trade Request cards)

**Status:** ready-for-human

- [x] Each button sends its single command for the card's buyer and nothing else
- [x] The user's previous clipboard content is restored afterwards
- [ ] Works with a non-US keyboard layout (V is looked up in the keymap like C; not tried with another layout)
- [x] No action is ever triggered by a log event

## Comments

### Implemented (agent, 2026-11-16, one commit with ticket 13)

- **Commands (`poe1::chat`):** Invite `/invite NAME`, Trade `/tradewith NAME`, Thank `@NAME Thanks, enjoy!`, Kick `/kick NAME`, Sold `@NAME Sorry, it's already sold.` (texts the user chose).
- **Sending:** keyboard checked, clipboard saved, line offered, Enter, Ctrl+V, wait up to 1.5 s for the game to read the clipboard (+150 ms for it to land in chat), Enter, clipboard put back, whatever failed in between. A paste the game never read leaves an empty chat line, which the last Enter closes, and the card says so. Ctrl is let go even when V fails. Keys and clipboard live on a chat thread of their own (its own virtual keyboard and clipboard connection), which afterwards answers pastes of the put-back clipboard until something else is copied, sleeping otherwise (woken through a pipe).
- **Clipboard:** every offered type is saved (X11 atoms other than the text ones skipped, text read once for all text types, 16 MiB and 1 s at most, duplicates skipped) and offered again as it was; an empty clipboard stays empty; the Price Check placeholder isn't saved. The placeholder, the chat line and the restored clipboard now go through one path that writes to readers without blocking (2 s at most per reader).
- **Keyboard:** the virtual keyboard learnt Enter and V (V found in the player's keymap like C); `Key`/`Keyboard` moved from `item_copy` to `poe1::keyboard`, shared by the copy and chat.
- **Focus:** a click while the player types in a Price Check box gives the keys back, waits 200 ms, then sends if the game is in front.
- **Bug found and fixed (all Panels):** the Overlay's input region was set after a frame was committed, so it reached the compositor only with the next one. A Panel shown last with nothing repainting afterwards (a single Trade Request card) let clicks pass through to the game. A changed region now queues one more frame.
- **Tests:** `tests/chat.rs` (6): each button's command; the full key and clipboard order; clipboard restored and Ctrl let go when V fails; nothing pasted → empty line closed and reported; restore failure reported; nothing pressed without clipboard or keyboard. Each fails when the code it covers is broken.
- **Checked live** (stand-in game whose terminal pastes the X11 clipboard on Ctrl+V, the same Xwayland path as the game): all five incoming buttons sent exactly `\r`, the command, `\r`; text and a PNG on the clipboard came back byte-identical, the PNG still served 10 s later; with the player typing in a Price Check box, Invite gave the keys back and sent its line, and the next key reached the game. A Price Check still copied the item after the clipboard rewrite.

**Needs a check (human):**

| Check | Pass when | Result |
|---|---|---|
| In game | Invite on a buyer's card invites them; your clipboard holds what it held before | |
| Chat already open | With chat open and text typed, a click doesn't send your half-typed text along (it may; tell me) | |
| Other layout | With an AZERTY or Dvorak layout, Invite still pastes (Ctrl+V found in the keymap) | |

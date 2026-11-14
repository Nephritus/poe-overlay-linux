# 12: Trade Request actions for incoming requests

**What to build:** Incoming Trade Request cards get Invite, Trade, Thank, Kick and Reply "sold" buttons. Each click sends exactly one chat command to the game: save the clipboard, put the command on it, send Enter, Ctrl+V, Enter via uinput, then restore the clipboard. Pasting avoids keyboard-layout problems. One chat command per click and no automatic game actions, per GGG's one-action-per-input rule.

**Blocked by:** 07 (Price Check for poe.ninja-priced items), 11 (Incoming Trade Request cards)

**Status:** ready-for-agent

- [ ] Each button sends its single command for the card's buyer and nothing else
- [ ] The user's previous clipboard content is restored afterwards
- [ ] Works with a non-US keyboard layout
- [ ] No action is ever triggered by a log event

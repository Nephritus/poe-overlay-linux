# PoE Overlay

An overlay for Path of Exile 1 on Wayland that helps the player trade and level a character through the campaign.

## Game world

**Area**:
A single game location the character can be in, identified by the game's area ID (e.g. `1_1_2`); several Areas can share a display name.
_Avoid_: Zone, map, level

**Character**:
One of the player's in-game characters, identified by its name; Route progress belongs to a Character.
_Avoid_: Player, account, toon

## Overlay

**Overlay**:
The always-on-top, see-through layer over the game window that holds Panels; it never takes keyboard focus and shows only while the game is focused.
_Avoid_: Window, HUD, companion (the program as a whole)

**Panel**:
One box of content in the Overlay, shown or hidden on its own; only shown Panels catch clicks.
_Avoid_: Widget, popup, card (reserved for Trade Request cards inside a Panel)

**Action**:
A request the running Overlay carries out, such as toggling the Currency Rates Panel; sent by a Hotkey or by `poe-overlay trigger`.
_Avoid_: Command, trigger (the CLI verb that sends it), event

**Hotkey**:
A key combination, chosen in `[hotkeys]`, that the overlay grabs in Xwayland while the game is focused and turns into an Action.
_Avoid_: Shortcut, keybind (the compositor's own entry), macro

**Settings window**:
The normal window `poe-overlay settings` opens, where the player types and chooses settings; separate from the Overlay, which never takes keyboard focus.
_Avoid_: Preferences, options dialog, config editor

## Leveling

**Route**:
The ordered plan for taking a character through the campaign, made of Steps.
_Avoid_: Quest direction, guide, walkthrough

**Step**:
One instruction in a Route, tied to the Area where it is performed.
_Avoid_: Task, objective, quest

**Leg**:
The Steps of a Route done during one visit to an Area, ending with the Step that leaves it; a Route visits some Areas several times, each a separate Leg.
_Avoid_: Segment, stage, group

**Sub-step**:
A hint attached to a Step (direction, layout tip, recommended level), not an instruction of its own.
_Avoid_: Note, tip, substep

## Trading

**Price Check**:
Finding out what the item the player is hovering in game is worth: its poe.ninja price, or current market listings for items poe.ninja doesn't price.
_Avoid_: Price lookup, appraisal

**Trade Request**:
A trade-site whisper asking to buy a listed item; **incoming** when a buyer messages the player, **outgoing** when the player messages a seller.
_Avoid_: Whisper (any private chat message, not just trades), offer

**Currency Rate**:
The current exchange ratio between two currency items.
_Avoid_: Price (reserved for item listings), conversion

**Modifier**:
One line of an item's properties that rolls (`+73 to maximum Life`), by source: implicit, explicit, crafted, fractured or enchant; searched on the trade site as a stat.
_Avoid_: Affix (only explicit prefixes and suffixes), stat (the trade site's id for a modifier's text)

**Listing**:
An item put up for sale on the trade site, with its price, when it was listed and its seller; Price Check shows the cheapest of those like the checked item.
_Avoid_: Offer (reserved for trade), post, ad

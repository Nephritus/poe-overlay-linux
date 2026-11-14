# PoE Overlay

An overlay for Path of Exile 1 on Wayland that helps the player trade and level a character through the campaign.

## Game world

**Area**:
A single game location the character can be in, identified by the game's area ID (e.g. `1_1_2`); several Areas can share a display name.
_Avoid_: Zone, map, level

**Character**:
One of the player's in-game characters, identified by its name; Route progress belongs to a Character.
_Avoid_: Player, account, toon

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
Looking up current market listings for a specific item the player is hovering in game.
_Avoid_: Price lookup, appraisal

**Trade Request**:
A trade-site whisper asking to buy a listed item; **incoming** when a buyer messages the player, **outgoing** when the player messages a seller.
_Avoid_: Whisper (any private chat message, not just trades), offer

**Currency Rate**:
The current exchange ratio between two currency items.
_Avoid_: Price (reserved for item listings), conversion

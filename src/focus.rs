//! Game-focus tracking: decides whether the overlay is shown, and where, from
//! the compositor's toplevel window events.

use std::collections::HashMap;

use crate::poe1;

/// Latest known state of one toplevel window.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Toplevel {
    pub app_id: String,
    pub title: String,
    pub activated: bool,
    /// Names of the outputs the window is on (e.g. `DP-2`), the one it's
    /// shown on first: the first entered, or the one holding most of it where
    /// the compositor gives a position instead ([`outputs_under`]).
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToplevelEvent {
    Updated { id: u32, toplevel: Toplevel },
    Closed { id: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visibility {
    Hidden,
    /// Shown on the named output; `None` lets the compositor pick.
    Shown { output: Option<String> },
    /// The game is open but no window is focused: a layer surface holds the
    /// keyboard, such as the Overlay while the player types in a Panel.
    NoWindowFocused,
}

#[derive(Debug, Default)]
pub struct GameFocus {
    toplevels: HashMap<u32, Toplevel>,
}

impl GameFocus {
    pub fn apply(&mut self, event: ToplevelEvent) {
        match event {
            ToplevelEvent::Updated { id, toplevel } => {
                self.toplevels.insert(id, toplevel);
            }
            ToplevelEvent::Closed { id } => {
                self.toplevels.remove(&id);
            }
        }
    }

    pub fn visibility(&self) -> Visibility {
        let mut games = self.toplevels.values().filter(|t| poe1::is_game_window(&t.app_id, &t.title)).peekable();
        if games.peek().is_none() {
            return Visibility::Hidden;
        }
        if let Some(game) = games.find(|t| t.activated) {
            Visibility::Shown { output: game.outputs.first().cloned() }
        } else if self.toplevels.values().any(|t| t.activated) {
            Visibility::Hidden
        } else {
            Visibility::NoWindowFocused
        }
    }
}

/// An area in the compositor's logical coordinates, which span all outputs.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    /// The area `self` and `other` share, in square units.
    fn overlap(self, other: Rect) -> i64 {
        let span = |start: i32, len: i32, other_start: i32, other_len: i32| {
            let end = (i64::from(start) + i64::from(len)).min(i64::from(other_start) + i64::from(other_len));
            (end - i64::from(start.max(other_start))).max(0)
        };
        span(self.x, self.width, other.x, other.width) * span(self.y, self.height, other.y, other.height)
    }
}

/// Names of the outputs `window` lies on, the one holding most of it first,
/// for compositors that give a window's position instead of its outputs.
pub fn outputs_under<'a>(window: Rect, outputs: impl IntoIterator<Item = (&'a str, Rect)>) -> Vec<String> {
    let mut under: Vec<(i64, &str)> = outputs
        .into_iter()
        .map(|(name, area)| (window.overlap(area), name))
        .filter(|&(overlap, _)| overlap > 0)
        .collect();
    under.sort_by_key(|&(overlap, _)| std::cmp::Reverse(overlap));
    under.into_iter().map(|(_, name)| name.to_owned()).collect()
}

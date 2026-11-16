//! Game-focus tracking: decides whether the overlay is shown, and where, from
//! the compositor's toplevel window events.

use std::collections::HashMap;

use crate::poe1;

/// Latest known state of one toplevel window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toplevel {
    pub app_id: String,
    pub title: String,
    pub activated: bool,
    /// Names of the outputs the window is on (e.g. `DP-2`), in the order entered.
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

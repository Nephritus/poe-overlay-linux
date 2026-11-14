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
        self.toplevels
            .values()
            .find(|t| t.activated && poe1::is_game_window(&t.app_id, &t.title))
            .map_or(Visibility::Hidden, |game| Visibility::Shown {
                output: game.outputs.first().cloned(),
            })
    }
}

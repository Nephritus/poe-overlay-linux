//! KDE Plasma's window list (`org_kde_plasma_window_management`, KWin has no
//! wlr-foreign-toplevel-management) turned into [`ToplevelEvent`]s. KWin
//! gives a window's position, not its outputs, and sends its properties one
//! event at a time, with no "done" after each change.

use std::collections::HashMap;

use crate::focus::{Rect, Toplevel, ToplevelEvent, outputs_under};

/// `org_kde_plasma_window_management.state.active`.
const ACTIVE: u32 = 0x1;

/// One event about a Plasma window, as KWin sends it.
#[derive(Debug)]
pub enum PlasmaEvent {
    Title(String),
    AppId(String),
    /// The window's state flags.
    State(u32),
    /// Where the window is, in the compositor's logical coordinates.
    Geometry(Rect),
    /// Every property has been sent once.
    InitialState,
    /// The window is gone.
    Unmapped,
}

#[derive(Debug, Default)]
struct PlasmaWindow {
    /// As last reported, or to be reported at its initial state.
    toplevel: Toplevel,
    /// Its initial state is complete, so it's reported.
    known: bool,
}

#[derive(Debug, Default)]
pub struct PlasmaWindows {
    windows: HashMap<u32, PlasmaWindow>,
}

impl PlasmaWindows {
    /// Applies an event about window `id`. Returns the update to report: the
    /// window once its initial state is complete, then after each change of
    /// what decides where the Overlay shows (KWin also sends the geometry of
    /// every window being dragged, and state flags the Overlay ignores);
    /// `outputs` are the outputs' names and areas.
    pub fn apply<'a>(&mut self, id: u32, event: PlasmaEvent, outputs: impl IntoIterator<Item = (&'a str, Rect)>) -> Option<ToplevelEvent> {
        let window = self.windows.entry(id).or_default();
        let toplevel = &mut window.toplevel;
        let changed = match event {
            PlasmaEvent::Title(title) => replace(&mut toplevel.title, title),
            PlasmaEvent::AppId(app_id) => replace(&mut toplevel.app_id, app_id),
            PlasmaEvent::State(flags) => replace(&mut toplevel.activated, flags & ACTIVE != 0),
            PlasmaEvent::Geometry(area) => replace(&mut toplevel.outputs, outputs_under(area, outputs)),
            PlasmaEvent::InitialState => {
                window.known = true;
                true
            }
            PlasmaEvent::Unmapped => {
                self.windows.remove(&id);
                return Some(ToplevelEvent::Closed { id });
            }
        };
        (window.known && changed).then(|| ToplevelEvent::Updated { id, toplevel: window.toplevel.clone() })
    }
}

/// Sets `field` to `value`; returns whether that changed it.
fn replace<T: PartialEq>(field: &mut T, value: T) -> bool {
    let changed = *field != value;
    *field = value;
    changed
}

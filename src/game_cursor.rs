//! Where the pointer is over the game, asked from the game's X server
//! (Xwayland): a Wayland client can't see the pointer over other windows.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, Window};
use x11rb::rust_connection::RustConnection;

/// Connects on first use and again after a failure, so a game started
/// after the overlay (or a restarted Xwayland) is still found.
#[derive(Default)]
pub struct GameCursor {
    x: Option<(RustConnection, usize)>,
}

impl GameCursor {
    /// Whether the pointer is on the left half of the window under it (the
    /// game, when it is pressed in game); `None` without an X server (a
    /// native Wayland game) or on errors.
    pub fn on_left_half(&mut self) -> Option<bool> {
        if self.x.is_none() {
            self.x = x11rb::connect(None).ok();
        }
        let (conn, screen) = self.x.as_ref()?;
        let root = conn.setup().roots.get(*screen)?;
        let answer = left_half(conn, root.root, root.width_in_pixels);
        if answer.is_none() {
            self.x = None;
        }
        answer
    }
}

/// The root window spans every output, so the pointer is measured against
/// the top-level window it is over; against the root if there is none.
fn left_half(conn: &RustConnection, root: Window, root_width: u16) -> Option<bool> {
    let pointer = conn.query_pointer(root).ok()?.reply().ok()?;
    let x = i32::from(pointer.root_x);
    if pointer.child == x11rb::NONE {
        return Some(x < i32::from(root_width) / 2);
    }
    let geometry = conn.get_geometry(pointer.child).ok()?.reply().ok()?;
    let origin = conn.translate_coordinates(pointer.child, root, 0, 0).ok()?.reply().ok()?;
    Some(x < i32::from(origin.dst_x) + i32::from(geometry.width) / 2)
}

//! The compositor's clipboard without keyboard focus, through the
//! ext-data-control protocol on its own Wayland connection.
//!
//! Text a game copies under Xwayland reaches it as a new selection (the X11
//! bridge offers it); a read asks the selection's owner to write the text
//! into a pipe. What this connection offers itself (the Price Check
//! placeholder, a chat line, the player's clipboard put back) it hands over
//! when asked, as long as its events are dispatched.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::rc::Rc;
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use wayland_client::backend::ObjectId;
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, event_created_child};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1 as device, ext_data_control_manager_v1 as manager, ext_data_control_offer_v1 as offer,
    ext_data_control_source_v1 as source,
};

use crate::poe1::chat::ChatClipboard;
use crate::poe1::item_copy::Clipboard;

/// Text types, best first.
const TEXT_TYPES: [&str; 5] = ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain", "TEXT", "STRING"];

/// Not empty: some clipboard managers put the previous text back when the
/// clipboard is emptied, which would read as a copy.
const PLACEHOLDER: &[u8] = b"poe-overlay";

/// Offered along with the placeholder's text, so it is recognised without
/// reading it, also when a clipboard manager offers it again as its own.
const PLACEHOLDER_TYPE: &str = "application/x-poe-overlay-placeholder";

/// Item texts are a few KiB; anything far larger isn't one.
const MAX_TEXT: u64 = 1 << 20;

/// The most of the player's clipboard kept while a chat line is sent.
const MAX_SAVED: u64 = 16 << 20;
/// How long the clipboard's owner gets to hand over all of it.
const SAVE_WAIT: Duration = Duration::from_secs(1);
/// How long a reader of what this connection offers gets.
const SERVE_WAIT: Duration = Duration::from_secs(2);
/// After the game reads a pasted line, until its chat shows it.
const PASTE_SETTLE: Duration = Duration::from_millis(150);

/// Offered with a chat line: clipboard managers that honour it (Klipper,
/// cliphist's sensitive filter) keep it out of their history.
const SECRET_HINT_TYPE: &str = "x-kde-passwordManagerHint";
/// How soon clipboard managers read what's new.
const MANAGERS_READ_WITHIN: Duration = Duration::from_millis(100);

/// Content by type, the same bytes shared between types.
type Contents = Vec<(Cow<'static, str>, Rc<[u8]>)>;

pub struct DataControlClipboard {
    conn: Connection,
    queue: EventQueue<State>,
    state: State,
    /// `state.pastes` when the chat line was put on the clipboard.
    pastes_before: u64,
}

#[derive(Default)]
struct State {
    seat: Option<wl_seat::WlSeat>,
    manager: Option<manager::ExtDataControlManagerV1>,
    device: Option<device::ExtDataControlDeviceV1>,
    /// Types of offers not yet replaced.
    offers: HashMap<ObjectId, Vec<String>>,
    selection: Option<offer::ExtDataControlOfferV1>,
    /// Counts selection changes.
    changes: u64,
    /// Our source while it's the clipboard, and what it offers.
    source: Option<source::ExtDataControlSourceV1>,
    serving: Contents,
    /// Counts reads of our source.
    pastes: u64,
    finished: bool,
}

impl DataControlClipboard {
    pub fn connect() -> Result<DataControlClipboard, String> {
        let conn = Connection::connect_to_env().map_err(|err| format!("no Wayland connection: {err}"))?;
        let mut queue = conn.new_event_queue();
        let qh = queue.handle();
        conn.display().get_registry(&qh, ());
        let mut state = State::default();
        queue.roundtrip(&mut state).map_err(|err| err.to_string())?;
        let (Some(seat), Some(manager)) = (&state.seat, &state.manager) else {
            return Err("the compositor lacks the ext-data-control clipboard protocol".into());
        };
        state.device = Some(manager.get_data_device(seat, &qh, ()));
        // Takes in the current selection.
        queue.roundtrip(&mut state).map_err(|err| err.to_string())?;
        Ok(DataControlClipboard { conn, queue, state, pastes_before: 0 })
    }

    /// Handles events until `deadline`, returning early once any came.
    fn dispatch_until(&mut self, deadline: Instant) -> Result<(), String> {
        let failed = |err: &dyn std::fmt::Display| format!("clipboard connection: {err}");
        if self.queue.dispatch_pending(&mut self.state).map_err(|e| failed(&e))? > 0 {
            return Ok(());
        }
        self.conn.flush().map_err(|e| failed(&e))?;
        let Some(guard) = self.queue.prepare_read() else { return Ok(()) };
        let left = deadline.saturating_duration_since(Instant::now());
        let readable = {
            let fd = guard.connection_fd();
            let mut fds = [PollFd::new(&fd, PollFlags::IN)];
            poll(&mut fds, Some(&timespec(left))).map_err(|e| failed(&e))? > 0
        };
        if readable {
            guard.read().map_err(|e| failed(&e))?;
        }
        self.queue.dispatch_pending(&mut self.state).map_err(|e| failed(&e))?;
        Ok(())
    }

    /// Whether something this connection put there is the clipboard.
    pub fn owns_selection(&self) -> bool {
        self.state.source.is_some()
    }

    /// `slot`'s connection, or a new one: after a failure (a compositor
    /// restart) the next use connects again.
    pub fn reuse(slot: &mut Option<DataControlClipboard>) -> Result<DataControlClipboard, String> {
        match slot.take() {
            Some(connected) => Ok(connected),
            None => DataControlClipboard::connect(),
        }
    }

    /// Answers reads of what this connection offers until `wake` is readable
    /// or something else is copied; returns at once when offering nothing.
    pub fn serve(&mut self, wake: BorrowedFd) -> Result<(), String> {
        let failed = |err: &dyn std::fmt::Display| format!("clipboard connection: {err}");
        while self.state.source.is_some() {
            self.queue.dispatch_pending(&mut self.state).map_err(|e| failed(&e))?;
            self.conn.flush().map_err(|e| failed(&e))?;
            let Some(guard) = self.queue.prepare_read() else { continue };
            let (readable, woken) = {
                let fd = guard.connection_fd();
                let mut fds = [PollFd::new(&fd, PollFlags::IN), PollFd::new(&wake, PollFlags::IN)];
                poll(&mut fds, None).map_err(|e| failed(&e))?;
                (!fds[0].revents().is_empty(), !fds[1].revents().is_empty())
            };
            if readable {
                guard.read().map_err(|e| failed(&e))?;
            }
            self.queue.dispatch_pending(&mut self.state).map_err(|e| failed(&e))?;
            if woken {
                return Ok(());
            }
        }
        Ok(())
    }

    /// Asks the selection's owner for `mime` and reads it from a pipe.
    fn receive(&mut self, offer: &offer::ExtDataControlOfferV1, mime: &str, deadline: Instant, max: u64) -> Result<Vec<u8>, String> {
        let (read, write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).map_err(|err| format!("pipe: {err}"))?;
        offer.receive(mime.to_owned(), write.as_fd());
        self.conn.flush().map_err(|err| format!("clipboard connection: {err}"))?;
        drop(write);
        read_all(read, deadline, max)
    }

    /// Offers `contents` as the clipboard, or empties it.
    fn set_selection(&mut self, contents: Contents) -> Result<(), String> {
        let qh = self.queue.handle();
        let (Some(manager), Some(device)) = (&self.state.manager, &self.state.device) else {
            return Err("no clipboard device".into());
        };
        let source = (!contents.is_empty()).then(|| manager.create_data_source(&qh, ()));
        if let Some(source) = &source {
            for (mime, _) in &contents {
                source.offer(mime.to_string());
            }
        }
        device.set_selection(source.as_ref());
        if let Some(old) = std::mem::replace(&mut self.state.source, source) {
            old.destroy();
        }
        self.state.serving = contents;
        // In place before anything else happens: the game's copy would
        // otherwise be replaced, or its paste read the old clipboard.
        self.queue.roundtrip(&mut self.state).map_err(|err| format!("clipboard connection: {err}"))?;
        Ok(())
    }
}

impl Clipboard for DataControlClipboard {
    fn put_placeholder(&mut self) -> Result<(), String> {
        let text: Rc<[u8]> = Rc::from(PLACEHOLDER);
        self.set_selection(TEXT_TYPES.into_iter().chain([PLACEHOLDER_TYPE]).map(|mime| (Cow::Borrowed(mime), text.clone())).collect())
    }

    fn read_next(&mut self, wait: Duration) -> Result<Option<String>, String> {
        let deadline = Instant::now() + wait;
        let mut seen = self.state.changes;
        loop {
            if self.state.finished {
                return Err("the compositor ended clipboard access".into());
            }
            if self.state.changes != seen
                && let Some(offer) = self.state.selection.clone()
            {
                let types = self.state.offers.get(&offer.id()).map(Vec::as_slice).unwrap_or_default();
                if !types.iter().any(|t| t == PLACEHOLDER_TYPE) {
                    let Some(mime) = TEXT_TYPES.into_iter().find(|t| types.iter().any(|offered| offered == t)) else {
                        return Ok(None);
                    };
                    let bytes = self.receive(&offer, mime, deadline, MAX_TEXT)?;
                    return Ok(Some(String::from_utf8_lossy(&bytes).into_owned()));
                }
                seen = self.state.changes;
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            self.dispatch_until(deadline)?;
        }
    }
}

/// What the player had on the clipboard, by type.
pub struct SavedClipboard(Contents);

impl ChatClipboard for DataControlClipboard {
    type Saved = SavedClipboard;

    fn save(&mut self) -> Result<SavedClipboard, String> {
        // Up to date with what was copied since.
        self.queue.roundtrip(&mut self.state).map_err(|err| format!("clipboard connection: {err}"))?;
        if self.state.source.is_some() {
            return Ok(SavedClipboard(self.state.serving.clone()));
        }
        let Some(offer) = self.state.selection.clone() else { return Ok(SavedClipboard(Vec::new())) };
        let mut types = self.state.offers.get(&offer.id()).cloned().unwrap_or_default();
        // The Price Check placeholder isn't the player's.
        if types.iter().any(|mime| mime == PLACEHOLDER_TYPE) {
            return Ok(SavedClipboard(Vec::new()));
        }
        // Text first: a slow image mustn't use up the time it gets.
        types.sort_by_key(|mime| !TEXT_TYPES.contains(&mime.as_str()));
        let deadline = Instant::now() + SAVE_WAIT;
        let mut saved: Contents = Vec::with_capacity(types.len());
        let mut room = MAX_SAVED;
        // The text types hold the same text: read once.
        let mut text: Option<Rc<[u8]>> = None;
        for mime in types {
            // Some programs offer a type twice.
            if saved.iter().any(|(saved, _)| *saved == mime) {
                continue;
            }
            let is_text = TEXT_TYPES.contains(&mime.as_str());
            if let (true, Some(text)) = (is_text, &text) {
                saved.push((Cow::Owned(mime), text.clone()));
                continue;
            }
            // X11 atoms other than text (TARGETS…) aren't content.
            if !is_text && !mime.contains('/') {
                continue;
            }
            // A type its owner doesn't hand over in time is left out.
            let Ok(bytes) = self.receive(&offer, &mime, deadline, room) else { continue };
            room -= bytes.len() as u64;
            let bytes: Rc<[u8]> = Rc::from(bytes);
            if is_text {
                text = Some(bytes.clone());
            }
            saved.push((Cow::Owned(mime), bytes));
        }
        Ok(SavedClipboard(saved))
    }

    fn put(&mut self, text: &str) -> Result<(), String> {
        let text: Rc<[u8]> = Rc::from(text.as_bytes());
        let hint = (Cow::Borrowed(SECRET_HINT_TYPE), Rc::from(&b"secret"[..]));
        self.set_selection(TEXT_TYPES.into_iter().map(|mime| (Cow::Borrowed(mime), text.clone())).chain([hint]).collect())?;
        // A clipboard manager reads what's new at once; only reads after
        // that are the game's paste.
        let settled = Instant::now() + MANAGERS_READ_WITHIN;
        while Instant::now() < settled {
            self.dispatch_until(settled)?;
        }
        self.pastes_before = self.state.pastes;
        Ok(())
    }

    fn wait_pasted(&mut self, wait: Duration) -> Result<bool, String> {
        let deadline = Instant::now() + wait;
        while self.state.pastes == self.pastes_before {
            if Instant::now() >= deadline {
                return Ok(false);
            }
            self.dispatch_until(deadline)?;
        }
        let settled = Instant::now() + PASTE_SETTLE;
        while Instant::now() < settled {
            self.dispatch_until(settled)?;
        }
        Ok(true)
    }

    fn restore(&mut self, saved: SavedClipboard) -> Result<(), String> {
        self.set_selection(saved.0)
    }
}

/// Reads until the writer closes the pipe, or `deadline`; at most `max`
/// bytes.
fn read_all(read: OwnedFd, deadline: Instant, max: u64) -> Result<Vec<u8>, String> {
    let mut file = std::fs::File::from(read);
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let mut fds = [PollFd::new(&file, PollFlags::IN)];
        if poll(&mut fds, Some(&timespec(left))).map_err(|err| format!("clipboard read: {err}"))? == 0 {
            return Err("the clipboard's owner didn't hand over its content in time".into());
        }
        match file.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) if (bytes.len() + n) as u64 <= max => bytes.extend_from_slice(&chunk[..n]),
            Ok(_) => return Err("the clipboard holds too much".into()),
            Err(err) if err.kind() == ErrorKind::Interrupted => {}
            Err(err) => return Err(format!("clipboard read: {err}")),
        }
    }
    Ok(bytes)
}

/// Writes `bytes` into a reader's pipe, giving up at `deadline`: a reader
/// that never reads mustn't hold up the overlay.
fn write_all(fd: OwnedFd, bytes: &[u8], deadline: Instant) -> Result<(), String> {
    rustix::fs::fcntl_setfl(&fd, rustix::fs::OFlags::NONBLOCK).map_err(|err| err.to_string())?;
    let mut file = std::fs::File::from(fd);
    let mut left = bytes;
    while !left.is_empty() {
        match file.write(left) {
            Ok(written) => left = &left[written..],
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                let wait = deadline.saturating_duration_since(Instant::now());
                let mut fds = [PollFd::new(&file, PollFlags::OUT)];
                if poll(&mut fds, Some(&timespec(wait))).map_err(|err| err.to_string())? == 0 {
                    return Err("the reader took too long".into());
                }
            }
            Err(err) if err.kind() == ErrorKind::Interrupted => {}
            Err(err) => return Err(err.to_string()),
        }
    }
    Ok(())
}

fn timespec(duration: Duration) -> Timespec {
    Timespec { tv_sec: duration.as_secs() as _, tv_nsec: duration.subsec_nanos() as _ }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "wl_seat" if state.seat.is_none() => state.seat = Some(registry.bind(name, version.min(1), qh, ())),
                "ext_data_control_manager_v1" => state.manager = Some(registry.bind(name, 1, qh, ())),
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(_: &mut Self, _: &wl_seat::WlSeat, _: wl_seat::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<manager::ExtDataControlManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &manager::ExtDataControlManagerV1,
        _: manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<device::ExtDataControlDeviceV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &device::ExtDataControlDeviceV1,
        event: device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            device::Event::DataOffer { id } => {
                state.offers.insert(id.id(), Vec::new());
            }
            device::Event::Selection { id } => {
                if let Some(old) = state.selection.take() {
                    state.offers.remove(&old.id());
                    old.destroy();
                }
                state.selection = id;
                state.changes += 1;
            }
            // Not the clipboard: let its offer go.
            device::Event::PrimarySelection { id: Some(offer) } => {
                state.offers.remove(&offer.id());
                offer.destroy();
            }
            device::Event::Finished => state.finished = true,
            _ => {}
        }
    }

    event_created_child!(State, device::ExtDataControlDeviceV1, [
        device::EVT_DATA_OFFER_OPCODE => (offer::ExtDataControlOfferV1, ()),
    ]);
}

impl Dispatch<offer::ExtDataControlOfferV1, ()> for State {
    fn event(
        state: &mut Self,
        offer: &offer::ExtDataControlOfferV1,
        event: offer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let offer::Event::Offer { mime_type } = event
            && let Some(types) = state.offers.get_mut(&offer.id())
        {
            types.push(mime_type);
        }
    }
}

impl Dispatch<source::ExtDataControlSourceV1, ()> for State {
    fn event(
        state: &mut Self,
        source: &source::ExtDataControlSourceV1,
        event: source::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let ours = state.source.as_ref() == Some(source);
        match event {
            source::Event::Send { fd, mime_type } if ours => {
                // Only text reads can be a paste (managers also read the hint).
                if TEXT_TYPES.contains(&mime_type.as_str()) {
                    state.pastes += 1;
                }
                if let Some((_, bytes)) = state.serving.iter().find(|(mime, _)| *mime == mime_type) {
                    // A reader that went away mustn't stop the overlay.
                    let _ = write_all(fd, bytes, Instant::now() + SERVE_WAIT);
                }
            }
            source::Event::Cancelled => {
                source.destroy();
                if ours {
                    state.source = None;
                    state.serving.clear();
                }
            }
            _ => {}
        }
    }
}

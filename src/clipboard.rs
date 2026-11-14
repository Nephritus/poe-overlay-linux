//! The compositor's clipboard without keyboard focus, through the
//! ext-data-control protocol on its own Wayland connection.
//!
//! Text a game copies under Xwayland reaches it as a new selection (the X11
//! bridge offers it); a read asks the selection's owner to write the text
//! into a pipe.

use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use wayland_client::backend::ObjectId;
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, event_created_child};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1 as device, ext_data_control_manager_v1 as manager, ext_data_control_offer_v1 as offer,
    ext_data_control_source_v1 as source,
};

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

pub struct DataControlClipboard {
    conn: Connection,
    queue: EventQueue<State>,
    state: State,
}

#[derive(Default)]
struct State {
    seat: Option<wl_seat::WlSeat>,
    manager: Option<manager::ExtDataControlManagerV1>,
    device: Option<device::ExtDataControlDeviceV1>,
    /// Text types of offers not yet replaced.
    offers: HashMap<ObjectId, Vec<String>>,
    selection: Option<offer::ExtDataControlOfferV1>,
    /// Counts selection changes.
    changes: u64,
    /// Our placeholder while it's offered.
    source: Option<source::ExtDataControlSourceV1>,
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
        Ok(DataControlClipboard { conn, queue, state })
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

    /// Asks the selection's owner for its text and reads it from a pipe.
    fn receive(&mut self, offer: &offer::ExtDataControlOfferV1, deadline: Instant) -> Result<Option<String>, String> {
        let types = self.state.offers.get(&offer.id()).map(Vec::as_slice).unwrap_or_default();
        let Some(mime) = TEXT_TYPES.into_iter().find(|t| types.iter().any(|offered| offered == t)) else {
            return Ok(None);
        };
        let (read, write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).map_err(|err| format!("pipe: {err}"))?;
        offer.receive(mime.to_owned(), write.as_fd());
        self.conn.flush().map_err(|err| format!("clipboard connection: {err}"))?;
        drop(write);
        read_all(read, deadline).map(Some)
    }
}

impl Clipboard for DataControlClipboard {
    fn put_placeholder(&mut self) -> Result<(), String> {
        let qh = self.queue.handle();
        let (Some(manager), Some(device)) = (&self.state.manager, &self.state.device) else {
            return Err("no clipboard device".into());
        };
        let source = manager.create_data_source(&qh, ());
        for mime in TEXT_TYPES.into_iter().chain([PLACEHOLDER_TYPE]) {
            source.offer(mime.to_owned());
        }
        device.set_selection(Some(&source));
        if let Some(old) = self.state.source.replace(source) {
            old.destroy();
        }
        // The placeholder must be in place before the game copies, or it
        // would replace the copy.
        self.queue.roundtrip(&mut self.state).map_err(|err| format!("clipboard connection: {err}"))?;
        Ok(())
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
                let types = self.state.offers.get(&offer.id());
                if !types.is_some_and(|types| types.iter().any(|t| t == PLACEHOLDER_TYPE)) {
                    return self.receive(&offer, deadline);
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

/// Reads until the writer closes the pipe, or `deadline`.
fn read_all(read: OwnedFd, deadline: Instant) -> Result<String, String> {
    let mut file = std::fs::File::from(read);
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let mut fds = [PollFd::new(&file, PollFlags::IN)];
        if poll(&mut fds, Some(&timespec(left))).map_err(|err| format!("clipboard read: {err}"))? == 0 {
            return Err("the game didn't hand over the copied text in time".into());
        }
        match file.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) if (bytes.len() + n) as u64 <= MAX_TEXT => bytes.extend_from_slice(&chunk[..n]),
            Ok(_) => return Err("the clipboard holds too much text for an item".into()),
            Err(err) if err.kind() == ErrorKind::Interrupted => {}
            Err(err) => return Err(format!("clipboard read: {err}")),
        }
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
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
        match event {
            source::Event::Send { fd, .. } => {
                // A reader that went away mustn't stop the overlay.
                let _ = std::fs::File::from(fd).write_all(PLACEHOLDER);
            }
            source::Event::Cancelled => {
                source.destroy();
                if state.source.as_ref() == Some(source) {
                    state.source = None;
                }
            }
            _ => {}
        }
    }
}

//! Feeds the compositor's toplevel windows into [`GameFocus`] over the
//! wlr-foreign-toplevel-management protocol, on its own Wayland connection.

use std::collections::HashMap;

use wayland_client::backend::ObjectId;
use wayland_client::protocol::{wl_output, wl_registry};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1 as handle, zwlr_foreign_toplevel_manager_v1 as manager,
};

use crate::focus::{GameFocus, Toplevel, ToplevelEvent, Visibility};

/// Spawns the tracking thread. The receiver yields each change of where the
/// overlay should be; it closes if tracking is impossible or fails.
pub fn watch() -> async_channel::Receiver<Visibility> {
    let (tx, rx) = async_channel::unbounded();
    std::thread::spawn(move || {
        if let Err(err) = run(tx) {
            eprintln!("poe-overlay: window tracking stopped, overlay stays hidden: {err}");
        }
    });
    rx
}

#[derive(Default)]
struct Pending {
    app_id: String,
    title: String,
    activated: bool,
    outputs: Vec<ObjectId>,
}

struct State {
    output_names: HashMap<ObjectId, String>,
    toplevels: HashMap<ObjectId, Pending>,
    manager: Option<manager::ZwlrForeignToplevelManagerV1>,
    focus: GameFocus,
    shown: Visibility,
    tx: async_channel::Sender<Visibility>,
}

impl State {
    fn apply(&mut self, event: ToplevelEvent) {
        self.focus.apply(event);
        let now = self.focus.visibility();
        if now != self.shown {
            self.shown = now.clone();
            let _ = self.tx.send_blocking(now);
        }
    }
}

fn run(tx: async_channel::Sender<Visibility>) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    conn.display().get_registry(&qh, ());
    let mut state = State {
        output_names: HashMap::new(),
        toplevels: HashMap::new(),
        manager: None,
        focus: GameFocus::default(),
        shown: Visibility::Hidden,
        tx,
    };
    queue.roundtrip(&mut state)?;
    if state.manager.is_none() {
        return Err("compositor lacks zwlr_foreign_toplevel_manager_v1".into());
    }
    loop {
        queue.blocking_dispatch(&mut state)?;
    }
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
                // v4 adds the output name (e.g. `DP-2`) that GDK monitors share.
                "wl_output" if version >= 4 => {
                    registry.bind::<wl_output::WlOutput, _, _>(name, 4, qh, ());
                }
                "zwlr_foreign_toplevel_manager_v1" => {
                    state.manager = Some(registry.bind(name, version.min(3), qh, ()));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for State {
    fn event(
        state: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event {
            state.output_names.insert(output.id(), name);
        }
    }
}

impl Dispatch<manager::ZwlrForeignToplevelManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &manager::ZwlrForeignToplevelManagerV1,
        event: manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let manager::Event::Toplevel { toplevel } = event {
            state.toplevels.insert(toplevel.id(), Pending::default());
        }
    }

    event_created_child!(State, manager::ZwlrForeignToplevelManagerV1, [
        manager::EVT_TOPLEVEL_OPCODE => (handle::ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<handle::ZwlrForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        toplevel: &handle::ZwlrForeignToplevelHandleV1,
        event: handle::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let key = toplevel.id();
        let id = key.protocol_id();
        if let handle::Event::Closed = event {
            state.toplevels.remove(&key);
            toplevel.destroy();
            state.apply(ToplevelEvent::Closed { id });
            return;
        }
        let Some(pending) = state.toplevels.get_mut(&key) else { return };
        match event {
            handle::Event::Title { title } => pending.title = title,
            handle::Event::AppId { app_id } => pending.app_id = app_id,
            handle::Event::OutputEnter { output } => pending.outputs.push(output.id()),
            handle::Event::OutputLeave { output } => pending.outputs.retain(|o| *o != output.id()),
            handle::Event::State { state: states } => {
                pending.activated = states
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|s| u32::from_ne_bytes(*s) == handle::State::Activated as u32);
            }
            handle::Event::Done => {
                let snapshot = Toplevel {
                    app_id: pending.app_id.clone(),
                    title: pending.title.clone(),
                    activated: pending.activated,
                    outputs: pending
                        .outputs
                        .iter()
                        .filter_map(|o| state.output_names.get(o).cloned())
                        .collect(),
                };
                state.apply(ToplevelEvent::Updated { id, toplevel: snapshot });
            }
            _ => {}
        }
    }
}

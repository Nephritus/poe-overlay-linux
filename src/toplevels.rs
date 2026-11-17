//! Feeds the compositor's toplevel windows into [`GameFocus`], on its own
//! Wayland connection: over wlr-foreign-toplevel-management (niri, Hyprland,
//! sway), else over KDE Plasma's window list, whose windows give a position
//! that the outputs' areas (xdg-output) turn into outputs.

use std::collections::HashMap;
use std::time::Duration;

use wayland_client::backend::ObjectId;
use wayland_client::protocol::{wl_output, wl_registry};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, event_created_child};
use wayland_protocols::xdg::xdg_output::zv1::client::{zxdg_output_manager_v1 as xdg_output_manager, zxdg_output_v1 as xdg_output};
use wayland_protocols_plasma::plasma_window_management::client::{
    org_kde_plasma_window as plasma_window, org_kde_plasma_window_management as plasma_manager,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1 as handle, zwlr_foreign_toplevel_manager_v1 as manager,
};

use crate::focus::{GameFocus, Rect, Toplevel, ToplevelEvent, Visibility};
use crate::plasma_windows::{PlasmaEvent, PlasmaWindows};

/// Plasma's window list from `window_with_uuid` (KWin 5.2x and later) on.
const PLASMA_MIN_VERSION: u32 = 13;

const NO_WINDOW_LIST: &str = "the compositor offers no window list (wlr-foreign-toplevel-management, or KDE Plasma's window management)";

/// Spawns the tracking thread. The receiver yields each change of where the
/// overlay should be; it closes if tracking is impossible or fails.
/// `kwin_entry_just_written`: KWin may not yet see the entry that lets the
/// overlay read its window list, so a missing list is asked for once more.
pub fn watch(kwin_entry_just_written: bool) -> async_channel::Receiver<Visibility> {
    let (tx, rx) = async_channel::unbounded();
    std::thread::spawn(move || {
        if let Err(err) = run(tx, kwin_entry_just_written) {
            eprintln!("poe-overlay: window tracking stopped, overlay stays hidden: {err}");
        }
    });
    rx
}

fn run(tx: async_channel::Sender<Visibility>, kwin_entry_just_written: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (mut queue, mut state) = match connect(tx.clone())? {
        Some(tracking) => tracking,
        None if kwin_entry_just_written => {
            std::thread::sleep(Duration::from_secs(2));
            connect(tx)?.ok_or(NO_WINDOW_LIST)?
        }
        None => return Err(NO_WINDOW_LIST.into()),
    };
    loop {
        queue.blocking_dispatch(&mut state)?;
    }
}

/// A subscription to the compositor's window list.
type Tracking = (EventQueue<State>, State);

/// Connects and subscribes to whichever window list the compositor offers;
/// `None` if it offers neither.
fn connect(tx: async_channel::Sender<Visibility>) -> Result<Option<Tracking>, Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let registry = conn.display().get_registry(&qh, ());
    let mut state = State {
        outputs: HashMap::new(),
        wlr: None,
        wlr_toplevels: HashMap::new(),
        plasma_global: None,
        xdg_output_global: None,
        xdg_outputs: None,
        plasma_windows: PlasmaWindows::default(),
        focus: GameFocus::default(),
        shown: Visibility::Hidden,
        tx,
    };
    queue.roundtrip(&mut state)?;
    if state.wlr.is_some() {
        return Ok(Some((queue, state)));
    }
    let (Some((plasma, version)), Some(xdg_output_global)) = (state.plasma_global, state.xdg_output_global) else {
        return Ok(None);
    };
    let xdg_outputs: xdg_output_manager::ZxdgOutputManagerV1 = registry.bind(xdg_output_global, 1, &qh, ());
    for output in state.outputs.values_mut() {
        output.xdg = Some(xdg_outputs.get_xdg_output(&output.proxy, &qh, output.proxy.id()));
    }
    state.xdg_outputs = Some(xdg_outputs);
    registry.bind::<plasma_manager::OrgKdePlasmaWindowManagement, _, _>(plasma, version, &qh, ());
    Ok(Some((queue, state)))
}

/// A window as wlr-foreign-toplevel reports it, until its next `done`.
#[derive(Default)]
struct Pending {
    app_id: String,
    title: String,
    activated: bool,
    outputs: Vec<ObjectId>,
}

struct Output {
    /// Its registry name, by which its removal is announced.
    global: u32,
    proxy: wl_output::WlOutput,
    name: Option<String>,
    /// Where it is among the outputs (Plasma's window list only).
    area: Rect,
    xdg: Option<xdg_output::ZxdgOutputV1>,
}

struct State {
    outputs: HashMap<ObjectId, Output>,
    wlr: Option<manager::ZwlrForeignToplevelManagerV1>,
    wlr_toplevels: HashMap<ObjectId, Pending>,
    /// Plasma's window list: `(registry name, version to bind)`.
    plasma_global: Option<(u32, u32)>,
    xdg_output_global: Option<u32>,
    /// Bound once Plasma's window list is used.
    xdg_outputs: Option<xdg_output_manager::ZxdgOutputManagerV1>,
    plasma_windows: PlasmaWindows,
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

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global { name, interface, version } => match interface.as_str() {
                // v4 adds the output name (e.g. `DP-2`) that GDK monitors share.
                "wl_output" if version >= 4 => {
                    let proxy: wl_output::WlOutput = registry.bind(name, 4, qh, ());
                    let xdg = state.xdg_outputs.as_ref().map(|xdg_outputs| xdg_outputs.get_xdg_output(&proxy, qh, proxy.id()));
                    state.outputs.insert(proxy.id(), Output { global: name, proxy, name: None, area: Rect::default(), xdg });
                }
                "zwlr_foreign_toplevel_manager_v1" => {
                    state.wlr = Some(registry.bind(name, version.min(3), qh, ()));
                }
                "org_kde_plasma_window_management" if version >= PLASMA_MIN_VERSION => {
                    state.plasma_global = Some((name, version.min(plasma_manager::OrgKdePlasmaWindowManagement::interface().version)));
                }
                "zxdg_output_manager_v1" => state.xdg_output_global = Some(name),
                _ => {}
            },
            // An unplugged monitor: no window is on it any more.
            wl_registry::Event::GlobalRemove { name } => state.outputs.retain(|_, output| {
                if output.global != name {
                    return true;
                }
                if let Some(xdg) = &output.xdg {
                    xdg.destroy();
                }
                output.proxy.release();
                false
            }),
            _ => {}
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
        if let wl_output::Event::Name { name } = event
            && let Some(output) = state.outputs.get_mut(&output.id())
        {
            output.name = Some(name);
        }
    }
}

impl Dispatch<xdg_output_manager::ZxdgOutputManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &xdg_output_manager::ZxdgOutputManagerV1,
        _: xdg_output_manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

/// User data: the `wl_output` it describes.
impl Dispatch<xdg_output::ZxdgOutputV1, ObjectId> for State {
    fn event(
        state: &mut Self,
        _: &xdg_output::ZxdgOutputV1,
        event: xdg_output::Event,
        output: &ObjectId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(output) = state.outputs.get_mut(output) else { return };
        match event {
            xdg_output::Event::LogicalPosition { x, y } => (output.area.x, output.area.y) = (x, y),
            xdg_output::Event::LogicalSize { width, height } => (output.area.width, output.area.height) = (width, height),
            _ => {}
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
            state.wlr_toplevels.insert(toplevel.id(), Pending::default());
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
            state.wlr_toplevels.remove(&key);
            toplevel.destroy();
            state.apply(ToplevelEvent::Closed { id });
            return;
        }
        let Some(pending) = state.wlr_toplevels.get_mut(&key) else { return };
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
                        .filter_map(|o| state.outputs.get(o)?.name.clone())
                        .collect(),
                };
                state.apply(ToplevelEvent::Updated { id, toplevel: snapshot });
            }
            _ => {}
        }
    }
}

impl Dispatch<plasma_manager::OrgKdePlasmaWindowManagement, ()> for State {
    fn event(
        _: &mut Self,
        windows: &plasma_manager::OrgKdePlasmaWindowManagement,
        event: plasma_manager::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let plasma_manager::Event::WindowWithUuid { uuid, .. } = event {
            windows.get_window_by_uuid(uuid, qh, ());
        }
    }
}

impl Dispatch<plasma_window::OrgKdePlasmaWindow, ()> for State {
    fn event(
        state: &mut Self,
        window: &plasma_window::OrgKdePlasmaWindow,
        event: plasma_window::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let event = match event {
            plasma_window::Event::TitleChanged { title } => PlasmaEvent::Title(title),
            plasma_window::Event::AppIdChanged { app_id } => PlasmaEvent::AppId(app_id),
            plasma_window::Event::StateChanged { flags } => PlasmaEvent::State(flags),
            plasma_window::Event::Geometry { x, y, width, height } => PlasmaEvent::Geometry(Rect {
                x,
                y,
                width: width.try_into().unwrap_or(i32::MAX),
                height: height.try_into().unwrap_or(i32::MAX),
            }),
            plasma_window::Event::InitialState => PlasmaEvent::InitialState,
            plasma_window::Event::Unmapped => {
                window.destroy();
                PlasmaEvent::Unmapped
            }
            _ => return,
        };
        let outputs = state.outputs.values().filter_map(|output| Some((output.name.as_deref()?, output.area)));
        if let Some(event) = state.plasma_windows.apply(window.id().protocol_id(), event, outputs) {
            state.apply(event);
        }
    }
}

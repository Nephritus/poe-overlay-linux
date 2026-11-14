//! The running overlay program: at most one per user session, reachable by
//! `poe-overlay trigger <action>` (which compositor hotkeys run).
//!
//! It holds a lock file and listens on a Unix socket, both in
//! `$XDG_RUNTIME_DIR`. The lock alone decides who is running; the socket is
//! only rebound by the lock holder, so one left behind by a crash is harmless.
//! A trigger writes the Action's name and a newline; the Overlay answers `ok`.

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, ErrorKind, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::ValueEnum;
use rustix::fs::{FlockOperation, flock};

/// How long a trigger waits for the Overlay to answer; compositor binds must
/// never hang on a wedged Overlay.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Action {
    /// Show or hide the Currency Rates Panel.
    CurrencyRates,
}

impl Action {
    /// The name on the command line, also used on the socket.
    fn name(self) -> String {
        self.to_possible_value().expect("no skipped Actions").get_name().to_owned()
    }
}

const SOCKET: &str = "poe-overlay.sock";

/// Held while the overlay program runs; dropping it (or the process dying)
/// lets another one start.
pub struct RunningOverlay {
    _lock: File,
    actions: async_channel::Receiver<Action>,
}

impl RunningOverlay {
    /// Starts in `$XDG_RUNTIME_DIR`. `None` if another overlay program runs.
    pub fn start() -> io::Result<Option<RunningOverlay>> {
        Self::start_in(&runtime_dir())
    }

    /// Starts in `dir`: takes the lock, then listens for Actions.
    pub fn start_in(dir: &Path) -> io::Result<Option<RunningOverlay>> {
        let lock = OpenOptions::new().create(true).truncate(false).write(true).open(dir.join("poe-overlay.lock"))?;
        match flock(&lock, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => {}
            Err(rustix::io::Errno::WOULDBLOCK) => return Ok(None),
            Err(err) => return Err(err.into()),
        }
        let socket = dir.join(SOCKET);
        match std::fs::remove_file(&socket) {
            Err(err) if err.kind() != ErrorKind::NotFound => return Err(err),
            _ => {}
        }
        let listener = UnixListener::bind(&socket)?;
        let (tx, actions) = async_channel::unbounded();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else {
                    // e.g. out of file descriptors: don't spin.
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                };
                if let Some(action) = receive(stream)
                    && tx.send_blocking(action).is_err()
                {
                    return;
                }
            }
        });
        Ok(Some(RunningOverlay { _lock: lock, actions }))
    }

    /// Actions sent by triggers, in arrival order.
    pub fn actions(&self) -> async_channel::Receiver<Action> {
        self.actions.clone()
    }
}

/// Reads one Action from a trigger and acknowledges it.
fn receive(stream: UnixStream) -> Option<Action> {
    stream.set_read_timeout(Some(ANSWER_TIMEOUT)).ok()?;
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).ok()?;
    let action = Action::from_str(line.trim_end(), false).ok();
    let answer: &[u8] = if action.is_some() { b"ok\n" } else { b"unknown action\n" };
    let _ = (&stream).write_all(answer);
    action
}

#[derive(Debug)]
pub enum SendError {
    NotRunning,
    Failed(io::Error),
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SendError::NotRunning => write!(f, "no overlay is running (it starts with the game)"),
            SendError::Failed(err) => write!(f, "the overlay did not take the action: {err}"),
        }
    }
}

/// Sends `action` to the running Overlay and waits for its answer.
pub fn send(action: Action) -> Result<(), SendError> {
    let socket = runtime_dir().join(SOCKET);
    let stream = UnixStream::connect(socket).map_err(|err| match err.kind() {
        ErrorKind::NotFound | ErrorKind::ConnectionRefused => SendError::NotRunning,
        _ => SendError::Failed(err),
    })?;
    let exchange = || -> io::Result<()> {
        stream.set_read_timeout(Some(ANSWER_TIMEOUT))?;
        stream.set_write_timeout(Some(ANSWER_TIMEOUT))?;
        (&stream).write_all(format!("{}\n", action.name()).as_bytes())?;
        let mut answer = String::new();
        BufReader::new(&stream).read_line(&mut answer)?;
        match answer.trim_end() {
            "ok" => Ok(()),
            other => Err(io::Error::other(format!("overlay answered {other:?}"))),
        }
    };
    exchange().map_err(SendError::Failed)
}

fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

//! At most one overlay per user session.

use std::fs::{File, OpenOptions};
use std::path::PathBuf;

use rustix::fs::{FlockOperation, flock};

/// Held for the overlay's lifetime; the lock is released when dropped or when
/// the process dies.
pub struct InstanceLock(#[allow(dead_code)] File);

/// Takes the session-wide overlay lock, or `None` if another overlay holds it.
pub fn acquire() -> std::io::Result<Option<InstanceLock>> {
    let file = OpenOptions::new().create(true).truncate(false).write(true).open(lock_path())?;
    match flock(&file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Ok(Some(InstanceLock(file))),
        Err(rustix::io::Errno::WOULDBLOCK) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn lock_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("poe-overlay.lock")
}

//! Reads lines appended to a log file, like `tail -F`.

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

pub struct LogFollower {
    path: PathBuf,
    /// Inode of the file being read; a new one means the log was replaced.
    inode: Option<u64>,
    offset: u64,
    /// Bytes of a line whose end hasn't been written yet.
    partial: Vec<u8>,
}

impl LogFollower {
    /// Starts at the current end of `path`, so existing lines are skipped. A
    /// file that doesn't exist yet is read from its beginning once created.
    pub fn new(path: &Path) -> Self {
        let meta = fs::metadata(path).ok();
        Self {
            path: path.to_owned(),
            inode: meta.as_ref().map(MetadataExt::ino),
            offset: meta.map_or(0, |m| m.len()),
            partial: Vec::new(),
        }
    }

    /// Complete lines appended since the last call, without line terminators
    /// (`\n`, `\r\n` or `\r`). Empty lines are dropped.
    pub fn poll(&mut self) -> io::Result<Vec<String>> {
        let meta = match fs::metadata(&self.path) {
            Ok(meta) => meta,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err),
        };
        if self.inode != Some(meta.ino()) || meta.len() < self.offset {
            self.inode = Some(meta.ino());
            self.offset = 0;
            self.partial.clear();
        }
        if meta.len() == self.offset {
            return Ok(Vec::new());
        }
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(self.offset))?;
        self.offset += file.read_to_end(&mut self.partial)? as u64;

        let Some(last_end) = self.partial.iter().rposition(|b| matches!(b, b'\r' | b'\n')) else {
            return Ok(Vec::new());
        };
        let complete: Vec<u8> = self.partial.drain(..=last_end).collect();
        Ok(complete
            .split(|b| matches!(b, b'\r' | b'\n'))
            .filter(|line| !line.is_empty())
            .map(|line| String::from_utf8_lossy(line).into_owned())
            .collect())
    }
}

/// How often the log is checked for new lines.
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

/// Follows `path` on a background thread; the receiver yields each new line.
/// The thread ends when the receiver is dropped.
pub fn watch(path: PathBuf) -> async_channel::Receiver<String> {
    let (tx, rx) = async_channel::unbounded();
    std::thread::spawn(move || {
        let mut follower = LogFollower::new(&path);
        loop {
            match follower.poll() {
                Ok(lines) => {
                    for line in lines {
                        if tx.send_blocking(line).is_err() {
                            return;
                        }
                    }
                }
                Err(err) => eprintln!("poe-overlay: reading {}: {err}", path.display()),
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    });
    rx
}

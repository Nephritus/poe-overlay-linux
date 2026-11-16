//! The Price Check's copy, on a thread of its own: the virtual keyboard,
//! the clipboard and the X pointer query all block.

use crate::clipboard::DataControlClipboard;
use crate::game_cursor::GameCursor;
use crate::poe1::item_copy::{CopyError, copy_item};
use crate::poe1::keyboard::Modifier;
use crate::uinput::{LetterKeys, VirtualKeyboard};

pub struct Copied {
    /// Where the pointer was at the press; `None` if unknown.
    pub left_half: Option<bool>,
    pub text: Result<String, CopyError>,
}

#[derive(Clone)]
pub struct ItemCopier {
    requests: async_channel::Sender<async_channel::Sender<Copied>>,
}

impl ItemCopier {
    /// Starts the thread, which sets up the virtual keyboard and clipboard
    /// right away. `held`: the Price Check hotkey's modifiers, which the
    /// player may still hold.
    pub fn spawn(letters: LetterKeys, held: Vec<Modifier>) -> ItemCopier {
        let (requests, incoming) = async_channel::unbounded::<async_channel::Sender<Copied>>();
        std::thread::spawn(move || {
            let mut keyboard = VirtualKeyboard::new(letters);
            let mut clipboard = connect();
            let mut cursor = GameCursor::default();
            while let Ok(reply) = incoming.recv_blocking() {
                let left_half = cursor.on_left_half();
                // Connected again after a failure, e.g. a compositor restart.
                let connected = match clipboard.take() {
                    Some(clipboard) => Ok(clipboard),
                    None => DataControlClipboard::connect(),
                };
                let text = match connected {
                    Ok(mut connected) => {
                        let text = copy_item(&mut keyboard, &mut connected, &held);
                        if !matches!(text, Err(CopyError::Clipboard(_))) {
                            clipboard = Some(connected);
                        }
                        text
                    }
                    Err(err) => Err(CopyError::Clipboard(err)),
                };
                let _ = reply.send_blocking(Copied { left_half, text });
            }
        });
        ItemCopier { requests }
    }

    /// Copies the hovered item.
    pub async fn copy(&self) -> Copied {
        let (reply, result) = async_channel::bounded(1);
        let gone = || Copied { left_half: None, text: Err(CopyError::Keyboard("the copy thread stopped".into())) };
        if self.requests.send(reply).await.is_err() {
            return gone();
        }
        result.recv().await.unwrap_or_else(|_| gone())
    }
}

fn connect() -> Option<DataControlClipboard> {
    DataControlClipboard::connect()
        .inspect_err(|err| eprintln!("poe-overlay: Price Check can't use the clipboard yet: {err}"))
        .ok()
}

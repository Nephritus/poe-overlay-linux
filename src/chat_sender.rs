//! Chat lines from Trade Request cards, sent on a thread of its own: the
//! virtual keyboard and the clipboard block, and after a line is sent this
//! thread hands the player's clipboard (put back) to whoever pastes it.

use std::os::fd::{AsFd, OwnedFd};
use std::sync::Arc;

use crate::clipboard::DataControlClipboard;
use crate::poe1::chat::{ChatError, send_chat};
use crate::uinput::{LetterKeys, VirtualKeyboard};

type Request = (String, async_channel::Sender<Result<(), ChatError>>);

#[derive(Clone)]
pub struct ChatSender {
    requests: async_channel::Sender<Request>,
    /// Written to after each request: wakes the thread while it serves the
    /// clipboard.
    wake: Arc<OwnedFd>,
}

impl ChatSender {
    pub fn spawn(letters: LetterKeys) -> ChatSender {
        let (requests, incoming) = async_channel::unbounded::<Request>();
        let (woken, wake) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK)
            .expect("a pipe for waking the chat thread");
        std::thread::spawn(move || {
            let mut keyboard = VirtualKeyboard::new(letters);
            let mut clipboard: Option<DataControlClipboard> = None;
            loop {
                // Hands over the clipboard put back until a request comes
                // (or no one can send any more).
                while incoming.is_empty()
                    && !incoming.is_closed()
                    && let Some(connected) = clipboard.as_mut().filter(|connected| connected.owns_selection())
                {
                    if connected.serve(woken.as_fd()).is_err() {
                        clipboard = None;
                    }
                    let mut drained = [0u8; 64];
                    while rustix::io::read(&woken, &mut drained).is_ok_and(|read| read > 0) {}
                }
                let Ok((line, reply)) = incoming.recv_blocking() else { return };
                let sent = match DataControlClipboard::reuse(&mut clipboard) {
                    Ok(mut connected) => {
                        let sent = send_chat(&mut keyboard, &mut connected, &line);
                        if !matches!(sent, Err(ChatError::Clipboard(_))) {
                            clipboard = Some(connected);
                        }
                        sent
                    }
                    Err(err) => Err(ChatError::Clipboard(err)),
                };
                let _ = reply.send_blocking(sent);
            }
        });
        ChatSender { requests, wake: Arc::new(wake) }
    }

    /// Sends `line` in the game's chat.
    pub async fn send(&self, line: String) -> Result<(), ChatError> {
        let (reply, result) = async_channel::bounded(1);
        let gone = || Err(ChatError::Keyboard("the chat thread stopped".into()));
        if self.requests.send((line, reply)).await.is_err() {
            return gone();
        }
        // A full pipe already wakes the thread.
        let _ = rustix::io::write(&*self.wake, &[1]);
        result.recv().await.unwrap_or_else(|_| gone())
    }
}

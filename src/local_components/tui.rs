use std::io::{stderr, Stderr};

use anyhow::Result;
use crossterm::event::Event as TerminalEvent;
use crossterm::terminal::enable_raw_mode;
use futures::{FutureExt, StreamExt};
use ratatui::backend::CrosstermBackend as Backend;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::component::LocalComponent;
use crate::message::Message;

pub struct Tui {
    pub terminal: ratatui::Terminal<Backend<Stderr>>,
}

impl Tui {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        crossterm::execute!(stderr(), crossterm::terminal::EnterAlternateScreen)?;
        let terminal = Terminal::new(Backend::new(stderr()))?;

        Ok(Self { terminal })
    }
}

impl LocalComponent for Tui {
    async fn run(
        &mut self,
        mut cx: crate::component::Context,
        cancel_token: CancellationToken,
    ) -> Result<()> {
        let (terminal_tx, mut terminal_rx) = mpsc::channel::<TerminalEvent>(10);

        let mut event_stream = crossterm::event::EventStream::new();
        let _cancel_token = cancel_token.clone();

        tokio::spawn(async move {
            loop {
                let event_future = event_stream.next().fuse();
                let cancel_future = _cancel_token.cancelled();

                tokio::select! {
                maybe_event = event_future => {
                    if let Some(Ok(event)) = maybe_event {
                        // TODO: error handling
                        let _ = terminal_tx.send(event).await;
                    }
                }
                _ = cancel_future => {
                    break
                }}
            }
        });

        loop {
            let message_future = cx.recv_message().fuse();
            let event_future = terminal_rx.recv().fuse();
            let cancel_future = cancel_token.cancelled();
            tokio::select! {
                maybe_message = message_future => {
                    if let Some(message) = maybe_message {
                        match message {
                            Message::Broadcast { ref id, .. } => {

                            }
                            Message::Call { ref callback_id, .. } => {
                            }
                        }
                    }
                },
                maybe_event = event_future => {
                    if let Some(event) = maybe_event {

                        cx.broadcast_message(event.into())?;
                    }
                },
                _ = cancel_future => {
                    break
                }
            }
        }

        Ok(())
    }
}

impl From<TerminalEvent> for Message {
    fn from(val: TerminalEvent) -> Self {
        let id = match val {
            TerminalEvent::Key(_) => "terminal.event.key",
            TerminalEvent::Mouse(_) => "terminal.event.key",
            TerminalEvent::Paste(_) => "terminal.event.paste",
            TerminalEvent::Resize(_, _) => "terminal.event.resize",
            TerminalEvent::FocusLost => "terminal.event.focuslost",
            TerminalEvent::FocusGained => "terminal.event.focusgained",
        };

        Message::new_broadcast(id, Some(val), None).unwrap()
    }
}

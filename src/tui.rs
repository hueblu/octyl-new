use std::io::{stderr, Stderr};

use anyhow::Result;
use crossterm::event::Event;
use crossterm::terminal::enable_raw_mode;
use futures::{FutureExt, StreamExt};
use ratatui::backend::CrosstermBackend as Backend;
use ratatui::Terminal;
use tokio::spawn;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::message::Message;

pub struct Tui {
    pub terminal: ratatui::Terminal<Backend<Stderr>>,

    pub task: JoinHandle<()>,
    pub cancellation_token: CancellationToken,
}

impl Tui {
    pub fn new(public_tx: mpsc::UnboundedSender<Message>) -> Result<Self> {
        enable_raw_mode()?;
        // crossterm::execute!(stderr(), crossterm::terminal::EnterAlternateScreen)?;
        let terminal = Terminal::new(Backend::new(stderr()))?;

        let cancellation_token = CancellationToken::new();
        let _cancellation_token = cancellation_token.clone();

        let task = spawn(async move {
            let mut event_stream = crossterm::event::EventStream::new();

            loop {
                let event_future = event_stream.next().fuse();

                tokio::select! {
                    _ = _cancellation_token.cancelled() => {
                        break;
                    }
                    maybe_event = event_future => {
                        if let Some(Ok(evt)) = maybe_event {
                            //TODO: error handling
                            let _ = public_tx.send(Message::from(evt));
                        }
                    }
                }
            }
        });

        Ok(Self {
            terminal,
            task,
            cancellation_token,
        })
    }
}

impl From<Event> for Message {
    fn from(value: Event) -> Self {
        let id = match value {
            Event::Key(_) => "terminal.event.key",
            Event::Mouse(_) => "terminal.event.key",
            Event::Paste(_) => "terminal.event.paste",
            Event::Resize(_, _) => "terminal.event.resize",
            Event::FocusLost => "terminal.event.focuslost",
            Event::FocusGained => "terminal.event.focusgained",
        };

        Message::new_broadcast(id, Some(value), None).unwrap()
    }
}

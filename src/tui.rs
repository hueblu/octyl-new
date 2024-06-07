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

pub struct Tui {
    pub terminal: ratatui::Terminal<Backend<Stderr>>,
    pub task: JoinHandle<()>,
    pub cancellation_token: CancellationToken,
    pub event_rx: mpsc::UnboundedReceiver<Event>,
}

impl Tui {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        // crossterm::execute!(stderr(), crossterm::terminal::EnterAlternateScreen)?;
        let terminal = Terminal::new(Backend::new(stderr()))?;

        let cancellation_token = CancellationToken::new();
        let _cancellation_token = cancellation_token.clone();

        let (event_tx, event_rx) = mpsc::unbounded_channel::<Event>();

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
                            event_tx.send(evt).unwrap();
                        }
                    }
                }
            }
        });

        Ok(Self {
            terminal,
            task,
            cancellation_token,
            event_rx,
        })
    }

    pub async fn recv(&mut self) -> Option<Event> {
        self.event_rx.recv().await
    }
}

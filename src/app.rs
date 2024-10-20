use std::{collections::HashMap, thread::sleep, time::Duration};

use anyhow::{anyhow, Result};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    component::{ComponentHandler, ComponentId, LocalComponent},
    local_components::{editor, tui},
    message::Message,
};

pub struct App {
    buffers: Vec<Buffer>,

    components: HashMap<ComponentId, ComponentHandler>,
    callback_registry: HashMap<String, ComponentId>,

    public_rx: mpsc::UnboundedReceiver<Message>,
    public_tx: mpsc::UnboundedSender<Message>,

    cancel_token: CancellationToken,
}

pub struct Buffer {
    inner: String,
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<Message>();

        let callback_registry = HashMap::new();

        Self {
            buffers: Vec::new(),

            components: HashMap::new(),

            callback_registry,

            public_rx: rx,
            public_tx: tx,

            cancel_token: CancellationToken::new(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        self.add_component(editor::Editor::new())?;
        self.add_component(tui::Tui::new()?)?;

        loop {
            if let Some(message) = self.public_rx.recv().await {
                self.handle_message(message).await?;
            }

            if self.cancel_token.is_cancelled() {
                sleep(Duration::from_millis(100));
                break;
            }
        }

        Ok(())
    }

    pub async fn handle_message(&mut self, message: Message) -> Result<()> {
        println!("{:?}", message);
        match message {
            Message::Broadcast { ref id, .. }
                if id == "terminal.event.key"
                    && let Event::Key(KeyEvent {
                        code, modifiers, ..
                    }) = message.get_inner::<Event>()?
                    && code == KeyCode::Char('c')
                    && modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.cancel_token.cancel();
            }

            Message::Call {
                ref callback_id, ..
            } if callback_id.starts_with("core") => match &**callback_id {
                "core.print" => {
                    println!("{:?}", message.get_inner::<String>()?);
                }
                "core.quit" => {
                    self.cancel_token.cancel();
                }
                "core.getinfo" => {
                    message.respond("skibidi rizz".to_string()).await?;
                }
                "core.makeWindow" => {}
                _ => {}
            },

            Message::Call {
                ref response_tx,
                ref callback_id,
                ..
            } => {
                if let Some(component_id) = self.callback_registry.get(callback_id) {
                    self.components
                        .get_mut(component_id)
                        .ok_or(anyhow!("component with registered callbacks not found"))?
                        .handle_message(message)
                        .await?;
                } else {
                    response_tx
                        .send(Result::Err(anyhow!("couldn't find callback specified")))
                        .await?;
                }
            }

            Message::Broadcast { .. } => {
                // TODO: custom error type for component failing + logging to a file
                let mut to_remove = vec![];
                for (key, component) in self.components.iter() {
                    if (component.handle_message(message.clone()).await).is_err() {
                        to_remove.push(*key);
                    };
                }
                for key in to_remove {
                    self.components.remove(&key);
                }
            }
        }

        Ok(())
    }

    pub fn add_component(&mut self, component: impl LocalComponent + 'static) -> Result<()> {
        let handler = ComponentHandler::new_local(
            component,
            self.public_tx.clone(),
            self.cancel_token.child_token(),
        );
        self.components.insert(handler.id(), handler);

        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

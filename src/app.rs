use std::collections::HashMap;

use anyhow::{anyhow, Result};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::{
    component::{ComponentHandler, ComponentId, LocalComponent},
    local_components::editor,
    message::Message,
    tui::Tui,
};

pub struct App {
    buffers: Vec<Buffer>,

    components: HashMap<ComponentId, ComponentHandler>,
    callback_registry: HashMap<String, ComponentId>,

    public_rx: mpsc::UnboundedReceiver<Message>,
    public_tx: mpsc::UnboundedSender<Message>,

    quit: bool,
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

            quit: false,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        self.add_component(editor::Editor::new())?;

        let _tui = Tui::new(self.public_tx.clone())?;

        loop {
            if let Some(message) = self.public_rx.recv().await {
                self.handle_message(message).await?;
            } else {
                self.quit = true;
            }

            if self.quit {
                break;
            }
        }

        Ok(())
    }

    pub async fn handle_message(&mut self, message: Message) -> Result<()> {
        match message {
            Message::Broadcast { ref id, .. }
                if id == "terminal.event.key"
                    && let Event::Key(KeyEvent {
                        code, modifiers, ..
                    }) = message.get_inner::<Event>()?
                    && code == KeyCode::Char('c')
                    && modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.quit = true
            }

            Message::Call {
                ref callback_id, ..
            } if callback_id.starts_with("core") => match &**callback_id {
                "core.print" => {
                    println!("{:?}", message.get_inner::<String>()?);
                }
                "core.quit" => {
                    self.quit = true;
                }
                "core.getinfo" => {
                    message.respond("skibidi rizz".to_string()).await?;
                }
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
                        .handle_message(message);
                } else {
                    response_tx
                        .send(Result::Err(anyhow!("couldn't find callback specified")))
                        .await?;
                }
            }

            Message::Broadcast { .. } => {
                for component in self.components.values() {
                    component.handle_message(message.clone());
                }
            }
        }

        Ok(())
    }

    pub fn add_component(&mut self, component: impl LocalComponent + 'static) -> Result<()> {
        let handler = ComponentHandler::new(component, self.public_tx.clone());
        self.components.insert(handler.id(), handler);

        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

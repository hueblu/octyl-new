use std::collections::HashMap;

use anyhow::Result;
use crossterm::event::Event;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{
    component::{Callback, Component, MountedComponent},
    components::editor,
    message::Message,
    tui::Tui,
};

pub struct App {
    buffers: Vec<Buffer>,

    // components have an id thats just a string
    // methods are called with "component id"."method name"
    //
    // for example:
    // context.call("editor.getBuffer", Value::Int(2))
    components: HashMap<Uuid, MountedComponent>,

    callback_registry: HashMap<String, Callback>,

    message_rx: broadcast::Receiver<Message>,
    message_tx: broadcast::Sender<Message>,

    quit: bool,
}

pub struct Buffer {
    inner: String,
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = broadcast::channel::<Message>(100);

        let callback_registry = HashMap::new();

        Self {
            buffers: Vec::new(),

            components: HashMap::new(),

            callback_registry,

            message_rx: rx,
            message_tx: tx,

            quit: false,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        self.add_component(editor::Editor::new())?;

        let mut tui = Tui::new()?;

        loop {
            let event_future = tui.recv();
            let message_future = self.message_rx.recv();

            tokio::select! {
                maybe_event = event_future => {
                    if let Some(event) = maybe_event {
                        self.handle_event(event).await
                    } else { Ok(()) }
                }
                maybe_message = message_future => {
                    if let Ok(message) = maybe_message {
                        self.handle_message(message).await
                    } else { Ok(()) }
                }
            }?;

            if self.quit {
                break;
            }
        }

        Ok(())
    }

    pub async fn handle_event(&mut self, event: Event) -> Result<()> {
        self.handle_message(Message::new_broadcast(
            "terminal.event:core",
            Some(event),
            None,
        )?)
        .await
    }

    pub async fn handle_message(&mut self, message: Message) -> Result<()> {
        match message {
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

            component_message => {
                for component in self.components.values() {
                    component.handle_message(component_message.clone());
                }
            }
        }

        Ok(())
    }

    pub fn add_component(&mut self, component: impl Component + 'static) -> Result<()> {
        let (uuid, mounted) = MountedComponent::new(component, self.message_tx.clone());
        self.components.insert(uuid, mounted);

        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

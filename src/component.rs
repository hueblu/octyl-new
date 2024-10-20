use anyhow::anyhow;
use std::{
    any::Any,
    ops::{Deref, DerefMut},
};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    sync::mpsc::{self, error::SendError},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::message::Message;

pub trait LocalComponent: Sync + Send {
    fn run(
        &mut self,
        cx: Context,
        cancel_token: CancellationToken,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}

pub struct Context {
    public_tx: mpsc::UnboundedSender<Message>,
    private_rx: mpsc::Receiver<Message>,

    broadcast_filters: Vec<String>,
    toggle_filter_broadcasts: bool,

    id: ComponentId,
}

pub struct ComponentHandler {
    thread: JoinHandle<()>,
    private_tx: mpsc::Sender<Message>,

    id: ComponentId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentId {
    inner: Uuid,
}

impl Context {
    pub fn new(
        public_tx: mpsc::UnboundedSender<Message>,
        private_rx: mpsc::Receiver<Message>,
        id: ComponentId,
    ) -> Self {
        Self {
            public_tx,
            private_rx,

            broadcast_filters: Vec::new(),
            toggle_filter_broadcasts: false,

            id,
        }
    }

    pub fn add_broadcast_filter(&mut self, id: String) -> Result<()> {
        self.broadcast_filters.push(id);

        Ok(())
    }

    pub fn filter_broadcasts(&mut self, toggle: bool) {
        self.toggle_filter_broadcasts = toggle;
    }

    pub fn broadcast<S, T>(&mut self, id: S, data: Option<T>) -> Result<()>
    where
        S: ToString,
        for<'a> T: Any + Serialize + Deserialize<'a>,
    {
        self.public_tx
            .send(Message::new_broadcast(id, data, Some(self.id))?)?;
        Ok(())
    }

    pub fn broadcast_message(&mut self, message: Message) -> Result<()> {
        if let Message::Broadcast { .. } = message {
            self.public_tx.send(message)?;
        } else {
            bail!("tried to broadcast message with Message::Call type");
        }
        Ok(())
    }

    pub async fn call<T, R>(
        &mut self,
        callback_id: impl ToString,
        data: Option<T>,
    ) -> Result<Option<R>>
    where
        for<'a> T: Any + Serialize + Deserialize<'a>,
        for<'b> R: Any + Serialize + Deserialize<'b>,
    {
        let (response_tx, mut response_rx) = mpsc::channel::<Result<Value>>(1);

        self.public_tx.send(Message::new_call::<_, T, R>(
            callback_id,
            data,
            self.id,
            response_tx,
        )?)?;
        match response_rx.recv().await {
            Some(Ok(Value::Null)) => Ok(None),
            Some(Ok(data)) if let Ok(result) = serde_json::from_value::<R>(data.clone()) => {
                Ok(Some(result))
            }
            Some(Ok(data)) => {
                bail!("failed to parse return value: {}", data);
            }
            Some(Err(e)) => {
                bail!(e)
            }
            None => Ok(None),
        }
    }

    pub async fn recv_message(&mut self) -> Option<Message> {
        let mut message = None;
        while message.is_none() {
            message = self
                .private_rx
                .recv()
                .await
                .filter(|message| match message {
                    Message::Broadcast { id, .. } => {
                        let mut matches = false;
                        for filter in &self.broadcast_filters {
                            if id.starts_with(filter) {
                                matches = true;
                                break;
                            }
                        }
                        matches
                    }
                    _ => true,
                });
        }

        message
    }
}

impl ComponentHandler {
    pub fn new_local(
        mut component: impl LocalComponent + 'static,
        public_tx: mpsc::UnboundedSender<Message>,
        cancel_token: CancellationToken,
    ) -> Self {
        let (private_tx, private_rx) = mpsc::channel::<Message>(100);
        let id = ComponentId::new();
        let context = Context::new(public_tx, private_rx, id);

        let thread = tokio::spawn(async move {
            let _ = component.run(context, cancel_token).await;
        });

        Self {
            thread,
            private_tx,

            id,
        }
    }

    pub fn id(&self) -> ComponentId {
        self.id
    }

    pub async fn handle_message(&self, message: Message) -> Result<()> {
        if let Err(SendError(_)) = self.private_tx.send(message).await {
            bail!("component failed")
        }
        Ok(())
    }
}

impl ComponentId {
    pub fn new() -> Self {
        Self {
            inner: Uuid::new_v4(),
        }
    }
}

impl Default for ComponentId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for ComponentId {
    fn from(value: Uuid) -> Self {
        Self { inner: value }
    }
}

impl Deref for ComponentId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for ComponentId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

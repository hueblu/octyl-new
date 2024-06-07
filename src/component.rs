use std::{
    any::Any,
    ops::{Deref, DerefMut},
};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{sync::mpsc, task::JoinHandle};
use uuid::Uuid;

use crate::message::Message;

pub trait LocalComponent: Sync + Send {
    fn run(&mut self, cx: Context) -> impl std::future::Future<Output = Result<()>> + Send;
}

pub struct Context {
    public_tx: mpsc::UnboundedSender<Message>,

    private_tx: mpsc::Sender<Message>,
    private_rx: mpsc::Receiver<Message>,

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
        private_tx: mpsc::Sender<Message>,
        private_rx: mpsc::Receiver<Message>,
        id: ComponentId,
    ) -> Self {
        Self {
            public_tx,
            private_tx,
            private_rx,
            id,
        }
    }

    pub fn add_broadcast_subscription(&mut self, id: String) -> Result<()> {
        Ok(())
    }

    pub fn add_call_subscription(&mut self, id: String) -> Result<()> {
        Ok(())
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

    pub async fn call<T, R>(
        &mut self,
        callback_id: impl ToString,
        data: Option<T>,
    ) -> Result<Option<R>>
    where
        for<'a> T: Any + Serialize + Deserialize<'a>,
        for<'b> R: Any + Serialize + Deserialize<'b>,
    {
        let (response_tx, mut response_rx) = mpsc::channel::<Result<Value>>(10);

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
        self.private_rx.recv().await
    }
}

impl ComponentHandler {
    pub fn new(
        mut component: impl LocalComponent + 'static,
        public_tx: mpsc::UnboundedSender<Message>,
    ) -> Self {
        let (private_tx, private_rx) = mpsc::channel::<Message>(100);
        let id = ComponentId::new();
        let context = Context::new(public_tx, private_tx.clone(), private_rx, id);

        let thread = tokio::spawn(async move {
            let _ = component.run(context).await;
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

    pub fn handle_message(&self, message: Message) {}
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

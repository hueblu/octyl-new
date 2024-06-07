use std::{any::Any, marker::PhantomData};

use anyhow::{bail, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    sync::{broadcast, mpsc},
    task::JoinHandle,
};
use uuid::Uuid;

use crate::message::Message;

pub type Callback = Box<dyn FnMut(Option<Value>) -> Result<Option<Value>>>;

pub trait Component: Sync + Send {
    fn run(
        &mut self,
        cx: Context,
    ) -> impl std::future::Future<Output = Result<()>> + std::marker::Send;
}

// broadcasted messages from components are like:
// "component id"."message detail"."message detail"...
pub struct Context {
    public_tx: broadcast::Sender<Message>,

    private_tx: mpsc::Sender<Message>,
    private_rx: mpsc::Receiver<Message>,

    uuid: Uuid,
}

pub struct MountedComponent {
    thread: JoinHandle<()>,
    private_tx: mpsc::Sender<Message>,

    uuid: Uuid,
}

#[derive(Debug)]
pub struct CallReciever<'a, R: Any + Serialize + Deserialize<'a>> {
    phantom_data: PhantomData<&'a R>,

    inner: mpsc::Receiver<Value>,
}

impl Context {
    pub fn new(
        public_tx: broadcast::Sender<Message>,
        private_tx: mpsc::Sender<Message>,
        private_rx: mpsc::Receiver<Message>,
        uuid: Uuid,
    ) -> Self {
        Self {
            public_tx,
            private_tx,
            private_rx,
            uuid,
        }
    }

    pub fn set_message_subscriptions(&mut self) {}

    pub fn get_event_stream(&mut self) {}

    pub fn add_call(&mut self, id: String, callback: Callback) -> Result<()> {
        Ok(())
    }

    pub fn broadcast<S, T>(&mut self, id: S, data: Option<T>) -> Result<()>
    where
        S: ToString,
        for<'a> T: Any + Serialize + Deserialize<'a>,
    {
        self.public_tx
            .send(Message::new_broadcast(id, data, Some(self.uuid))?)?;
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
        let (response_tx, mut response_rx) = mpsc::channel::<Value>(10);

        self.public_tx.send(Message::new_call(
            callback_id,
            data,
            self.uuid,
            response_tx,
        )?)?;
        match response_rx.recv().await {
            Some(Value::Null) => Ok(None),
            Some(data) if let Ok(result) = serde_json::from_value::<R>(data.clone()) => {
                Ok(Some(result))
            }
            Some(data) => {
                bail!("failed to parse return value: {}", data);
            }
            None => Ok(None),
        }
    }

    pub async fn recv_message(&mut self) -> Option<Message> {
        self.private_rx.recv().await
    }
}

impl MountedComponent {
    pub fn new(
        mut component: impl Component + 'static,
        message_tx: broadcast::Sender<Message>,
    ) -> (Uuid, Self) {
        let (private_tx, private_rx) = mpsc::channel::<Message>(100);
        let uuid = Uuid::new_v4();
        let context = Context::new(message_tx, private_tx.clone(), private_rx, uuid);

        let thread = tokio::spawn(async move {
            //TODO: error handling
            component.run(context).await;
        });

        (
            uuid,
            Self {
                thread,
                private_tx,
                uuid,
            },
        )
    }

    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    pub fn handle_message(&self, message: Message) {
        // if matches subscriptions: send to component
    }
}

impl<'a, R> CallReciever<'a, R>
where
    R: Any + Serialize + DeserializeOwned,
{
    pub async fn recv(&mut self) -> Result<Option<R>> {
        if let Some(data) = self.inner.recv().await {
            Ok(Some(serde_json::from_value::<R>(data)?))
        } else {
            Ok(None)
        }
    }
}

impl<'a, R> From<mpsc::Receiver<Value>> for CallReciever<'a, R>
where
    R: Any + Serialize + Deserialize<'a>,
{
    fn from(value: mpsc::Receiver<Value>) -> CallReciever<'a, R> {
        Self {
            phantom_data: PhantomData,
            inner: value,
        }
    }
}

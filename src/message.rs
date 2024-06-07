use std::any::{Any, TypeId};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::component::ComponentId;

#[derive(Clone, Debug)]
pub enum Message {
    Broadcast {
        id: String,

        inner: Option<Value>,
        inner_type: Option<TypeId>,

        sender: Option<ComponentId>,
    },
    Call {
        callback_id: String,

        inner: Option<Value>,
        inner_type: Option<TypeId>,

        response_type: Option<TypeId>,

        caller_id: ComponentId,
        response_tx: mpsc::Sender<Result<Value>>,
    },
}

impl Message {
    pub fn new_broadcast<S, T>(id: S, data: Option<T>, sender: Option<ComponentId>) -> Result<Self>
    where
        S: ToString,
        for<'a> T: Any + Serialize + Deserialize<'a>,
    {
        if let Some(data) = data {
            let inner = Some(serde_json::to_value(&data)?);
            let inner_type = Some(data.type_id());

            Ok(Message::Broadcast {
                id: id.to_string(),
                inner,
                inner_type,
                sender,
            })
        } else {
            Ok(Message::Broadcast {
                id: id.to_string(),
                inner: None,
                inner_type: None,
                sender,
            })
        }
    }

    pub fn new_call<S, T, R>(
        id: S,
        data: Option<T>,
        caller_id: ComponentId,
        response_channel: mpsc::Sender<Result<Value>>,
    ) -> Result<Self>
    where
        S: ToString,
        for<'a> T: Any + Serialize + Deserialize<'a>,
        for<'b> R: Any + Serialize + Deserialize<'b>,
    {
        if let Some(data) = data {
            let inner = Some(serde_json::to_value(&data)?);
            let inner_type = Some(data.type_id());

            Ok(Message::Call {
                callback_id: id.to_string(),

                inner,
                inner_type,

                response_type: Some(TypeId::of::<R>()),

                caller_id,
                response_tx: response_channel,
            })
        } else {
            Ok(Message::Call {
                callback_id: id.to_string(),

                inner: None,
                inner_type: None,

                response_type: Some(TypeId::of::<R>()),

                caller_id,
                response_tx: response_channel,
            })
        }
    }

    pub async fn respond<R>(self, response: R) -> Result<()>
    where
        for<'a> R: Any + Serialize + Deserialize<'a>,
    {
        if let Message::Call { response_tx, .. } = self {
            response_tx
                .send(Ok(serde_json::to_value(response)?))
                .await?;
        }

        Ok(())
    }

    pub fn get_inner<T>(&self) -> Result<T>
    where
        for<'a> T: Any + Serialize + Deserialize<'a>,
    {
        let (inner, inner_type) = match self.get_contents() {
            Some((inner, inner_type)) => (inner, inner_type),
            None => bail!("no inner data"),
        };

        if *inner_type != TypeId::of::<T>() {
            bail!("type provided doesn't match internal type");
        }

        Ok(serde_json::from_value::<T>(inner.clone())?)
    }

    fn get_contents(&self) -> Option<(&Value, &TypeId)> {
        let (inner, inner_type) = match self {
            Self::Broadcast {
                inner, inner_type, ..
            } => (inner, inner_type),
            Self::Call {
                inner, inner_type, ..
            } => (inner, inner_type),
        };
        if let Some(inner) = inner {
            if let Some(inner_type) = inner_type {
                return Some((inner, inner_type));
            }
        }
        None
    }

    fn get_contents_mut(&mut self) -> Option<(&mut Value, &mut TypeId)> {
        let (inner, inner_type) = match self {
            Self::Broadcast {
                inner, inner_type, ..
            } => (inner, inner_type),
            Self::Call {
                inner, inner_type, ..
            } => (inner, inner_type),
        };
        if let Some(inner) = inner {
            if let Some(inner_type) = inner_type {
                return Some((inner, inner_type));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::Message;

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    struct A {
        a: u32,
        b: String,
        c: bool,
    }

    #[test]
    fn broadcast_message_from_struct() {
        let something = A {
            a: 13,
            b: String::from("foo"),
            c: true,
        };

        let message = Message::new_broadcast("balls", Some(something.clone()), None).unwrap();

        assert_eq!(something, message.get_inner().unwrap());
    }
}

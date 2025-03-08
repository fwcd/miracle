use anyhow::{anyhow, bail, Context, Result};
use async_tungstenite::{tokio::TokioAdapter, tungstenite::{self, error::ProtocolError, Message}, WebSocketStream};
use futures::StreamExt;
use lighthouse_protocol::{to_value, ClientMessage, Value, Verb};
use serde::Deserialize;
use tokio::net::TcpStream;
use tracing::{error, warn};

use crate::{model::{Directory, Node, Resource}, state::State};

/// A handler that receives and responds to messages from a single client.
pub struct ClientHandler {
    web_socket: WebSocketStream<TokioAdapter<TcpStream>>,
    state: State,
}

impl ClientHandler {
    /// Creates a new handler from the given stream.
    pub async fn from_stream(stream: TcpStream, state: State) -> Result<Self> {
        Ok(Self {
            web_socket: async_tungstenite::tokio::accept_async(stream).await?,
            state,
        })
    }

    /// Creates a new handler from the given stream and starts a blocking
    /// receive loop until the connection closes.
    pub async fn handle_stream(stream: TcpStream, state: State) -> Result<()> {
        let handler = Self::from_stream(stream, state).await?;
        handler.run().await
    }

    /// Starts a blocking receive loop that parses and responds to messages from
    /// the client. Returns when the connection closes.
    pub async fn run(mut self) -> Result<()> {
        while let Some(message) = self.receive_message::<Value>().await {
            match message {
                Ok(message) => {
                    if let Err(e) = self.handle_message(message).await {
                        error!("Could not handle message: {:?}", e);
                    }
                },
                Err(e) => error!("Bad message: {:?}", e),
            }
        }
        Ok(())
    }

    /// Receives and parses a single high-level message from the client or
    /// `None` if there are no more.
    async fn receive_message<P>(&mut self) -> Option<Result<ClientMessage<P>>>
    where
        P: for<'de> Deserialize<'de> {
        let bytes = self.receive_raw().await?;
        Some(bytes.and_then(|b| Ok(rmp_serde::from_slice(&b)?)))
    }

    // Receives a single binary WebSocket message from the client or `None` if
    /// there are no more.
    async fn receive_raw(&mut self) -> Option<Result<Vec<u8>>> {
        while let Some(message) = self.web_socket.next().await {
            match message {
                Ok(Message::Binary(bytes)) => return Some(Ok(bytes)),
                Ok(Message::Ping(_)) => {}, // Ignore pings for now
                Ok(Message::Close(_)) => return None,
                Ok(_) => warn!("Got non-binary message: {:?}", message),
                Err(tungstenite::Error::Protocol(ProtocolError::ResetWithoutClosingHandshake)) => {
                    warn!("Closed without handshake");
                    return None;
                },
                Err(e) => return Some(Err(e.into())),
            }
        }
        None
    }

    async fn handle_message(&self, message: ClientMessage<Value>) -> Result<()> {
        let mut tree = self.state.lock_tree().await;

        let parent_path = &message.path[..message.path.len() - 1];
        let name = message.path[message.path.len() - 1].clone();

        let parent: &mut Directory = {
            match tree.get_path_mut(parent_path).ok_or_else(|| anyhow!("Parent path does not exist: {parent_path:?}"))? {
                Node::Resource(_) => bail!("Parent path points to a resource: {parent_path:?}"),
                Node::Directory(directory) => directory,
            } 
        };

        let response_payload = match message.verb {
            Verb::Post => {
                parent.insert(name, Node::Resource(Resource::from(message.payload)));
                Value::Nil
            },
            Verb::Create => {
                parent.insert(name, Node::Resource(Resource::new()));
                Value::Nil
            },
            Verb::Mkdir => {
                parent.insert(name, Node::Directory(Directory::new()));
                Value::Nil
            },
            Verb::Delete => {
                parent.remove(&name);
                Value::Nil
            },
            Verb::List => {
                let tree = parent.get(&name)
                    .and_then(|c| c.as_directory())
                    .map(|d| d.list_tree())
                    .context("Could not fetch directory tree")?;
                to_value(tree)?
            },
            Verb::Get => {
                let resource = parent.get(&name)
                    .and_then(|c| c.as_resource())
                    .context("Could not get resource")?;
                resource.value().clone()
            },
            Verb::Put => {
                // TODO: Check if exists
                parent.insert(name, Node::Resource(Resource::from(message.payload)));
                Value::Nil
            },
            Verb::Stream => todo!(),
            Verb::Stop => todo!(),
            verb => bail!("Unimplemented verb: {verb:?}"),
        };

        Ok(())
    }
}

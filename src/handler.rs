use std::sync::Arc;

use anyhow::{bail, Result};
use async_tungstenite::{tokio::TokioAdapter, tungstenite::{self, error::ProtocolError, Bytes, Message}, WebSocketStream};
use dashmap::DashMap;
use futures::StreamExt;
use lighthouse_protocol::{to_value, ClientMessage, ServerMessage, Value, Verb};
use serde::Serialize;
use serde::Deserialize;
use tokio::net::TcpStream;
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};

use crate::{model::{Directory, Resource}, state::State};

/// A handler that receives and responds to messages from a single client.
pub struct ClientHandler {
    web_socket: WebSocketStream<TokioAdapter<TcpStream>>,
    state: State,
    streams: Arc<DashMap<i32, StreamInfo>>,
}

struct StreamInfo {
    path: Vec<String>,
    token: CancellationToken,
}

impl ClientHandler {
    /// Creates a new handler from the given stream.
    pub async fn from_stream(stream: TcpStream, state: State) -> Result<Self> {
        Ok(Self {
            web_socket: async_tungstenite::tokio::accept_async(stream).await?,
            state,
            streams: Arc::new(DashMap::new()),
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
    async fn receive_raw(&mut self) -> Option<Result<Bytes>> {
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

    async fn handle_message(&mut self, message: ClientMessage<Value>) -> Result<()> {
        let request_id = message.request_id;

        let response = self.process_message(message).await
            .map(|(payload, code)| ServerMessage {
                request_id: Some(request_id),
                code,
                payload,
                response: None,
                warnings: Vec::new(),
            })
            .unwrap_or_else(|e| ServerMessage {
                request_id: Some(request_id),
                code: 400, // TODO: Provide a code as part of the error (instead of using anyhow::Result)?
                payload: Value::Nil,
                response: Some(format!("{:?}", e)),
                warnings: Vec::new(),
            });

        self.send(response).await?;

        Ok(())
    }

    async fn process_message(&self, message: ClientMessage<Value>) -> Result<(Value, i32)> {
        let state = &self.state;
        let ClientMessage { request_id, path, verb, payload, .. } = message;

        let response_payload = match verb {
            Verb::Post => to_value(state.insert_resource(&path, Resource::from(payload)).await?)?,
            Verb::Create => {
                if state.exists(&path)? {
                    bail!("Path {:?} already exists", &path);
                }
                to_value(state.insert_resource(&path, Resource::new()).await?)?
            },
            Verb::Mkdir => to_value(state.insert_directory(&path, Directory::new()).await?)?,
            Verb::Delete => to_value(state.remove(&path)?)?,
            Verb::List => to_value(state.list_tree(&path)?)?,
            Verb::Get => to_value(state.get(&path)?)?,
            Verb::Put => {
                if !state.exists(&path)? {
                    bail!("Resource at {:?} does not exist", &path);
                }
                to_value(state.insert_resource(&path, Resource::from(payload)).await?)?
            },
            Verb::Stream => {
                if self.streams.contains_key(&request_id) {
                    bail!("A stream with request id {request_id} already exists");
                };

                let value = to_value(state.get(&path)?)?;
                let token = CancellationToken::new();

                tokio::spawn({
                    let path = path.to_vec();
                    let streams = self.streams.clone();
                    async move {
                        Self::run_stream(request_id, &path).await;
                        streams.remove(&request_id);
                    }
                });

                self.streams.insert(request_id, StreamInfo {
                    path,
                    token,
                });

                value
            },
            Verb::Stop => {
                let Some((_, info)) = self.streams.remove(&request_id) else {
                    bail!("No stream with request id {request_id} is currently running")
                };
                if &info.path != &path {
                    bail!("A stream with request id {} exists, but on another path ({:?})", request_id, &info.path);
                }

                info.token.cancel();
                Value::Nil
            },
            verb => bail!("Unimplemented verb: {verb:?}"),
        };

        Ok((response_payload, 200))
    }

    async fn run_stream(request_id: i32, path: &[String]) {
        // TODO
    }

    async fn send<P>(&mut self, message: ServerMessage<P>) -> Result<()> where P: Serialize {
        self.send_raw(rmp_serde::to_vec_named(&message)?).await
    }

    async fn send_raw(&mut self, raw_message: Vec<u8>) -> Result<()> {
        Ok(self.web_socket.send(Message::Binary(raw_message.into())).await?)
    }
}

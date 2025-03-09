use std::sync::Arc;

use anyhow::{bail, Result};
use async_tungstenite::tungstenite::{self, error::ProtocolError, Bytes, Message};
use dashmap::DashMap;
use futures::StreamExt;
use lighthouse_protocol::{to_value, ClientMessage, ServerMessage, Value, Verb};
use serde::Serialize;
use serde::Deserialize;
use tokio::{net::TcpStream, sync::mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};

use crate::{model::{Directory, Resource}, state::State, ws::{self, ws_channel}};

/// A handler that receives and responds to messages from a single client.
#[derive(Clone)]
pub struct ClientHandler {
    ws_tx: mpsc::Sender<Message>,
    state: State,
    streams: Arc<DashMap<i32, StreamInfo>>,
}

struct StreamInfo {
    path: Vec<String>,
    token: CancellationToken,
}

impl ClientHandler {
    /// Creates a new handler from the given stream and starts a blocking
    /// receive loop until the connection closes.
    pub async fn handle_stream(stream: TcpStream, state: State) -> Result<()> {
        let ws_stream = async_tungstenite::tokio::accept_async(stream).await?;
        let (ws_tx, ws_rx) = ws_channel(ws_stream);

        let handler = Self {
            ws_tx,
            state,
            streams: Arc::new(DashMap::new()),
        };

        handler.run(ws_rx).await
    }

    /// Starts a blocking receive loop that parses and responds to messages from
    /// the client. Returns when the connection closes.
    async fn run(self, mut ws_rx: mpsc::Receiver<ws::Result<Message>>) -> Result<()> {
        while let Some(message) = self.receive_message::<Value>(&mut ws_rx).await {
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
    async fn receive_message<P>(&self, ws_rx: &mut mpsc::Receiver<ws::Result<Message>>) -> Option<Result<ClientMessage<P>>>
    where
        P: for<'de> Deserialize<'de> {
        let bytes = self.receive_raw(ws_rx).await?;
        Some(bytes.and_then(|b| Ok(rmp_serde::from_slice(&b)?)))
    }

    // Receives a single binary WebSocket message from the client or `None` if
    /// there are no more.
    async fn receive_raw(&self, ws_rx: &mut mpsc::Receiver<ws::Result<Message>>) -> Option<Result<Bytes>> {
        while let Some(message) = ws_rx.recv().await {
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

                tokio::spawn({
                    let path = path.to_vec();
                    let this = self.clone();
                    async move {
                        this.run_stream(request_id, &path).await;
                    }
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

    async fn run_stream(&self, request_id: i32, path: &[String]) {
        let token = CancellationToken::new();

        self.streams.insert(request_id, StreamInfo {
            path: path.to_vec(),
            token,
        });

        if let Err(e) = self.stream(request_id, path).await {
            error!("Error while streaming {request_id} (path: {path:?}): {e}");
        }

        self.streams.remove(&request_id);
    }

    async fn stream(&self, request_id: i32, path: &[String]) -> Result<()> {
        let mut stream = self.state.stream(path)?;
        while let Some(value) = stream.next().await {
            self.send(ServerMessage {
                request_id: Some(request_id),
                code: 200,
                payload: value,
                response: None,
                warnings: Vec::new(),
            }).await?; // TODO: Should we handle the error gracefully here (e.g. log it) to avoid stopping the stream if something goes wrong?
        }
        Ok(())
    }

    async fn send<P>(&self, message: ServerMessage<P>) -> Result<()> where P: Serialize {
        self.send_raw(rmp_serde::to_vec_named(&message)?).await
    }

    async fn send_raw(&self, raw_message: Vec<u8>) -> Result<()> {
        Ok(self.ws_tx.send(Message::Binary(raw_message.into())).await?)
    }
}

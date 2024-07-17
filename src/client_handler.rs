use std::fmt::Debug;

use anyhow::Result;
use async_tungstenite::{tokio::TokioAdapter, tungstenite::{self, error::ProtocolError, Message}, WebSocketStream};
use futures::{SinkExt, StreamExt};
use lighthouse_protocol::{ClientMessage, ServerMessage, Value};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tracing::{error, info, warn};

/// A handler that receives and responds to messages from a single client.
pub struct ClientHandler {
    web_socket: WebSocketStream<TokioAdapter<TcpStream>>,
}

impl ClientHandler {
    /// Creates a new handler from the given stream.
    pub async fn from_stream(stream: TcpStream) -> Result<Self> {
        Ok(Self {
            web_socket: async_tungstenite::tokio::accept_async(stream).await?,
        })
    }

    /// Creates a new handler from the given stream and starts a blocking
    /// receive loop until the connection closes.
    pub async fn handle_stream(stream: TcpStream) -> Result<()> {
        let handler = Self::from_stream(stream).await?;
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

    /// Handles and responds to the given client message.
    async fn handle_message<P>(&mut self, message: ClientMessage<P>) -> Result<()>
    where P: for<'de> Deserialize<'de> {
        self.send_message(&ServerMessage {
            request_id: Some(message.request_id),
            code: 200,
            warnings: vec![],
            response: None,
            payload: (),
        }).await?;
        Ok(())
    }

    async fn receive_message<P>(&mut self) -> Option<Result<ClientMessage<P>>>
    where
        P: for<'de> Deserialize<'de> {
        let bytes = self.receive_raw().await?;
        Some(bytes.and_then(|b| Ok(rmp_serde::from_slice(&b)?)))
    }

    async fn receive_raw(&mut self) -> Option<Result<Vec<u8>>> {
        while let Some(message) = self.web_socket.next().await {
            match message {
                Ok(Message::Binary(bytes)) => return Some(Ok(bytes)),
                Ok(Message::Ping(_)) => {
                    // Ignore pings for now
                },
                Ok(_) => {
                    warn!("Got non-binary message: {:?}", message)
                },
                Err(tungstenite::Error::Protocol(ProtocolError::ResetWithoutClosingHandshake)) => {
                    warn!("Closed without handshake");
                    return None;
                },
                Err(e) => return Some(Err(e.into())),
            }
        }
        None
    }

    async fn send_message<P>(&mut self, message: &ServerMessage<P>) -> Result<()>
    where
        P: Serialize {
        self.send_raw(rmp_serde::to_vec_named(message)?).await
    }

    async fn send_raw(&mut self, bytes: impl Into<Vec<u8>> + Debug) -> Result<()> {
        Ok(self.web_socket.send(Message::Binary(bytes.into())).await?)
    }
}

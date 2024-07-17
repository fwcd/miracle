use anyhow::Result;
use async_tungstenite::{tokio::TokioAdapter, tungstenite::Message, WebSocketStream};
use futures::StreamExt;
use lighthouse_protocol::{ServerMessage, Value};
use serde::Deserialize;
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
        while let Some(msg) = self.receive_message::<Value>().await {
            match msg {
                Ok(msg) => info!("{:?}", msg), // TODO
                Err(e) => error!("Bad message: {:?}", e),
            }
        }
        Ok(())
    }

    async fn receive_message<P>(&mut self) -> Option<Result<ServerMessage<P>>>
    where
        P: for<'de> Deserialize<'de> {
        let bytes = self.receive_raw().await?;
        Some(bytes.and_then(|b| Ok(rmp_serde::from_slice(&b)?)))
    }

    async fn receive_raw(&mut self) -> Option<Result<Vec<u8>>> {
        while let Some(message) = self.web_socket.next().await {
            match message {
                Ok(Message::Binary(bytes)) => return Some(Ok(bytes)),
                // We ignore pings for now
                Ok(Message::Ping(_)) => {},
                Ok(_) => warn!("Got non-binary message: {:?}", message),
                Err(e) => return Some(Err(e.into())),
            }
        }
        None
    }
}

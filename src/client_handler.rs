use anyhow::{bail, Error, Result};
use async_tungstenite::{tokio::TokioAdapter, tungstenite::Message, WebSocketStream};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tracing::warn;

pub struct ClientHandler {
    web_socket: WebSocketStream<TokioAdapter<TcpStream>>,
}

impl ClientHandler {
    pub async fn from_stream(stream: TcpStream) -> Result<Self> {
        Ok(Self {
            web_socket: async_tungstenite::tokio::accept_async(stream).await?,
        })
    }

    pub async fn handle_stream(stream: TcpStream) -> Result<()> {
        let handler = Self::from_stream(stream).await?;
        handler.run().await
    }

    pub async fn run(mut self) -> Result<()> {
        while let Some(message) = self.web_socket.next().await {
            let message = message?;
            match message {
                Message::Binary(bytes) => {},
                Message::Ping(_) => {},
                _ => warn!("Got non-binary message: {:?}", message),
            }
            // TODO
        }
        Ok(())
    }

    async fn receive_raw(&mut self) -> Result<Vec<u8>> {
        loop {
            let Some(message) = self.web_socket.next().await else {
                bail!("No message");
            };
            let message = message?;
            // TODO
        }
    }
}

use anyhow::Result;
use async_tungstenite::{tokio::TokioAdapter, WebSocketStream};
use futures::StreamExt;
use tokio::net::TcpStream;

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
            // TODO
        }
        Ok(())
    }
}

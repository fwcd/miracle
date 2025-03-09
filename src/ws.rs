use async_tungstenite::{tokio::TokioAdapter, tungstenite::Message, WebSocketStream};
use futures::{SinkExt, StreamExt};
use tokio::{net::TcpStream, sync::mpsc};
use tracing::error;

pub type Result<T> = async_tungstenite::tungstenite::Result<T>;

/// Wraps the given WebSocket in two channels. This makes the sender cloneable.
pub fn ws_channel(ws_stream: WebSocketStream<TokioAdapter<TcpStream>>) -> (
    mpsc::Sender<Message>,
    mpsc::Receiver<Result<Message>>,
) {
    let (mut ws_sink, mut ws_source) = ws_stream.split();

    let (sink_tx, mut sink_rx) = mpsc::channel(4);
    let (source_tx, source_rx) = mpsc::channel(4);

    tokio::spawn(async move {
        while let Some(message) = ws_source.next().await {
            if let Err(e) = source_tx.send(message).await {
                error!("Could not forward WebSocket message to receiver: {e}");
            }
        }
    });

    tokio::spawn(async move {
        while let Some(message) = sink_rx.recv().await {
            if let Err(e) = ws_sink.send(message).await {
                error!("Could not send message to WebSocket sink: {e}");
            }
        }
    });

    (
        sink_tx,
        source_rx,
    )
}

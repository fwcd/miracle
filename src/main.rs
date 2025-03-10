use anyhow::Result;
use clap::Parser;
use state::State;
use tokio::net::TcpListener;
use tracing::{error, info, info_span, level_filters::LevelFilter, Instrument};
use tracing_subscriber::EnvFilter;

use crate::handler::ClientHandler;

mod handler;
mod model;
mod state;
mod ws;

#[derive(Parser)]
#[command(about, version, disable_help_flag = true)]
struct Args {
    /// The host to bind to.
    #[arg(short, long, default_value = "::1", env = "MIRACLE_HOST")]
    host: String,
    /// The port to bind to.
    #[arg(short, long, default_value = "4042", env = "MIRACLE_PORT")]
    port: u16,
    /// Prints this help.
    #[clap(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,
}

#[tokio::main]
async fn main() -> Result<()> {
    _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::builder().with_default_directive(LevelFilter::INFO.into()).from_env()?)
        .init();

    let args = Args::parse();

    let state = State::new();

    let listener = TcpListener::bind((args.host, args.port)).await?;
    info!("Listening on {}", listener.local_addr()?);

    while let Ok((stream, addr)) = listener.accept().await {
        let state = state.clone();
        tokio::spawn(async move {
            info!("Opened connection");
            if let Err(e) = ClientHandler::handle_stream(stream, state).await {
                error!("{}", e);
            }
            info!("Closed connection");
        }.instrument(info_span!("Client", %addr)));
    }

    Ok(())
}

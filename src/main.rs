use anyhow::Result;
use clap::Parser;
use tokio::net::TcpListener;
use tracing::info;

#[derive(Parser)]
#[command(about, version, disable_help_flag = true)]
struct Args {
    /// The host to bind to.
    #[arg(short, long, default_value = "::1", env = "MIRACLE_HOST")]
    host: String,
    /// The port to bind to.
    #[arg(short, long, default_value = "4000", env = "MIRACLE_PORT")]
    port: u16,
    /// Prints this help.
    #[clap(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().init();
    _ = dotenvy::dotenv();

    let args = Args::parse();

    let listener = TcpListener::bind((args.host, args.port)).await?;
    info!("Listening on {}", listener.local_addr()?);

    while let Ok((stream, addr)) = listener.accept().await {
        info!("Incoming connection from {}", addr);
        // TODO
    }

    Ok(())
}

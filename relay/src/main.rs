mod handler;
mod session;

use handler::handle_connection;
use session::SessionRegistry;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    let bind_addr = env::var("RELAY_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let addr: SocketAddr = bind_addr.parse()?;
    let listener = TcpListener::bind(&addr).await?;

    info!("==========================================");
    info!(" Android Desktop Bridge - Relay Server   ");
    info!(" Listening on: ws://{}", addr);
    info!("==========================================");

    let registry = Arc::new(SessionRegistry::new());

    while let Ok((stream, peer_addr)) = listener.accept().await {
        let registry = Arc::clone(&registry);
        tokio::spawn(async move {
            info!("Incoming connection from TCP peer: {}", peer_addr);
            match accept_async(stream).await {
                Ok(ws_stream) => {
                    handle_connection(ws_stream, registry).await;
                }
                Err(e) => {
                    error!("WebSocket handshake failed with {}: {}", peer_addr, e);
                }
            }
        });
    }

    Ok(())
}

mod client;
mod identity;
mod storage;
mod ui;

use client::{BridgeClient, ConnectionStatus};
use identity::IdentityManager;
use storage::Database;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    let args: Vec<String> = env::args().collect();
    let mut relay_url = "ws://127.0.0.1:8080".to_string();
    let mut custom_config_dir: Option<PathBuf> = None;
    let mut headless = false;
    let mut pair_code: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--relay" => {
                if i + 1 < args.len() {
                    relay_url = args[i + 1].clone();
                    i += 1;
                }
            }
            "--config-dir" => {
                if i + 1 < args.len() {
                    custom_config_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--headless" => {
                headless = true;
            }
            "--pair-code" => {
                if i + 1 < args.len() {
                    pair_code = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    info!("==========================================");
    info!(" Android Desktop Bridge - Linux Controller");
    info!(" Relay URL: {}", relay_url);
    info!("==========================================");

    let identity = Arc::new(IdentityManager::load_or_create(custom_config_dir.as_deref())?);
    let db_path = identity.config_dir().join("bridge.db");
    let db = Arc::new(Database::open(&db_path)?);

    info!("Controller Device ID: {}", identity.device_id());

    let (client_instance, outbound_rx) = BridgeClient::new(Arc::clone(&identity), Arc::clone(&db));
    let client = Arc::new(client_instance);

    // Spawn async background connection task
    let client_for_loop = Arc::clone(&client);
    let relay_url_clone = relay_url.clone();
    tokio::spawn(async move {
        client_for_loop.run_loop(relay_url_clone, outbound_rx).await;
    });

    if let Some(pin) = pair_code {
        let client_clone = Arc::clone(&client);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            info!("Sending pairing request with PIN: {}", pin);
            client_clone.send_pairing_request(pin, None);
        });
    }

    #[cfg(feature = "gui")]
    if !headless && (std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok()) {
        info!("Launching GTK4 user interface...");
        ui::gtk_ui::build_and_run("org.bridge.linux.controller", client, identity, db);
        return Ok(());
    }

    info!("Running in headless/CLI mode. Press Ctrl+C to exit.");
    let mut status_rx = client.status_receiver();
    while status_rx.changed().await.is_ok() {
        let current = status_rx.borrow().clone();
        match current {
            ConnectionStatus::AndroidConnected { device_id, device_name } => {
                info!("STATUS: Android Connected (Device: {}, ID: {})", device_name, device_id);
            }
            s => {
                info!("STATUS: {:?}", s);
            }
        }
    }

    Ok(())
}

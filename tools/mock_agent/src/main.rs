use bridge_protocol::crypto::DeviceKeyPair;
use bridge_protocol::{MessageEnvelope, MessagePayload};
use futures_util::{SinkExt, StreamExt};
use std::env;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    let args: Vec<String> = env::args().collect();
    let mut relay_url = "ws://127.0.0.1:8080".to_string();
    let mut expected_pin = "123456".to_string();
    let mut auto_exit = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--relay" => {
                if i + 1 < args.len() {
                    relay_url = args[i + 1].clone();
                    i += 1;
                }
            }
            "--pin" => {
                if i + 1 < args.len() {
                    expected_pin = args[i + 1].clone();
                    i += 1;
                }
            }
            "--auto-exit" => {
                auto_exit = true;
            }
            _ => {}
        }
        i += 1;
    }

    let keypair = DeviceKeyPair::generate();
    let device_id = keypair.device_id();
    let pubkey_hex = keypair.public_key_hex();
    let device_name = "Google Pixel 8 Pro (Mock Agent)";

    info!("==========================================");
    info!(" Android Desktop Bridge - Mock Agent      ");
    info!(" Device ID:    {}", device_id);
    info!(" Public Key:   {}...", &pubkey_hex[..24]);
    info!(" Expected PIN: {}", expected_pin);
    info!(" Relay URL:    {}", relay_url);
    info!("==========================================");

    let (ws_stream, _) = connect_async(&relay_url).await?;
    info!("Connected to relay server!");
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Send Authenticated Handshake
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let nonce = format!("{:x}", rand::random::<u64>());
    let challenge = format!("{}:{}:{}", device_id, timestamp, nonce);
    let signature = keypair.sign(challenge.as_bytes());

    let handshake = MessageEnvelope::new(
        device_id.clone(),
        None,
        MessagePayload::HandshakeRequest {
            device_id: device_id.clone(),
            public_key: pubkey_hex.clone(),
            timestamp,
            nonce,
            signature,
            role: "agent".into(),
        },
    );

    ws_sender.send(Message::Text(handshake.to_json()?)).await?;

    // Await Handshake Response
    if let Some(Ok(Message::Text(res_text))) = ws_receiver.next().await {
        let envelope = MessageEnvelope::from_json(&res_text)?;
        if let MessagePayload::HandshakeResponse { success, message, .. } = envelope.payload {
            if !success {
                error!("Handshake rejected by relay: {}", message);
                return Err("Handshake rejected".into());
            }
            info!("Relay handshake accepted: {}", message);
        }
    }

    // Main event loop
    while let Some(msg_res) = ws_receiver.next().await {
        let text = match msg_res {
            Ok(Message::Text(t)) => t,
            Ok(Message::Ping(_)) => continue,
            Ok(Message::Close(_)) => break,
            Err(e) => {
                error!("Error reading WS: {}", e);
                break;
            }
            _ => continue,
        };

        if let Ok(envelope) = MessageEnvelope::from_json(&text) {
            match envelope.payload {
                MessagePayload::PairingRequest {
                    initiator_id,
                    initiator_name,
                    initiator_pubkey: _,
                    pairing_code,
                } => {
                    info!(
                        "Received pairing request from {} ({}) with PIN '{}'",
                        initiator_name, initiator_id, pairing_code
                    );

                    let accepted = pairing_code == expected_pin;
                    let response = MessageEnvelope::new(
                        device_id.clone(),
                        Some(initiator_id.clone()),
                        MessagePayload::PairingResponse {
                            accepted,
                            responder_id: device_id.clone(),
                            responder_name: device_name.to_string(),
                            responder_pubkey: pubkey_hex.clone(),
                            message: if accepted {
                                Some("Pairing accepted by Android".into())
                            } else {
                                Some("Invalid pairing code".into())
                            },
                        },
                    );

                    ws_sender.send(Message::Text(response.to_json()?)).await?;

                    if accepted {
                        info!("Pairing completed successfully with controller {}!", initiator_id);

                        // Send status update indicating connected
                        let status_msg = MessageEnvelope::new(
                            device_id.clone(),
                            Some(initiator_id),
                            MessagePayload::StatusUpdate {
                                state: "Android Connected".into(),
                                details: Some(format!("Paired with {}", device_name)),
                            },
                        );
                        ws_sender.send(Message::Text(status_msg.to_json()?)).await?;

                        if auto_exit {
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            info!("Auto-exit requested after successful pairing. Exiting.");
                            return Ok(());
                        }
                    } else {
                        info!("Pairing rejected due to PIN mismatch");
                    }
                }
                MessagePayload::Ping { nonce } => {
                    let pong = MessageEnvelope::new(
                        device_id.clone(),
                        Some(envelope.sender),
                        MessagePayload::Pong { nonce },
                    );
                    ws_sender.send(Message::Text(pong.to_json()?)).await?;
                }
                _ => {
                    info!("Received message: {:?}", envelope.payload);
                }
            }
        }
    }

    Ok(())
}

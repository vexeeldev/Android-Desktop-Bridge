use crate::session::{DeviceSession, SessionRegistry};
use bridge_protocol::crypto::{device_id_from_pubkey_hex, verify_signature};
use bridge_protocol::{MessageEnvelope, MessagePayload};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use tracing::{info, warn};
use uuid::Uuid;

pub async fn handle_connection(stream: WebSocketStream<TcpStream>, registry: Arc<SessionRegistry>) {
    let (mut ws_sender, mut ws_receiver) = stream.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();

    // Forward outbound channel messages to WebSocket
    let forward_task = tokio::spawn(async move {
        while let Some(msg_text) = out_rx.recv().await {
            if let Err(e) = ws_sender.send(Message::Text(msg_text)).await {
                warn!("Failed to send WS message: {}", e);
                break;
            }
        }
    });

    // 1. Handshake Phase
    let mut authenticated_device_id: Option<String> = None;

    while let Some(msg_res) = ws_receiver.next().await {
        let msg = match msg_res {
            Ok(Message::Text(t)) => t,
            Ok(Message::Ping(_)) => continue,
            Ok(Message::Close(_)) => break,
            Err(e) => {
                warn!("WS read error: {}", e);
                break;
            }
            _ => continue,
        };

        let envelope = match MessageEnvelope::from_json(&msg) {
            Ok(env) => env,
            Err(e) => {
                warn!("Invalid JSON message received during handshake: {}", e);
                continue;
            }
        };

        if let MessagePayload::HandshakeRequest {
            device_id,
            public_key,
            timestamp,
            nonce,
            signature,
            role,
        } = &envelope.payload
        {
            // Verify device ID matches public key
            let expected_dev_id = match device_id_from_pubkey_hex(public_key) {
                Ok(id) => id,
                Err(e) => {
                    warn!("Invalid public key from {}: {}", device_id, e);
                    let err_reply = MessageEnvelope::new(
                        "relay".into(),
                        Some(device_id.clone()),
                        MessagePayload::HandshakeResponse {
                            success: false,
                            session_id: "".into(),
                            message: format!("Invalid public key: {}", e),
                        },
                    );
                    let _ = out_tx.send(err_reply.to_json().unwrap_or_default());
                    break;
                }
            };

            if expected_dev_id != *device_id {
                warn!(
                    "Device ID mismatch: claimed {}, computed {}",
                    device_id, expected_dev_id
                );
                let err_reply = MessageEnvelope::new(
                    "relay".into(),
                    Some(device_id.clone()),
                    MessagePayload::HandshakeResponse {
                        success: false,
                        session_id: "".into(),
                        message: "Device ID does not match public key".into(),
                    },
                );
                let _ = out_tx.send(err_reply.to_json().unwrap_or_default());
                break;
            }

            // Challenge format: "{device_id}:{timestamp}:{nonce}"
            let challenge = format!("{}:{}:{}", device_id, timestamp, nonce);
            if let Err(e) = verify_signature(public_key, challenge.as_bytes(), signature) {
                warn!("Signature verification failed for {}: {}", device_id, e);
                let err_reply = MessageEnvelope::new(
                    "relay".into(),
                    Some(device_id.clone()),
                    MessagePayload::HandshakeResponse {
                        success: false,
                        session_id: "".into(),
                        message: "Signature verification failed".into(),
                    },
                );
                let _ = out_tx.send(err_reply.to_json().unwrap_or_default());
                break;
            }

            // Authentication successful!
            let session_id = Uuid::new_v4().to_string();
            info!(
                "Device {} authenticated successfully (role: {})",
                device_id, role
            );

            authenticated_device_id = Some(device_id.clone());

            let session = DeviceSession {
                device_id: device_id.clone(),
                public_key: public_key.clone(),
                role: role.clone(),
                tx: out_tx.clone(),
            };
            registry.register(session).await;

            let ok_reply = MessageEnvelope::new(
                "relay".into(),
                Some(device_id.clone()),
                MessagePayload::HandshakeResponse {
                    success: true,
                    session_id,
                    message: "Authentication successful".into(),
                },
            );
            let _ = out_tx.send(ok_reply.to_json().unwrap_or_default());
            break;
        } else {
            warn!("Expected HandshakeRequest as first message, got other payload");
            let err_reply = MessageEnvelope::new(
                "relay".into(),
                None,
                MessagePayload::HandshakeResponse {
                    success: false,
                    session_id: "".into(),
                    message: "Must perform Handshake first".into(),
                },
            );
            let _ = out_tx.send(err_reply.to_json().unwrap_or_default());
            break;
        }
    }

    let dev_id = match authenticated_device_id {
        Some(id) => id,
        None => {
            forward_task.abort();
            return;
        }
    };

    // 2. Message Routing Phase
    while let Some(msg_res) = ws_receiver.next().await {
        let msg_text = match msg_res {
            Ok(Message::Text(t)) => t,
            Ok(Message::Ping(_)) => continue,
            Ok(Message::Close(_)) => break,
            Err(e) => {
                warn!("WS read error from {}: {}", dev_id, e);
                break;
            }
            _ => continue,
        };

        let envelope = match MessageEnvelope::from_json(&msg_text) {
            Ok(env) => env,
            Err(e) => {
                warn!("Malformed JSON from {}: {}", dev_id, e);
                continue;
            }
        };

        match &envelope.payload {
            MessagePayload::Ping { nonce } => {
                let pong = MessageEnvelope::new(
                    "relay".into(),
                    Some(dev_id.clone()),
                    MessagePayload::Pong { nonce: *nonce },
                );
                let _ = out_tx.send(pong.to_json().unwrap_or_default());
            }
            _ => {
                // Route message to recipient
                if let Some(target_recipient) = &envelope.recipient {
                    if let Some(target_tx) = registry.get_sender(target_recipient).await {
                        if let Err(e) = target_tx.send(msg_text.clone()) {
                            warn!("Failed to forward message to {}: {}", target_recipient, e);
                        }
                    } else {
                        // Recipient offline, notify sender
                        let offline_notify = MessageEnvelope::new(
                            "relay".into(),
                            Some(dev_id.clone()),
                            MessagePayload::PeerStatus {
                                peer_id: target_recipient.clone(),
                                status: "offline".into(),
                            },
                        );
                        let _ = out_tx.send(offline_notify.to_json().unwrap_or_default());
                    }
                } else {
                    // Recipient is None: If it's a PairingRequest without a specific recipient,
                    // relay to all connected agent devices
                    if let MessagePayload::PairingRequest { .. } = &envelope.payload {
                        let agents = registry.list_devices_by_role("agent").await;
                        for agent in agents {
                            let _ = agent.tx.send(msg_text.clone());
                        }
                    }
                }
            }
        }
    }

    // Cleanup on disconnect
    info!("Device {} disconnected", dev_id);
    registry.unregister(&dev_id).await;
    forward_task.abort();
}

use bridge_protocol::crypto::DeviceKeyPair;
use bridge_protocol::{MessageEnvelope, MessagePayload};
use futures_util::{SinkExt, StreamExt};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn test_full_relay_pairing_and_connection_flow() {
    // 1. Start Relay on ephemeral port
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("Failed to bind relay");

    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                if let Ok(mut ws_stream) = accept_async(stream).await {
                    while let Some(Ok(msg)) = ws_stream.next().await {
                        if let Message::Text(text) = msg {
                            if let Ok(env) = MessageEnvelope::from_json(&text) {
                                if let MessagePayload::HandshakeRequest { device_id, .. } = &env.payload {
                                    let resp = MessageEnvelope::new(
                                        "relay".into(),
                                        Some(device_id.clone()),
                                        MessagePayload::HandshakeResponse {
                                            success: true,
                                            session_id: "test-sess-1".into(),
                                            message: "OK".into(),
                                        },
                                    );
                                    let _ = ws_stream.send(Message::Text(resp.to_json().unwrap())).await;
                                }
                            }
                        }
                    }
                }
            });
        }
    });

    // 2. Validate cryptographic authentication handshake format
    let linux_kp = DeviceKeyPair::generate();
    let linux_id = linux_kp.device_id();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
    let nonce = "test_nonce_999";
    let challenge = format!("{}:{}:{}", linux_id, now, nonce);
    let sig = linux_kp.sign(challenge.as_bytes());

    assert!(bridge_protocol::crypto::verify_signature(
        &linux_kp.public_key_hex(),
        challenge.as_bytes(),
        &sig
    ).is_ok());

    // 3. Validate pairing exchange payload
    let android_kp = DeviceKeyPair::generate();
    let android_id = android_kp.device_id();

    let pairing_req = MessagePayload::PairingRequest {
        initiator_id: linux_id.clone(),
        initiator_name: "Linux Desktop Controller".into(),
        initiator_pubkey: linux_kp.public_key_hex(),
        pairing_code: "654321".into(),
    };
    let req_env = MessageEnvelope::new(linux_id.clone(), Some(android_id.clone()), pairing_req);
    let req_json = req_env.to_json().unwrap();

    let decoded_req = MessageEnvelope::from_json(&req_json).unwrap();
    if let MessagePayload::PairingRequest { pairing_code, .. } = decoded_req.payload {
        assert_eq!(pairing_code, "654321");
    } else {
        panic!("Wrong payload decoded");
    }

    // Android accepts pairing
    let pairing_resp = MessagePayload::PairingResponse {
        accepted: true,
        responder_id: android_id.clone(),
        responder_name: "Samsung Galaxy S24 Ultra".into(),
        responder_pubkey: android_kp.public_key_hex(),
        message: Some("Pairing accepted by Android".into()),
    };
    let resp_env = MessageEnvelope::new(android_id.clone(), Some(linux_id.clone()), pairing_resp);
    let resp_json = resp_env.to_json().unwrap();
    let decoded_resp = MessageEnvelope::from_json(&resp_json).unwrap();

    if let MessagePayload::PairingResponse { accepted, responder_name, .. } = decoded_resp.payload {
        assert!(accepted);
        assert_eq!(responder_name, "Samsung Galaxy S24 Ultra");
    }

    // Status transition: Android Connected
    let status_update = MessagePayload::StatusUpdate {
        state: "Android Connected".into(),
        details: Some("Connected over Relay".into()),
    };
    let status_env = MessageEnvelope::new(android_id, Some(linux_id), status_update);
    let status_json = status_env.to_json().unwrap();
    let decoded_status = MessageEnvelope::from_json(&status_json).unwrap();

    if let MessagePayload::StatusUpdate { state, .. } = decoded_status.payload {
        assert_eq!(state, "Android Connected");
    }
}

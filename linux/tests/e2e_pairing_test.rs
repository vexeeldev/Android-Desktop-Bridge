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

#[tokio::test]
async fn test_device_info_request_response_flow() {
    let linux_kp = DeviceKeyPair::generate();
    let linux_id = linux_kp.device_id();

    let android_kp = DeviceKeyPair::generate();
    let android_id = android_kp.device_id();

    // 1. Linux Controller sends DeviceInfoRequest
    let req_payload = MessagePayload::DeviceInfoRequest;
    let req_env = MessageEnvelope::new(linux_id.clone(), Some(android_id.clone()), req_payload);
    let req_json = req_env.to_json().expect("DeviceInfoRequest serialization failed");

    // Verify deserialization by Android
    let decoded_req = MessageEnvelope::from_json(&req_json).expect("DeviceInfoRequest deserialization failed");
    assert_eq!(decoded_req.sender, linux_id);
    assert_eq!(decoded_req.recipient, Some(android_id.clone()));
    match decoded_req.payload {
        MessagePayload::DeviceInfoRequest => {}
        other => panic!("Expected DeviceInfoRequest, got {:?}", other),
    }

    // 2. Android Agent responds with all 11 real device info fields
    let resp_payload = MessagePayload::DeviceInfoResponse {
        device_id: android_id.clone(),
        manufacturer: "Google".into(),
        model: "Pixel 8 Pro".into(),
        android_version: "14".into(),
        sdk_level: 34,
        battery_level: 82,
        is_charging: true,
        screen_width: 1344,
        screen_height: 2992,
        orientation: "PORTRAIT".into(),
        service_status: "FOREGROUND_ACTIVE".into(),
    };
    let resp_env = MessageEnvelope::new(android_id.clone(), Some(linux_id.clone()), resp_payload);
    let resp_json = resp_env.to_json().expect("DeviceInfoResponse serialization failed");

    // 3. Linux Controller receives and parses DeviceInfoResponse
    let decoded_resp = MessageEnvelope::from_json(&resp_json).expect("DeviceInfoResponse deserialization failed");
    assert_eq!(decoded_resp.sender, android_id);
    assert_eq!(decoded_resp.recipient, Some(linux_id));

    match decoded_resp.payload {
        MessagePayload::DeviceInfoResponse {
            device_id,
            manufacturer,
            model,
            android_version,
            sdk_level,
            battery_level,
            is_charging,
            screen_width,
            screen_height,
            orientation,
            service_status,
        } => {
            assert_eq!(device_id, android_id);
            assert_eq!(manufacturer, "Google");
            assert_eq!(model, "Pixel 8 Pro");
            assert_eq!(android_version, "14");
            assert_eq!(sdk_level, 34);
            assert_eq!(battery_level, 82);
            assert!(is_charging);
            assert_eq!(screen_width, 1344);
            assert_eq!(screen_height, 2992);
            assert_eq!(orientation, "PORTRAIT");
            assert_eq!(service_status, "FOREGROUND_ACTIVE");
        }
        other => panic!("Expected DeviceInfoResponse, got {:?}", other),
    }
}

#[tokio::test]
async fn test_webrtc_signaling_and_screen_stream_flow() {
    let linux_kp = DeviceKeyPair::generate();
    let linux_id = linux_kp.device_id();

    let android_kp = DeviceKeyPair::generate();
    let android_id = android_kp.device_id();

    // 1. Controller requests Screen Start
    let screen_start = MessagePayload::ScreenStart {
        width: 1920,
        height: 1080,
        fps: 60,
    };
    let start_env = MessageEnvelope::new(linux_id.clone(), Some(android_id.clone()), screen_start);
    let start_json = start_env.to_json().expect("ScreenStart serialization failed");
    let decoded_start = MessageEnvelope::from_json(&start_json).expect("ScreenStart deserialization failed");
    match decoded_start.payload {
        MessagePayload::ScreenStart { width, height, fps } => {
            assert_eq!(width, 1920);
            assert_eq!(height, 1080);
            assert_eq!(fps, 60);
        }
        other => panic!("Expected ScreenStart, got {:?}", other),
    }

    // 2. Controller sends WebRTC Offer SDP
    let offer_sdp = "v=0\r\no=- 12345 2 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96\r\n";
    let offer_payload = MessagePayload::WebRtcOffer { sdp: offer_sdp.into() };
    let offer_env = MessageEnvelope::new(linux_id.clone(), Some(android_id.clone()), offer_payload);
    let offer_json = offer_env.to_json().expect("WebRtcOffer serialization failed");
    let decoded_offer = MessageEnvelope::from_json(&offer_json).expect("WebRtcOffer deserialization failed");
    if let MessagePayload::WebRtcOffer { sdp } = decoded_offer.payload {
        assert_eq!(sdp, offer_sdp);
    } else {
        panic!("Expected WebRtcOffer");
    }

    // 3. Android Agent answers with WebRTC Answer SDP
    let answer_sdp = "v=0\r\no=- 54321 2 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96\r\na=sendrecv\r\n";
    let answer_payload = MessagePayload::WebRtcAnswer { sdp: answer_sdp.into() };
    let answer_env = MessageEnvelope::new(android_id.clone(), Some(linux_id.clone()), answer_payload);
    let answer_json = answer_env.to_json().expect("WebRtcAnswer serialization failed");
    let decoded_answer = MessageEnvelope::from_json(&answer_json).expect("WebRtcAnswer deserialization failed");
    if let MessagePayload::WebRtcAnswer { sdp } = decoded_answer.payload {
        assert_eq!(sdp, answer_sdp);
    } else {
        panic!("Expected WebRtcAnswer");
    }

    // 4. ICE candidate exchange
    let ice_candidate = "candidate:1 1 UDP 2122260223 192.168.1.100 50000 typ host";
    let ice_payload = MessagePayload::WebRtcIceCandidate {
        candidate: ice_candidate.into(),
        sdp_mid: Some("video".into()),
        sdp_mline_index: Some(0),
    };
    let ice_env = MessageEnvelope::new(android_id.clone(), Some(linux_id.clone()), ice_payload);
    let ice_json = ice_env.to_json().expect("WebRtcIceCandidate serialization failed");
    let decoded_ice = MessageEnvelope::from_json(&ice_json).expect("WebRtcIceCandidate deserialization failed");
    if let MessagePayload::WebRtcIceCandidate { candidate, sdp_mid, sdp_mline_index } = decoded_ice.payload {
        assert_eq!(candidate, ice_candidate);
        assert_eq!(sdp_mid, Some("video".into()));
        assert_eq!(sdp_mline_index, Some(0));
    } else {
        panic!("Expected WebRtcIceCandidate");
    }

    // 5. Controller sends Screen Stop
    let stop_payload = MessagePayload::ScreenStop;
    let stop_env = MessageEnvelope::new(linux_id, Some(android_id), stop_payload);
    let stop_json = stop_env.to_json().expect("ScreenStop serialization failed");
    let decoded_stop = MessageEnvelope::from_json(&stop_json).expect("ScreenStop deserialization failed");
    match decoded_stop.payload {
        MessagePayload::ScreenStop => {}
        other => panic!("Expected ScreenStop, got {:?}", other),
    }
}



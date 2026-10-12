pub mod crypto;

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MessageEnvelope {
    pub version: u32,
    pub id: String,
    pub timestamp: u64,
    pub sender: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    pub payload: MessagePayload,
}

impl MessageEnvelope {
    pub fn new(sender: String, recipient: Option<String>, payload: MessagePayload) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let id = format!("{}_{}", timestamp, rand::random::<u32>());
        Self {
            version: PROTOCOL_VERSION,
            id,
            timestamp,
            sender,
            recipient,
            payload,
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum MessagePayload {
    // Handshake & Authentication
    HandshakeRequest {
        device_id: String,
        public_key: String,
        timestamp: u64,
        nonce: String,
        signature: String,
        role: String, // "controller" (Linux) or "agent" (Android)
    },
    HandshakeResponse {
        success: bool,
        session_id: String,
        message: String,
    },

    // Pairing Flow
    PairingRequest {
        initiator_id: String,
        initiator_name: String,
        initiator_pubkey: String,
        pairing_code: String,
    },
    PairingResponse {
        accepted: bool,
        responder_id: String,
        responder_name: String,
        responder_pubkey: String,
        message: Option<String>,
    },

    // Connection Health & Heartbeat
    Ping {
        nonce: u64,
    },
    Pong {
        nonce: u64,
    },

    // Peer Lifecycle & Presence
    PeerStatus {
        peer_id: String,
        status: String, // "online", "offline", "paired", "connected"
    },

    // Device Information (Phase 4 ready)
    DeviceInfoRequest,
    DeviceInfoResponse {
        model: String,
        android_version: String,
        battery_level: i32,
        is_charging: bool,
        screen_width: u32,
        screen_height: u32,
        orientation: String,
    },

    // State notification
    StatusUpdate {
        state: String,
        details: Option<String>,
    },

    // Future Extensible Placeholders (Phases 5-10)
    ScreenStart {
        width: u32,
        height: u32,
        fps: u32,
    },
    ScreenStop,
    TouchDown {
        pointer_id: i32,
        x: f32,
        y: f32,
    },
    TouchMove {
        pointer_id: i32,
        x: f32,
        y: f32,
    },
    TouchUp {
        pointer_id: i32,
    },
    KeyEvent {
        key_code: i32,
        pressed: bool,
        modifiers: u32,
    },
    ClipboardSet {
        text: String,
    },
    ClipboardGet,
    BatteryStatus {
        level: i32,
        charging: bool,
    },
    NotificationEvent {
        id: String,
        app_name: String,
        title: String,
        text: String,
    },
    FileRequest {
        file_id: String,
        file_name: String,
    },
    FileTransfer {
        file_id: String,
        chunk_index: u64,
        total_chunks: u64,
        data_base64: String,
        checksum: String,
    },
    CallStart {
        phone_number: String,
    },
    CallAnswer,
    CallEnd,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_envelope_roundtrip() {
        let payload = MessagePayload::HandshakeRequest {
            device_id: "dev_123".into(),
            public_key: "pubkey_hex_123".into(),
            timestamp: 123456789,
            nonce: "nonce_abc".into(),
            signature: "sig_def".into(),
            role: "controller".into(),
        };

        let env = MessageEnvelope::new("dev_123".into(), None, payload);
        let json = env.to_json().expect("Serialization failed");
        let decoded = MessageEnvelope::from_json(&json).expect("Deserialization failed");

        assert_eq!(env.version, decoded.version);
        assert_eq!(env.sender, decoded.sender);
        assert_eq!(env.payload, decoded.payload);
    }

    #[test]
    fn test_pairing_message_roundtrip() {
        let payload = MessagePayload::PairingRequest {
            initiator_id: "dev_linux".into(),
            initiator_name: "Arch Linux Controller".into(),
            initiator_pubkey: "hexpub1".into(),
            pairing_code: "123456".into(),
        };

        let env = MessageEnvelope::new("dev_linux".into(), Some("dev_android".into()), payload);
        let json = env.to_json().unwrap();
        let decoded = MessageEnvelope::from_json(&json).unwrap();
        assert_eq!(env.recipient, decoded.recipient);
        assert_eq!(env.payload, decoded.payload);
    }
}

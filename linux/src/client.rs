use crate::identity::IdentityManager;
use crate::storage::{Database, TrustedDevice};
use bridge_protocol::crypto::device_id_from_pubkey_hex;
use bridge_protocol::{MessageEnvelope, MessagePayload};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, watch, Mutex};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

use crate::webrtc::WebRtcReceiver;

#[derive(Clone, Debug, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    ConnectingToRelay,
    RelayConnectedWaitingForAndroid,
    PairingInProgress,
    AndroidConnected {
        device_id: String,
        device_name: String,
    },
    Error(String),
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub manufacturer: String,
    pub model: String,
    pub android_version: String,
    pub sdk_level: i32,
    pub battery_level: i32,
    pub is_charging: bool,
    pub screen_width: u32,
    pub screen_height: u32,
    pub orientation: String,
    pub service_status: String,
}

pub struct BridgeClient {
    identity: Arc<IdentityManager>,
    db: Arc<Database>,
    status_tx: watch::Sender<ConnectionStatus>,
    status_rx: watch::Receiver<ConnectionStatus>,
    info_tx: watch::Sender<Option<DeviceInfo>>,
    info_rx: watch::Receiver<Option<DeviceInfo>>,
    outbound_tx: mpsc::UnboundedSender<String>,
    webrtc: Arc<WebRtcReceiver>,
    ipc: Option<Arc<crate::ipc::IpcServer>>,
}

impl BridgeClient {
    pub fn new(
        identity: Arc<IdentityManager>,
        db: Arc<Database>,
        ipc: Option<Arc<crate::ipc::IpcServer>>,
    ) -> (Self, mpsc::UnboundedReceiver<String>) {
        let (status_tx, status_rx) = watch::channel(ConnectionStatus::Disconnected);
        let (info_tx, info_rx) = watch::channel(None);
        let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();
        let webrtc = Arc::new(WebRtcReceiver::new(
            identity.device_id(),
            outbound_tx.clone(),
        ));

        let client = Self {
            identity,
            db,
            status_tx,
            status_rx,
            info_tx,
            info_rx,
            outbound_tx,
            webrtc,
            ipc,
        };

        (client, outbound_rx)
    }

    pub fn status_receiver(&self) -> watch::Receiver<ConnectionStatus> {
        self.status_rx.clone()
    }

    pub fn info_receiver(&self) -> watch::Receiver<Option<DeviceInfo>> {
        self.info_rx.clone()
    }

    #[allow(dead_code)]
    pub fn current_status(&self) -> ConnectionStatus {
        self.status_rx.borrow().clone()
    }

    pub fn send_pairing_request(&self, pairing_code: String, target_id: Option<String>) {
        let payload = MessagePayload::PairingRequest {
            initiator_id: self.identity.device_id(),
            initiator_name: "Linux Desktop Controller".into(),
            initiator_pubkey: self.identity.public_key_hex(),
            pairing_code,
        };
        let envelope = MessageEnvelope::new(self.identity.device_id(), target_id, payload);
        let _ = self.status_tx.send(ConnectionStatus::PairingInProgress);
        if let Ok(json) = envelope.to_json() {
            let _ = self.outbound_tx.send(json);
        }
    }

    pub fn request_device_info(&self, target_id: Option<String>) {
        let envelope = MessageEnvelope::new(
            self.identity.device_id(),
            target_id,
            MessagePayload::DeviceInfoRequest,
        );
        if let Ok(json) = envelope.to_json() {
            let _ = self.outbound_tx.send(json);
            info!("Queued DeviceInfoRequest for peer");
        }
    }

    #[allow(dead_code)]
    pub fn send_envelope(&self, envelope: MessageEnvelope) {
        if let Ok(json) = envelope.to_json() {
            let _ = self.outbound_tx.send(json);
        }
    }

    pub fn webrtc(&self) -> Arc<WebRtcReceiver> {
        Arc::clone(&self.webrtc)
    }

    pub fn start_screen_stream(&self, target_peer_id: String) {
        let webrtc = Arc::clone(&self.webrtc);
        let outbound_tx = self.outbound_tx.clone();
        let my_id = self.identity.device_id();
        let target_clone = target_peer_id.clone();
        tokio::spawn(async move {
            let start_env = MessageEnvelope::new(
                my_id,
                Some(target_clone.clone()),
                MessagePayload::ScreenStart {
                    width: 1920,
                    height: 1080,
                    fps: 60,
                },
            );
            if let Ok(json) = start_env.to_json() {
                let _ = outbound_tx.send(json);
            }
            if let Err(e) = webrtc.start_stream_session(target_clone).await {
                error!("Failed to start WebRTC session: {}", e);
            }
        });
    }

    pub fn stop_screen_stream(&self, target_peer_id: Option<String>) {
        let webrtc = Arc::clone(&self.webrtc);
        let outbound_tx = self.outbound_tx.clone();
        let my_id = self.identity.device_id();
        tokio::spawn(async move {
            let stop_env = MessageEnvelope::new(my_id, target_peer_id, MessagePayload::ScreenStop);
            if let Ok(json) = stop_env.to_json() {
                let _ = outbound_tx.send(json);
            }
            webrtc.stop_stream().await;
        });
    }

    pub fn status_sender(&self) -> watch::Sender<ConnectionStatus> {
        self.status_tx.clone()
    }

    pub fn info_sender(&self) -> watch::Sender<Option<DeviceInfo>> {
        self.info_tx.clone()
    }

    pub async fn run_loop(
        self: Arc<Self>,
        relay_url: String,
        outbound_rx: mpsc::UnboundedReceiver<String>,
    ) {
        let identity = Arc::clone(&self.identity);
        let db = Arc::clone(&self.db);
        let status_tx = self.status_sender();
        let info_tx = self.info_sender();
        let shared_outbound_rx = Arc::new(Mutex::new(outbound_rx));
        let ipc = self.ipc.clone();

        loop {
            let _ = status_tx.send(ConnectionStatus::ConnectingToRelay);
            if let Some(ipc_ref) = &ipc {
                ipc_ref
                    .update_device_status(false, None, None, None, None)
                    .await;
            }
            info!("Connecting to relay at {}", relay_url);

            match connect_async(&relay_url).await {
                Ok((ws_stream, _)) => {
                    info!("WebSocket connection established with relay");
                    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

                    // Perform Authenticated Handshake
                    let timestamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    let nonce = format!("{:x}", rand::random::<u64>());
                    let challenge = format!("{}:{}:{}", identity.device_id(), timestamp, nonce);
                    let signature = identity.keypair().sign(challenge.as_bytes());

                    let handshake = MessageEnvelope::new(
                        identity.device_id(),
                        None,
                        MessagePayload::HandshakeRequest {
                            device_id: identity.device_id(),
                            public_key: identity.public_key_hex(),
                            timestamp,
                            nonce,
                            signature,
                            role: "controller".into(),
                        },
                    );

                    let hs_json = handshake.to_json().unwrap_or_default();
                    if let Err(e) = ws_sender.send(Message::Text(hs_json)).await {
                        warn!("Failed to send handshake: {}", e);
                        let _ =
                            status_tx.send(ConnectionStatus::Error("Handshake send failed".into()));
                        tokio::time::sleep(Duration::from_secs(3)).await;
                        continue;
                    }

                    // Await Handshake Response
                    let mut handshake_ok = false;
                    if let Some(Ok(Message::Text(res_text))) = ws_receiver.next().await {
                        if let Ok(envelope) = MessageEnvelope::from_json(&res_text) {
                            if let MessagePayload::HandshakeResponse {
                                success, message, ..
                            } = envelope.payload
                            {
                                if success {
                                    info!("Relay authenticated: {}", message);
                                    handshake_ok = true;
                                } else {
                                    error!("Handshake rejected by relay: {}", message);
                                }
                            }
                        }
                    }

                    if !handshake_ok {
                        let _ = status_tx.send(ConnectionStatus::Error(
                            "Relay authentication failed".into(),
                        ));
                        tokio::time::sleep(Duration::from_secs(3)).await;
                        continue;
                    }

                    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<Message>();

                    // Handshake OK: check if already paired with any trusted device
                    let trusted_list = db.list_trusted_devices().unwrap_or_default();
                    if let Some(primary_dev) = trusted_list.first() {
                        let _ = status_tx.send(ConnectionStatus::AndroidConnected {
                            device_id: primary_dev.device_id.clone(),
                            device_name: primary_dev.name.clone(),
                        });
                        if let Some(ipc_ref) = &ipc {
                            ipc_ref
                                .update_device_status(
                                    true,
                                    Some(primary_dev.device_id.clone()),
                                    Some(primary_dev.name.clone()),
                                    None,
                                    None,
                                )
                                .await;
                        }
                        // Automatically request DeviceInfo upon connect
                        let req = MessageEnvelope::new(
                            identity.device_id(),
                            Some(primary_dev.device_id.clone()),
                            MessagePayload::DeviceInfoRequest,
                        );
                        if let Ok(json) = req.to_json() {
                            let _ = cmd_tx.send(Message::Text(json));
                            info!(
                                "Automatically sent DeviceInfoRequest to {}",
                                primary_dev.device_id
                            );
                        }
                    } else {
                        let _ = status_tx.send(ConnectionStatus::RelayConnectedWaitingForAndroid);
                    }

                    // Spawn heartbeat task
                    let mut ping_interval = tokio::time::interval(Duration::from_secs(15));
                    let dev_id_for_ping = identity.device_id();
                    let cmd_tx_ping = cmd_tx.clone();
                    let heartbeat_task = tokio::spawn(async move {
                        loop {
                            ping_interval.tick().await;
                            let ping_env = MessageEnvelope::new(
                                dev_id_for_ping.clone(),
                                None,
                                MessagePayload::Ping {
                                    nonce: rand::random(),
                                },
                            );
                            if let Ok(json) = ping_env.to_json() {
                                if cmd_tx_ping.send(Message::Text(json)).is_err() {
                                    break;
                                }
                            }
                        }
                    });

                    // Forward outbound messages from caller
                    let cmd_tx_out = cmd_tx.clone();
                    let rx_ref = Arc::clone(&shared_outbound_rx);
                    let outbound_task = tokio::spawn(async move {
                        let mut rx = rx_ref.lock().await;
                        while let Some(msg_text) = rx.recv().await {
                            if cmd_tx_out.send(Message::Text(msg_text)).is_err() {
                                break;
                            }
                        }
                    });

                    // Loop sending messages to WS
                    let send_task = tokio::spawn(async move {
                        while let Some(msg) = cmd_rx.recv().await {
                            if let Err(e) = ws_sender.send(msg).await {
                                warn!("Error sending message to relay: {}", e);
                                break;
                            }
                        }
                    });

                    // Loop receiving messages from WS
                    while let Some(msg_res) = ws_receiver.next().await {
                        let text = match msg_res {
                            Ok(Message::Text(t)) => t,
                            Ok(Message::Ping(_)) => continue,
                            Ok(Message::Close(_)) => break,
                            Err(e) => {
                                warn!("WebSocket error: {}", e);
                                break;
                            }
                            _ => continue,
                        };

                        if let Ok(envelope) = MessageEnvelope::from_json(&text) {
                            match envelope.payload {
                                MessagePayload::PairingResponse {
                                    accepted,
                                    responder_id,
                                    responder_name,
                                    responder_pubkey,
                                    message,
                                } => {
                                    if accepted {
                                        // Validate responder ID corresponds to public key
                                        if let Ok(expected_id) =
                                            device_id_from_pubkey_hex(&responder_pubkey)
                                        {
                                            if expected_id == responder_id {
                                                let now = SystemTime::now()
                                                    .duration_since(UNIX_EPOCH)
                                                    .unwrap_or_default()
                                                    .as_millis()
                                                    as u64;

                                                let trusted = TrustedDevice {
                                                    device_id: responder_id.clone(),
                                                    name: responder_name.clone(),
                                                    public_key: responder_pubkey,
                                                    paired_at: now,
                                                    last_seen: now,
                                                };

                                                let _ = db.save_trusted_device(&trusted);
                                                info!(
                                                    "Pairing successful with {} ({})",
                                                    responder_name, responder_id
                                                );
                                                let _ = status_tx.send(
                                                    ConnectionStatus::AndroidConnected {
                                                        device_id: responder_id.clone(),
                                                        device_name: responder_name,
                                                    },
                                                );

                                                // Automatically request DeviceInfo upon new pairing
                                                let req = MessageEnvelope::new(
                                                    identity.device_id(),
                                                    Some(responder_id),
                                                    MessagePayload::DeviceInfoRequest,
                                                );
                                                if let Ok(json) = req.to_json() {
                                                    let _ = cmd_tx.send(Message::Text(json));
                                                }
                                            } else {
                                                warn!("Pairing rejected: responder ID does not match public key");
                                            }
                                        }
                                    } else {
                                        warn!("Pairing rejected by device: {:?}", message);
                                        let _ = status_tx.send(
                                            ConnectionStatus::RelayConnectedWaitingForAndroid,
                                        );
                                    }
                                }
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
                                    info!(
                                        "Received DeviceInfo from {}: {} {} (Android {} / SDK {}), Battery: {}% (charging={}), Screen: {}x{} ({})",
                                        device_id, manufacturer, model, android_version, sdk_level, battery_level, is_charging, screen_width, screen_height, orientation
                                    );
                                    let dev_info = DeviceInfo {
                                        device_id: device_id.clone(),
                                        manufacturer: manufacturer.clone(),
                                        model: model.clone(),
                                        android_version,
                                        sdk_level,
                                        battery_level,
                                        is_charging,
                                        screen_width,
                                        screen_height,
                                        orientation,
                                        service_status,
                                    };
                                    if let Some(ipc_ref) = &ipc {
                                        ipc_ref
                                            .update_device_status(
                                                true,
                                                Some(device_id),
                                                Some(format!("{} {}", manufacturer, model)),
                                                Some(battery_level),
                                                Some(is_charging),
                                            )
                                            .await;
                                    }
                                    let _ = info_tx.send(Some(dev_info));
                                }
                                MessagePayload::PeerStatus { peer_id, status } => {
                                    info!("Peer {} status: {}", peer_id, status);
                                    if status == "online" {
                                        if let Ok(Some(trusted)) = db.get_trusted_device(&peer_id) {
                                            let _ = status_tx.send(
                                                ConnectionStatus::AndroidConnected {
                                                    device_id: trusted.device_id.clone(),
                                                    device_name: trusted.name,
                                                },
                                            );
                                            // Request device info
                                            let req = MessageEnvelope::new(
                                                identity.device_id(),
                                                Some(trusted.device_id),
                                                MessagePayload::DeviceInfoRequest,
                                            );
                                            if let Ok(json) = req.to_json() {
                                                let _ = cmd_tx.send(Message::Text(json));
                                            }
                                        }
                                    }
                                }
                                MessagePayload::StatusUpdate { state, details } => {
                                    info!("Received status update: {} ({:?})", state, details);
                                    if state == "Android Connected" {
                                        let trusted = db.list_trusted_devices().unwrap_or_default();
                                        let name = trusted
                                            .first()
                                            .map(|d| d.name.clone())
                                            .unwrap_or_else(|| "Android Device".into());
                                        let dev_id = trusted
                                            .first()
                                            .map(|d| d.device_id.clone())
                                            .unwrap_or_else(|| envelope.sender.clone());
                                        let _ =
                                            status_tx.send(ConnectionStatus::AndroidConnected {
                                                device_id: dev_id.clone(),
                                                device_name: name,
                                            });
                                        // Request device info
                                        let req = MessageEnvelope::new(
                                            identity.device_id(),
                                            Some(dev_id),
                                            MessagePayload::DeviceInfoRequest,
                                        );
                                        if let Ok(json) = req.to_json() {
                                            let _ = cmd_tx.send(Message::Text(json));
                                        }
                                    }
                                }
                                MessagePayload::WebRtcOffer { sdp } => {
                                    let webrtc_c = Arc::clone(&self.webrtc);
                                    let sender = envelope.sender.clone();
                                    tokio::spawn(async move {
                                        if let Err(e) =
                                            webrtc_c.handle_remote_offer(sdp, sender).await
                                        {
                                            error!("Failed to handle WebRTC offer: {}", e);
                                        }
                                    });
                                }
                                MessagePayload::WebRtcAnswer { sdp } => {
                                    let webrtc_c = Arc::clone(&self.webrtc);
                                    tokio::spawn(async move {
                                        if let Err(e) = webrtc_c.handle_remote_answer(sdp).await {
                                            error!("Failed to handle WebRTC answer: {}", e);
                                        }
                                    });
                                }
                                MessagePayload::WebRtcIceCandidate {
                                    candidate,
                                    sdp_mid,
                                    sdp_mline_index,
                                } => {
                                    let webrtc_c = Arc::clone(&self.webrtc);
                                    tokio::spawn(async move {
                                        if let Err(e) = webrtc_c
                                            .handle_remote_candidate(
                                                candidate,
                                                sdp_mid,
                                                sdp_mline_index,
                                            )
                                            .await
                                        {
                                            error!("Failed to handle WebRTC ICE candidate: {}", e);
                                        }
                                    });
                                }
                                MessagePayload::ActivityEvent { .. } => {
                                    info!("Received ActivityEvent: {:?}", envelope.payload);
                                    if let Some(ipc_ref) = &ipc {
                                        ipc_ref.broadcast_payload(&envelope.payload);
                                    }
                                }
                                MessagePayload::ActivityRemoved { .. } => {
                                    info!("Received ActivityRemoved: {:?}", envelope.payload);
                                    if let Some(ipc_ref) = &ipc {
                                        ipc_ref.broadcast_payload(&envelope.payload);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    heartbeat_task.abort();
                    outbound_task.abort();
                    send_task.abort();
                }
                Err(e) => {
                    warn!("Connection failed: {}. Retrying in 3 seconds...", e);
                    let _ =
                        status_tx.send(ConnectionStatus::Error(format!("Connection error: {}", e)));
                }
            }

            let _ = status_tx.send(ConnectionStatus::Disconnected);
            if let Some(ipc_ref) = &ipc {
                ipc_ref
                    .update_device_status(false, None, None, None, None)
                    .await;
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }
}

use bridge_protocol::{MessageEnvelope, MessagePayload};
use openh264::formats::YUVSource;
use rtp::codecs::h264::H264Packet;
use rtp::packetizer::Depacketizer;
use std::sync::Arc;
use tokio::sync::{mpsc, watch, Mutex};
use tracing::{debug, error, info, warn};
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_H264, MIME_TYPE_OPUS, MIME_TYPE_VP8};
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::{
    RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType,
};
use webrtc::rtp_transceiver::rtp_transceiver_direction::RTCRtpTransceiverDirection;
use webrtc::rtp_transceiver::RTCPFeedback;
use webrtc::rtp_transceiver::RTCRtpTransceiverInit;

#[derive(Clone, Debug, PartialEq)]
pub enum WebRtcState {
    Disconnected,
    Connecting,
    ConnectedDirectP2P,
    ConnectedRelayFallback,
    Reconnecting,
    Failed(String),
}

#[derive(Clone, Debug, Default)]
pub struct StreamStats {
    pub packets_received: u64,
    pub bytes_received: u64,
    pub estimated_bitrate_kbps: u32,
    pub estimated_fps: u32,
    pub width: u32,
    pub height: u32,
    pub is_landscape: bool,
}

#[derive(Clone)]
pub struct DecodedFrame {
    pub width: u32,
    pub height: u32,
    pub bgra_data: Arc<Vec<u8>>,
}

impl std::fmt::Debug for DecodedFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecodedFrame")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("bgra_bytes", &self.bgra_data.len())
            .finish()
    }
}

pub struct WebRtcReceiver {
    my_device_id: String,
    target_peer_id: Mutex<Option<String>>,
    peer_connection: Arc<Mutex<Option<Arc<RTCPeerConnection>>>>,
    state_tx: watch::Sender<WebRtcState>,
    state_rx: watch::Receiver<WebRtcState>,
    stats_tx: watch::Sender<StreamStats>,
    stats_rx: watch::Receiver<StreamStats>,
    frame_tx: watch::Sender<Option<Arc<DecodedFrame>>>,
    frame_rx: watch::Receiver<Option<Arc<DecodedFrame>>>,
    outbound_tx: mpsc::UnboundedSender<String>,
}

fn create_media_engine() -> Result<MediaEngine, webrtc::Error> {
    let mut media_engine = MediaEngine::default();
    let video_rtcp_feedback = vec![
        RTCPFeedback {
            typ: "goog-remb".to_owned(),
            parameter: "".to_owned(),
        },
        RTCPFeedback {
            typ: "ccm".to_owned(),
            parameter: "fir".to_owned(),
        },
        RTCPFeedback {
            typ: "nack".to_owned(),
            parameter: "".to_owned(),
        },
        RTCPFeedback {
            typ: "nack".to_owned(),
            parameter: "pli".to_owned(),
        },
    ];

    // Register H264 first so that SDP Offer / Answer prioritizes H264
    for (pt, fmtp) in [
        (
            102,
            "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f",
        ),
        (
            125,
            "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42001f",
        ),
        (
            127,
            "level-asymmetry-allowed=1;packetization-mode=0;profile-level-id=42e01f",
        ),
        (
            126,
            "level-asymmetry-allowed=1;packetization-mode=0;profile-level-id=42001f",
        ),
    ] {
        media_engine.register_codec(
            RTCRtpCodecParameters {
                capability: RTCRtpCodecCapability {
                    mime_type: MIME_TYPE_H264.to_owned(),
                    clock_rate: 90000,
                    channels: 0,
                    sdp_fmtp_line: fmtp.to_owned(),
                    rtcp_feedback: video_rtcp_feedback.clone(),
                },
                payload_type: pt,
                ..Default::default()
            },
            RTPCodecType::Video,
        )?;
    }

    // Register VP8 as secondary fallback
    media_engine.register_codec(
        RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_VP8.to_owned(),
                clock_rate: 90000,
                channels: 0,
                sdp_fmtp_line: "".to_owned(),
                rtcp_feedback: video_rtcp_feedback.clone(),
            },
            payload_type: 96,
            ..Default::default()
        },
        RTPCodecType::Video,
    )?;

    // Register Opus audio
    media_engine.register_codec(
        RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_OPUS.to_owned(),
                clock_rate: 48000,
                channels: 2,
                sdp_fmtp_line: "minptime=10;useinbandfec=1".to_owned(),
                rtcp_feedback: vec![],
            },
            payload_type: 111,
            ..Default::default()
        },
        RTPCodecType::Audio,
    )?;

    Ok(media_engine)
}

fn setup_track_handler(
    pc: Arc<RTCPeerConnection>,
    stats_tx: watch::Sender<StreamStats>,
    frame_tx: watch::Sender<Option<Arc<DecodedFrame>>>,
) {
    let pc_weak = Arc::downgrade(&pc);

    pc.on_track(Box::new(move |track, _, _| {
        let stats_tx = stats_tx.clone();
        let frame_tx = frame_tx.clone();
        let pc_weak = pc_weak.clone();

        Box::pin(async move {
            let codec = track.codec();
            let mime = codec.capability.mime_type.to_lowercase();
            let pt = track.payload_type();
            let ssrc = track.ssrc();
            let track_id = track.id();
            let kind = track.kind();

            info!(
                "Incoming WebRTC track: {} (id: {}, ssrc: {}, payload_type: {}, mime: {}, clock_rate: {}, fmtp: '{}')",
                kind, track_id, ssrc, pt, codec.capability.mime_type, codec.capability.clock_rate, codec.capability.sdp_fmtp_line
            );

            if kind != RTPCodecType::Video {
                info!("Ignoring non-video WebRTC track: {}", kind);
                return;
            }

            if !mime.contains("h264") {
                warn!("Received video track with non-H264 codec: '{}' (payload_type: {})", mime, pt);
            }

            // Send initial RTCP PLI and continuous REMB (10 Mbps) + periodic PLI so Android encoder operates in real-time
            if let Some(pc) = pc_weak.upgrade() {
                let pc_for_rtcp = Arc::clone(&pc);
                tokio::spawn(async move {
                    let mut count = 0u32;
                    loop {
                        count += 1;
                        let mut packets: Vec<Box<dyn webrtc::rtcp::packet::Packet + Send + Sync>> = Vec::new();

                        // Send PLI on first 3 attempts to establish baseline IDR
                        if count <= 3 {
                            info!(
                                "Sending RTCP Picture Loss Indication (PLI) for video SSRC {} (attempt {})",
                                ssrc, count
                            );
                            packets.push(Box::new(
                                webrtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication {
                                    sender_ssrc: 0,
                                    media_ssrc: ssrc,
                                },
                            ));
                        }

                        // Send REMB (8 Mbps) continuously to unlock Android WebRTC bandwidth estimator
                        packets.push(Box::new(
                            webrtc::rtcp::payload_feedbacks::receiver_estimated_maximum_bitrate::ReceiverEstimatedMaximumBitrate {
                                sender_ssrc: 0,
                                bitrate: 8_000_000.0,
                                ssrcs: vec![ssrc],
                            },
                        ));

                        if let Err(e) = pc_for_rtcp.write_rtcp(&packets).await {
                            debug!("RTCP write loop ended: {:?}", e);
                            break;
                        }
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    }
                });
            }

            let pc_for_decode_pli = pc_weak.clone();
            tokio::spawn(async move {
                let mut depacketizer = H264Packet::default();
                let mut decoder = match openh264::decoder::Decoder::new() {
                    Ok(dec) => dec,
                    Err(e) => {
                        error!("Failed to initialize OpenH264 decoder: {:?}", e);
                        return;
                    }
                };

                let mut pkt_count = 0u64;
                let mut byte_count = 0u64;
                let mut frame_count = 0u64;
                let mut last_fps_instant = tokio::time::Instant::now();
                let mut frames_this_second = 0u32;
                let mut last_dimensions = (0usize, 0usize);
                let mut last_seq: Option<u16> = None;
                let mut last_pli_instant = tokio::time::Instant::now();
                let mut consecutive_decode_errors = 0u32;

                info!("Started RTP depacketization and OpenH264 decoding loop for video track");

                while let Ok((rtp_packet, _)) = track.read_rtp().await {
                    pkt_count += 1;
                    let payload_len = rtp_packet.payload.len();
                    byte_count += payload_len as u64;

                    let seq = rtp_packet.header.sequence_number;
                    if let Some(prev) = last_seq {
                        if seq != prev.wrapping_add(1) {
                            // Packet loss detected! Discard partial FU-A buffer and request fresh IDR
                            depacketizer = H264Packet::default();
                            if last_pli_instant.elapsed() >= tokio::time::Duration::from_millis(300) {
                                last_pli_instant = tokio::time::Instant::now();
                                if let Some(pc) = pc_for_decode_pli.upgrade() {
                                    let pli: Box<dyn webrtc::rtcp::packet::Packet + Send + Sync> = Box::new(
                                        webrtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication {
                                            sender_ssrc: 0,
                                            media_ssrc: ssrc,
                                        },
                                    );
                                    let _ = pc.write_rtcp(&[pli]).await;
                                    info!("RTP packet gap (seq {} -> {}). Sent RTCP PLI keyframe request", prev, seq);
                                }
                            }
                        }
                    }
                    last_seq = Some(seq);

                    if pkt_count <= 5 || pkt_count % 180 == 0 {
                        info!(
                            "RTP packet #{} received: payload {} bytes, seq={}, pt={}, ssrc={}",
                            pkt_count,
                            payload_len,
                            seq,
                            rtp_packet.header.payload_type,
                            rtp_packet.header.ssrc
                        );
                    }

                    match depacketizer.depacketize(&rtp_packet.payload) {
                        Ok(nal_unit) => {
                            if !nal_unit.is_empty() {
                                match decoder.decode(&nal_unit) {
                                    Ok(Some(yuv)) => {
                                        consecutive_decode_errors = 0;
                                        let (w, h) = yuv.dimensions();
                                        frame_count += 1;
                                        frames_this_second += 1;

                                        if last_dimensions != (w, h)
                                            || frame_count <= 5
                                            || frame_count % 180 == 0
                                        {
                                            info!(
                                                "Decoded video frame #{}: {}x{} (YUV strides: {:?}, payload: {} bytes)",
                                                frame_count,
                                                w,
                                                h,
                                                yuv.strides(),
                                                payload_len
                                            );
                                            last_dimensions = (w, h);
                                        }

                                        let mut bgra = vec![0u8; w * h * 4];
                                        yuv.write_rgba8(&mut bgra);
                                        for pixel in bgra.chunks_exact_mut(4) {
                                            pixel.swap(0, 2); // RGBA -> BGRA for Cairo
                                        }

                                        let frame = Arc::new(DecodedFrame {
                                            width: w as u32,
                                            height: h as u32,
                                            bgra_data: Arc::new(bgra),
                                        });

                                        let _ = frame_tx.send(Some(frame));
                                    }
                                    Ok(None) => {
                                        // Intermediate NAL unit (e.g. SPS/PPS header or slice fragment)
                                    }
                                    Err(e) => {
                                        consecutive_decode_errors += 1;
                                        depacketizer = H264Packet::default();
                                        if consecutive_decode_errors >= 5 {
                                            if let Ok(new_dec) = openh264::decoder::Decoder::new() {
                                                decoder = new_dec;
                                                consecutive_decode_errors = 0;
                                            }
                                        }
                                        if last_pli_instant.elapsed() >= tokio::time::Duration::from_millis(300) {
                                            last_pli_instant = tokio::time::Instant::now();
                                            if let Some(pc) = pc_for_decode_pli.upgrade() {
                                                let pli: Box<dyn webrtc::rtcp::packet::Packet + Send + Sync> = Box::new(
                                                    webrtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication {
                                                        sender_ssrc: 0,
                                                        media_ssrc: ssrc,
                                                    },
                                                );
                                                let _ = pc.write_rtcp(&[pli]).await;
                                                warn!("OpenH264 decode error: {:?}. Sent RTCP PLI request for SSRC {}", e, ssrc);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            depacketizer = H264Packet::default();
                            if last_pli_instant.elapsed() >= tokio::time::Duration::from_millis(300) {
                                last_pli_instant = tokio::time::Instant::now();
                                if let Some(pc) = pc_for_decode_pli.upgrade() {
                                    let pli: Box<dyn webrtc::rtcp::packet::Packet + Send + Sync> = Box::new(
                                        webrtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication {
                                            sender_ssrc: 0,
                                            media_ssrc: ssrc,
                                        },
                                    );
                                    let _ = pc.write_rtcp(&[pli]).await;
                                    warn!("Depacketize error: {:?}. Sent RTCP PLI request", e);
                                }
                            }
                        }
                    }

                    if last_fps_instant.elapsed() >= tokio::time::Duration::from_secs(1) {
                        let kbps = ((byte_count * 8) / 1000) as u32;
                        let fps = frames_this_second;
                        frames_this_second = 0;
                        byte_count = 0;
                        last_fps_instant = tokio::time::Instant::now();

                        let (w, h) = if last_dimensions.0 > 0 {
                            (last_dimensions.0 as u32, last_dimensions.1 as u32)
                        } else {
                            (1920, 1080)
                        };

                        let _ = stats_tx.send(StreamStats {
                            packets_received: pkt_count,
                            bytes_received: byte_count,
                            estimated_bitrate_kbps: kbps,
                            estimated_fps: fps,
                            width: w,
                            height: h,
                            is_landscape: w >= h,
                        });
                    }
                }

                info!("WebRTC track read loop ended (track closed or disconnected)");
            });
        })
    }));
}

impl WebRtcReceiver {
    pub fn new(my_device_id: String, outbound_tx: mpsc::UnboundedSender<String>) -> Self {
        let (state_tx, state_rx) = watch::channel(WebRtcState::Disconnected);
        let (stats_tx, stats_rx) = watch::channel(StreamStats {
            width: 1920,
            height: 1080,
            is_landscape: true,
            ..Default::default()
        });
        let (frame_tx, frame_rx) = watch::channel(None);

        Self {
            my_device_id,
            target_peer_id: Mutex::new(None),
            peer_connection: Arc::new(Mutex::new(None)),
            state_tx,
            state_rx,
            stats_tx,
            stats_rx,
            frame_tx,
            frame_rx,
            outbound_tx,
        }
    }

    pub fn state_receiver(&self) -> watch::Receiver<WebRtcState> {
        self.state_rx.clone()
    }

    pub fn stats_receiver(&self) -> watch::Receiver<StreamStats> {
        self.stats_rx.clone()
    }

    pub fn frame_receiver(&self) -> watch::Receiver<Option<Arc<DecodedFrame>>> {
        self.frame_rx.clone()
    }

    pub async fn start_stream_session(
        &self,
        target_peer_id: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        *self.target_peer_id.lock().await = Some(target_peer_id.clone());
        let _ = self.state_tx.send(WebRtcState::Connecting);

        // 1. Setup Media Engine with H264 prioritization
        let media_engine = create_media_engine()?;

        let api = APIBuilder::new().with_media_engine(media_engine).build();

        // 2. Setup ICE configuration with STUN servers
        let rtc_config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec![
                    "stun:stun.l.google.com:19302".to_string(),
                    "stun:stun1.l.google.com:19302".to_string(),
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        let pc = Arc::new(api.new_peer_connection(rtc_config).await?);

        // 3. Add RecvOnly Video Transceiver for receiving screen stream
        let transceiver_init = RTCRtpTransceiverInit {
            direction: RTCRtpTransceiverDirection::Recvonly,
            send_encodings: vec![],
        };
        pc.add_transceiver_from_kind(RTPCodecType::Video, Some(transceiver_init))
            .await?;

        // 4. Handle ICE candidates generated by Linux
        let outbound_tx = self.outbound_tx.clone();
        let my_id = self.my_device_id.clone();
        let target_id = target_peer_id.clone();

        pc.on_ice_candidate(Box::new(move |candidate| {
            let outbound_tx = outbound_tx.clone();
            let my_id = my_id.clone();
            let target_id = target_id.clone();
            Box::pin(async move {
                if let Some(cand) = candidate {
                    if let Ok(json_cand) = cand.to_json() {
                        let payload = MessagePayload::WebRtcIceCandidate {
                            candidate: json_cand.candidate,
                            sdp_mid: json_cand.sdp_mid,
                            sdp_mline_index: json_cand.sdp_mline_index.map(|i| i as u32),
                        };
                        let env = MessageEnvelope::new(my_id, Some(target_id), payload);
                        if let Ok(json) = env.to_json() {
                            let _ = outbound_tx.send(json);
                        }
                    }
                }
            })
        }));

        // 5. Handle connection state changes
        let state_tx = self.state_tx.clone();
        pc.on_peer_connection_state_change(Box::new(move |state| {
            let state_tx = state_tx.clone();
            Box::pin(async move {
                info!("WebRTC PeerConnection state changed: {:?}", state);
                match state {
                    RTCPeerConnectionState::Connected => {
                        let _ = state_tx.send(WebRtcState::ConnectedDirectP2P);
                    }
                    RTCPeerConnectionState::Disconnected => {
                        let _ = state_tx.send(WebRtcState::Reconnecting);
                    }
                    RTCPeerConnectionState::Failed => {
                        let _ = state_tx.send(WebRtcState::Failed("P2P media failed".into()));
                    }
                    RTCPeerConnectionState::Closed => {
                        let _ = state_tx.send(WebRtcState::Disconnected);
                    }
                    _ => {}
                }
            })
        }));

        // 6. Handle incoming video track and decode frames
        setup_track_handler(pc.clone(), self.stats_tx.clone(), self.frame_tx.clone());

        // 7. Create and send Offer to Android Agent
        let offer = pc.create_offer(None).await?;
        pc.set_local_description(offer.clone()).await?;

        let envelope = MessageEnvelope::new(
            self.my_device_id.clone(),
            Some(target_peer_id),
            MessagePayload::WebRtcOffer { sdp: offer.sdp },
        );
        if let Ok(json) = envelope.to_json() {
            let _ = self.outbound_tx.send(json);
            info!("Sent WebRtcOffer to Android Agent");
        }

        *self.peer_connection.lock().await = Some(pc);
        Ok(())
    }

    pub async fn handle_remote_answer(
        &self,
        answer_sdp: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let pc_guard = self.peer_connection.lock().await;
        if let Some(pc) = pc_guard.as_ref() {
            let desc = RTCSessionDescription::answer(answer_sdp)?;
            pc.set_remote_description(desc).await?;
            info!("WebRTC remote answer applied successfully");
        } else {
            warn!("Received WebRtcAnswer but PeerConnection is not active");
        }
        Ok(())
    }

    pub async fn handle_remote_offer(
        &self,
        offer_sdp: String,
        sender_id: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        *self.target_peer_id.lock().await = Some(sender_id.clone());
        let _ = self.state_tx.send(WebRtcState::Connecting);

        let media_engine = create_media_engine()?;

        let api = APIBuilder::new().with_media_engine(media_engine).build();

        let rtc_config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec![
                    "stun:stun.l.google.com:19302".to_string(),
                    "stun:stun1.l.google.com:19302".to_string(),
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        let pc = Arc::new(api.new_peer_connection(rtc_config).await?);

        // Track listener
        setup_track_handler(pc.clone(), self.stats_tx.clone(), self.frame_tx.clone());

        let desc = RTCSessionDescription::offer(offer_sdp)?;
        pc.set_remote_description(desc).await?;

        let answer = pc.create_answer(None).await?;
        pc.set_local_description(answer.clone()).await?;

        let envelope = MessageEnvelope::new(
            self.my_device_id.clone(),
            Some(sender_id),
            MessagePayload::WebRtcAnswer { sdp: answer.sdp },
        );
        if let Ok(json) = envelope.to_json() {
            let _ = self.outbound_tx.send(json);
            info!("Sent WebRtcAnswer back to Android Agent");
        }

        *self.peer_connection.lock().await = Some(pc);
        Ok(())
    }

    pub async fn handle_remote_candidate(
        &self,
        candidate_str: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u32>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let pc_guard = self.peer_connection.lock().await;
        if let Some(pc) = pc_guard.as_ref() {
            let candidate_init = RTCIceCandidateInit {
                candidate: candidate_str,
                sdp_mid,
                sdp_mline_index: sdp_mline_index.map(|i| i as u16),
                username_fragment: None,
            };
            pc.add_ice_candidate(candidate_init).await?;
        }
        Ok(())
    }

    pub async fn stop_stream(&self) {
        let mut pc_guard = self.peer_connection.lock().await;
        if let Some(pc) = pc_guard.take() {
            let _ = pc.close().await;
            info!("WebRTC connection closed");
        }
        let _ = self.state_tx.send(WebRtcState::Disconnected);
        let _ = self.frame_tx.send(None);
    }
}

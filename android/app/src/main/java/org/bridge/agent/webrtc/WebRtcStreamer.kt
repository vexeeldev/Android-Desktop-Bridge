package org.bridge.agent.webrtc

import android.content.Context
import android.content.Intent
import android.util.Log
import org.bridge.agent.network.RelayClient
import org.bridge.agent.protocol.MessageEnvelope
import org.bridge.agent.protocol.WebRtcAnswerData
import org.bridge.agent.protocol.WebRtcIceCandidateData
import org.webrtc.*
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

class WebRtcStreamer(
    private val context: Context,
    private val relayClient: RelayClient,
    private val myDeviceId: String,
    private var peerDeviceId: String? = null,
    private val listener: StreamStateListener? = null
) {
    interface StreamStateListener {
        fun onConnectionStateChanged(state: String)
        fun onError(message: String)
    }

    private val executor = Executors.newSingleThreadExecutor()
    private val rootEglBase: EglBase = EglBase.create()
    private var peerConnectionFactory: PeerConnectionFactory? = null
    private var peerConnection: PeerConnection? = null
    private var videoSource: VideoSource? = null
    private var videoTrack: VideoTrack? = null
    private var captureManager: ScreenCaptureManager? = null
    private val isStreaming = AtomicBoolean(false)
    private val pendingRemoteCandidates = mutableListOf<IceCandidate>()
    @Volatile
    private var hasRemoteDescription = false

    init {
        initializeFactory()
    }

    private fun initializeFactory() {
        val options = PeerConnectionFactory.InitializationOptions.builder(context)
            .setEnableInternalTracer(false)
            .createInitializationOptions()
        PeerConnectionFactory.initialize(options)

        val encoderFactory = DefaultVideoEncoderFactory(
            rootEglBase.eglBaseContext,
            /* enableIntelVp8 = */ true,
            /* enableH264HighProfile = */ false
        )
        val decoderFactory = DefaultVideoDecoderFactory(rootEglBase.eglBaseContext)

        peerConnectionFactory = PeerConnectionFactory.builder()
            .setVideoEncoderFactory(encoderFactory)
            .setVideoDecoderFactory(decoderFactory)
            .setOptions(PeerConnectionFactory.Options())
            .createPeerConnectionFactory()
    }

    fun startScreenStreaming(captureIntentData: Intent, targetPeerId: String) {
        if (isStreaming.get()) return
        peerDeviceId = targetPeerId

        executor.execute {
            try {
                val factory = peerConnectionFactory ?: return@execute
                val iceServers = listOf(
                    PeerConnection.IceServer.builder("stun:stun.l.google.com:19302").createIceServer(),
                    PeerConnection.IceServer.builder("stun:stun1.l.google.com:19302").createIceServer()
                )

                val rtcConfig = PeerConnection.RTCConfiguration(iceServers).apply {
                    sdpSemantics = PeerConnection.SdpSemantics.UNIFIED_PLAN
                    continualGatheringPolicy = PeerConnection.ContinualGatheringPolicy.GATHER_ONCE
                    bundlePolicy = PeerConnection.BundlePolicy.MAXBUNDLE
                    rtcpMuxPolicy = PeerConnection.RtcpMuxPolicy.REQUIRE
                    tcpCandidatePolicy = PeerConnection.TcpCandidatePolicy.DISABLED
                    enableDscp = false
                }

                peerConnection = factory.createPeerConnection(rtcConfig, object : PeerConnection.Observer {
                    override fun onSignalingChange(state: PeerConnection.SignalingState) {
                        Log.i(TAG, "WebRTC SignalingState changed: $state")
                    }

                    override fun onIceConnectionChange(state: PeerConnection.IceConnectionState) {
                        Log.i(TAG, "WebRTC IceConnectionState changed: $state")
                        if (state == PeerConnection.IceConnectionState.FAILED || state == PeerConnection.IceConnectionState.DISCONNECTED) {
                            Log.w(TAG, "WebRTC ICE connection degraded/failed: $state (streamingActive=${isStreaming.get()})")
                        }
                        listener?.onConnectionStateChanged(state.name)
                    }

                    override fun onConnectionChange(newState: PeerConnection.PeerConnectionState) {
                        Log.i(TAG, "WebRTC PeerConnection state changed: $newState")
                        listener?.onConnectionStateChanged(newState.name)
                    }

                    override fun onIceConnectionReceivingChange(receiving: Boolean) {
                        Log.d(TAG, "WebRTC IceConnectionReceiving changed: receiving=$receiving")
                    }

                    override fun onIceGatheringChange(state: PeerConnection.IceGatheringState) {
                        Log.i(TAG, "WebRTC IceGatheringState changed: $state")
                    }

                    override fun onIceCandidate(candidate: IceCandidate) {
                        val target = peerDeviceId ?: return
                        Log.i(TAG, "WebRTC local ICE candidate gathered: mid=${candidate.sdpMid}, line=${candidate.sdpMLineIndex}, ${candidate.sdp.trim()}")
                        val candidateData = WebRtcIceCandidateData(
                            candidate = candidate.sdp,
                            sdp_mid = candidate.sdpMid,
                            sdp_mline_index = candidate.sdpMLineIndex
                        )
                        val env = MessageEnvelope.create(
                            sender = myDeviceId,
                            recipient = target,
                            type = "WebRtcIceCandidate",
                            data = candidateData
                        )
                        relayClient.sendEnvelope(env)
                    }

                    override fun onIceCandidatesRemoved(candidates: Array<out IceCandidate>?) {
                        Log.d(TAG, "WebRTC local ICE candidates removed: count=${candidates?.size ?: 0}")
                    }
                    override fun onAddStream(stream: MediaStream?) {}
                    override fun onRemoveStream(stream: MediaStream?) {}
                    override fun onDataChannel(dataChannel: DataChannel?) {}
                    override fun onRenegotiationNeeded() {
                        Log.d(TAG, "WebRTC onRenegotiationNeeded()")
                    }
                    override fun onAddTrack(receiver: RtpReceiver?, mediaStreams: Array<out MediaStream>?) {}
                })

                // Create Video Track from ScreenCaptureManager
                val capturerManager = ScreenCaptureManager(context, rootEglBase, captureIntentData)
                captureManager = capturerManager

                val vSource = factory.createVideoSource(/* isScreencast = */ true)
                videoSource = vSource
                capturerManager.startCapture(vSource, targetFps = 30)

                val vTrack = factory.createVideoTrack("ARDAMSv0", vSource)
                videoTrack = vTrack
                vTrack.setEnabled(true)

                val sender = peerConnection?.addTrack(vTrack, listOf("ARDAMS"))
                applySenderBitrate(sender)

                isStreaming.set(true)
                Log.i(TAG, "WebRTC Screen Streaming initialized and capturing @ 30 FPS")
            } catch (e: Exception) {
                Log.e(TAG, "Failed to start WebRTC streaming", e)
                listener?.onError("Failed to start WebRTC streaming: ${e.message}")
            }
        }
    }

    private fun applySenderBitrate(sender: RtpSender?) {
        try {
            val params = sender?.parameters ?: return
            var modified = false
            for (encoding in params.encodings) {
                encoding.minBitrateBps = 1_000_000 // 1.0 Mbps min
                encoding.maxBitrateBps = 8_000_000 // 8 Mbps max (ample headroom for swipe bursts)
                encoding.maxFramerate = 30
                modified = true
            }
            if (modified) {
                sender.parameters = params
                Log.i(TAG, "Applied RtpSender bitrate constraints: 1.0-8.0 Mbps @ 30 FPS")
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to apply sender bitrate parameters", e)
        }
    }

    private fun setBitrateInSdp(sdp: String, bitrateKbps: Int): String {
        val lines = sdp.split("\r\n").toMutableList()
        var lineIndex = -1
        for (i in lines.indices) {
            if (lines[i].startsWith("m=video")) {
                lineIndex = i
                break
            }
        }
        if (lineIndex == -1) return sdp

        var hasBitrate = false
        for (i in lineIndex + 1 until lines.size) {
            if (lines[i].startsWith("m=")) break
            if (lines[i].startsWith("b=AS:")) {
                lines[i] = "b=AS:$bitrateKbps"
                hasBitrate = true
                break
            }
        }
        if (!hasBitrate) {
            lines.add(lineIndex + 1, "b=AS:$bitrateKbps")
        }
        return lines.joinToString("\r\n")
    }

    fun handleRemoteOffer(offerSdp: String, senderId: String) {
        peerDeviceId = senderId
        executor.execute {
            val pc = peerConnection ?: run {
                Log.w(TAG, "Received offer before PeerConnection initialized")
                return@execute
            }

            Log.i(TAG, "Setting remote description (OFFER)...")
            val sessionDescription = SessionDescription(SessionDescription.Type.OFFER, offerSdp)
            pc.setRemoteDescription(object : SdpObserverAdapter() {
                override fun onSetSuccess() {
                    Log.i(TAG, "Remote offer set successfully. Flushing ${pendingRemoteCandidates.size} queued ICE candidates...")
                    hasRemoteDescription = true
                    for (candidate in pendingRemoteCandidates) {
                        val added = pc.addIceCandidate(candidate)
                        Log.d(TAG, "Added queued ICE candidate: ${candidate.sdpMid} (success=$added)")
                    }
                    pendingRemoteCandidates.clear()
                    createAnswer()
                }

                override fun onSetFailure(error: String?) {
                    Log.e(TAG, "Failed to set remote offer: $error")
                    listener?.onError("Failed to set remote offer: $error")
                }
            }, sessionDescription)
        }
    }

    private fun createAnswer() {
        val pc = peerConnection ?: return
        val constraints = MediaConstraints().apply {
            mandatory.add(MediaConstraints.KeyValuePair("OfferToReceiveAudio", "false"))
            mandatory.add(MediaConstraints.KeyValuePair("OfferToReceiveVideo", "false"))
        }

        pc.createAnswer(object : SdpObserverAdapter() {
            override fun onCreateSuccess(desc: SessionDescription) {
                Log.i(TAG, "Created WebRTC answer successfully. Injecting 8 Mbps SDP bitrate...")
                val modifiedSdp = setBitrateInSdp(desc.description, 8000)
                val modifiedDesc = SessionDescription(desc.type, modifiedSdp)
                pc.setLocalDescription(object : SdpObserverAdapter() {
                    override fun onSetSuccess() {
                        Log.i(TAG, "Local description set successfully. Sending WebRtcAnswer to peer.")
                        for (sender in pc.senders) {
                            if (sender.track()?.kind() == "video") {
                                applySenderBitrate(sender)
                            }
                        }
                        val target = peerDeviceId ?: return
                        val answerData = WebRtcAnswerData(sdp = modifiedDesc.description)
                        val env = MessageEnvelope.create(
                            sender = myDeviceId,
                            recipient = target,
                            type = "WebRtcAnswer",
                            data = answerData
                        )
                        relayClient.sendEnvelope(env)
                    }

                    override fun onSetFailure(error: String?) {
                        Log.e(TAG, "Failed to set local answer: $error")
                    }
                }, modifiedDesc)
            }

            override fun onCreateFailure(error: String?) {
                Log.e(TAG, "Failed to create answer: $error")
            }
        }, constraints)
    }

    fun handleRemoteCandidate(candidateSdp: String, sdpMid: String?, sdpMLineIndex: Int?) {
        executor.execute {
            val candidate = IceCandidate(
                sdpMid ?: "0",
                sdpMLineIndex ?: 0,
                candidateSdp
            )
            val pc = peerConnection
            if (pc != null && hasRemoteDescription) {
                val success = pc.addIceCandidate(candidate)
                Log.d(TAG, "Added remote ICE candidate immediately: $sdpMid (success=$success)")
            } else {
                Log.d(TAG, "Queued remote ICE candidate (hasRemoteDesc=$hasRemoteDescription, pc=${pc != null}): $sdpMid")
                pendingRemoteCandidates.add(candidate)
            }
        }
    }

    fun stopStreaming() {
        if (!isStreaming.getAndSet(false)) return
        executor.execute {
            try {
                hasRemoteDescription = false
                pendingRemoteCandidates.clear()

                captureManager?.stopCapture()
                captureManager = null

                videoTrack?.setEnabled(false)
                videoTrack?.dispose()
                videoTrack = null

                videoSource?.dispose()
                videoSource = null

                peerConnection?.close()
                peerConnection?.dispose()
                peerConnection = null

                Log.i(TAG, "WebRTC Screen Streaming stopped and resources disposed")
            } catch (e: Exception) {
                Log.e(TAG, "Error stopping streaming", e)
            }
        }
    }

    fun isStreaming(): Boolean = isStreaming.get()

    open class SdpObserverAdapter : SdpObserver {
        override fun onCreateSuccess(desc: SessionDescription) {}
        override fun onSetSuccess() {}
        override fun onCreateFailure(error: String?) {}
        override fun onSetFailure(error: String?) {}
    }

    companion object {
        private const val TAG = "WebRtcStreamer"
    }
}

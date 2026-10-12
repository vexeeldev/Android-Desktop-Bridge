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
            /* enableH264HighProfile = */ true
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
                    continualGatheringPolicy = PeerConnection.ContinualGatheringPolicy.GATHER_CONTINUALLY
                    enableDscp = true
                }

                peerConnection = factory.createPeerConnection(rtcConfig, object : PeerConnection.Observer {
                    override fun onSignalingChange(state: PeerConnection.SignalingState) {
                        Log.d(TAG, "SignalingState: $state")
                    }

                    override fun onIceConnectionChange(state: PeerConnection.IceConnectionState) {
                        Log.i(TAG, "ICE connection state changed: $state")
                        listener?.onConnectionStateChanged(state.name)
                    }

                    override fun onConnectionChange(newState: PeerConnection.PeerConnectionState) {
                        Log.i(TAG, "PeerConnection state changed: $newState")
                        listener?.onConnectionStateChanged(newState.name)
                    }

                    override fun onIceConnectionReceivingChange(receiving: Boolean) {}

                    override fun onIceGatheringChange(state: PeerConnection.IceGatheringState) {
                        Log.d(TAG, "IceGatheringState: $state")
                    }

                    override fun onIceCandidate(candidate: IceCandidate) {
                        val target = peerDeviceId ?: return
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
                        Log.d(TAG, "Sent ICE candidate to $target: ${candidate.sdpMid}")
                    }

                    override fun onIceCandidatesRemoved(candidates: Array<out IceCandidate>?) {}
                    override fun onAddStream(stream: MediaStream?) {}
                    override fun onRemoveStream(stream: MediaStream?) {}
                    override fun onDataChannel(dataChannel: DataChannel?) {}
                    override fun onRenegotiationNeeded() {}
                    override fun onAddTrack(receiver: RtpReceiver?, mediaStreams: Array<out MediaStream>?) {}
                })

                // Create Video Track from ScreenCaptureManager
                val capturerManager = ScreenCaptureManager(context, rootEglBase, captureIntentData)
                captureManager = capturerManager
                capturerManager.createVideoCapturer()

                val vSource = factory.createVideoSource(/* isScreencast = */ true)
                videoSource = vSource
                capturerManager.startCapture(vSource, targetFps = 60)

                val vTrack = factory.createVideoTrack("ARDAMSv0", vSource)
                videoTrack = vTrack
                vTrack.setEnabled(true)

                peerConnection?.addTrack(vTrack, listOf("ARDAMS"))

                isStreaming.set(true)
                Log.i(TAG, "WebRTC Screen Streaming initialized and capturing")
            } catch (e: Exception) {
                Log.e(TAG, "Failed to start WebRTC streaming", e)
                listener?.onError("Failed to start WebRTC streaming: ${e.message}")
            }
        }
    }

    fun handleRemoteOffer(offerSdp: String, senderId: String) {
        peerDeviceId = senderId
        executor.execute {
            val pc = peerConnection ?: run {
                Log.w(TAG, "Received offer before PeerConnection initialized")
                return@execute
            }

            val sessionDescription = SessionDescription(SessionDescription.Type.OFFER, offerSdp)
            pc.setRemoteDescription(object : SdpObserverAdapter() {
                override fun onSetSuccess() {
                    Log.i(TAG, "Remote offer set successfully. Creating answer...")
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
                Log.i(TAG, "Created WebRTC answer successfully")
                pc.setLocalDescription(object : SdpObserverAdapter() {
                    override fun onSetSuccess() {
                        Log.i(TAG, "Local description set successfully. Sending WebRtcAnswer to peer.")
                        val target = peerDeviceId ?: return
                        val answerData = WebRtcAnswerData(sdp = desc.description)
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
                }, desc)
            }

            override fun onCreateFailure(error: String?) {
                Log.e(TAG, "Failed to create answer: $error")
            }
        }, constraints)
    }

    fun handleRemoteCandidate(candidateSdp: String, sdpMid: String?, sdpMLineIndex: Int?) {
        executor.execute {
            val pc = peerConnection ?: return@execute
            val candidate = IceCandidate(
                sdpMid ?: "0",
                sdpMLineIndex ?: 0,
                candidateSdp
            )
            pc.addIceCandidate(candidate)
            Log.d(TAG, "Added remote ICE candidate: $sdpMid")
        }
    }

    fun stopStreaming() {
        if (!isStreaming.getAndSet(false)) return
        executor.execute {
            try {
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

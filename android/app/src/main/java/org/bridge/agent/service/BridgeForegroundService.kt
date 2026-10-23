package org.bridge.agent.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import android.util.Log
import androidx.core.app.NotificationCompat
import com.google.gson.Gson
import org.bridge.agent.crypto.KeyStoreManager
import org.bridge.agent.network.RelayClient
import org.bridge.agent.protocol.ActivityEventData
import org.bridge.agent.protocol.ActivityRemovedData
import org.bridge.agent.protocol.MessageEnvelope
import org.bridge.agent.protocol.WebRtcIceCandidateData
import org.bridge.agent.protocol.WebRtcOfferData
import org.bridge.agent.ui.MainActivity
import org.bridge.agent.webrtc.WebRtcStreamer

class BridgeForegroundService : Service(), RelayClient.ConnectionListener, WebRtcStreamer.StreamStateListener {

    private var wakeLock: PowerManager.WakeLock? = null
    private var wifiLock: WifiManager.WifiLock? = null
    private var relayClient: RelayClient? = null
    private lateinit var keyStoreManager: KeyStoreManager
    private var webRtcStreamer: WebRtcStreamer? = null
    private var currentPeerId: String? = null
    private var isStreamingActive = false
    private val gson = Gson()

    override fun onCreate() {
        super.onCreate()
        instance = this
        keyStoreManager = KeyStoreManager(this)

        val powerManager = getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock = powerManager.newWakeLock(
            PowerManager.PARTIAL_WAKE_LOCK,
            "BridgeAgent::ServiceWakeLock"
        ).apply {
            setReferenceCounted(false)
            acquire(24 * 60 * 60 * 1000L) // 24 hours max
            Log.i(TAG, "Acquired WakeLock for foreground service")
        }

        val wifiManager = applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager
        wifiLock = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            wifiManager?.createWifiLock(WifiManager.WIFI_MODE_FULL_LOW_LATENCY, "BridgeAgent::WifiLock")
        } else {
            @Suppress("DEPRECATION")
            wifiManager?.createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "BridgeAgent::WifiLock")
        }?.apply {
            setReferenceCounted(false)
            acquire()
            Log.i(TAG, "Acquired WifiLock in low-latency/high-perf mode")
        }

        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val action = intent?.action
        if (action == ACTION_STOP) {
            stopSelf()
            return START_NOT_STICKY
        }

        val relayUrl = intent?.getStringExtra(EXTRA_RELAY_URL) ?: "ws://10.0.2.2:8080"
        updateForegroundNotification("Initializing connection...")

        val identity = keyStoreManager.getOrCreateIdentity()
        if (relayClient == null) {
            val client = RelayClient(this, relayUrl, identity, keyStoreManager, this)
            relayClient = client
            client.start()

            webRtcStreamer = WebRtcStreamer(
                context = this,
                relayClient = client,
                myDeviceId = identity.deviceId,
                peerDeviceId = currentPeerId,
                listener = this
            )
        }

        if (action == ACTION_START_STREAM) {
            val captureIntent = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                intent.getParcelableExtra(EXTRA_CAPTURE_INTENT_DATA, Intent::class.java)
            } else {
                @Suppress("DEPRECATION")
                intent.getParcelableExtra(EXTRA_CAPTURE_INTENT_DATA)
            }

            val peerId = intent.getStringExtra(EXTRA_PEER_ID)
                ?: currentPeerId
                ?: keyStoreManager.getLastTrustedControllerId()
                ?: "controller"

            currentPeerId = peerId
            if (captureIntent != null) {
                Log.i(TAG, "Starting WebRTC screen capture with target peer: $peerId")
                isStreamingActive = true
                // CRITICAL: Elevate foreground service to MEDIA_PROJECTION *before* acquiring projection
                updateForegroundNotification("Streaming screen to controller")
                webRtcStreamer?.startScreenStreaming(captureIntent, peerId)
                broadcastStreamState("STREAMING_ACTIVE")
            }
        } else if (action == ACTION_STOP_STREAM) {
            Log.i(TAG, "Stopping WebRTC screen capture")
            isStreamingActive = false
            webRtcStreamer?.stopStreaming()
            updateForegroundNotification("Connected to controller")
            broadcastStreamState("STREAMING_STOPPED")
        }

        return START_STICKY
    }

    private fun updateForegroundNotification(statusText: String) {
        val notification = buildNotification(statusText)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            var serviceType = ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE
            if (isStreamingActive) {
                serviceType = serviceType or ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION
            }
            try {
                startForeground(NOTIFICATION_ID, notification, serviceType)
                Log.i(TAG, "startForeground called with serviceType=$serviceType (streamingActive=$isStreamingActive)")
            } catch (e: Exception) {
                Log.e(TAG, "Failed startForeground with type $serviceType, falling back to basic", e)
                startForeground(NOTIFICATION_ID, notification)
            }
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    override fun onStateChanged(state: RelayClient.State, details: String) {
        val statusText = when (state) {
            RelayClient.State.DISCONNECTED -> "Disconnected"
            RelayClient.State.CONNECTING -> "Connecting to relay..."
            RelayClient.State.RELAY_CONNECTED -> "Relay connected (waiting for controller)"
            RelayClient.State.PAIRED_AND_CONNECTED -> if (isStreamingActive) "Android Connected (Streaming)" else "Android Connected"
            RelayClient.State.ERROR -> "Error: $details"
        }

        val notification = buildNotification(statusText)
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        manager.notify(NOTIFICATION_ID, notification)

        // Broadcast to Activity
        val broadcast = Intent(ACTION_STATUS_CHANGED).apply {
            putExtra(EXTRA_STATE, state.name)
            putExtra(EXTRA_DETAILS, details)
        }
        sendBroadcast(broadcast)
    }

    override fun onMessageReceived(envelope: MessageEnvelope) {
        try {
            val type = envelope.payload.get("type")?.asString ?: return
            val data = envelope.payload.get("data")
            currentPeerId = envelope.sender

            when (type) {
                "WebRtcOffer" -> {
                    val offer = gson.fromJson(data, WebRtcOfferData::class.java)
                    Log.i(TAG, "Received WebRtcOffer from ${envelope.sender}")
                    webRtcStreamer?.handleRemoteOffer(offer.sdp, envelope.sender)
                }
                "WebRtcIceCandidate" -> {
                    val candidate = gson.fromJson(data, WebRtcIceCandidateData::class.java)
                    Log.d(TAG, "Received WebRtcIceCandidate from ${envelope.sender}")
                    webRtcStreamer?.handleRemoteCandidate(
                        candidate.candidate,
                        candidate.sdp_mid,
                        candidate.sdp_mline_index
                    )
                }
                "ScreenStop" -> {
                    Log.i(TAG, "Received ScreenStop request from ${envelope.sender}")
                    isStreamingActive = false
                    webRtcStreamer?.stopStreaming()
                    updateForegroundNotification("Connected to controller")
                    broadcastStreamState("STREAMING_STOPPED")
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error processing incoming message envelope", e)
        }
    }

    override fun onConnectionStateChanged(state: String) {
        Log.i(TAG, "WebRTC connection state: $state")
        if (state == "CLOSED" || state == "FAILED") {
            if (isStreamingActive) {
                Log.w(TAG, "WebRTC connection state reached $state while isStreamingActive=true")
            }
        }
        broadcastStreamState(state)
    }

    override fun onError(message: String) {
        Log.e(TAG, "WebRTC streamer error: $message")
        broadcastStreamState("ERROR: $message")
    }

    private fun broadcastStreamState(state: String) {
        val intent = Intent(ACTION_STREAM_STATE_CHANGED).apply {
            putExtra(EXTRA_STREAM_STATE, state)
        }
        sendBroadcast(intent)
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "Desktop Bridge Service",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Keeps connection open with Linux Desktop controller"
            }
            val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            manager.createNotificationChannel(channel)
        }
    }

    private fun buildNotification(statusText: String): Notification {
        val pendingIntent = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("Android Desktop Bridge")
            .setContentText(statusText)
            .setSmallIcon(android.R.drawable.stat_notify_sync)
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .build()
    }

    override fun onDestroy() {
        super.onDestroy()
        webRtcStreamer?.stopStreaming()
        webRtcStreamer = null
        relayClient?.stop()
        relayClient = null
        try {
            if (wakeLock?.isHeld == true) {
                wakeLock?.release()
                Log.i(TAG, "Released WakeLock")
            }
        } catch (_: Exception) {}
        try {
            if (wifiLock?.isHeld == true) {
                wifiLock?.release()
                Log.i(TAG, "Released WifiLock")
            }
        } catch (_: Exception) {}

        val broadcast = Intent(ACTION_STATUS_CHANGED).apply {
            putExtra(EXTRA_STATE, RelayClient.State.DISCONNECTED.name)
            putExtra(EXTRA_DETAILS, "Service stopped")
        }
        sendBroadcast(broadcast)
        if (instance == this) {
            instance = null
        }
    }

    fun sendActivityEvent(eventData: ActivityEventData): Boolean {
        val client = relayClient ?: return false
        val identity = keyStoreManager.getOrCreateIdentity()
        val peerId = currentPeerId ?: keyStoreManager.getLastTrustedControllerId()
        val envelope = MessageEnvelope.create(
            sender = identity.deviceId,
            recipient = peerId,
            type = "ActivityEvent",
            data = eventData
        )
        val sent = client.sendEnvelope(envelope)
        Log.i(TAG, "Sent ActivityEvent [${eventData.event_id}] app=${eventData.app_name} title=${eventData.title} -> success=$sent")
        return sent
    }

    fun sendActivityRemoved(removedData: ActivityRemovedData): Boolean {
        val client = relayClient ?: return false
        val identity = keyStoreManager.getOrCreateIdentity()
        val peerId = currentPeerId ?: keyStoreManager.getLastTrustedControllerId()
        val envelope = MessageEnvelope.create(
            sender = identity.deviceId,
            recipient = peerId,
            type = "ActivityRemoved",
            data = removedData
        )
        val sent = client.sendEnvelope(envelope)
        Log.i(TAG, "Sent ActivityRemoved [${removedData.event_id}] -> success=$sent")
        return sent
    }

    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        private const val TAG = "BridgeService"
        const val CHANNEL_ID = "bridge_foreground_channel"
        const val NOTIFICATION_ID = 1001
        const val EXTRA_RELAY_URL = "extra_relay_url"
        const val ACTION_STOP = "org.bridge.agent.STOP_SERVICE"
        const val ACTION_STATUS_CHANGED = "org.bridge.agent.STATUS_CHANGED"
        const val EXTRA_STATE = "extra_state"
        const val EXTRA_DETAILS = "extra_details"

        const val ACTION_START_STREAM = "org.bridge.agent.START_STREAM"
        const val ACTION_STOP_STREAM = "org.bridge.agent.STOP_STREAM"
        const val EXTRA_CAPTURE_INTENT_DATA = "extra_capture_intent_data"
        const val EXTRA_PEER_ID = "extra_peer_id"

        const val ACTION_STREAM_STATE_CHANGED = "org.bridge.agent.STREAM_STATE_CHANGED"
        const val EXTRA_STREAM_STATE = "extra_stream_state"

        @Volatile
        var instance: BridgeForegroundService? = null
            private set

        fun sendEvent(eventData: ActivityEventData): Boolean {
            return instance?.sendActivityEvent(eventData) ?: false
        }

        fun sendRemoved(removedData: ActivityRemovedData): Boolean {
            return instance?.sendActivityRemoved(removedData) ?: false
        }
    }
}


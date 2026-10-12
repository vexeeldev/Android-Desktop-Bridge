package org.bridge.agent.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import androidx.core.app.NotificationCompat
import org.bridge.agent.crypto.KeyStoreManager
import org.bridge.agent.network.RelayClient
import org.bridge.agent.protocol.MessageEnvelope
import org.bridge.agent.ui.MainActivity

class BridgeForegroundService : Service(), RelayClient.ConnectionListener {

    private var wakeLock: PowerManager.WakeLock? = null
    private var relayClient: RelayClient? = null
    private lateinit var keyStoreManager: KeyStoreManager

    override fun onCreate() {
        super.onCreate()
        keyStoreManager = KeyStoreManager(this)

        val powerManager = getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock = powerManager.newWakeLock(
            PowerManager.PARTIAL_WAKE_LOCK,
            "BridgeAgent::ServiceWakeLock"
        ).apply {
            setReferenceCounted(false)
            acquire(24 * 60 * 60 * 1000L) // 24 hours max
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
        val notification = buildNotification("Initializing connection...")
        startForeground(NOTIFICATION_ID, notification)

        val identity = keyStoreManager.getOrCreateIdentity()
        relayClient?.stop()
        relayClient = RelayClient(relayUrl, identity, keyStoreManager, this).apply {
            start()
        }

        return START_STICKY
    }

    override fun onStateChanged(state: RelayClient.State, details: String) {
        val statusText = when (state) {
            RelayClient.State.DISCONNECTED -> "Disconnected"
            RelayClient.State.CONNECTING -> "Connecting to relay..."
            RelayClient.State.RELAY_CONNECTED -> "Relay connected (waiting for controller)"
            RelayClient.State.PAIRED_AND_CONNECTED -> "Android Connected"
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
        // Reserved for future event dispatching
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
        relayClient?.stop()
        relayClient = null
        try {
            if (wakeLock?.isHeld == true) {
                wakeLock?.release()
            }
        } catch (_: Exception) {}

        val broadcast = Intent(ACTION_STATUS_CHANGED).apply {
            putExtra(EXTRA_STATE, RelayClient.State.DISCONNECTED.name)
            putExtra(EXTRA_DETAILS, "Service stopped")
        }
        sendBroadcast(broadcast)
    }

    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        const val CHANNEL_ID = "bridge_foreground_channel"
        const val NOTIFICATION_ID = 1001
        const val EXTRA_RELAY_URL = "extra_relay_url"
        const val ACTION_STOP = "org.bridge.agent.STOP_SERVICE"
        const val ACTION_STATUS_CHANGED = "org.bridge.agent.STATUS_CHANGED"
        const val EXTRA_STATE = "extra_state"
        const val EXTRA_DETAILS = "extra_details"
    }
}

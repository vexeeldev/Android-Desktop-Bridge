package org.bridge.agent.service

import android.app.Notification
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.provider.Settings
import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification
import android.util.Log
import androidx.core.app.NotificationManagerCompat
import org.bridge.agent.protocol.ActivityEventData
import org.bridge.agent.protocol.ActivityRemovedData

class BridgeNotificationListenerService : NotificationListenerService() {

    override fun onListenerConnected() {
        super.onListenerConnected()
        Log.i(TAG, "BridgeNotificationListenerService connected and active")
    }

    override fun onListenerDisconnected() {
        super.onListenerDisconnected()
        Log.w(TAG, "BridgeNotificationListenerService disconnected")
    }

    override fun onNotificationPosted(sbn: StatusBarNotification?) {
        if (sbn == null) return

        val packageName = sbn.packageName ?: return
        // Ignore notifications originating from our own agent service
        if (packageName == applicationContext.packageName) {
            return
        }

        val notification = sbn.notification ?: return
        val extras = notification.extras ?: return

        // Extract title
        val title = extras.getCharSequence(Notification.EXTRA_TITLE)?.toString()
            ?: extras.getString(Notification.EXTRA_TITLE)
            ?: ""

        // Extract body / text
        val text = extras.getCharSequence(Notification.EXTRA_TEXT)?.toString()
            ?: extras.getCharSequence(Notification.EXTRA_BIG_TEXT)?.toString()
            ?: extras.getString(Notification.EXTRA_TEXT)
            ?: ""

        // Skip completely empty notifications (e.g. background keepalives)
        if (title.isBlank() && text.isBlank()) {
            return
        }

        // Resolve human-readable App Name
        val appName = try {
            val pm = packageManager
            val appInfo = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                pm.getApplicationInfo(packageName, PackageManager.ApplicationInfoFlags.of(0))
            } else {
                @Suppress("DEPRECATION")
                pm.getApplicationInfo(packageName, 0)
            }
            pm.getApplicationLabel(appInfo).toString()
        } catch (_: Exception) {
            packageName
        }

        // Determine category
        val category = determineCategory(packageName, notification)

        val eventId = sbn.key ?: "${packageName}_${sbn.id}"
        val timestamp = sbn.postTime.takeIf { it > 0 } ?: System.currentTimeMillis()

        val isOngoing = sbn.isOngoing
        val isClearable = sbn.isClearable
        val canReply = notification.actions?.any { action ->
            action.remoteInputs?.any { it.allowFreeFormInput } == true
        } ?: false

        val event = ActivityEventData(
            event_id = eventId,
            category = category,
            package_name = packageName,
            app_name = appName,
            title = title,
            body = text,
            timestamp = timestamp,
            unread = true,
            icon = null,
            conversation_id = null,
            person_name = title.takeIf { category == "MESSAGE" },
            is_ongoing = isOngoing,
            is_clearable = isClearable,
            can_reply = canReply
        )

        Log.d(TAG, "Notification posted: [$category] $appName: $title - $text")
        BridgeForegroundService.sendEvent(event)
    }

    override fun onNotificationRemoved(sbn: StatusBarNotification?) {
        if (sbn == null) return
        val packageName = sbn.packageName ?: return
        if (packageName == applicationContext.packageName) return

        val eventId = sbn.key ?: "${packageName}_${sbn.id}"
        val removed = ActivityRemovedData(event_id = eventId)

        Log.d(TAG, "Notification removed: $eventId")
        BridgeForegroundService.sendRemoved(removed)
    }

    private fun determineCategory(packageName: String, notification: Notification): String {
        val notifCategory = notification.category

        if (notifCategory == Notification.CATEGORY_CALL) {
            return "CALL"
        }
        if (notifCategory == Notification.CATEGORY_MESSAGE) {
            return "MESSAGE"
        }
        if (notifCategory == Notification.CATEGORY_EMAIL) {
            return "EMAIL"
        }

        val pkgLower = packageName.lowercase()
        return when {
            pkgLower.contains("whatsapp") ||
            pkgLower.contains("telegram") ||
            pkgLower.contains("signal") ||
            pkgLower.contains("discord") ||
            pkgLower.contains("messaging") ||
            pkgLower.contains("mms") ||
            pkgLower.contains("im") -> "MESSAGE"

            pkgLower.contains("dialer") ||
            pkgLower.contains("phone") ||
            pkgLower.contains("call") -> "CALL"

            pkgLower.contains("mail") ||
            pkgLower.contains("gmail") ||
            pkgLower.contains("outlook") -> "EMAIL"

            pkgLower.contains("school") ||
            pkgLower.contains("classroom") ||
            pkgLower.contains("canvas") ||
            pkgLower.contains("moodle") -> "SCHOOL"

            else -> "NOTIFICATION"
        }
    }

    companion object {
        private const val TAG = "BridgeNotifListener"

        fun isNotificationAccessGranted(context: Context): Boolean {
            val enabledListeners = NotificationManagerCompat.getEnabledListenerPackages(context)
            return enabledListeners.contains(context.packageName)
        }
    }
}

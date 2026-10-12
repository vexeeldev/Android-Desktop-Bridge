package org.bridge.agent.info

import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.res.Configuration
import android.os.BatteryManager
import android.os.Build
import android.util.DisplayMetrics
import android.view.WindowManager
import org.bridge.agent.crypto.DeviceIdentity
import org.bridge.agent.protocol.DeviceInfoResponseData

object DeviceInfoCollector {

    fun collectDeviceInfo(
        context: Context,
        identity: DeviceIdentity,
        serviceStatus: String = "running"
    ): DeviceInfoResponseData {
        // 1. Manufacturer, Model, OS, SDK
        val manufacturer = Build.MANUFACTURER ?: "Unknown"
        val model = Build.MODEL ?: "Unknown"
        val androidVersion = Build.VERSION.RELEASE ?: "Unknown"
        val sdkLevel = Build.VERSION.SDK_INT

        // 2. Battery & Charging Status
        val (batteryLevel, isCharging) = getBatteryInfo(context)

        // 3. Screen Dimensions & Orientation
        val (width, height, orientation) = getScreenInfo(context)

        return DeviceInfoResponseData(
            device_id = identity.deviceId,
            manufacturer = manufacturer,
            model = model,
            android_version = androidVersion,
            sdk_level = sdkLevel,
            battery_level = batteryLevel,
            is_charging = isCharging,
            screen_width = width,
            screen_height = height,
            orientation = orientation,
            service_status = serviceStatus
        )
    }

    private fun getBatteryInfo(context: Context): Pair<Int, Boolean> {
        return try {
            val batteryManager = context.getSystemService(Context.BATTERY_SERVICE) as? BatteryManager
            val capacity = batteryManager?.getIntProperty(BatteryManager.BATTERY_PROPERTY_CAPACITY) ?: -1

            val filter = IntentFilter(Intent.ACTION_BATTERY_CHANGED)
            val batteryIntent = context.registerReceiver(null, filter)

            val level = if (capacity in 0..100) {
                capacity
            } else {
                val rawLevel = batteryIntent?.getIntExtra(BatteryManager.EXTRA_LEVEL, -1) ?: -1
                val scale = batteryIntent?.getIntExtra(BatteryManager.EXTRA_SCALE, -1) ?: -1
                if (rawLevel >= 0 && scale > 0) {
                    (rawLevel * 100) / scale
                } else {
                    -1
                }
            }

            val status = batteryIntent?.getIntExtra(BatteryManager.EXTRA_STATUS, -1) ?: -1
            val isCharging = status == BatteryManager.BATTERY_STATUS_CHARGING ||
                    status == BatteryManager.BATTERY_STATUS_FULL

            Pair(level, isCharging)
        } catch (_: Exception) {
            Pair(-1, false)
        }
    }

    private fun getScreenInfo(context: Context): Triple<Int, Int, String> {
        return try {
            val displayMetrics = context.resources.displayMetrics
            val width = displayMetrics.widthPixels
            val height = displayMetrics.heightPixels

            val orientationStr = if (context.resources.configuration.orientation == Configuration.ORIENTATION_LANDSCAPE) {
                "landscape"
            } else {
                "portrait"
            }

            Triple(width, height, orientationStr)
        } catch (_: Exception) {
            Triple(1080, 2400, "portrait")
        }
    }
}

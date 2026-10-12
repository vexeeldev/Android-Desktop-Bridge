package org.bridge.agent.webrtc

import android.content.Context
import android.content.Intent
import android.hardware.display.DisplayManager
import android.media.projection.MediaProjection
import android.os.Handler
import android.os.Looper
import android.util.DisplayMetrics
import android.util.Log
import android.view.WindowManager
import org.webrtc.EglBase
import org.webrtc.ScreenCapturerAndroid
import org.webrtc.SurfaceTextureHelper
import org.webrtc.VideoCapturer
import org.webrtc.VideoSource
import java.util.concurrent.atomic.AtomicBoolean

class ScreenCaptureManager(
    private val context: Context,
    private val rootEglBase: EglBase,
    private val captureIntentData: Intent
) {
    private var screenCapturer: ScreenCapturerAndroid? = null
    private var surfaceTextureHelper: SurfaceTextureHelper? = null
    private val isCapturing = AtomicBoolean(false)
    private var displayListener: DisplayManager.DisplayListener? = null

    private var currentWidth = 1920
    private var currentHeight = 1080
    private var currentFps = 60

    fun createVideoCapturer(): VideoCapturer {
        val capturer = ScreenCapturerAndroid(
            captureIntentData,
            object : MediaProjection.Callback() {
                override fun onStop() {
                    Log.i(TAG, "MediaProjection stopped by system")
                    stopCapture()
                }
            }
        )
        screenCapturer = capturer
        return capturer
    }

    fun startCapture(videoSource: VideoSource, targetFps: Int = 60) {
        if (isCapturing.get()) return

        val wm = context.getSystemService(Context.WINDOW_SERVICE) as WindowManager
        val metrics = DisplayMetrics()
        @Suppress("DEPRECATION")
        wm.defaultDisplay.getRealMetrics(metrics)

        // Default presentation: Landscape orientation handling
        val screenWidth = metrics.widthPixels
        val screenHeight = metrics.heightPixels

        // Adaptive scaling: limit to 1920 max dimension for optimal low latency
        val maxDim = 1920
        var width = screenWidth
        var height = screenHeight

        if (width > maxDim || height > maxDim) {
            val scale = maxDim.toFloat() / maxOf(width, height)
            width = ((width * scale).toInt() / 2) * 2 // ensure even dimensions
            height = ((height * scale).toInt() / 2) * 2
        }

        currentWidth = width
        currentHeight = height
        currentFps = targetFps

        surfaceTextureHelper = SurfaceTextureHelper.create("ScreenCaptureThread", rootEglBase.eglBaseContext)
        screenCapturer?.initialize(surfaceTextureHelper, context, videoSource.capturerObserver)
        screenCapturer?.startCapture(currentWidth, currentHeight, currentFps)
        isCapturing.set(true)
        Log.i(TAG, "Started screen capture at ${currentWidth}x${currentHeight} @ ${currentFps}fps")

        // Register display rotation listener
        registerDisplayListener()
    }

    private fun registerDisplayListener() {
        val dm = context.getSystemService(Context.DISPLAY_SERVICE) as DisplayManager
        val listener = object : DisplayManager.DisplayListener {
            override fun onDisplayAdded(displayId: Int) {}
            override fun onDisplayRemoved(displayId: Int) {}
            override fun onDisplayChanged(displayId: Int) {
                if (!isCapturing.get()) return
                val wm = context.getSystemService(Context.WINDOW_SERVICE) as WindowManager
                val metrics = DisplayMetrics()
                @Suppress("DEPRECATION")
                wm.defaultDisplay.getRealMetrics(metrics)

                val newWidth = ((metrics.widthPixels / 2) * 2)
                val newHeight = ((metrics.heightPixels / 2) * 2)

                if (newWidth != currentWidth || newHeight != currentHeight) {
                    currentWidth = newWidth
                    currentHeight = newHeight
                    Log.i(TAG, "Display rotated or resized: changing capture format to ${currentWidth}x${currentHeight}")
                    screenCapturer?.changeCaptureFormat(currentWidth, currentHeight, currentFps)
                }
            }
        }
        displayListener = listener
        dm.registerDisplayListener(listener, Handler(Looper.getMainLooper()))
    }

    fun stopCapture() {
        if (!isCapturing.getAndSet(false)) return
        try {
            displayListener?.let {
                val dm = context.getSystemService(Context.DISPLAY_SERVICE) as DisplayManager
                dm.unregisterDisplayListener(it)
                displayListener = null
            }
            screenCapturer?.stopCapture()
            screenCapturer?.dispose()
            surfaceTextureHelper?.dispose()
            Log.i(TAG, "Screen capture stopped successfully")
        } catch (e: Exception) {
            Log.e(TAG, "Error stopping screen capture", e)
        }
    }

    companion object {
        private const val TAG = "ScreenCaptureManager"
    }
}

package org.bridge.agent.webrtc

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.hardware.display.DisplayManager
import android.hardware.display.VirtualDisplay
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.Handler
import android.os.Looper
import android.util.DisplayMetrics
import android.util.Log
import android.view.Display
import android.view.Surface
import android.view.WindowManager
import org.webrtc.CapturerObserver
import org.webrtc.EglBase
import org.webrtc.SurfaceTextureHelper
import org.webrtc.ThreadUtils
import org.webrtc.VideoFrame
import org.webrtc.VideoSink
import org.webrtc.VideoSource
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong

/**
 * ScreenCaptureManager manages full-device display capture via Android MediaProjection
 * and WebRTC SurfaceTextureHelper.
 *
 * Ensures:
 * 1. VirtualDisplay is created with VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR | VIRTUAL_DISPLAY_FLAG_PUBLIC
 *    to mirror the entire device display across all apps and the Home screen.
 * 2. MediaProjection lifecycle and callbacks are tracked with full diagnostics.
 * 3. SurfaceTextureHelper listens on a dedicated background OpenGL thread and feeds
 *    video frames directly to WebRTC's VideoSource capturerObserver.
 * 4. Display orientation changes are tracked and handled cleanly without dropping capture.
 */
class ScreenCaptureManager(
    private val context: Context,
    private val rootEglBase: EglBase,
    private val captureIntentData: Intent
) : VideoSink {

    private var mediaProjection: MediaProjection? = null
    private var virtualDisplay: VirtualDisplay? = null
    private var surface: Surface? = null
    private var surfaceTextureHelper: SurfaceTextureHelper? = null
    private var capturerObserver: CapturerObserver? = null

    private val isCapturing = AtomicBoolean(false)
    private var displayListener: DisplayManager.DisplayListener? = null
    private val numCapturedFrames = AtomicLong(0)

    var currentWidth = 1920
        private set
    var currentHeight = 1080
        private set
    var currentFps = 60
        private set
    var currentDensityDpi = 400
        private set

    private var lastFrameDeliveredTimestampNs = 0L

    fun startCapture(videoSource: VideoSource, targetFps: Int = 30) {
        if (isCapturing.get()) {
            Log.w(TAG, "startCapture called but capture is already active")
            return
        }

        val wm = context.getSystemService(Context.WINDOW_SERVICE) as WindowManager
        val metrics = DisplayMetrics()
        @Suppress("DEPRECATION")
        wm.defaultDisplay.getRealMetrics(metrics)

        val screenWidth = metrics.widthPixels
        val screenHeight = metrics.heightPixels
        currentDensityDpi = metrics.densityDpi

        // Real-time Low-Latency Scaling:
        // Budget SoCs (e.g. MediaTek Helio G35) hardware encoder cannot keep up with
        // 720x1600 @ 30 FPS without accumulating internal buffer lag on swipes.
        // Capping max dimension to 1200 downscales 720x1600 to 540x1200 (exact 9:20 aspect ratio),
        // cutting pixel processing by ~44% and keeping encode queue delay near 0ms.
        val maxDim = 1200
        var width = screenWidth
        var height = screenHeight

        if (width > maxDim || height > maxDim) {
            val scale = maxDim.toFloat() / maxOf(width, height)
            width = ((width * scale).toInt() / 2) * 2 // ensure even dimensions
            height = ((height * scale).toInt() / 2) * 2
        } else {
            width = (width / 2) * 2
            height = (height / 2) * 2
        }

        currentWidth = width
        currentHeight = height
        currentFps = targetFps
        capturerObserver = videoSource.capturerObserver

        Log.i(
            TAG,
            "Initializing ScreenCaptureManager: physical screen=${screenWidth}x${screenHeight}, " +
                "capture target=${currentWidth}x${currentHeight} @ ${currentFps}fps, dpi=${currentDensityDpi}"
        )

        val helper = SurfaceTextureHelper.create("ScreenCaptureThread", rootEglBase.eglBaseContext)
        surfaceTextureHelper = helper
        val handler = helper.handler

        val mpm = context.getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
        Log.i(TAG, "Acquiring MediaProjection instance from MediaProjectionManager...")

        val mp = mpm.getMediaProjection(Activity.RESULT_OK, captureIntentData)
        if (mp == null) {
            Log.e(TAG, "MediaProjectionManager.getMediaProjection returned null! Capture permission may be expired or invalid.")
            capturerObserver?.onCapturerStopped()
            helper.dispose()
            surfaceTextureHelper = null
            return
        }
        mediaProjection = mp

        mp.registerCallback(object : MediaProjection.Callback() {
            override fun onStop() {
                Log.w(TAG, "MediaProjection.Callback: onStop() invoked by Android system (capture session revoked or ended)")
                stopCapture()
            }
        }, handler)

        // Create VirtualDisplay on the helper thread to ensure surface attachment
        ThreadUtils.invokeAtFrontUninterruptibly(handler) {
            createVirtualDisplayLocked()
            helper.startListening(this)
        }

        capturerObserver?.onCapturerStarted(true)
        isCapturing.set(true)
        Log.i(TAG, "Screen capture successfully started and listening for surface frames")

        registerDisplayListener()
    }

    private fun createVirtualDisplayLocked() {
        val helper = surfaceTextureHelper ?: run {
            Log.e(TAG, "Cannot create VirtualDisplay: surfaceTextureHelper is null")
            return
        }
        val mp = mediaProjection ?: run {
            Log.e(TAG, "Cannot create VirtualDisplay: mediaProjection is null")
            return
        }

        helper.setTextureSize(currentWidth, currentHeight)
        val surf = Surface(helper.surfaceTexture)
        surface = surf

        val flags = DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR or
            DisplayManager.VIRTUAL_DISPLAY_FLAG_PUBLIC

        Log.i(
            TAG,
            "Creating VirtualDisplay: name='Bridge_ScreenCapture', size=${currentWidth}x${currentHeight}, " +
                "dpi=${currentDensityDpi}, flags=AUTO_MIRROR|PUBLIC ($flags)"
        )

        virtualDisplay = mp.createVirtualDisplay(
            "Bridge_ScreenCapture",
            currentWidth,
            currentHeight,
            currentDensityDpi,
            flags,
            surf,
            object : VirtualDisplay.Callback() {
                override fun onPaused() {
                    Log.w(TAG, "VirtualDisplay.Callback: onPaused() - display mirroring paused by system")
                }

                override fun onResumed() {
                    Log.i(TAG, "VirtualDisplay.Callback: onResumed() - display mirroring resumed")
                }

                override fun onStopped() {
                    Log.w(TAG, "VirtualDisplay.Callback: onStopped() - display mirroring stopped")
                }
            },
            helper.handler
        )

        Log.i(TAG, "VirtualDisplay created: display=${virtualDisplay?.display?.displayId}")
    }

    override fun onFrame(frame: VideoFrame) {
        val now = System.nanoTime()
        val minIntervalNs = 1_000_000_000L / (currentFps + 2)
        if (lastFrameDeliveredTimestampNs != 0L && (now - lastFrameDeliveredTimestampNs) < minIntervalNs) {
            // Drop excessive display refresh bursts to protect hardware encoder queue
            return
        }
        lastFrameDeliveredTimestampNs = now

        val count = numCapturedFrames.incrementAndGet()
        if (count <= 5L || count % 90L == 0L) {
            Log.i(
                TAG,
                "VirtualDisplay frame #$count delivered: ${frame.rotatedWidth}x${frame.rotatedHeight}, " +
                    "rotation=${frame.rotation}, timestampNs=${frame.timestampNs}"
            )
        }
        capturerObserver?.onFrameCaptured(frame)
    }

    fun changeCaptureFormat(newWidth: Int, newHeight: Int, newFps: Int) {
        if (!isCapturing.get()) return
        currentWidth = (newWidth / 2) * 2
        currentHeight = (newHeight / 2) * 2
        currentFps = newFps
        val helper = surfaceTextureHelper ?: return

        ThreadUtils.invokeAtFrontUninterruptibly(helper.handler) {
            try {
                Log.i(TAG, "Recreating VirtualDisplay for format change: ${currentWidth}x${currentHeight} @ ${currentFps}fps")
                virtualDisplay?.release()
                virtualDisplay = null
                surface?.release()
                surface = null
                createVirtualDisplayLocked()
            } catch (e: Exception) {
                Log.e(TAG, "Error changing VirtualDisplay capture format", e)
            }
        }
    }

    private fun registerDisplayListener() {
        val dm = context.getSystemService(Context.DISPLAY_SERVICE) as DisplayManager
        val listener = object : DisplayManager.DisplayListener {
            override fun onDisplayAdded(displayId: Int) {
                Log.d(TAG, "DisplayManager: onDisplayAdded(id=$displayId)")
            }

            override fun onDisplayRemoved(displayId: Int) {
                Log.d(TAG, "DisplayManager: onDisplayRemoved(id=$displayId)")
            }

            override fun onDisplayChanged(displayId: Int) {
                if (!isCapturing.get()) return
                // Only respond to changes on the primary/default display (displayId 0)
                if (displayId != Display.DEFAULT_DISPLAY) {
                    return
                }

                val wm = context.getSystemService(Context.WINDOW_SERVICE) as WindowManager
                val metrics = DisplayMetrics()
                @Suppress("DEPRECATION")
                wm.defaultDisplay.getRealMetrics(metrics)

                val maxDim = 1200
                var newWidth = metrics.widthPixels
                var newHeight = metrics.heightPixels
                if (newWidth > maxDim || newHeight > maxDim) {
                    val scale = maxDim.toFloat() / maxOf(newWidth, newHeight)
                    newWidth = ((newWidth * scale).toInt() / 2) * 2
                    newHeight = ((newHeight * scale).toInt() / 2) * 2
                } else {
                    newWidth = (newWidth / 2) * 2
                    newHeight = (newHeight / 2) * 2
                }

                if (newWidth != currentWidth || newHeight != currentHeight) {
                    Log.i(TAG, "Default display rotated or resized: ${currentWidth}x${currentHeight} -> ${newWidth}x${newHeight}")
                    changeCaptureFormat(newWidth, newHeight, currentFps)
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

            val helper = surfaceTextureHelper
            if (helper != null) {
                ThreadUtils.invokeAtFrontUninterruptibly(helper.handler) {
                    try {
                        helper.stopListening()
                        capturerObserver?.onCapturerStopped()
                        virtualDisplay?.release()
                        virtualDisplay = null
                        surface?.release()
                        surface = null
                        mediaProjection?.stop()
                        mediaProjection = null
                        Log.i(TAG, "VirtualDisplay, Surface, and MediaProjection released successfully")
                    } catch (e: Exception) {
                        Log.e(TAG, "Error releasing capture resources on helper thread", e)
                    }
                }
                helper.dispose()
                surfaceTextureHelper = null
            }

            Log.i(TAG, "Screen capture stopped cleanly (total frames captured: ${numCapturedFrames.get()})")
        } catch (e: Exception) {
            Log.e(TAG, "Error stopping screen capture", e)
        }
    }

    companion object {
        private const val TAG = "ScreenCaptureManager"
    }
}

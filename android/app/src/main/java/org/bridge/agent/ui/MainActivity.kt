package org.bridge.agent.ui

import android.Manifest
import android.app.Activity
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.graphics.Color
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.Bundle
import android.widget.Button
import android.widget.EditText
import android.widget.TextView
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import org.bridge.agent.R
import org.bridge.agent.crypto.KeyStoreManager
import org.bridge.agent.network.RelayClient
import org.bridge.agent.service.BridgeForegroundService

class MainActivity : AppCompatActivity() {

    private lateinit var keyStoreManager: KeyStoreManager
    private lateinit var tvStatusBadge: TextView
    private lateinit var tvStatusDetails: TextView
    private lateinit var tvDeviceId: TextView
    private lateinit var tvPublicKey: TextView
    private lateinit var tvPairingPin: TextView
    private lateinit var btnRegeneratePin: Button
    private lateinit var etRelayUrl: EditText
    private lateinit var btnToggleService: Button

    private lateinit var tvStreamStatus: TextView
    private lateinit var btnScreenStream: Button

    private var isServiceRunning = false
    private var isScreenStreaming = false

    private val screenCaptureLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) { result ->
        if (result.resultCode == Activity.RESULT_OK && result.data != null) {
            val streamIntent = Intent(this, BridgeForegroundService::class.java).apply {
                action = BridgeForegroundService.ACTION_START_STREAM
                putExtra(BridgeForegroundService.EXTRA_CAPTURE_INTENT_DATA, result.data)
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                startForegroundService(streamIntent)
            } else {
                startService(streamIntent)
            }
            isScreenStreaming = true
            btnScreenStream.text = "Stop Screen Sharing"
            tvStreamStatus.text = "Screen streaming is active (WebRTC)"
        } else {
            tvStreamStatus.text = "Screen capture permission declined"
        }
    }

    private val statusReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            if (intent?.action == BridgeForegroundService.ACTION_STATUS_CHANGED) {
                val stateName = intent.getStringExtra(BridgeForegroundService.EXTRA_STATE) ?: return
                val details = intent.getStringExtra(BridgeForegroundService.EXTRA_DETAILS) ?: ""
                updateStatusUi(stateName, details)
            } else if (intent?.action == BridgeForegroundService.ACTION_STREAM_STATE_CHANGED) {
                val streamState = intent.getStringExtra(BridgeForegroundService.EXTRA_STREAM_STATE) ?: return
                updateStreamUi(streamState)
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        keyStoreManager = KeyStoreManager(this)

        initViews()
        loadIdentityAndPin()
        checkPermissions()
    }

    private fun initViews() {
        tvStatusBadge = findViewById(R.id.tvStatusBadge)
        tvStatusDetails = findViewById(R.id.tvStatusDetails)
        tvDeviceId = findViewById(R.id.tvDeviceId)
        tvPublicKey = findViewById(R.id.tvPublicKey)
        tvPairingPin = findViewById(R.id.tvPairingPin)
        btnRegeneratePin = findViewById(R.id.btnRegeneratePin)
        etRelayUrl = findViewById(R.id.etRelayUrl)
        btnToggleService = findViewById(R.id.btnToggleService)

        tvStreamStatus = findViewById(R.id.tvStreamStatus)
        btnScreenStream = findViewById(R.id.btnScreenStream)

        btnRegeneratePin.setOnClickListener {
            val newPin = keyStoreManager.generateNewPin()
            tvPairingPin.text = formatPin(newPin)
        }

        btnToggleService.setOnClickListener {
            if (isServiceRunning) {
                stopBridgeService()
            } else {
                startBridgeService()
            }
        }

        btnScreenStream.setOnClickListener {
            if (isScreenStreaming) {
                stopScreenStream()
            } else {
                startScreenStream()
            }
        }
    }

    private fun startScreenStream() {
        if (!isServiceRunning) {
            startBridgeService()
        }
        val mediaProjectionManager = getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
        screenCaptureLauncher.launch(mediaProjectionManager.createScreenCaptureIntent())
    }

    private fun stopScreenStream() {
        val streamIntent = Intent(this, BridgeForegroundService::class.java).apply {
            action = BridgeForegroundService.ACTION_STOP_STREAM
        }
        startService(streamIntent)
        isScreenStreaming = false
        btnScreenStream.text = "Share Screen with Controller"
        tvStreamStatus.text = "MediaProjection stream is inactive"
    }

    private fun loadIdentityAndPin() {
        val identity = keyStoreManager.getOrCreateIdentity()
        tvDeviceId.text = "Device ID: ${identity.deviceId}"
        tvPublicKey.text = "Public Key: ${identity.publicKeyHex.take(24)}..."

        val pin = keyStoreManager.getPairingPin()
        tvPairingPin.text = formatPin(pin)
    }

    private fun formatPin(pin: String): String {
        return if (pin.length == 6) {
            "${pin.substring(0, 3)} ${pin.substring(3)}"
        } else {
            pin
        }
    }

    private fun startBridgeService() {
        val relayUrl = etRelayUrl.text.toString().trim()
        val intent = Intent(this, BridgeForegroundService::class.java).apply {
            putExtra(BridgeForegroundService.EXTRA_RELAY_URL, relayUrl)
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            startForegroundService(intent)
        } else {
            startService(intent)
        }
        isServiceRunning = true
        btnToggleService.text = "Stop Bridge Service"
        updateStatusUi(RelayClient.State.CONNECTING.name, "Starting connection...")
    }

    private fun stopBridgeService() {
        if (isScreenStreaming) {
            stopScreenStream()
        }
        val intent = Intent(this, BridgeForegroundService::class.java).apply {
            action = BridgeForegroundService.ACTION_STOP
        }
        startService(intent)
        isServiceRunning = false
        btnToggleService.text = "Start Bridge Service"
        updateStatusUi(RelayClient.State.DISCONNECTED.name, "Service stopped")
    }

    private fun updateStatusUi(stateName: String, details: String) {
        when (stateName) {
            RelayClient.State.CONNECTING.name -> {
                tvStatusBadge.text = "◌ CONNECTING"
                tvStatusBadge.setBackgroundColor(Color.parseColor("#FEF3C7"))
                tvStatusBadge.setTextColor(Color.parseColor("#92400E"))
                tvStatusDetails.text = details
                isServiceRunning = true
                btnToggleService.text = "Stop Bridge Service"
            }
            RelayClient.State.RELAY_CONNECTED.name -> {
                tvStatusBadge.text = "◌ WAITING FOR LINUX"
                tvStatusBadge.setBackgroundColor(Color.parseColor("#FEF3C7"))
                tvStatusBadge.setTextColor(Color.parseColor("#92400E"))
                tvStatusDetails.text = details
                isServiceRunning = true
                btnToggleService.text = "Stop Bridge Service"
            }
            RelayClient.State.PAIRED_AND_CONNECTED.name -> {
                tvStatusBadge.text = "● Android Connected"
                tvStatusBadge.setBackgroundColor(Color.parseColor("#DCFCE7"))
                tvStatusBadge.setTextColor(Color.parseColor("#166534"))
                tvStatusDetails.text = details
                isServiceRunning = true
                btnToggleService.text = "Stop Bridge Service"
            }
            RelayClient.State.ERROR.name -> {
                tvStatusBadge.text = "● ERROR"
                tvStatusBadge.setBackgroundColor(Color.parseColor("#FEE2E2"))
                tvStatusBadge.setTextColor(Color.parseColor("#991B1B"))
                tvStatusDetails.text = details
            }
            else -> {
                tvStatusBadge.text = "● DISCONNECTED"
                tvStatusBadge.setBackgroundColor(Color.parseColor("#F3F4F6"))
                tvStatusBadge.setTextColor(Color.parseColor("#6B7280"))
                tvStatusDetails.text = details
                isServiceRunning = false
                btnToggleService.text = "Start Bridge Service"
            }
        }
    }

    private fun updateStreamUi(streamState: String) {
        when (streamState) {
            "CONNECTED", "STREAMING_ACTIVE" -> {
                isScreenStreaming = true
                btnScreenStream.text = "Stop Screen Sharing"
                tvStreamStatus.text = "Live Screen Streaming (WebRTC Connected)"
            }
            "STREAMING_STOPPED", "DISCONNECTED", "CLOSED" -> {
                isScreenStreaming = false
                btnScreenStream.text = "Share Screen with Controller"
                tvStreamStatus.text = "MediaProjection stream is inactive"
            }
            else -> {
                tvStreamStatus.text = "Stream State: $streamState"
            }
        }
    }

    private fun checkPermissions() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS)
                != PackageManager.PERMISSION_GRANTED
            ) {
                ActivityCompat.requestPermissions(
                    this,
                    arrayOf(Manifest.permission.POST_NOTIFICATIONS),
                    101
                )
            }
        }
    }

    override fun onResume() {
        super.onResume()
        val filter = IntentFilter().apply {
            addAction(BridgeForegroundService.ACTION_STATUS_CHANGED)
            addAction(BridgeForegroundService.ACTION_STREAM_STATE_CHANGED)
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            registerReceiver(statusReceiver, filter, RECEIVER_NOT_EXPORTED)
        } else {
            registerReceiver(statusReceiver, filter)
        }
    }

    override fun onPause() {
        super.onPause()
        try {
            unregisterReceiver(statusReceiver)
        } catch (_: Exception) {}
    }
}


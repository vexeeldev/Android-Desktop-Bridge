package org.bridge.agent.ui

import android.Manifest
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.widget.Button
import android.widget.EditText
import android.widget.TextView
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

    private var isServiceRunning = false

    private val statusReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            val stateName = intent?.getStringExtra(BridgeForegroundService.EXTRA_STATE) ?: return
            val details = intent.getStringExtra(BridgeForegroundService.EXTRA_DETAILS) ?: ""
            updateStatusUi(stateName, details)
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
                // SPEC: Android Connected
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
        val filter = IntentFilter(BridgeForegroundService.ACTION_STATUS_CHANGED)
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

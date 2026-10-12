package org.bridge.agent.network

import android.os.Build
import android.util.Log
import com.google.gson.Gson
import okhttp3.*
import org.bridge.agent.crypto.DeviceIdentity
import org.bridge.agent.crypto.KeyStoreManager
import org.bridge.agent.protocol.*
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

class RelayClient(
    private val relayUrl: String,
    private val identity: DeviceIdentity,
    private val keyStoreManager: KeyStoreManager,
    private val listener: ConnectionListener
) {
    interface ConnectionListener {
        fun onStateChanged(state: State, details: String)
        fun onMessageReceived(envelope: MessageEnvelope)
    }

    enum class State {
        DISCONNECTED,
        CONNECTING,
        RELAY_CONNECTED,
        PAIRED_AND_CONNECTED,
        ERROR
    }

    private val client = OkHttpClient.Builder()
        .pingInterval(15, TimeUnit.SECONDS)
        .connectTimeout(10, TimeUnit.SECONDS)
        .build()

    private var webSocket: WebSocket? = null
    private val isRunning = AtomicBoolean(false)
    private val gson = Gson()

    fun start() {
        isRunning.set(true)
        connect()
    }

    fun stop() {
        isRunning.set(false)
        webSocket?.close(1000, "Client stopped")
        webSocket = null
        listener.onStateChanged(State.DISCONNECTED, "Service stopped")
    }

    private fun connect() {
        if (!isRunning.get()) return

        listener.onStateChanged(State.CONNECTING, "Connecting to relay at $relayUrl")
        val request = Request.Builder()
            .url(relayUrl)
            .build()

        webSocket = client.newWebSocket(request, object : WebSocketListener() {
            override fun onOpen(ws: WebSocket, response: Response) {
                Log.i(TAG, "WebSocket opened, initiating authentication handshake")
                performHandshake(ws)
            }

            override fun onMessage(ws: WebSocket, text: String) {
                handleIncomingMessage(ws, text)
            }

            override fun onFailure(ws: WebSocket, t: Throwable, response: Response?) {
                Log.w(TAG, "WebSocket failure: ${t.message}")
                listener.onStateChanged(State.ERROR, "Connection error: ${t.message}")
                scheduleReconnect()
            }

            override fun onClosed(ws: WebSocket, code: Int, reason: String) {
                Log.i(TAG, "WebSocket closed: $reason")
                listener.onStateChanged(State.DISCONNECTED, "Disconnected from relay")
                scheduleReconnect()
            }
        })
    }

    private fun performHandshake(ws: WebSocket) {
        val timestamp = System.currentTimeMillis()
        val nonce = (10000000..99999999).random().toString()
        val challenge = "${identity.deviceId}:$timestamp:$nonce"
        val signature = identity.sign(challenge.toByteArray(Charsets.UTF_8))

        val handshakeData = HandshakeRequestData(
            device_id = identity.deviceId,
            public_key = identity.publicKeyHex,
            timestamp = timestamp,
            nonce = nonce,
            signature = signature,
            role = "agent"
        )

        val envelope = MessageEnvelope.create(
            sender = identity.deviceId,
            recipient = null,
            type = "HandshakeRequest",
            data = handshakeData
        )

        ws.send(envelope.toJson())
    }

    private fun handleIncomingMessage(ws: WebSocket, text: String) {
        try {
            val envelope = MessageEnvelope.fromJson(text)
            listener.onMessageReceived(envelope)

            val type = envelope.payload.get("type")?.asString ?: return
            val data = envelope.payload.get("data")

            when (type) {
                "HandshakeResponse" -> {
                    val resp = gson.fromJson(data, HandshakeResponseData::class.java)
                    if (resp.success) {
                        Log.i(TAG, "Authenticated with relay successfully")
                        listener.onStateChanged(State.RELAY_CONNECTED, "Connected to Relay — Waiting for Controller")
                    } else {
                        Log.e(TAG, "Handshake rejected: ${resp.message}")
                        listener.onStateChanged(State.ERROR, "Authentication rejected: ${resp.message}")
                    }
                }

                "PairingRequest" -> {
                    val req = gson.fromJson(data, PairingRequestData::class.java)
                    val expectedPin = keyStoreManager.getPairingPin()
                    val accepted = req.pairing_code == expectedPin

                    Log.i(TAG, "Received pairing request with PIN ${req.pairing_code}. Expected $expectedPin. Accepted: $accepted")

                    val pairingResp = PairingResponseData(
                        accepted = accepted,
                        responder_id = identity.deviceId,
                        responder_name = "${Build.MANUFACTURER} ${Build.MODEL}",
                        responder_pubkey = identity.publicKeyHex,
                        message = if (accepted) "Pairing accepted" else "Invalid PIN"
                    )

                    val respEnv = MessageEnvelope.create(
                        sender = identity.deviceId,
                        recipient = req.initiator_id,
                        type = "PairingResponse",
                        data = pairingResp
                    )
                    ws.send(respEnv.toJson())

                    if (accepted) {
                        keyStoreManager.saveTrustedController(
                            req.initiator_id,
                            req.initiator_name,
                            req.initiator_pubkey
                        )

                        // Notify Linux Controller of active connection
                        val statusEnv = MessageEnvelope.create(
                            sender = identity.deviceId,
                            recipient = req.initiator_id,
                            type = "StatusUpdate",
                            data = StatusUpdateData(
                                state = "Android Connected",
                                details = "Paired with ${req.initiator_name}"
                            )
                        )
                        ws.send(statusEnv.toJson())

                        listener.onStateChanged(
                            State.PAIRED_AND_CONNECTED,
                            "Paired with ${req.initiator_name}"
                        )
                    }
                }

                "Ping" -> {
                    val ping = gson.fromJson(data, PingData::class.java)
                    val pongEnv = MessageEnvelope.create(
                        sender = identity.deviceId,
                        recipient = envelope.sender,
                        type = "Pong",
                        data = PongData(ping.nonce)
                    )
                    ws.send(pongEnv.toJson())
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error handling message", e)
        }
    }

    private fun scheduleReconnect() {
        if (!isRunning.get()) return
        Thread {
            try {
                Thread.sleep(3000)
                if (isRunning.get()) {
                    Log.i(TAG, "Attempting reconnect...")
                    connect()
                }
            } catch (_: InterruptedException) {
            }
        }.start()
    }

    companion object {
        private const val TAG = "RelayClient"
    }
}

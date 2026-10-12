package org.bridge.agent.protocol

import com.google.gson.Gson
import com.google.gson.JsonObject

const val PROTOCOL_VERSION = 1

data class MessageEnvelope(
    val version: Int = PROTOCOL_VERSION,
    val id: String = "${System.currentTimeMillis()}_${(1000..9999).random()}",
    val timestamp: Long = System.currentTimeMillis(),
    val sender: String,
    val recipient: String? = null,
    val payload: JsonObject
) {
    fun toJson(): String = Gson().toJson(this)

    companion object {
        fun fromJson(json: String): MessageEnvelope = Gson().fromJson(json, MessageEnvelope::class.java)

        fun create(sender: String, recipient: String?, type: String, data: Any): MessageEnvelope {
            val gson = Gson()
            val payloadObj = JsonObject().apply {
                addProperty("type", type)
                add("data", gson.toJsonTree(data))
            }
            return MessageEnvelope(
                version = PROTOCOL_VERSION,
                sender = sender,
                recipient = recipient,
                payload = payloadObj
            )
        }
    }
}

// Handshake
data class HandshakeRequestData(
    val device_id: String,
    val public_key: String,
    val timestamp: Long,
    val nonce: String,
    val signature: String,
    val role: String = "agent"
)

data class HandshakeResponseData(
    val success: Boolean,
    val session_id: String,
    val message: String
)

// Pairing
data class PairingRequestData(
    val initiator_id: String,
    val initiator_name: String,
    val initiator_pubkey: String,
    val pairing_code: String
)

data class PairingResponseData(
    val accepted: Boolean,
    val responder_id: String,
    val responder_name: String,
    val responder_pubkey: String,
    val message: String? = null
)

// Status & Heartbeat
data class StatusUpdateData(
    val state: String,
    val details: String? = null
)

data class PingData(
    val nonce: Long
)

data class PongData(
    val nonce: Long
)

// Device Info (Phase 4)
data class DeviceInfoResponseData(
    val device_id: String,
    val manufacturer: String,
    val model: String,
    val android_version: String,
    val sdk_level: Int,
    val battery_level: Int,
    val is_charging: Boolean,
    val screen_width: Int,
    val screen_height: Int,
    val orientation: String,
    val service_status: String
)

// WebRTC Signaling (Phase 5)
data class WebRtcOfferData(
    val sdp: String
)

data class WebRtcAnswerData(
    val sdp: String
)

data class WebRtcIceCandidateData(
    val candidate: String,
    val sdp_mid: String? = null,
    val sdp_mline_index: Int? = null
)


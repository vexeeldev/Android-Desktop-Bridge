package org.bridge.agent

import com.google.gson.Gson
import org.bridge.agent.protocol.*
import org.junit.Assert.*
import org.junit.Test

class ProtocolModelsTest {

    @Test
    fun testEnvelopeSerializationRoundtrip() {
        val handshakeData = HandshakeRequestData(
            device_id = "dev_android_123",
            public_key = "hex_pubkey_abc",
            timestamp = 1700000000L,
            nonce = "random_nonce_999",
            signature = "sig_hex_xyz",
            role = "agent"
        )

        val envelope = MessageEnvelope.create(
            sender = "dev_android_123",
            recipient = null,
            type = "HandshakeRequest",
            data = handshakeData
        )

        val json = envelope.toJson()
        val decoded = MessageEnvelope.fromJson(json)

        assertEquals(1, decoded.version)
        assertEquals("dev_android_123", decoded.sender)
        assertNull(decoded.recipient)

        val payloadType = decoded.payload.get("type").asString
        assertEquals("HandshakeRequest", payloadType)

        val dataObj = Gson().fromJson(decoded.payload.get("data"), HandshakeRequestData::class.java)
        assertEquals("dev_android_123", dataObj.device_id)
        assertEquals("agent", dataObj.role)
    }

    @Test
    fun testPairingResponseRoundtrip() {
        val pairingResp = PairingResponseData(
            accepted = true,
            responder_id = "dev_android_555",
            responder_name = "Google Pixel 8",
            responder_pubkey = "pub_abc_123",
            message = "Pairing accepted"
        )

        val envelope = MessageEnvelope.create(
            sender = "dev_android_555",
            recipient = "dev_linux_999",
            type = "PairingResponse",
            data = pairingResp
        )

        val json = envelope.toJson()
        val decoded = MessageEnvelope.fromJson(json)

        assertEquals("dev_linux_999", decoded.recipient)
        val dataObj = Gson().fromJson(decoded.payload.get("data"), PairingResponseData::class.java)
        assertTrue(dataObj.accepted)
        assertEquals("Google Pixel 8", dataObj.responder_name)
    }
}

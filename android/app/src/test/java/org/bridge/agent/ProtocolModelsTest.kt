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

    @Test
    fun testDeviceInfoRoundtrip() {
        val infoData = DeviceInfoResponseData(
            device_id = "dev_android_888",
            manufacturer = "Samsung",
            model = "Galaxy S24 Ultra",
            android_version = "14",
            sdk_level = 34,
            battery_level = 92,
            is_charging = true,
            screen_width = 1440,
            screen_height = 3120,
            orientation = "landscape",
            service_status = "running"
        )

        val envelope = MessageEnvelope.create(
            sender = "dev_android_888",
            recipient = "dev_linux_111",
            type = "DeviceInfoResponse",
            data = infoData
        )

        val json = envelope.toJson()
        val decoded = MessageEnvelope.fromJson(json)

        val dataObj = Gson().fromJson(decoded.payload.get("data"), DeviceInfoResponseData::class.java)
        assertEquals("Samsung", dataObj.manufacturer)
        assertEquals("Galaxy S24 Ultra", dataObj.model)
        assertEquals(34, dataObj.sdk_level)
        assertEquals(92, dataObj.battery_level)
        assertTrue(dataObj.is_charging)
        assertEquals(1440, dataObj.screen_width)
        assertEquals("landscape", dataObj.orientation)
    }

    @Test
    fun testWebRtcSignalingRoundtrip() {
        val offer = WebRtcOfferData(sdp = "v=0\r\no=- 4567 2 IN IP4 0.0.0.0")
        val envelope = MessageEnvelope.create(
            sender = "dev_android_888",
            recipient = "dev_linux_111",
            type = "WebRtcOffer",
            data = offer
        )
        val json = envelope.toJson()
        val decoded = MessageEnvelope.fromJson(json)
        val dataObj = Gson().fromJson(decoded.payload.get("data"), WebRtcOfferData::class.java)
        assertEquals(offer.sdp, dataObj.sdp)
    }
}

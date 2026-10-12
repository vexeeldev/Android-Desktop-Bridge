package org.bridge.agent

import org.bridge.agent.crypto.DeviceIdentity
import org.junit.Assert.*
import org.junit.Test

class DeviceIdentityTest {

    @Test
    fun testKeyGenerationAndSigning() {
        val identity = DeviceIdentity.generate()
        assertTrue(identity.deviceId.startsWith("dev_"))
        assertEquals(64, identity.publicKeyHex.length) // 32 bytes hex = 64 chars

        val message = "challenge_nonce_test_payload".toByteArray()
        val signature = identity.sign(message)
        assertEquals(128, signature.length) // 64 bytes hex = 128 chars

        // Verify with own public key
        val isValid = DeviceIdentity.verifySignature(identity.publicKeyHex, message, signature)
        assertTrue(isValid)

        // Verify with tampered message
        val isTamperedValid = DeviceIdentity.verifySignature(
            identity.publicKeyHex,
            "tampered_payload".toByteArray(),
            signature
        )
        assertFalse(isTamperedValid)
    }

    @Test
    fun testDeviceIdDeterminism() {
        val seed = ByteArray(32) { 7.toByte() }
        val id1 = DeviceIdentity.fromSeed(seed)
        val id2 = DeviceIdentity.fromSeed(seed)

        assertEquals(id1.deviceId, id2.deviceId)
        assertEquals(id1.publicKeyHex, id2.publicKeyHex)
    }
}

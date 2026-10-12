package org.bridge.agent.crypto

import org.bouncycastle.crypto.generators.Ed25519KeyPairGenerator
import org.bouncycastle.crypto.params.Ed25519KeyGenerationParameters
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer
import java.security.MessageDigest
import java.security.SecureRandom

class DeviceIdentity(
    private val privateKey: Ed25519PrivateKeyParameters,
    val publicKey: Ed25519PublicKeyParameters
) {
    val publicKeyBytes: ByteArray = publicKey.encoded
    val publicKeyHex: String = publicKeyBytes.toHex()
    val deviceId: String = deriveDeviceId(publicKeyBytes)

    fun getSeed(): ByteArray = privateKey.encoded

    fun sign(message: ByteArray): String {
        val signer = Ed25519Signer()
        signer.init(true, privateKey)
        signer.update(message, 0, message.size)
        val sigBytes = signer.generateSignature()
        return sigBytes.toHex()
    }

    companion object {
        fun generate(): DeviceIdentity {
            val generator = Ed25519KeyPairGenerator()
            generator.init(Ed25519KeyGenerationParameters(SecureRandom()))
            val pair = generator.generateKeyPair()
            val priv = pair.private as Ed25519PrivateKeyParameters
            val pub = pair.public as Ed25519PublicKeyParameters
            return DeviceIdentity(priv, pub)
        }

        fun fromSeed(seed: ByteArray): DeviceIdentity {
            val priv = Ed25519PrivateKeyParameters(seed, 0)
            val pub = priv.generatePublicKey()
            return DeviceIdentity(priv, pub)
        }

        fun deriveDeviceId(pubBytes: ByteArray): String {
            val digest = MessageDigest.getInstance("SHA-256")
            val hash = digest.digest(pubBytes)
            val prefix = hash.copyOfRange(0, 16).toHex()
            return "dev_$prefix"
        }

        fun verifySignature(pubKeyHex: String, message: ByteArray, signatureHex: String): Boolean {
            return try {
                val pubBytes = pubKeyHex.hexToByteArray()
                val sigBytes = signatureHex.hexToByteArray()
                val pubKey = Ed25519PublicKeyParameters(pubBytes, 0)
                val signer = Ed25519Signer()
                signer.init(false, pubKey)
                signer.update(message, 0, message.size)
                signer.verifySignature(sigBytes)
            } catch (e: Exception) {
                false
            }
        }
    }
}

fun ByteArray.toHex(): String = joinToString("") { "%02x".format(it) }

fun String.hexToByteArray(): ByteArray {
    check(length % 2 == 0) { "Must have an even length" }
    return chunked(2)
        .map { it.toInt(16).toByte() }
        .toByteArray()
}

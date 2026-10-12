package org.bridge.agent.crypto

import android.content.Context
import android.content.SharedPreferences
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey

class KeyStoreManager(private val context: Context) {

    private val prefs: SharedPreferences by lazy {
        try {
            val masterKey = MasterKey.Builder(context)
                .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
                .build()

            EncryptedSharedPreferences.create(
                context,
                PREFS_NAME,
                masterKey,
                EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
                EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
            )
        } catch (e: Exception) {
            // Fallback for testing environments without Android Keystore provider
            context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        }
    }

    fun getOrCreateIdentity(): DeviceIdentity {
        val seedHex = prefs.getString(KEY_SEED, null)
        return if (seedHex != null) {
            DeviceIdentity.fromSeed(seedHex.hexToByteArray())
        } else {
            val identity = DeviceIdentity.generate()
            prefs.edit().putString(KEY_SEED, identity.getSeed().toHex()).apply()
            identity
        }
    }

    fun getPairingPin(): String {
        var pin = prefs.getString(KEY_PIN, null)
        if (pin == null) {
            pin = generateNewPin()
        }
        return pin
    }

    fun generateNewPin(): String {
        val pin = (100000..999999).random().toString()
        prefs.edit().putString(KEY_PIN, pin).apply()
        return pin
    }

    fun saveTrustedController(deviceId: String, name: String, pubKey: String) {
        prefs.edit()
            .putString("trusted_${deviceId}_name", name)
            .putString("trusted_${deviceId}_pubkey", pubKey)
            .putString("last_trusted_controller", deviceId)
            .apply()
    }

    fun getLastTrustedControllerId(): String? {
        return prefs.getString("last_trusted_controller", null)
    }

    fun isControllerTrusted(deviceId: String): Boolean {
        return prefs.contains("trusted_${deviceId}_pubkey")
    }

    companion object {
        private const val PREFS_NAME = "bridge_secure_prefs"
        private const val KEY_SEED = "device_identity_seed"
        private const val KEY_PIN = "pairing_pin"
    }
}

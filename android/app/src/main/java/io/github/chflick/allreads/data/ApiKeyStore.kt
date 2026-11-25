package io.github.chflick.allreads.data

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Keeps the DeepL API key encrypted. An AES key that never leaves the Android
 * Keystore encrypts it; only the ciphertext is written to `SharedPreferences`.
 * The key is never logged and never reaches the database.
 */
class ApiKeyStore(context: Context) {
    private val prefs = context.getSharedPreferences("secrets", Context.MODE_PRIVATE)
    private val _hasDeeplKey = MutableStateFlow(prefs.contains(DEEPL))
    val hasDeeplKey: StateFlow<Boolean> = _hasDeeplKey.asStateFlow()

    /** The saved key, or null if there is none or it can no longer be decrypted (e.g. after a restore). */
    fun deeplKey(): String? {
        val stored = prefs.getString(DEEPL, null) ?: return null
        return runCatching {
            val bytes = Base64.decode(stored, Base64.NO_WRAP)
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.DECRYPT_MODE, secretKey(), GCMParameterSpec(128, bytes, 0, IV_SIZE))
            String(cipher.doFinal(bytes, IV_SIZE, bytes.size - IV_SIZE), Charsets.UTF_8)
        }.getOrNull()
    }

    /** The last four characters, to show which key is saved without showing it. */
    fun deeplKeyHint(): String? = deeplKey()?.takeLast(4)

    fun saveDeeplKey(key: String) {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, secretKey())
        val stored = cipher.iv + cipher.doFinal(key.trim().toByteArray(Charsets.UTF_8))
        prefs.edit().putString(DEEPL, Base64.encodeToString(stored, Base64.NO_WRAP)).apply()
        _hasDeeplKey.value = true
    }

    fun clearDeeplKey() {
        prefs.edit().remove(DEEPL).apply()
        _hasDeeplKey.value = false
    }

    private fun secretKey(): SecretKey {
        val keyStore = KeyStore.getInstance(KEYSTORE).apply { load(null) }
        (keyStore.getEntry(ALIAS, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
        generator.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .build(),
        )
        return generator.generateKey()
    }

    private companion object {
        const val KEYSTORE = "AndroidKeyStore"
        const val ALIAS = "allreads_api_keys"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val IV_SIZE = 12
        const val DEEPL = "deepl"
    }
}

package uz.arumediatv.com.core

import android.content.Context
import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Kichik maxfiy qiymatlar (sessiya tokeni, asosiy kalit) uchun: Android Keystore'dagi AES-GCM kaliti bilan shifrlangan
 * SharedPreferences. Kalit qurilmadan chiqmaydi. Keystore ishlamasa (ba'zi qurilmalarda nosoz) — qiymat saqlanmaydi (xavfsiz tomon).
 */
class SecureStore(context: Context, name: String = "aru_secure") {
    private val prefs: SharedPreferences = context.getSharedPreferences(name, Context.MODE_PRIVATE)

    fun getString(key: String): String? {
        val raw = prefs.getString(key, null) ?: return null
        return try {
            open(Base64.decode(raw, Base64.NO_WRAP)).toString(Charsets.UTF_8)
        } catch (_: Exception) {
            null
        }
    }

    fun putString(key: String, value: String): Boolean =
        try {
            prefs.edit().putString(key, Base64.encodeToString(seal(value.toByteArray(Charsets.UTF_8)), Base64.NO_WRAP)).apply()
            true
        } catch (_: Exception) {
            false
        }

    fun remove(key: String) {
        prefs.edit().remove(key).apply()
    }

    companion object {
        private const val ALIAS = "aru_master_wrap_v1"
        private const val PROVIDER = "AndroidKeyStore"

        @Volatile
        private var cachedKey: SecretKey? = null

        /** Keystore'dan kalitni har safar o'qish sekin (binder chaqiruvi) — bir marta olib, xotirada saqlaymiz. */
        private fun key(): SecretKey = cachedKey ?: synchronized(this) { cachedKey ?: loadKey().also { cachedKey = it } }

        private fun loadKey(): SecretKey {
            val ks = KeyStore.getInstance(PROVIDER).apply { load(null) }
            (ks.getKey(ALIAS, null) as? SecretKey)?.let { return it }
            val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, PROVIDER)
            gen.init(
                KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                    .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                    .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                    .setKeySize(256)
                    .build(),
            )
            return gen.generateKey()
        }

        /** `IV(12) + shifrmatn+tag`. */
        fun seal(plain: ByteArray): ByteArray {
            val c = Cipher.getInstance("AES/GCM/NoPadding")
            c.init(Cipher.ENCRYPT_MODE, key())
            return c.iv + c.doFinal(plain)
        }

        fun open(sealed: ByteArray): ByteArray {
            require(sealed.size > 12) { "qisqa" }
            val c = Cipher.getInstance("AES/GCM/NoPadding")
            c.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, sealed, 0, 12))
            return c.doFinal(sealed, 12, sealed.size - 12)
        }
    }
}

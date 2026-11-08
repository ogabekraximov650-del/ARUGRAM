package uz.arudrive.app.data

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.io.File
import java.security.KeyStore
import java.util.concurrent.atomic.AtomicInteger
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Video bo'laklarining telefondagi DOIMIY shifrlangan keshi (ARUMEDIATV `video_cache.rs` kabi): fayl 1 MiB bo'laklarga bo'linadi, har bo'lak
 * alohida AES-128-GCM bilan shifrlanib diskka yoziladi (`filesDir/vcache/<id>/<raqam>`: 12 bayt nonce + shifrmatn + teg). Kalit Android Keystore'da
 * (chiqarib bo'lmaydi); `<id>:<raqam>` AAD sifatida bog'lanadi — bo'laklarni almashtirib bo'lmaydi. Hajm [CAP_BYTES] dan oshsa eng eskilari o'chiriladi.
 */
object BlockCache {
    const val BLOCK = 1024 * 1024
    private const val CAP_BYTES = 2L * 1024 * 1024 * 1024
    private const val ALIAS = "arudrive_vcache_v1"
    private val writes = AtomicInteger()

    private fun key(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        if (!ks.containsAlias(ALIAS)) {
            KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").apply {
                init(
                    KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                        .setKeySize(128)
                        .build(),
                )
            }.generateKey()
        }
        return ks.getKey(ALIAS, null) as SecretKey
    }

    private fun root(ctx: Context) = File(ctx.applicationContext.filesDir, "vcache")
    private fun file(ctx: Context, id: String, idx: Long) = File(File(root(ctx), id), idx.toString())
    private fun aad(id: String, idx: Long) = "$id:$idx".toByteArray()

    /** Faylning hamma bo'laklarini telefondan o'chiradi (fayl o'chirilganda). */
    fun remove(ctx: Context, id: String) {
        try { File(root(ctx), id).deleteRecursively() } catch (_: Exception) {}
    }

    fun has(ctx: Context, id: String, idx: Long): Boolean = file(ctx, id, idx).isFile

    /** Bo'lakni o'qib shifrini ochadi; yo'q yoki buzilgan (kalit yo'qolgan, teg mos emas) bo'lsa — null (buzilgan fayl o'chiriladi). */
    fun read(ctx: Context, id: String, idx: Long, expectLen: Int): ByteArray? {
        val f = file(ctx, id, idx)
        if (!f.isFile) return null
        return try {
            val raw = f.readBytes()
            if (raw.size < 12 + 16) throw IllegalStateException("qisqa")
            val c = Cipher.getInstance("AES/GCM/NoPadding")
            c.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, raw, 0, 12))
            c.updateAAD(aad(id, idx))
            val plain = c.doFinal(raw, 12, raw.size - 12)
            if (plain.size != expectLen) throw IllegalStateException("hajm")
            f.setLastModified(System.currentTimeMillis())
            plain
        } catch (_: Exception) {
            f.delete()
            null
        }
    }

    /** Bo'lakni shifrlab yozadi (avval `.tmp`, so'ng nom almashtiriladi — yarim yozilgan bo'lak qolmaydi). Xato bo'lsa jim o'tadi. */
    fun write(ctx: Context, id: String, idx: Long, plain: ByteArray) {
        try {
            val dir = File(root(ctx), id).apply { mkdirs() }
            val c = Cipher.getInstance("AES/GCM/NoPadding")
            c.init(Cipher.ENCRYPT_MODE, key())
            c.updateAAD(aad(id, idx))
            val ct = c.doFinal(plain)
            val tmp = File(dir, "$idx.tmp")
            tmp.outputStream().use { it.write(c.iv); it.write(ct) }
            if (!tmp.renameTo(File(dir, idx.toString()))) tmp.delete()
            if (writes.incrementAndGet() % 32 == 0) trim(ctx)
        } catch (_: Exception) {}
    }

    /** Hajm chegaradan oshsa — eng eski (oxirgi marta o'qilgan) bo'laklar 90% gacha o'chiriladi. */
    private fun trim(ctx: Context) {
        val all = root(ctx).walkTopDown().filter { it.isFile }.toList()
        var total = all.sumOf { it.length() }
        if (total <= CAP_BYTES) return
        for (f in all.sortedBy { it.lastModified() }) {
            if (total <= CAP_BYTES * 9 / 10) break
            total -= f.length()
            f.delete()
        }
    }
}

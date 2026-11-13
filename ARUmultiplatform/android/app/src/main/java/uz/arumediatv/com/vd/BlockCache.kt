package uz.arumediatv.com.vd

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
    private const val ALIAS = "arumediatv_vd_cache_v1"
    private const val PIN = ".pin"
    private val writes = AtomicInteger()

    @Volatile private var cachedKey: SecretKey? = null

    /** Kalit tutqichi bir marta olinadi (har o'qishda Keystore'ga murojaat qilinmaydi: parallel o'qishlarda vaqtinchalik xatolar kamayadi). */
    private fun key(): SecretKey = cachedKey ?: synchronized(this) { cachedKey ?: loadKey().also { cachedKey = it } }

    private fun loadKey(): SecretKey {
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

    private fun root(ctx: Context) = File(ctx.applicationContext.filesDir, "vdcache")
    private fun file(ctx: Context, id: String, idx: Long) = File(File(root(ctx), id), idx.toString())
    private fun aad(id: String, idx: Long) = "$id:$idx".toByteArray()

    /** Faylning hamma bo'laklarini telefondan o'chiradi (fayl o'chirilganda). */
    fun remove(ctx: Context, id: String) {
        try { File(root(ctx), id).deleteRecursively() } catch (_: Exception) {}
    }

    fun has(ctx: Context, id: String, idx: Long): Boolean = file(ctx, id, idx).isFile

    /** Shu fayldan telefonda biror bo'lak bormi (papkasi bor). */
    fun exists(ctx: Context, id: String): Boolean = File(root(ctx), id).isDirectory

    /**
     * "Yuklab olingan" belgisi: belgilangan fayl bo'laklari hajm chegarasi ([CAP_BYTES]) sababli O'CHIRILMAYDI
     * (chegara faqat ko'rish paytida yig'ilgan keshga tegishli).
     */
    fun pin(ctx: Context, id: String) {
        try { File(File(root(ctx), id).apply { mkdirs() }, PIN).createNewFile() } catch (_: Exception) {}
    }

    /** Telefondagi bo'laklar hajmi (ochiq baytlarda); [total] — fayl hajmi (oxirgi bo'lak to'liq 1 MiB emas). */
    fun storedBytes(ctx: Context, id: String, total: Long): Long {
        val names = File(root(ctx), id).list() ?: return 0
        var n = 0L
        var last = false
        val lastIdx = if (total > 0) ((total - 1) / BLOCK).toString() else ""
        for (x in names) {
            if (x.isEmpty() || !x.all { it in '0'..'9' }) continue
            n++
            if (x == lastIdx) last = true
        }
        var bytes = n * BLOCK
        if (last) bytes -= BLOCK - (total - (total - 1) / BLOCK * BLOCK)
        return if (total > 0) bytes.coerceAtMost(total) else bytes
    }

    /** Diskdagi jami hajm (baytlarda). */
    fun diskBytes(ctx: Context): Long = root(ctx).walkTopDown().filter { it.isFile }.sumOf { it.length() }

    /** Hamma bo'laklarni o'chiradi (Xotira -> Videolar tozalash). */
    fun wipe(ctx: Context) {
        try { root(ctx).listFiles()?.forEach { it.deleteRecursively() } } catch (_: Exception) {}
    }

    /**
     * Bo'lakni o'qib shifrini ochadi; yo'q bo'lsa — null.
     * MUHIM: bo'lak FAQAT aniq buzilgan bo'lsa (GCM tegi mos emas, fayl qisqa) o'chiriladi. Vaqtinchalik xatoda (Keystore band, o'qish xatosi, uzilish)
     * bo'lak TEGILMAYDI: ilgari har qanday xato bo'lakni o'chirardi — oflayn ko'rishda parallel o'qishlar paytida yuklab olingan fayllardan bo'laklar
     * yo'qolib (100% -> 97-98%) video to'xtab qolardi, internet yo'q bo'lgani uchun qayta olib bo'lmasdi.
     */
    fun read(ctx: Context, id: String, idx: Long, expectLen: Int): ByteArray? {
        val f = file(ctx, id, idx)
        if (!f.isFile) return null
        for (attempt in 0 until 3) {
            try {
                val raw = f.readBytes()
                if (raw.size < 12 + 16) { f.delete(); return null } // chala yozilgan (haqiqatan buzilgan)
                val c = Cipher.getInstance("AES/GCM/NoPadding")
                c.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, raw, 0, 12))
                c.updateAAD(aad(id, idx))
                val plain = c.doFinal(raw, 12, raw.size - 12)
                if (plain.size != expectLen) return null // kutilgan hajm boshqa: o'chirmaymiz (ma'lumot noto'g'ri bo'lishi mumkin)
                f.setLastModified(System.currentTimeMillis())
                return plain
            } catch (_: javax.crypto.AEADBadTagException) {
                f.delete() // teg mos emas: bo'lak haqiqatan buzilgan (yoki kalit boshqa)
                return null
            } catch (_: Exception) {
                cachedKey = null // vaqtinchalik xato: kalit tutqichi qayta olinadi, qisqa kutib qayta uriniladi
                if (attempt < 2) try { Thread.sleep(60L * (attempt + 1)) } catch (_: InterruptedException) { return null }
            }
        }
        return null
    }

    /** Bo'lakni shifrlab yozadi (avval `.tmp`, so'ng nom almashtiriladi — yarim yozilgan bo'lak qolmaydi). Xato bo'lsa `false` (jim o'tadi). */
    fun write(ctx: Context, id: String, idx: Long, plain: ByteArray): Boolean {
        return try {
            val dir = File(root(ctx), id).apply { mkdirs() }
            val c = Cipher.getInstance("AES/GCM/NoPadding")
            c.init(Cipher.ENCRYPT_MODE, key())
            c.updateAAD(aad(id, idx))
            val ct = c.doFinal(plain)
            val tmp = File(dir, "$idx.tmp")
            tmp.outputStream().use { it.write(c.iv); it.write(ct) }
            val ok = tmp.renameTo(File(dir, idx.toString()))
            if (!ok) tmp.delete()
            if (writes.incrementAndGet() % 32 == 0) trim(ctx)
            ok
        } catch (_: Exception) {
            false
        }
    }

    /** Hajm chegaradan oshsa — eng eski (oxirgi marta o'qilgan) bo'laklar 90% gacha o'chiriladi. Yuklab olingan ([pin]) fayllar hisobga kirmaydi. */
    private fun trim(ctx: Context) {
        val all = (root(ctx).listFiles() ?: return).filter { it.isDirectory && !File(it, PIN).exists() }
            .flatMap { it.listFiles()?.filter { f -> f.isFile } ?: emptyList() }
        var total = all.sumOf { it.length() }
        if (total <= CAP_BYTES) return
        for (f in all.sortedBy { it.lastModified() }) {
            if (total <= CAP_BYTES * 9 / 10) break
            total -= f.length()
            f.delete()
        }
    }
}

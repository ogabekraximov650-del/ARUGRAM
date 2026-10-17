package uz.arumediatv.com.data

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.LruCache
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import kotlinx.coroutines.withContext
import uz.arumediatv.com.core.SecureStore
import java.io.File
import java.util.concurrent.ConcurrentHashMap

/**
 * Rasmlar: xotira keshi (LRU) -> shifrlangan disk keshi -> Telegram (`TelegramService.fetchBytes`).
 * Disk keshi Android Keystore kaliti bilan AES-GCM (Flutter ilovasidagi "hamma fayl shifrlanadi" qoidasiga mos).
 */
class ImageStore(context: Context, private val tg: TelegramService) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    // Doimiy joy (`filesDir`): tizim xotira kam bo'lganda ham o'chirib yubormaydi; faqat foydalanuvchi "tozalash" bossa o'chadi.
    private val dir = File(context.filesDir, "aru_img").apply { mkdirs() }
    private val decodeGate = Semaphore(if (uz.arumediatv.com.core.Perf.low) 2 else 3)
    private val inflight = ConcurrentHashMap<String, CompletableDeferred<Bitmap?>>()

    init {
        // Eski joydan (cacheDir) ko'chirish: qayta yuklanmasin.
        try {
            val old = File(context.cacheDir, "aru_img")
            old.listFiles()?.forEach { if (!File(dir, it.name).exists()) it.renameTo(File(dir, it.name)) else it.delete() }
            old.delete()
        } catch (_: Exception) {
        }
    }

    // Ekrandagi rasmlarni Compose o'zi ushlab turadi; bu kesh faqat yaqinda ko'rilganlar uchun. Kichraytirilgan nusxa diskda bo'lgani uchun
    // chiqarib yuborilgan rasm tez qaytadi.
    private val mem = object : LruCache<String, Bitmap>((Runtime.getRuntime().maxMemory() / 4L).toInt().coerceAtLeast(6 shl 20)) {
        override fun sizeOf(key: String, value: Bitmap): Int = value.byteCount
    }

    /** Xotiradagi tayyor rasm (bo'lsa) — kompozitsiya vaqtida miltillamasligi uchun. */
    fun peek(url: String): Bitmap? = Names.fileNameOf(url).takeIf { it.isNotEmpty() }?.let { mem.get(it) }

    suspend fun load(url: String, maxPx: Int = 720, network: Boolean = true): Bitmap? {
        val name = Names.fileNameOf(url)
        if (name.isEmpty()) return null
        mem.get(name)?.let { return it }

        val mine = CompletableDeferred<Bitmap?>()
        val key = if (network) name else "$name#disk"
        val other = inflight.putIfAbsent(key, mine)
        if (other != null) return other.await()
        scope.launch {
            val bmp = try {
                loadUncached(url, name, maxPx, network)
            } catch (_: Throwable) {
                null
            }
            if (bmp != null) mem.put(name, bmp)
            inflight.remove(key)
            mine.complete(bmp)
        }
        return mine.await()
    }

    /** Tizim xotira haqida signal berganda: ilova fonda -> hammasi, xotira kam -> yarmi tozalanadi. */
    fun trim(hidden: Boolean) {
        if (hidden) mem.evictAll() else mem.trimToSize(mem.size() / 2)
    }

    private fun writeThumb(t: File, bytes: ByteArray) {
        try {
            val bmp = BitmapFactory.Options().let { o ->
                o.inJustDecodeBounds = true
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size, o)
                var sample = 1
                while (o.outWidth / (sample * 2) >= THUMB_PX && o.outHeight / (sample * 2) >= THUMB_PX) sample *= 2
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size, BitmapFactory.Options().apply { inSampleSize = sample })
            } ?: return
            val out = java.io.ByteArrayOutputStream()
            @Suppress("DEPRECATION")
            bmp.compress(if (android.os.Build.VERSION.SDK_INT >= 30) Bitmap.CompressFormat.WEBP_LOSSY else Bitmap.CompressFormat.WEBP, 85, out)
            bmp.recycle()
            t.writeBytes(SecureStore.seal(out.toByteArray()))
        } catch (_: Throwable) {
        }
    }

    private suspend fun loadUncached(url: String, name: String, maxPx: Int, network: Boolean): Bitmap? {
        val f = File(dir, "$name.bin")
        val t = File(dir, "$name.t.bin")
        // Kichik so'rovlar kichraytirilgan nusxadan o'qiladi (tez, kam xotira).
        if (maxPx <= THUMB_PX && t.exists()) {
            val plain = try { SecureStore.open(t.readBytes()) } catch (_: Exception) { t.delete(); null }
            if (plain != null) decode(plain, maxPx)?.let { return it }
        }
        if (f.exists()) {
            val plain = try {
                SecureStore.open(f.readBytes())
            } catch (_: Exception) {
                f.delete()
                null
            }
            if (plain != null) {
                if (maxPx <= THUMB_PX && !t.exists()) withContext(Dispatchers.Default) { writeThumb(t, plain) }
                decode(plain, maxPx)?.let { return it }
            }
        }
        if (!network) return null
        val bytes = tg.fetchBytes(url) ?: return null
        val bmp = decode(bytes, maxPx) ?: return null
        try {
            f.writeBytes(SecureStore.seal(bytes))
        } catch (_: Exception) {
        }
        if (maxPx <= THUMB_PX) withContext(Dispatchers.Default) { writeThumb(t, bytes) }
        return bmp
    }

    private suspend fun decode(bytes: ByteArray, maxPxIn: Int): Bitmap? = decodeGate.withPermit {
        withContext(Dispatchers.Default) {
            val low = uz.arumediatv.com.core.Perf.low
            // Kuchsiz qurilmada rasm kichikroq va 2 baravar kam xotira oladigan formatda (RGB_565) dekodlanadi.
            val maxPx = if (low) minOf(maxPxIn, 540) else maxPxIn
            val o = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, o)
            var sample = 1
            while (o.outWidth / (sample * 2) >= maxPx && o.outHeight / (sample * 2) >= maxPx) sample *= 2
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, BitmapFactory.Options().apply { inSampleSize = sample; if (low) inPreferredConfig = Bitmap.Config.RGB_565 })
        }
    }

    fun clearDisk() {
        dir.listFiles()?.forEach { it.delete() }
        mem.evictAll()
    }

    private companion object {
        const val THUMB_PX = 720
    }

    fun diskBytes(): Long = dir.listFiles()?.sumOf { it.length() } ?: 0L
}

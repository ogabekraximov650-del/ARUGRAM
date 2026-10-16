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
    private val dir = File(context.cacheDir, "aru_img").apply { mkdirs() }
    private val decodeGate = Semaphore(3)
    private val inflight = ConcurrentHashMap<String, CompletableDeferred<Bitmap?>>()

    private val mem = object : LruCache<String, Bitmap>((Runtime.getRuntime().maxMemory() / 8L).toInt().coerceAtLeast(8 shl 20)) {
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

    private suspend fun loadUncached(url: String, name: String, maxPx: Int, network: Boolean): Bitmap? {
        val f = File(dir, "$name.bin")
        if (f.exists()) {
            val plain = try {
                SecureStore.open(f.readBytes())
            } catch (_: Exception) {
                f.delete()
                null
            }
            if (plain != null) decode(plain, maxPx)?.let { return it }
        }
        if (!network) return null
        val bytes = tg.fetchBytes(url) ?: return null
        val bmp = decode(bytes, maxPx) ?: return null
        try {
            f.writeBytes(SecureStore.seal(bytes))
        } catch (_: Exception) {
        }
        return bmp
    }

    private suspend fun decode(bytes: ByteArray, maxPx: Int): Bitmap? = decodeGate.withPermit {
        withContext(Dispatchers.Default) {
            val o = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, o)
            var sample = 1
            while (o.outWidth / (sample * 2) >= maxPx && o.outHeight / (sample * 2) >= maxPx) sample *= 2
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, BitmapFactory.Options().apply { inSampleSize = sample })
        }
    }

    fun clearDisk() {
        dir.listFiles()?.forEach { it.delete() }
        mem.evictAll()
    }

    fun diskBytes(): Long = dir.listFiles()?.sumOf { it.length() } ?: 0L
}

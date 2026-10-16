package uz.arumediatv.com.data

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.media.MediaMetadataRetriever
import android.os.Build
import android.util.LruCache
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import uz.arumediatv.com.core.SecureStore
import java.io.ByteArrayOutputStream
import java.io.File
import java.net.URLEncoder
import java.util.concurrent.ConcurrentHashMap

/**
 * Tomosha tarixi uchun TO'XTAGAN JOYDAGI KADR (Flutter `WatchHistory.thumbnail` ga mos).
 *
 * Kadr Rust yadrosining `/thumb?u=<manba>&ms=<vaqt>` yo'lidan olinadi: u faylning faqat kerakli baytlarini
 * (moov + bitta kalit kadr) olib, bitta kadrlik MP4 yasaydi; `MediaMetadataRetriever` undan JPEG chiqaradi.
 * Natija diskda shifrlangan (`SecureStore`) saqlanadi; kalit `<video>_<to'xtagan ms>` — joy o'zgarsa yangi kadr
 * yasaladi, eskisi o'chadi. Navbat bitta-bitta ishlaydi; pleyer ochiq paytda (`gate.busy`) kutadi.
 */
class ThumbStore(context: Context, private val tg: TelegramService, private val cache: VideoCache) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val dir = File(context.filesDir, "aru_thumb").apply { mkdirs() }
    private val queue = Mutex()
    private val inflight = ConcurrentHashMap<String, CompletableDeferred<Bitmap?>>()
    private val failedAt = ConcurrentHashMap<String, Long>()
    private val mem = object : LruCache<String, Bitmap>((Runtime.getRuntime().maxMemory() / 6L).toInt().coerceAtLeast(12 shl 20)) {
        override fun sizeOf(key: String, value: Bitmap): Int = value.byteCount
    }

    private fun videoKey(item: HistoryItem): String =
        item.videoUrl.substringBefore('?').substringAfterLast('/').replace(Regex("[^A-Za-z0-9_.-]"), "_")

    fun keyOf(item: HistoryItem): String = "${videoKey(item)}_${item.positionMs}"

    fun peek(item: HistoryItem): Bitmap? = if (item.videoUrl.isEmpty()) null else mem.get(keyOf(item))

    /** Kadr (xotira -> disk -> yasash). `null` — yasab bo'lmadi: chaqiruvchi posterni ko'rsatadi. */
    suspend fun load(item: HistoryItem): Bitmap? {
        if (item.videoUrl.isEmpty() || item.positionMs <= 0) return null
        val key = keyOf(item)
        mem.get(key)?.let { return it }
        // Diskda tayyor kadr bor — navbatni kutmasdan darhol (aylantirganda kadr yo'qolib qolmasin).
        withContext(Dispatchers.IO) { fromDisk(key) }?.let { mem.put(key, it); return it }
        val last = failedAt[key]
        if (last != null && System.currentTimeMillis() - last < RETRY_MS) return null
        val mine = CompletableDeferred<Bitmap?>()
        inflight.putIfAbsent(key, mine)?.let { return it.await() }
        scope.launch {
            val bmp = try {
                queue.withLock { build(item, key) }
            } catch (_: Throwable) {
                null
            }
            if (bmp != null) {
                mem.put(key, bmp)
                failedAt.remove(key)
            } else {
                failedAt[key] = System.currentTimeMillis()
            }
            inflight.remove(key)
            mine.complete(bmp)
        }
        return mine.await()
    }

    private fun file(key: String) = File(dir, "thumb_$key.bin")

    private fun decodeSmall(b: ByteArray): Bitmap? {
        val o = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(b, 0, b.size, o)
        var sample = 1
        while (o.outWidth / (sample * 2) >= 720) sample *= 2
        return BitmapFactory.decodeByteArray(b, 0, b.size, BitmapFactory.Options().apply { inSampleSize = sample })
    }

    private fun fromDisk(key: String): Bitmap? {
        val f = file(key)
        if (!f.exists()) return null
        return try {
            decodeSmall(SecureStore.open(f.readBytes()))
        } catch (_: Exception) {
            f.delete()
            null
        }
    }

    private suspend fun build(item: HistoryItem, key: String): Bitmap? {
        fromDisk(key)?.let { return it }
        val f = file(key)
        // Pleyer ochiq — tarmoq faqat videoniki.
        while (uz.arumediatv.com.AppServices.gate.busy.get() > 0) delay(400)
        val src = item.thumbUrl.ifEmpty { item.videoUrl }
        val source = if (cache.isComplete(src)) src else tg.playUrl(src)
        val bytes = if (source != null) grab(source, item.positionMs) else null
        if (bytes == null) return anyOfVideo(videoKey(item))
        try {
            f.writeBytes(SecureStore.seal(bytes))
        } catch (_: Exception) {
        }
        // Shu videoning eski kadrlari endi kerak emas.
        val prefix = "thumb_${videoKey(item)}_"
        dir.listFiles()?.forEach { if (it.name.startsWith(prefix) && it.name != f.name) it.delete() }
        return decodeSmall(bytes)
    }

    /** Shu videoning diskdagi ISTALGAN kadri (yangisini yasab bo'lmasa — posterdan yaxshi). */
    private fun anyOfVideo(vk: String): Bitmap? {
        val prefix = "thumb_${vk}_"
        for (f in dir.listFiles().orEmpty()) {
            if (!f.name.startsWith(prefix)) continue
            try {
                val plain = SecureStore.open(f.readBytes())
                decodeSmall(plain)?.let { return it }
            } catch (_: Exception) {
            }
        }
        return null
    }

    private suspend fun grab(source: String, ms: Long): ByteArray? = withContext(Dispatchers.IO) {
        val port = cache.ensureStarted()
        if (port <= 0) return@withContext null
        val url = "http://127.0.0.1:$port/thumb?u=${URLEncoder.encode(source, "UTF-8")}&ms=$ms"
        withTimeoutOrNull(60_000) { frameJpeg(url, 1280, 90) }
    }

    private fun frameJpeg(url: String, maxWidth: Int, quality: Int): ByteArray? {
        val r = MediaMetadataRetriever()
        try {
            r.setDataSource(url, HashMap<String, String>())
            val durationMs = r.extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION)?.toLongOrNull() ?: 0L
            fun at(us: Long, opt: Int): Bitmap? = try { r.getFrameAtTime(us, opt) } catch (_: Throwable) { null }
            // Bo'lakda bitta kadr bor: avval oxirgi, keyin boshi, keyin tizim tanlagan, keyin 0-raqamli.
            var bmp: Bitmap? = if (durationMs > 1) at((durationMs - 1) * 1000, MediaMetadataRetriever.OPTION_CLOSEST) else null
            if (bmp == null) bmp = at(0, MediaMetadataRetriever.OPTION_CLOSEST_SYNC)
            if (bmp == null) bmp = at(0, MediaMetadataRetriever.OPTION_CLOSEST)
            if (bmp == null) bmp = try { r.getFrameAtTime() } catch (_: Throwable) { null }
            if (bmp == null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) bmp = try { r.getFrameAtIndex(0) } catch (_: Throwable) { null }
            var frame = bmp ?: return null
            if (frame.width > maxWidth) {
                val h = (frame.height.toLong() * maxWidth / frame.width).toInt().coerceAtLeast(1)
                val scaled = Bitmap.createScaledBitmap(frame, maxWidth, h, true)
                if (scaled !== frame) {
                    frame.recycle()
                    frame = scaled
                }
            }
            val out = ByteArrayOutputStream()
            frame.compress(Bitmap.CompressFormat.JPEG, quality, out)
            frame.recycle()
            return out.toByteArray()
        } catch (_: Throwable) {
            return null
        } finally {
            try { r.release() } catch (_: Throwable) {}
        }
    }

    /** Tarixdan o'chirilgan yoki hisobdan chiqilganda. */
    fun clear() {
        dir.listFiles()?.forEach { it.delete() }
        mem.evictAll()
    }

    fun diskBytes(): Long = dir.listFiles()?.sumOf { it.length() } ?: 0L

    private companion object {
        const val RETRY_MS = 20_000L
    }
}

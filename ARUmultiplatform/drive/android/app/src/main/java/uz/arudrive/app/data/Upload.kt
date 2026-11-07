package uz.arudrive.app.data

import android.content.Context
import android.graphics.Bitmap
import android.media.MediaMetadataRetriever
import android.net.Uri
import android.provider.OpenableColumns
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.withContext

/** Tanlangan fayl haqida ma'lumot. */
class Picked(val uri: Uri, val name: String, val size: Long, val mime: String) {
    val isVideo get() = mime.startsWith("video/")
}

object Upload {
    private const val PARALLEL = 4

    fun pick(ctx: Context, uri: Uri): Picked {
        var name = "fayl"
        var size = -1L
        ctx.contentResolver.query(uri, null, null, null, null)?.use { c ->
            if (c.moveToFirst()) {
                c.getColumnIndex(OpenableColumns.DISPLAY_NAME).takeIf { it >= 0 }?.let { name = c.getString(it) ?: name }
                c.getColumnIndex(OpenableColumns.SIZE).takeIf { it >= 0 }?.let { size = c.getLong(it) }
            }
        }
        return Picked(uri, name, size, ctx.contentResolver.getType(uri).orEmpty())
    }

    /** Videoning o'lchami, davomiyligi (ms) va 1-soniyadagi kadr (JPEG, eni ≤ 720). */
    class VideoInfo(val w: Int, val h: Int, val durMs: Long, val jpeg: ByteArray?)

    fun videoInfo(ctx: Context, uri: Uri): VideoInfo {
        val r = MediaMetadataRetriever()
        return try {
            r.setDataSource(ctx, uri)
            var w = r.extractMetadata(MediaMetadataRetriever.METADATA_KEY_VIDEO_WIDTH)?.toIntOrNull() ?: 0
            var h = r.extractMetadata(MediaMetadataRetriever.METADATA_KEY_VIDEO_HEIGHT)?.toIntOrNull() ?: 0
            val rot = r.extractMetadata(MediaMetadataRetriever.METADATA_KEY_VIDEO_ROTATION)?.toIntOrNull() ?: 0
            if (rot == 90 || rot == 270) { val t = w; w = h; h = t }
            val dur = r.extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION)?.toLongOrNull() ?: 0
            val frame = r.getFrameAtTime(1_000_000, MediaMetadataRetriever.OPTION_CLOSEST_SYNC)
            var jpeg: ByteArray? = null
            if (frame != null) {
                val scaled = if (frame.width > 720) Bitmap.createScaledBitmap(frame, 720, (frame.height * 720L / frame.width).toInt().coerceAtLeast(1), true) else frame
                val out = java.io.ByteArrayOutputStream()
                scaled.compress(Bitmap.CompressFormat.JPEG, 82, out)
                jpeg = out.toByteArray()
            }
            VideoInfo(w, h, dur, jpeg)
        } catch (_: Exception) {
            VideoInfo(0, 0, 0, null)
        } finally {
            runCatching { r.release() }
        }
    }

    /**
     * Faylni BUTUN holda R2 ga yuboradi (18 MiB qismlar, ko'p qismli yuklash, 4 ta parallel); keyin WORKER o'zi uni R2'dan olib
     * 18 MiB bo'laklab Telegram'ga saqlaydi (`finish` dan keyin darhol + har daqiqada cron). [onProgress] — yuborilgan bayt.
     */
    suspend fun run(ctx: Context, p: Picked, note: (String) -> Unit = {}, onProgress: (Long) -> Unit): String = withContext(Dispatchers.IO) {
        note("Video tahlil qilinmoqda…")
        val info = if (p.isVideo) videoInfo(ctx, p.uri) else VideoInfo(0, 0, 0, null)
        note("Yozuv ochilmoqda…")
        val init = Api.init(p.name, p.size, if (p.isVideo) "video" else "file", p.mime, info.w, info.h, info.durMs)
        // PARALLEL: bir vaqtda 4 tagacha qism R2 ga ketadi (har biri alohida ulanishda) — sekin/uzoq tarmoqda ancha tez.
        // Xotira: 4 ta 18 MiB bufer qayta ishlatiladi (havza), o'qish faqat bo'sh bufer chiqqanda davom etadi.
        val pool = kotlinx.coroutines.channels.Channel<ByteArray>(PARALLEL)
        repeat(PARALLEL) { pool.trySend(ByteArray(init.partSize)) }
        val parts = java.util.concurrent.ConcurrentLinkedQueue<Pair<Int, String>>()
        val partBytes = java.util.concurrent.ConcurrentHashMap<Int, Long>()
        var sent = 0L
        kotlinx.coroutines.coroutineScope {
            val jobs = ArrayList<kotlinx.coroutines.Deferred<Unit>>()
            ctx.contentResolver.openInputStream(p.uri)!!.use { input ->
                var n = 1
                while (sent < p.size) {
                    val buf = pool.receive()
                    var len = 0
                    while (len < buf.size) {
                        val r = input.read(buf, len, buf.size - len)
                        if (r < 0) break
                        len += r
                    }
                    if (len == 0) { pool.send(buf); break }
                    sent += len
                    val partN = n
                    val l = len
                    jobs += async {
                        try {
                            val etag = Api.part(init.id, init.uploadId, partN, buf, l, { b -> partBytes[partN] = b; onProgress(partBytes.values.sum()) }, note)
                            parts.add(partN to etag)
                            partBytes[partN] = l.toLong()
                            onProgress(partBytes.values.sum())
                        } finally {
                            pool.send(buf)
                        }
                    }
                    n++
                }
            }
            jobs.forEach { it.await() }
        }
        if (sent != p.size) throw java.io.IOException("Fayl to'liq o'qilmadi ($sent / ${p.size})")
        // Kichik rasm fayl TO'LIQ yuklangandan KEYIN qo'shiladi (butun fayl R2 da, saqlash boshlanishidan oldin).
        info.jpeg?.let {
            note("Kichik rasm yuborilmoqda…")
            runCatching { Api.thumb(init.id, it) }.onFailure { e -> note("Rasm yuborilmadi: ${e.message}") }
        }
        note("Yakunlanmoqda…")
        Api.finish(init.id, init.uploadId, parts.sortedBy { it.first })
        init.id
    }
}

package uz.arumediatv.com.vd

import android.media.MediaDataSource
import okhttp3.Request
import uz.arumediatv.com.data.Http
import java.util.concurrent.TimeUnit

/**
 * Video kadrini (`MediaMetadataRetriever`) YANGI TIZIMDAN (vd) olish: Telegram hisobi kerak emas. Kerakli 1 MiB bloklar Cloudflare keshidan
 * (bo'lak oldindan isitiladi) shifrlangan holda olinib, qurilmada ochiladi. Faqat bir nechta blok o'qiladi (moov + kerakli kadr).
 * UZ: vd kadr manbasi. EN: frame source for vd files. RU: источник кадров для vd-файлов.
 */
class VdFrameSource(private val info: VdApi.Info) : MediaDataSource() {
    private val key = VdCtr.hex(info.key)
    private val http = Http.client.newBuilder().readTimeout(60, TimeUnit.SECONDS).build()
    private val blocks = object : LinkedHashMap<Long, ByteArray>(16, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Long, ByteArray>?) = size > 8
    }
    private val warmed = HashSet<Long>()

    override fun getSize(): Long = info.size

    private fun block(i: Long): ByteArray? {
        synchronized(blocks) { blocks[i]?.let { return it } }
        val start = i * BLOCK
        val end = minOf(start + BLOCK, info.size) - 1
        if (end < start) return null
        val chunk = start / VdCtr.CHUNK
        val already = synchronized(warmed) { chunk in warmed }
        if (!already) {
            if (!VdApi.warmBlocking(info.name, chunk)) return null
            synchronized(warmed) { warmed.add(chunk) }
        }
        val req = Request.Builder().url(VdApi.playUrl(info.name)).header("Range", "bytes=$start-$end").build()
        val enc = try {
            http.newCall(req).execute().use { r -> if (r.code != 206 && r.code != 200) null else r.body?.bytes() }
        } catch (_: Exception) { null } ?: return null
        if (enc.size.toLong() != end - start + 1) return null
        val plain = VdCtr.apply(key, start, enc)
        synchronized(blocks) { blocks[i] = plain }
        return plain
    }

    override fun readAt(position: Long, buffer: ByteArray, offset: Int, size: Int): Int {
        if (position >= info.size) return -1
        var pos = position
        var done = 0
        val want = minOf(size.toLong(), info.size - position).toInt()
        while (done < want) {
            val b = block(pos / BLOCK) ?: return if (done > 0) done else -1
            val inBlock = (pos % BLOCK).toInt()
            val n = minOf(want - done, b.size - inBlock)
            if (n <= 0) break
            System.arraycopy(b, inBlock, buffer, offset + done, n)
            done += n
            pos += n
        }
        return if (done == 0) -1 else done
    }

    override fun close() {
        synchronized(blocks) { blocks.clear() }
    }

    private companion object {
        const val BLOCK = 1L * 1024 * 1024
    }
}

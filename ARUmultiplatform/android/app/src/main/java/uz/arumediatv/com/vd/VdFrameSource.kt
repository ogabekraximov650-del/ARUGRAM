package uz.arumediatv.com.vd

import android.media.MediaDataSource

/**
 * Video kadrini (`MediaMetadataRetriever`) YANGI TIZIMDAN (vd) olish: Telegram hisobi kerak emas. Kerakli 1 MiB bloklar pleyer bilan umumiy
 * telefon keshidan, yo'q bo'lsa Cloudflare keshidan olinib qurilmada ochiladi. Faqat bir nechta blok o'qiladi (moov + kerakli kadr).
 * UZ: vd kadr manbasi. EN: frame source for vd files. RU: источник кадров для vd-файлов.
 */
class VdFrameSource(private val info: VdApi.Info) : MediaDataSource() {
    private val key = VdCtr.hex(info.key)
    private val blocks = object : LinkedHashMap<Long, ByteArray>(16, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Long, ByteArray>?) = size > 8
    }

    override fun getSize(): Long = info.size

    /** Bo'lak pleyer bilan UMUMIY keshdan ([VdBlocks]): pleyer ko'rgan joy qayta yuklanmaydi, bu yerda olingani pleyerga ham yaraydi. */
    private fun block(i: Long): ByteArray? {
        synchronized(blocks) { blocks[i]?.let { return it } }
        if (i * BLOCK >= info.size) return null
        val plain = try {
            VdBlocks.get(uz.arumediatv.com.AppServices.appContext, info.name, info.size, key, i)
        } catch (_: Exception) {
            return null
        }
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

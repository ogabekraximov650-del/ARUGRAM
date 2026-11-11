package uz.arumediatv.com.vd

import android.content.Context
import android.net.Uri
import androidx.media3.common.C
import androidx.media3.common.PlaybackException
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.BaseDataSource
import androidx.media3.datasource.DataSourceException
import androidx.media3.datasource.DataSpec

/**
 * ExoPlayer uchun manba: baytlarni 1 MiB bo'laklab [VdBlocks] dan oladi — bo'lak telefondagi shifrlangan keshda ([BlockCache]) bo'lsa tarmoqsiz,
 * bo'lmasa aniq shu bo'lak serverdan olinib, shifri ochilib keshga yoziladi. Ko'rilgan joy keyingi safar tarmoqsiz ochiladi.
 * [total] — fayl hajmi; kesh kaliti — fayl `id`si; [key] — faylning AES-128-CTR kaliti.
 */
@UnstableApi
class EncCacheDataSource(
    private val ctx: Context,
    private val id: String,
    private val total: Long,
    private val key: ByteArray,
) : BaseDataSource(false) {
    private var uri: Uri? = null
    private var spec: DataSpec? = null
    private var pos = 0L
    private var remaining = 0L
    private var opened = false
    private var block: ByteArray? = null
    private var blockIdx = -1L

    override fun open(dataSpec: DataSpec): Long {
        uri = dataSpec.uri
        spec = dataSpec
        transferInitializing(dataSpec)
        pos = dataSpec.position
        if (pos >= total) throw DataSourceException(PlaybackException.ERROR_CODE_IO_READ_POSITION_OUT_OF_RANGE)
        val want = if (dataSpec.length != C.LENGTH_UNSET.toLong()) dataSpec.length else total - pos
        remaining = minOf(want, total - pos)
        opened = true
        transferStarted(dataSpec)
        return remaining
    }

    override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
        if (length == 0) return 0
        if (remaining <= 0L) return C.RESULT_END_OF_INPUT
        val idx = pos / BlockCache.BLOCK
        if (blockIdx != idx) {
            block = loadBlock(idx)
            blockIdx = idx
        }
        val b = block ?: return C.RESULT_END_OF_INPUT
        val inBlock = (pos - idx * BlockCache.BLOCK).toInt()
        val n = minOf(length.toLong(), remaining, (b.size - inBlock).toLong()).toInt()
        if (n <= 0) return C.RESULT_END_OF_INPUT
        System.arraycopy(b, inBlock, buffer, offset, n)
        pos += n
        remaining -= n
        bytesTransferred(n)
        return n
    }

    private fun loadBlock(idx: Long): ByteArray = VdBlocks.get(ctx, id, total, key, idx)

    override fun getUri(): Uri? = uri

    override fun close() {
        block = null
        blockIdx = -1
        if (opened) {
            opened = false
            transferEnded()
        }
    }
}

package uz.arudrive.app.data

import android.content.Context
import android.net.Uri
import androidx.media3.common.C
import androidx.media3.common.PlaybackException
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.BaseDataSource
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DataSourceException
import androidx.media3.datasource.DataSpec
import java.io.EOFException
import java.io.IOException

/**
 * ExoPlayer uchun manba: avval telefondagi shifrlangan 1 MiB bo'laklar keshidan ([BlockCache]) o'qiydi; bo'lak yo'q bo'lsa [upstream]
 * (worker orqali) dan ketma-ket o'qib, bo'lakni shifrlab doimiy xotiraga yozadi va pleyerga uzatadi. Ko'rilgan joy keyingi safar tarmoqsiz ochiladi.
 * [total] — fayl hajmi (ro'yxatdan); kesh kaliti — fayl `id`si (havoladagi token o'zgarsa ham kesh saqlanadi).
 */
@UnstableApi
class EncCacheDataSource(
    private val ctx: Context,
    private val id: String,
    private val total: Long,
    private val upstream: DataSource,
) : BaseDataSource(false) {
    private var uri: Uri? = null
    private var spec: DataSpec? = null
    private var pos = 0L
    private var remaining = 0L
    private var opened = false
    private var block: ByteArray? = null
    private var blockIdx = -1L
    private var upPos = -1L // upstream keyingi beradigan bayt joyi; -1 — yopiq

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

    private fun blockLen(idx: Long): Int = minOf(BlockCache.BLOCK.toLong(), total - idx * BlockCache.BLOCK).toInt()

    private fun loadBlock(idx: Long): ByteArray {
        val len = blockLen(idx)
        BlockCache.read(ctx, id, idx, len)?.let { return it }
        val start = idx * BlockCache.BLOCK
        var lastErr: IOException? = null
        repeat(2) {
            try {
                if (upPos != start) {
                    closeUp()
                    upstream.open(spec!!.buildUpon().setPosition(start).setLength(C.LENGTH_UNSET.toLong()).build())
                    upPos = start
                }
                val buf = ByteArray(len)
                var got = 0
                while (got < len) {
                    val r = upstream.read(buf, got, len - got)
                    if (r == C.RESULT_END_OF_INPUT) throw EOFException("kutilmagan oqim oxiri")
                    got += r
                }
                upPos = start + len
                BlockCache.write(ctx, id, idx, buf)
                return buf
            } catch (e: IOException) {
                lastErr = e
                closeUp() // ulanish uzilgan/eskirgan bo'lishi mumkin — bir marta qayta ochib ko'ramiz
            }
        }
        throw lastErr ?: IOException("bo'lak o'qilmadi")
    }

    private fun closeUp() {
        if (upPos >= 0) {
            try { upstream.close() } catch (_: IOException) {}
            upPos = -1
        }
    }

    override fun getUri(): Uri? = uri

    override fun close() {
        closeUp()
        block = null
        blockIdx = -1
        if (opened) {
            opened = false
            transferEnded()
        }
    }
}

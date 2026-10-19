package uz.arugram.app.ui.main

import android.net.Uri
import android.util.Base64
import androidx.annotation.OptIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.MediaItem
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.BaseDataSource
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DataSpec
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.ProgressiveMediaSource
import androidx.media3.ui.PlayerView
import uz.arugram.app.core.RustCore
import java.io.IOException

private const val CHUNK = 512 * 1024

/** Telegram videosini yuklab bo'lmasdan o'qiydigan manba: Rust'dan 512 KB bo'laklar so'raydi, oxirgi bir nechtasini xotirada saqlaydi. */
@OptIn(UnstableApi::class)
class TgStreamDataSource(private val spec: String, private val msgId: Int) : BaseDataSource(true) {
    private var uri: Uri? = null
    private var pos = 0L
    private var end = 0L
    private var opened = false

    companion object {
        private val chunks = object : LinkedHashMap<String, ByteArray>(16, 0.75f, true) {
            override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, ByteArray>?) = size > 12
        }
        private val sizes = HashMap<String, Long>()
    }

    private fun chunk(i: Int): ByteArray {
        val k = "${spec}_${msgId}_$i"
        synchronized(chunks) { chunks[k]?.let { return it } }
        var last: String? = null
        repeat(3) {
            val j = RustCore.tgMediaRead(spec, msgId, i)
            val err = j.optString("error")
            if (err.isEmpty()) {
                val data = Base64.decode(j.optString("data"), Base64.DEFAULT)
                synchronized(chunks) { chunks[k] = data; sizes["${spec}_$msgId"] = j.optLong("size") }
                return data
            }
            last = err
        }
        throw IOException(last ?: "Yuklab bo‘lmadi")
    }

    override fun open(dataSpec: DataSpec): Long {
        uri = dataSpec.uri
        transferInitializing(dataSpec)
        val sk = "${spec}_$msgId"
        var total = synchronized(chunks) { sizes[sk] }
        if (total == null) {
            chunk(0)
            total = synchronized(chunks) { sizes[sk] } ?: 0L
        }
        pos = dataSpec.position
        end = if (dataSpec.length >= 0) pos + dataSpec.length else total
        if (end > total) end = total
        opened = true
        transferStarted(dataSpec)
        return end - pos
    }

    override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
        if (length == 0) return 0
        if (pos >= end) return -1
        val idx = (pos / CHUNK).toInt()
        val data = chunk(idx)
        val inChunk = (pos - idx.toLong() * CHUNK).toInt()
        if (inChunk >= data.size) return -1
        val n = minOf(length, data.size - inChunk, (end - pos).toInt())
        System.arraycopy(data, inChunk, buffer, offset, n)
        pos += n
        bytesTransferred(n)
        // Keyingi bo'lakni oldindan yuklash (ijro to'xtab qolmasligi uchun).
        if (inChunk + n >= data.size && pos < end) Thread { runCatching { chunk(idx + 1) } }.start()
        return n
    }

    override fun getUri(): Uri? = uri

    override fun close() {
        uri = null
        if (opened) {
            opened = false
            transferEnded()
        }
    }
}

/** Telegram videosini to'liq yuklamasdan o'ynatadi (ijro darhol boshlanadi, surish ishlaydi). */
@OptIn(UnstableApi::class)
@Composable
fun StreamVideo(spec: String, msgId: Int, modifier: Modifier = Modifier) {
    val ctx = LocalContext.current
    val player = remember(spec, msgId) {
        val factory = DataSource.Factory { TgStreamDataSource(spec, msgId) }
        ExoPlayer.Builder(ctx).build().apply {
            setMediaSource(ProgressiveMediaSource.Factory(factory).createMediaSource(MediaItem.fromUri("tg://$msgId.mp4")))
            playWhenReady = true
            prepare()
        }
    }
    DisposableEffect(player) { onDispose { player.release() } }
    AndroidView(modifier = modifier, factory = { c -> PlayerView(c).also { it.player = player; it.useController = true } })
}

/** Oqim bilan ijro etuvchi pleyer (ko'rgich o'zi boshqaruv chizadi). */
@OptIn(UnstableApi::class)
fun createStreamPlayer(ctx: android.content.Context, spec: String, msgId: Int): ExoPlayer {
    val factory = DataSource.Factory { TgStreamDataSource(spec, msgId) }
    return ExoPlayer.Builder(ctx).build().apply {
        setMediaSource(ProgressiveMediaSource.Factory(factory).createMediaSource(MediaItem.fromUri("tg://$msgId.mp4")))
        playWhenReady = true
        prepare()
    }
}

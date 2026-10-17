package uz.arumediatv.com.ui.season

import android.content.Context
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import io.flutter.plugins.videoplayer.AruDataSource

/**
 * Media3 ExoPlayer o'rami: ijro holati Compose `State` sifatida (ekran kuzatadi).
 * Manba — Rust mahalliy serveri (Telegram, Range'ni qo'llaydi) yoki worker `/api/play/...`.
 */
@androidx.annotation.OptIn(androidx.media3.common.util.UnstableApi::class)
@Stable
class AruPlayer(context: Context) {
    val exo: ExoPlayer = ExoPlayer.Builder(
        context.applicationContext,
        androidx.media3.exoplayer.DefaultRenderersFactory(context.applicationContext)
            .setEnableDecoderFallback(true)
            .setExtensionRendererMode(androidx.media3.exoplayer.DefaultRenderersFactory.EXTENSION_RENDERER_MODE_PREFER),
    ).setLoadControl(
        // Flutterdagi AruLoadControl bilan bir xil: oldinga 15..30 s, orqaga 10 s, xotira chegarasi 24 MB.
        androidx.media3.exoplayer.DefaultLoadControl.Builder()
            .setBufferDurationsMs(15_000, 30_000, 1_000, 2_000)
            .setBackBuffer(10_000, true)
            .setTargetBufferBytes(if (uz.arumediatv.com.core.Perf.low) 12 * 1024 * 1024 else 24 * 1024 * 1024)
            .setPrioritizeTimeOverSizeThresholds(false)
            .build(),
    )    .build()

    var playing by mutableStateOf(false)
        private set
    var buffering by mutableStateOf(false)
        private set
    var ended by mutableStateOf(false)
        private set
    var ready by mutableStateOf(false)
        private set
    var positionMs by mutableLongStateOf(0L)
        private set
    var durationMs by mutableLongStateOf(0L)
        private set
    var speed by mutableFloatStateOf(1f)
        private set

    /** Xato matni (`null` — yo'q). */
    var error by mutableStateOf<String?>(null)

    private val listener = object : Player.Listener {
        override fun onIsPlayingChanged(isPlaying: Boolean) {
            playing = isPlaying
        }

        override fun onPlaybackStateChanged(state: Int) {
            buffering = state == Player.STATE_BUFFERING
            ended = state == Player.STATE_ENDED
            if (state == Player.STATE_READY) {
                ready = true
                tick()
            }
        }

        override fun onTracksChanged(tracks: androidx.media3.common.Tracks) {
            // Ovoz bor, video tanlanmagan (kodek/profil qo'llab-quvvatlanmaydi) — jim qora ekran o'rniga sababni ko'rsatamiz.
            val v = tracks.groups.filter { it.type == androidx.media3.common.C.TRACK_TYPE_VIDEO }
            if (v.isNotEmpty() && v.none { it.isSelected }) {
                val f = v.first().getTrackFormat(0)
                error = "Video kodek qo'llab-quvvatlanmaydi: ${f.sampleMimeType} ${f.width}x${f.height} ${f.codecs ?: ""}".trim()
            }
        }

        override fun onPlayerError(e: PlaybackException) {
            val root = generateSequence<Throwable>(e) { it.cause }.last()
            val http = (e.cause as? androidx.media3.datasource.HttpDataSource.InvalidResponseCodeException)?.responseCode
            val where = when {
                lastUri.startsWith("aru://") || (lastUri.contains("127.0.0.1") && lastUri.contains("/tg/")) -> "Telegram"
                lastUri.contains("127.0.0.1") -> "mahalliy kesh"
                else -> "worker"
            }
            error = "Video ochilmadi [$where] ${e.errorCodeName}" +
                (http?.let { " HTTP $it" } ?: "") +
                (root.message?.takeIf { it.isNotBlank() }?.let { " — ${it.take(120)}" } ?: "")
            playing = false
        }
    }

    init {
        exo.addListener(listener)
    }

    private var lastUri = ""
    private val reporter = java.util.concurrent.Executors.newSingleThreadExecutor()

    fun open(uri: String, startMs: Long, play: Boolean) {
        lastUri = uri
        report(close = true)
        posName = if (uri.startsWith("aru://")) uri.substringAfterLast('/').substringBefore('?') else null
        error = null
        ended = false
        ready = false
        positionMs = startMs
        durationMs = 0
        if (uri.startsWith("aru://")) {
            val item = MediaItem.Builder().setUri(uri).setMimeType(androidx.media3.common.MimeTypes.VIDEO_MP4).build()
            val factory = androidx.media3.exoplayer.source.ProgressiveMediaSource.Factory(AruDataSource.Factory())
                .setContinueLoadingCheckIntervalBytes(256 * 1024)
            exo.setMediaSource(factory.createMediaSource(item), startMs.coerceAtLeast(0))
        } else {
            exo.setMediaItem(MediaItem.fromUri(uri), startMs.coerceAtLeast(0))
        }
        exo.setPlaybackSpeed(speed)
        exo.prepare()
        exo.playWhenReady = play
    }

    /** Yuklangan (buferdagi) ulush 0..1. */
    fun bufferedRatio(): Float {
        val d = exo.duration
        return if (d > 0) (exo.bufferedPosition.toFloat() / d).coerceIn(0f, 1f) else 0f
    }

    // ── Sirpanuvchi oyna: ijro joyidan ko'pi bilan 1 daqiqa oldinga yuklanadi (Rust `rust_player_position`) ──
    private var posName: String? = null
    private var lastReport = 0L

    private fun report(force: Boolean = false, close: Boolean = false) {
        val n = posName ?: return
        val now = android.os.SystemClock.uptimeMillis()
        if (!force && !close && now - lastReport < 800) return
        lastReport = now
        val pos = exo.currentPosition.coerceAtLeast(0)
        val dur = if (close) 0L else (exo.duration.takeIf { it > 0 } ?: 0L)
        if (!close && dur <= 0) return
        val buf = exo.playbackState == Player.STATE_BUFFERING
        try { reporter.execute { uz.arumediatv.com.core.RustCore.playerPosition(n, pos, dur, buf) } } catch (_: Exception) {}
    }

    fun tick() {
        positionMs = exo.currentPosition.coerceAtLeast(0)
        val d = exo.duration
        if (d > 0) durationMs = d
        report()
    }

    fun play() {
        if (ended) exo.seekTo(0)
        exo.playWhenReady = true
    }

    fun pause() {
        exo.playWhenReady = false
    }

    fun toggle() = if (exo.playWhenReady && !ended) pause() else play()

    fun seekTo(ms: Long) {
        val max = if (durationMs > 0) durationMs else Long.MAX_VALUE
        exo.seekTo(ms.coerceIn(0, max))
        positionMs = exo.currentPosition
        report(force = true)
    }

    fun seekBy(deltaMs: Long) = seekTo(exo.currentPosition + deltaMs)

    fun changeSpeed(v: Float) {
        speed = v
        exo.setPlaybackSpeed(v)
    }

    fun release() {
        report(close = true)
        reporter.shutdown()
        exo.removeListener(listener)
        exo.release()
    }
}

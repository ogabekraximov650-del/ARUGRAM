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
            .setTargetBufferBytes(24 * 1024 * 1024)
            .setPrioritizeTimeOverSizeThresholds(false)
            .build(),
    ).build().apply {
        trackSelectionParameters = trackSelectionParameters.buildUpon()
            .setExceedVideoConstraintsIfNecessary(true)
            .build()
    }

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
                lastUri.contains("127.0.0.1") && lastUri.contains("/tg/") -> "Telegram"
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

    fun open(uri: String, startMs: Long, play: Boolean) {
        lastUri = uri
        error = null
        ended = false
        ready = false
        positionMs = startMs
        durationMs = 0
        exo.setMediaItem(MediaItem.fromUri(uri), startMs.coerceAtLeast(0))
        exo.setPlaybackSpeed(speed)
        exo.prepare()
        exo.playWhenReady = play
    }

    /** Yuklangan (buferdagi) ulush 0..1. */
    fun bufferedRatio(): Float {
        val d = exo.duration
        return if (d > 0) (exo.bufferedPosition.toFloat() / d).coerceIn(0f, 1f) else 0f
    }

    fun tick() {
        positionMs = exo.currentPosition.coerceAtLeast(0)
        val d = exo.duration
        if (d > 0) durationMs = d
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
    }

    fun seekBy(deltaMs: Long) = seekTo(exo.currentPosition + deltaMs)

    fun changeSpeed(v: Float) {
        speed = v
        exo.setPlaybackSpeed(v)
    }

    fun release() {
        exo.removeListener(listener)
        exo.release()
    }
}

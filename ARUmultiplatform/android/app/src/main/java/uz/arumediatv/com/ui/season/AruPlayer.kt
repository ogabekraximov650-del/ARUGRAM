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
@Stable
class AruPlayer(context: Context) {
    val exo: ExoPlayer = ExoPlayer.Builder(context.applicationContext).build()

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

        override fun onPlayerError(e: PlaybackException) {
            error = "Video ochilmadi (${e.errorCodeName})"
            playing = false
        }
    }

    init {
        exo.addListener(listener)
    }

    fun open(uri: String, startMs: Long, play: Boolean) {
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

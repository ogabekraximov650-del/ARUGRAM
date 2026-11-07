package uz.arutube.app

import android.app.Activity
import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

/**
 * Ilova darajasidagi pleyer egasi: video ekranidan chiqilganda ham ijro davom etadi (mini pleyer).
 * Holat: `status` — "" (tayyor yoki yo'q), "loading" (havola olinmoqda), "failed" (reklamasiz havola topilmadi yoki xato).
 */
class PlayerHolder(private val ctx: Context, private val scope: CoroutineScope) {
    var video by mutableStateOf<Video?>(null)
        private set
    var exo by mutableStateOf<ExoPlayer?>(null)
        private set
    var status by mutableStateOf("")
        private set
    var playing by mutableStateOf(false)
        private set
    private var job: Job? = null
    private var act: Activity? = null
    private var retriedFor: String? = null

    /** Shu video allaqachon ochiq (yoki ochilayotgan) bo'lsa, hech narsa qilmaydi — mini pleyerdan qaytganda ijro uzilmaydi. */
    fun open(v: Video, act: Activity) {
        if (video?.code == v.code && (exo != null || status == "loading")) return
        close()
        this.act = act
        video = v
        status = "loading"
        job = scope.launch {
            // Havola oldindan tayyorlangan bo'lsa (LinkCache) darhol olinadi.
            val d = LinkCache.resolve(scope, act, v)
            if (d == null) {
                status = "failed"
                return@launch
            }
            val p = buildPlayer(ctx, d, false) { onPlayerFailed(v) }
            LocalStore.progress(v.code)?.let { s ->
                if (s.posMs > 5_000 && (s.durMs == 0L || s.posMs < s.durMs - 10_000)) p.seekTo(s.posMs)
            }
            p.addListener(object : Player.Listener {
                override fun onIsPlayingChanged(isPlaying: Boolean) {
                    playing = isPlaying
                }
            })
            exo = p
            status = ""
            LocalStore.record(v, p.currentPosition, v.duration * 1000)
        }
    }

    /** Havola eskirgan bo'lishi mumkin: keshdan o'chirilib, bir marta yangisi olinadi; yana xato bo'lsa — "failed". */
    private fun onPlayerFailed(v: Video) {
        val a = act
        if (a != null && retriedFor != v.code) {
            retriedFor = v.code
            LinkCache.invalidate(v.code)
            close()
            open(v, a)
        } else {
            status = "failed"
        }
    }

    fun toggle() {
        exo?.let { if (it.isPlaying) it.pause() else it.play() }
    }

    /** Joriy o'rinni tarixga yozadi. */
    fun save() {
        val p = exo ?: return
        val v = video ?: return
        val dur = if (p.duration > 0) p.duration else v.duration * 1000
        LocalStore.record(v, p.currentPosition, dur)
    }

    fun close() {
        job?.cancel()
        job = null
        save()
        exo?.release()
        exo = null
        video = null
        status = ""
        playing = false
    }
}

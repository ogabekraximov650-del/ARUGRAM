package uz.arutube.app

import android.app.Activity
import android.media.AudioManager
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.PlayerView
import kotlinx.coroutines.delay
import kotlin.math.roundToInt

/**
 * Tayyor ExoPlayer'ni ko'rsatadi (mini pleyer bilan bir xil pleyer). Gesturelar markaziy hududda (pastdagi vaqt chizig'i va tepadagi
 * tugmalarga tegmaydi): bir marta bosish — boshqaruvni ko'rsatish/yashirish; ikki marta bosish — chap tomon -10 s, o'ng tomon +10 s,
 * o'rta — pauza; vertikal surish — chap yarmi yorqinlik, o'ng yarmi ovoz.
 */
@Composable
fun PlayerSurface(exo: ExoPlayer, onFullscreen: (Boolean) -> Unit, modifier: Modifier = Modifier) {
    val ctx = LocalContext.current
    val act = ctx as Activity
    val audio = remember { ctx.getSystemService(AudioManager::class.java) }
    var view by remember { mutableStateOf<PlayerView?>(null) }
    var hud by remember { mutableStateOf("") }
    var vol by remember { mutableFloatStateOf(audio.getStreamVolume(AudioManager.STREAM_MUSIC).toFloat()) }
    LaunchedEffect(hud) {
        if (hud.isNotEmpty()) {
            delay(800)
            hud = ""
        }
    }
    Box(modifier.background(Color.Black)) {
        AndroidView(
            modifier = Modifier.fillMaxSize(),
            factory = {
                PlayerView(ctx).apply {
                    player = exo
                    useController = true
                    setShowBuffering(PlayerView.SHOW_BUFFERING_ALWAYS)
                    setFullscreenButtonClickListener { onFullscreen(it) }
                    view = this
                }
            },
            update = { it.player = exo },
            onRelease = { it.player = null },
        )
        Box(
            Modifier
                .align(Alignment.Center)
                .fillMaxWidth()
                .fillMaxHeight(0.55f)
                .pointerInput(exo) {
                    detectTapGestures(
                        onTap = {
                            view?.let { v -> if (v.isControllerFullyVisible) v.hideController() else v.showController() }
                        },
                        onDoubleTap = { o ->
                            val w = size.width
                            when {
                                o.x < w / 3f -> {
                                    exo.seekTo((exo.currentPosition - 10_000).coerceAtLeast(0))
                                    hud = "« 10 s"
                                }
                                o.x > w * 2f / 3f -> {
                                    exo.seekTo(exo.currentPosition + 10_000)
                                    hud = "10 s »"
                                }
                                else -> if (exo.isPlaying) exo.pause() else exo.play()
                            }
                        },
                    )
                }
                .pointerInput(Unit) {
                    var left = false
                    detectVerticalDragGestures(
                        onDragStart = { o ->
                            left = o.x < size.width / 2f
                            vol = audio.getStreamVolume(AudioManager.STREAM_MUSIC).toFloat()
                        },
                        onVerticalDrag = { change, dy ->
                            change.consume()
                            val frac = dy / size.height.toFloat()
                            if (left) {
                                val lp = act.window.attributes
                                val cur = if (lp.screenBrightness < 0f) 0.5f else lp.screenBrightness
                                val n = (cur - frac * 1.2f).coerceIn(0.02f, 1f)
                                lp.screenBrightness = n
                                act.window.attributes = lp
                                hud = "Yorqinlik ${(n * 100).roundToInt()}%"
                            } else {
                                val max = audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC).toFloat()
                                vol = (vol - frac * max * 1.5f).coerceIn(0f, max)
                                audio.setStreamVolume(AudioManager.STREAM_MUSIC, vol.roundToInt(), 0)
                                hud = "Ovoz ${(vol / max * 100).roundToInt()}%"
                            }
                        },
                    )
                },
        )
        if (hud.isNotEmpty()) {
            Text(
                hud,
                color = Color.White,
                fontSize = 15.sp,
                modifier = Modifier
                    .align(Alignment.Center)
                    .clip(RoundedCornerShape(10.dp))
                    .background(Color(0xAA000000))
                    .padding(horizontal = 14.dp, vertical = 8.dp),
            )
        }
    }
}

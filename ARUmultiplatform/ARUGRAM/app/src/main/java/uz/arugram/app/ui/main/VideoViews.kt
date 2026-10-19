package uz.arugram.app.ui.main

import android.net.Uri
import android.view.SurfaceView
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.PlayerView
import java.io.File

/**
 * GIF / qisqa video: ovozsiz, cheksiz takrorlanadi (Telegram'dagi GIF kabi). Konteyner o'lchami video nisbatiga teng bo'lishi kerak.
 */
@Composable
fun LoopingVideo(file: File, modifier: Modifier = Modifier) {
    val ctx = LocalContext.current
    val player = remember(file.path) {
        ExoPlayer.Builder(ctx).build().apply {
            setMediaItem(MediaItem.fromUri(Uri.fromFile(file)))
            repeatMode = Player.REPEAT_MODE_ALL
            volume = 0f
            playWhenReady = true
            prepare()
        }
    }
    DisposableEffect(player) { onDispose { player.release() } }
    AndroidView(modifier = modifier, factory = { c -> SurfaceView(c).also { player.setVideoSurfaceView(it) } })
}

/** Video ko'rgich (to'liq ekran): boshqaruv paneli bilan (ijro / pauza, surish). */
@Composable
fun ControlledVideo(file: File, modifier: Modifier = Modifier, autoPlay: Boolean = true) {
    val ctx = LocalContext.current
    val player = remember(file.path) {
        ExoPlayer.Builder(ctx).build().apply {
            setMediaItem(MediaItem.fromUri(Uri.fromFile(file)))
            playWhenReady = autoPlay
            prepare()
        }
    }
    DisposableEffect(player) { onDispose { player.release() } }
    AndroidView(modifier = modifier, factory = { c -> PlayerView(c).also { it.player = player; it.useController = true } })
}

/** Saqlangan GIFlar paneli (emoji panelining "GIF" yorlig'i): 2 ustun, har biri ovozsiz takrorlanadi; bosilsa chatga yuboriladi. */
@Composable
fun GifPanel(onPick: (String) -> Unit) {
    val ctx = LocalContext.current
    val c = uz.arugram.app.ui.theme.LocalTg.current
    var gifs by androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateOf<List<Triple<String, Int, Int>>?>(null) }
    androidx.compose.runtime.LaunchedEffect(Unit) {
        val j = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { uz.arugram.app.core.RustCore.tgSavedGifs() }
        val a = j.optJSONArray("gifs")
        gifs = buildList { for (i in 0 until (a?.length() ?: 0)) { val o = a!!.getJSONObject(i); add(Triple(o.optString("ref"), o.optInt("w"), o.optInt("h"))) } }
    }
    val l = gifs
    androidx.compose.foundation.layout.Box(androidx.compose.ui.Modifier.fillMaxWidth().height(260.dp)) {
        when {
            l == null -> androidx.compose.material3.CircularProgressIndicator(androidx.compose.ui.Modifier.align(androidx.compose.ui.Alignment.Center).size(28.dp), color = c.inputActive, strokeWidth = 3.dp)
            l.isEmpty() -> androidx.compose.foundation.text.BasicText("Saqlangan GIF yo‘q. GIFni chatda bosib ushlab, saqlab qo‘ying.", androidx.compose.ui.Modifier.align(androidx.compose.ui.Alignment.Center).padding(24.dp), style = androidx.compose.ui.text.TextStyle(color = c.gray, fontSize = 14.sp, textAlign = androidx.compose.ui.text.style.TextAlign.Center))
            else -> androidx.compose.foundation.lazy.grid.LazyVerticalGrid(androidx.compose.foundation.lazy.grid.GridCells.Fixed(2), androidx.compose.ui.Modifier.fillMaxSize().padding(2.dp)) {
                androidx.compose.foundation.lazy.grid.items(l, key = { it.first }) { (ref, w, h) ->
                    var file by androidx.compose.runtime.remember(ref) { androidx.compose.runtime.mutableStateOf<File?>(null) }
                    androidx.compose.runtime.LaunchedEffect(ref) {
                        val dir = File(ctx.cacheDir, "gifs").apply { mkdirs() }
                        val r = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { uz.arugram.app.core.RustCore.tgDocFile(ref, dir.absolutePath) }
                        r.optString("path").takeIf { it.isNotEmpty() }?.let { file = File(it) }
                    }
                    val ratio = if (w > 0 && h > 0) w.toFloat() / h else 1f
                    androidx.compose.foundation.layout.Box(
                        androidx.compose.ui.Modifier.padding(2.dp).fillMaxWidth().aspectRatio(ratio.coerceIn(0.6f, 2f)).background(androidx.compose.ui.graphics.Color(0xFF2B2B2B))
                            .clickable { onPick(ref) },
                    ) {
                        file?.let { LoopingVideo(it, androidx.compose.ui.Modifier.fillMaxSize()) }
                    }
                }
            }
        }
    }
}

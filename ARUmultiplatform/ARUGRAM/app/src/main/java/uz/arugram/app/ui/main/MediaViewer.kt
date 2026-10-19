package uz.arugram.app.ui.main

import android.content.ContentValues
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.widget.MediaController
import android.widget.Toast
import android.widget.VideoView
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.ui.geometry.Offset
import androidx.media3.ui.PlayerView
import kotlinx.coroutines.launch
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.text.BasicText
import androidx.compose.material3.CircularProgressIndicator
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
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import uz.arugram.app.R
import uz.arugram.app.ui.components.CloseIcon
import uz.arugram.app.ui.components.TgIcon
import java.io.File

/**
 * Rasm / video ko'rgich (DrKLO `PhotoViewer`): qora to'liq ekran; tepada orqaga, nom + sana ("kecha 16:47 da"), uzatish, video sozlamalari (tezlik) va ⋮ menyu;
 * markazda "i / n" ko'rsatkichi; pastda izoh va surish chizig'i bilan "00:05 / 22:55". Video yuklab bo'lmasdan oqim bilan ijro etiladi.
 */
@Composable
fun MediaViewer(
    chat: ChatRow, msg: MsgRow, onClose: () -> Unit,
    media: List<MsgRow> = emptyList(),
    onReply: (MsgRow) -> Unit = {}, onDelete: (MsgRow) -> Unit = {}, onForward: (MsgRow) -> Unit = {}, onShowInChat: (MsgRow) -> Unit = {},
) {
    val ctx = LocalContext.current
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    val isVideo = msg.kind == "video"
    val isAnim = msg.kind == "gif" || msg.kind == "round"
    var loaded by remember { mutableStateOf<LoadedFile?>(null) }
    var failed by remember { mutableStateOf(false) }
    var uiVisible by remember { mutableStateOf(true) }
    var menu by remember { mutableStateOf(false) }
    var speedMenu by remember { mutableStateOf(false) }
    var dlJob by remember { mutableStateOf<kotlinx.coroutines.Job?>(null) }
    LaunchedEffect(msg.id) {
        if (isVideo) return@LaunchedEffect
        val f = MediaCache.file(ctx, chat, msg.id)
        if (f == null) failed = true else loaded = f
    }
    // Faylni (to'liq yuklab) kerakli amalga beradi: saqlash / ochish / ulashish.
    fun withFile(action: (LoadedFile) -> Unit) {
        val ready = loaded
        if (ready != null) { action(ready); return }
        Toast.makeText(ctx, "Yuklanmoqda...", Toast.LENGTH_SHORT).show()
        dlJob = scope.launch {
            val f = MediaCache.file(ctx, chat, msg.id)
            dlJob = null
            if (f != null) { loaded = f; action(f) } else Toast.makeText(ctx, "Yuklab bo‘lmadi", Toast.LENGTH_SHORT).show()
        }
    }
    val player = remember(msg.id) { if (isVideo) createStreamPlayer(ctx, chat.spec, msg.id) else null }
    androidx.compose.runtime.DisposableEffect(player) { onDispose { player?.release() } }

    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        Box(Modifier.fillMaxSize().background(Color.Black)) {
            val f = loaded
            when {
                isVideo && player != null -> VideoSurface(player, msg, uiVisible, onToggleUi = { uiVisible = !uiVisible })
                f == null && !failed -> CircularProgressIndicator(Modifier.align(Alignment.Center).size(40.dp), color = Color.White, strokeWidth = 3.dp)
                f == null -> BasicText("Yuklab bo‘lmadi", Modifier.align(Alignment.Center), style = TextStyle(color = Color.White, fontSize = 16.sp))
                isAnim -> ControlledVideo(f.file, Modifier.fillMaxSize())
                else -> Box(Modifier.fillMaxSize().clickable(indication = null, interactionSource = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }) { uiVisible = !uiVisible }) { ZoomImage(f.file) }
            }
            if (uiVisible) {
                // Tepa panel
                Column(Modifier.fillMaxWidth().statusBarsPadding()) {
                    Row(Modifier.fillMaxWidth().height(56.dp), verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.size(56.dp).clickable(onClick = onClose), contentAlignment = Alignment.Center) { uz.arugram.app.ui.components.BackArrowIcon(Color.White) }
                        Column(Modifier.weight(1f).padding(start = 4.dp)) {
                            BasicText(msg.sender.ifEmpty { chat.name }, style = TextStyle(color = Color(0xFFFAFAFA), fontSize = 20.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.Bold), maxLines = 1)
                            BasicText(viewerDate(msg.date), style = TextStyle(color = Color(0xFFB0B0B0), fontSize = 15.sp), maxLines = 1)
                        }
                        Box(Modifier.size(48.dp).clickable { onForward(msg) }, contentAlignment = Alignment.Center) { TgIcon(R.drawable.msg_header_share, Color.White) }
                        if (isVideo) Box(Modifier.size(48.dp).clickable { speedMenu = true }, contentAlignment = Alignment.Center) { TgIcon(R.drawable.video_settings, Color.White) }
                        Box(Modifier.size(48.dp).clickable { menu = true }, contentAlignment = Alignment.Center) { TgIcon(R.drawable.ic_ab_other, Color.White) }
                    }
                    val items = remember(media) { media.filter { it.kind == "photo" || it.kind == "video" || it.kind == "gif" || it.kind == "round" }.sortedBy { it.id } }
                    val idx = items.indexOfFirst { it.id == msg.id }
                    if (idx >= 0) BasicText("${idx + 1} / ${items.size}", Modifier.align(Alignment.CenterHorizontally).padding(top = 14.dp), style = TextStyle(color = Color(0xFFFAFAFA), fontSize = 17.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.Medium))
                }
                // Izoh (rasm uchun; video ichida VideoSurface chizadi)
                if (!isVideo && msg.text.isNotEmpty()) BasicText(msg.text, Modifier.align(Alignment.BottomStart).navigationBarsPadding().padding(start = 16.dp, end = 16.dp, bottom = 16.dp), style = TextStyle(color = Color.White, fontSize = 17.sp))
            }
            if (menu) {
                val rows = buildList<ViewerItem> {
                    add(ViewerItem(R.drawable.msg_gallery, "Galereyaga saqlash", divider = true) { withFile { saveToGallery(ctx, it.file, isVideo || isAnim) } })
                    add(ViewerItem(R.drawable.msg_openin, "...orqali ochish") { withFile { openExternal(ctx, it) } })
                    add(ViewerItem(R.drawable.msg_message, "Chatda ko‘rsatish") { onShowInChat(msg); onClose() })
                    add(ViewerItem(R.drawable.menu_reply, "Javob yozish") { onReply(msg); onClose() })
                    add(ViewerItem(R.drawable.msg_shareout, "Ulashish") { withFile { shareFile(ctx, it.file, it.mime) } })
                    add(ViewerItem(R.drawable.msg_delete, "O‘chirish") { onDelete(msg); onClose() })
                    if (dlJob != null) add(ViewerItem(R.drawable.msg_cancel, "Yuklab olishni to‘xtatish") { dlJob?.cancel(); dlJob = null })
                }
                ViewerMenu(rows, onDismiss = { menu = false })
            }
            if (speedMenu && player != null) SpeedMenu(player, onDismiss = { speedMenu = false })
        }
    }
}

private class ViewerItem(val icon: Int, val label: String, val divider: Boolean = false, val action: () -> Unit)

/** "kecha 16:47 da" / "bugun 09:10 da" / "18-oktabr 16:47 da". */
private fun viewerDate(unix: Long): String {
    val d = dayKey(System.currentTimeMillis() / 1000) - dayKey(unix)
    val day = when (d) { 0L -> "bugun"; 1L -> "kecha"; else -> dateLabel(unix) }
    return "$day ${clock(unix)} da"
}

/** ⋮ menyusi (DrKLO `ActionBarPopupWindow`): o'ng yuqorida, to'q karta, 48 dp qatorlar. */
@Composable
private fun ViewerMenu(items: List<ViewerItem>, onDismiss: () -> Unit) {
    androidx.compose.ui.window.Popup(alignment = Alignment.TopEnd, onDismissRequest = onDismiss, properties = androidx.compose.ui.window.PopupProperties(focusable = true)) {
        Column(
            Modifier.statusBarsPadding().padding(top = 4.dp, end = 4.dp).widthIn(min = 250.dp).clip(androidx.compose.foundation.shape.RoundedCornerShape(14.dp)).background(Color(0xFF1C1C1E)).padding(vertical = 4.dp),
        ) {
            items.forEach { it0 ->
                Row(Modifier.fillMaxWidth().height(48.dp).clickable { onDismiss(); it0.action() }.padding(horizontal = 18.dp), verticalAlignment = Alignment.CenterVertically) {
                    TgIcon(it0.icon, Color(0xFFFAFAFA), size = 24.dp)
                    BasicText(it0.label, Modifier.padding(start = 24.dp), style = TextStyle(color = Color(0xFFFAFAFA), fontSize = 17.sp), maxLines = 1)
                }
                if (it0.divider) Box(Modifier.fillMaxWidth().height(8.dp).background(Color(0xFF151517)))
            }
        }
    }
}

/** Video sozlamalari (DrKLO tezlik menyusi): tezlik surgichi, 0.2x…2x tayyor qiymatlar va takrorlash. */
@Composable
private fun SpeedMenu(player: androidx.media3.exoplayer.ExoPlayer, onDismiss: () -> Unit) {
    var speed by remember { mutableFloatStateOf(player.playbackParameters.speed) }
    var loop by remember { mutableStateOf(player.repeatMode == androidx.media3.common.Player.REPEAT_MODE_ONE) }
    val blue = Color(0xFF4C9CE2)
    fun apply(v: Float) { val s = (Math.round(v.coerceIn(0.2f, 2.5f) * 10) / 10f); speed = s; player.setPlaybackSpeed(s) }
    androidx.compose.ui.window.Popup(alignment = Alignment.TopEnd, onDismissRequest = onDismiss, properties = androidx.compose.ui.window.PopupProperties(focusable = true)) {
        Column(Modifier.statusBarsPadding().padding(top = 4.dp, end = 4.dp).width(204.dp).clip(androidx.compose.foundation.shape.RoundedCornerShape(14.dp)).background(Color(0xFF1C1C1E))) {
            Box(
                Modifier.fillMaxWidth().height(48.dp).background(Color(0xFF151517)).pointerInput(Unit) {
                    detectTransformGestures { centroid, _, _, _ -> apply(0.2f + (centroid.x / size.width).coerceIn(0f, 1f) * 2.3f) }
                }.pointerInput(Unit) { androidx.compose.foundation.gestures.detectTapGestures { apply(0.2f + (it.x / size.width) * 2.3f) } },
                contentAlignment = Alignment.CenterStart,
            ) {
                Box(Modifier.fillMaxHeight().fillMaxWidth(((speed - 0.2f) / 2.3f).coerceIn(0.02f, 1f)).background(Color(0xFF2F5FC4)))
                Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                    BasicText("Tezlik", Modifier.weight(1f), style = TextStyle(color = Color.White, fontSize = 17.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.Bold))
                    BasicText(if (speed == speed.toInt().toFloat()) "${speed.toInt()}x" else "${speed}x", style = TextStyle(color = Color.White, fontSize = 17.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.Bold))
                }
            }
            listOf(0.2f to "Juda sekin", 0.5f to "Sekin", 1f to "Normal", 1.5f to "Tez", 2f to "Juda tez").forEach { (v, label) ->
                val sel = kotlin.math.abs(speed - v) < 0.05f
                Row(Modifier.fillMaxWidth().height(48.dp).clickable { apply(v) }.padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                    BasicText(if (v == v.toInt().toFloat()) "${v.toInt()}x" else "${v}x", Modifier.width(48.dp), style = TextStyle(color = if (sel) blue else Color(0xFFB0B0B0), fontSize = 14.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.Bold))
                    BasicText(label, style = TextStyle(color = if (sel) blue else Color(0xFFFAFAFA), fontSize = 17.sp))
                }
            }
            Box(Modifier.fillMaxWidth().height(8.dp).background(Color(0xFF151517)))
            Row(
                Modifier.fillMaxWidth().height(48.dp).clickable {
                    loop = !loop
                    player.repeatMode = if (loop) androidx.media3.common.Player.REPEAT_MODE_ONE else androidx.media3.common.Player.REPEAT_MODE_OFF
                }.padding(horizontal = 16.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TgIcon(R.drawable.menu_video_loop, if (loop) blue else Color(0xFFFAFAFA), Modifier.width(48.dp), 24.dp)
                BasicText("Takrorlash", style = TextStyle(color = if (loop) blue else Color(0xFFFAFAFA), fontSize = 17.sp))
            }
        }
    }
}

/** Video yuzasi va pastki boshqaruv: izoh, surish chizig'i, vaqt, ijro/pauza, to'liq ekran. */
@Composable
private fun BoxScope.VideoSurface(player: androidx.media3.exoplayer.ExoPlayer, msg: MsgRow, ui: Boolean, onToggleUi: () -> Unit) {
    val ctx = LocalContext.current
    var playing by remember { mutableStateOf(player.playWhenReady) }
    var pos by remember { mutableStateOf(0L) }
    var buf by remember { mutableStateOf(0L) }
    var dur by remember { mutableStateOf(0L) }
    var ratio by remember { mutableFloatStateOf(if (msg.pw > 0 && msg.ph > 0) msg.pw.toFloat() / msg.ph else 16f / 9f) }
    var seeking by remember { mutableStateOf(false) }
    var landscape by remember { mutableStateOf(false) }
    androidx.compose.runtime.LaunchedEffect(player) {
        while (true) {
            playing = player.isPlaying || (player.playWhenReady && player.playbackState == androidx.media3.common.Player.STATE_BUFFERING)
            if (!seeking) pos = player.currentPosition
            buf = player.bufferedPosition
            dur = player.duration.takeIf { it > 0 } ?: (msg.dur * 1000).toLong()
            val vs = player.videoSize
            if (vs.width > 0 && vs.height > 0) ratio = vs.width.toFloat() / vs.height * vs.pixelWidthHeightRatio
            kotlinx.coroutines.delay(200)
        }
    }
    val act = remember { var c = ctx; while (c is android.content.ContextWrapper && c !is android.app.Activity) c = c.baseContext; c as? android.app.Activity }
    androidx.compose.runtime.DisposableEffect(Unit) { onDispose { act?.requestedOrientation = android.content.pm.ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED } }
    Box(Modifier.align(Alignment.Center).fillMaxWidth().aspectRatio(ratio.coerceIn(0.3f, 4f)).clickable(indication = null, interactionSource = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }, onClick = onToggleUi)) {
        AndroidView(modifier = Modifier.fillMaxSize(), factory = { c -> PlayerView(c).also { it.player = player; it.useController = false; it.setShutterBackgroundColor(android.graphics.Color.BLACK) } })
        if (ui) Box(Modifier.align(Alignment.BottomEnd).padding(4.dp).size(48.dp).clickable {
            landscape = !landscape
            act?.requestedOrientation = if (landscape) android.content.pm.ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE else android.content.pm.ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
        }, contentAlignment = Alignment.Center) { TgIcon(if (landscape) R.drawable.msg_minvideo else R.drawable.msg_maxvideo, Color.White) }
    }
    if (ui) {
        // Markaziy ijro / pauza
        Box(Modifier.align(Alignment.Center).size(72.dp).clip(androidx.compose.foundation.shape.CircleShape).clickable {
            if (player.isPlaying || player.playWhenReady) player.pause() else { if (player.playbackState == androidx.media3.common.Player.STATE_ENDED) player.seekTo(0); player.play() }
            playing = player.playWhenReady
        }, contentAlignment = Alignment.Center) {
            androidx.compose.foundation.Canvas(Modifier.size(44.dp)) {
                if (playing) {
                    val w = size.width * 0.26f
                    drawRect(Color.White, Offset(size.width * 0.18f, size.height * 0.12f), androidx.compose.ui.geometry.Size(w, size.height * 0.76f))
                    drawRect(Color.White, Offset(size.width * 0.56f, size.height * 0.12f), androidx.compose.ui.geometry.Size(w, size.height * 0.76f))
                } else {
                    val p = androidx.compose.ui.graphics.Path().apply { moveTo(size.width * 0.2f, size.height * 0.08f); lineTo(size.width * 0.92f, size.height * 0.5f); lineTo(size.width * 0.2f, size.height * 0.92f); close() }
                    drawPath(p, Color.White)
                }
            }
        }
        Column(Modifier.align(Alignment.BottomCenter).fillMaxWidth().navigationBarsPadding()) {
            if (msg.text.isNotEmpty()) BasicText(msg.text, Modifier.padding(start = 16.dp, end = 16.dp, bottom = 10.dp), style = TextStyle(color = Color.White, fontSize = 17.sp))
            Row(Modifier.fillMaxWidth().height(48.dp), verticalAlignment = Alignment.CenterVertically) {
                SeekBar(pos, buf, dur, Modifier.weight(1f).fillMaxHeight(), onSeeking = { seeking = true; pos = it }, onSeek = { seeking = false; player.seekTo(it) })
                BasicText("${playerTime(pos)} / ${playerTime(dur)}", Modifier.padding(start = 12.dp, end = 16.dp), style = TextStyle(color = Color(0xFFFAFAFA), fontSize = 15.sp))
            }
        }
    }
}

private fun playerTime(ms: Long): String {
    val s = (ms / 1000).coerceAtLeast(0)
    return if (s >= 3600) "%d:%02d:%02d".format(s / 3600, s % 3600 / 60, s % 60) else "%02d:%02d".format(s / 60, s % 60)
}

/** Surish chizig'i (DrKLO `VideoSeekBarView`): 3 dp yo'l, yuklangan qism xira, o'ynalgan qism oq, oq dumaloq tugma. */
@Composable
private fun SeekBar(pos: Long, buffered: Long, dur: Long, modifier: Modifier, onSeeking: (Long) -> Unit, onSeek: (Long) -> Unit) {
    var w by remember { mutableFloatStateOf(1f) }
    val d = dur.coerceAtLeast(1L)
    var last by remember { mutableStateOf(0L) }
    Box(
        modifier.pointerInput(d) {
            detectHorizontalDragGestures(
                onDragStart = { o -> last = (o.x / w * d).toLong().coerceIn(0, d); onSeeking(last) },
                onDragEnd = { onSeek(last) },
                onDragCancel = { onSeek(last) },
            ) { change, _ -> last = (change.position.x / w * d).toLong().coerceIn(0, d); onSeeking(last) }
        }.pointerInput(d) { androidx.compose.foundation.gestures.detectTapGestures { o -> val t = (o.x / w * d).toLong().coerceIn(0, d); onSeeking(t); onSeek(t) } },
    ) {
        androidx.compose.foundation.Canvas(Modifier.fillMaxSize()) {
            w = size.width
            val y = size.height / 2
            val th = 3.dp.toPx()
            val pad = 16.dp.toPx()
            val len = size.width - pad - 8.dp.toPx()
            drawRoundRect(Color(0x4DFFFFFF), Offset(pad, y - th / 2), androidx.compose.ui.geometry.Size(len, th), androidx.compose.ui.geometry.CornerRadius(th))
            drawRoundRect(Color(0x66FFFFFF), Offset(pad, y - th / 2), androidx.compose.ui.geometry.Size(len * (buffered.toFloat() / d).coerceIn(0f, 1f), th), androidx.compose.ui.geometry.CornerRadius(th))
            val px = pad + len * (pos.toFloat() / d).coerceIn(0f, 1f)
            drawRoundRect(Color.White, Offset(pad, y - th / 2), androidx.compose.ui.geometry.Size(px - pad, th), androidx.compose.ui.geometry.CornerRadius(th))
            drawCircle(Color.White, 6.dp.toPx(), Offset(px, y))
        }
    }
}

/** Faylni tashqi ilova bilan ochadi. */
private fun openExternal(ctx: android.content.Context, f: LoadedFile) {
    runCatching {
        val uri = androidx.core.content.FileProvider.getUriForFile(ctx, "${ctx.packageName}.files", f.file)
        ctx.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW).setDataAndType(uri, f.mime.ifEmpty { "*/*" }).addFlags(android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION or android.content.Intent.FLAG_ACTIVITY_NEW_TASK))
    }.onFailure { Toast.makeText(ctx, "Ochib bo‘lmadi", Toast.LENGTH_SHORT).show() }
}

@Composable
private fun ZoomImage(file: File) {
    val bmp = remember(file.path) { MediaCache.decode(file)?.asImageBitmap() }
    var scale by remember { mutableFloatStateOf(1f) }
    var ox by remember { mutableFloatStateOf(0f) }
    var oy by remember { mutableFloatStateOf(0f) }
    if (bmp == null) return
    Image(
        bmp, null,
        Modifier.fillMaxSize()
            .pointerInput(Unit) {
                detectTransformGestures { _, pan, zoom, _ ->
                    scale = (scale * zoom).coerceIn(1f, 6f)
                    if (scale > 1f) {
                        ox += pan.x
                        oy += pan.y
                    } else {
                        ox = 0f
                        oy = 0f
                    }
                }
            }
            .graphicsLayer { scaleX = scale; scaleY = scale; translationX = ox; translationY = oy },
        contentScale = ContentScale.Fit,
    )
}

/** Rasm/videoni galereyaga (Rasmlar / Filmlar papkasiga) saqlaydi. */
fun saveToGallery(ctx: android.content.Context, file: File, video: Boolean) {
    runCatching {
        if (Build.VERSION.SDK_INT >= 29) {
            val values = ContentValues().apply {
                put(MediaStore.MediaColumns.DISPLAY_NAME, file.name)
                put(MediaStore.MediaColumns.MIME_TYPE, if (video) "video/mp4" else "image/jpeg")
                put(MediaStore.MediaColumns.RELATIVE_PATH, if (video) Environment.DIRECTORY_MOVIES + "/ARUGRAM" else Environment.DIRECTORY_PICTURES + "/ARUGRAM")
            }
            val uri = ctx.contentResolver.insert(if (video) MediaStore.Video.Media.EXTERNAL_CONTENT_URI else MediaStore.Images.Media.EXTERNAL_CONTENT_URI, values)!!
            ctx.contentResolver.openOutputStream(uri)!!.use { out -> file.inputStream().use { it.copyTo(out) } }
            Toast.makeText(ctx, "Galereyaga saqlandi", Toast.LENGTH_SHORT).show()
        } else {
            val dir = File(Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS), "ARUGRAM").apply { mkdirs() }
            file.copyTo(File(dir, file.name), overwrite = true)
            Toast.makeText(ctx, "Yuklanganlar papkasiga saqlandi", Toast.LENGTH_SHORT).show()
        }
    }.onFailure { Toast.makeText(ctx, "Saqlab bo‘lmadi", Toast.LENGTH_SHORT).show() }
}

/** Profil rasmlari (DrKLO `PhotoViewer` profil rejimi): to'liq ekran, o'ngga/chapga suriladi, "2 / 5" ko'rsatkichi bilan. */
@Composable
fun ProfilePhotosViewer(chat: ChatRow, onClose: () -> Unit) {
    val ctx = LocalContext.current
    var paths by remember { mutableStateOf<List<String>?>(null) }
    LaunchedEffect(chat.id) {
        val dir = File(ctx.filesDir, "profile_photos").apply { mkdirs() }
        val j = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { uz.arugram.app.core.RustCore.tgProfilePhotos(chat.spec, dir.absolutePath) }
        val a = j.optJSONArray("paths")
        paths = List(a?.length() ?: 0) { a!!.getString(it) }
    }
    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        Box(Modifier.fillMaxSize().background(Color.Black)) {
            val l = paths
            when {
                l == null -> CircularProgressIndicator(Modifier.align(Alignment.Center).size(40.dp), color = Color.White, strokeWidth = 3.dp)
                l.isEmpty() -> BasicText("Profil rasmi yo‘q", Modifier.align(Alignment.Center), style = TextStyle(color = Color.White, fontSize = 16.sp))
                else -> {
                    val pager = androidx.compose.foundation.pager.rememberPagerState { l.size }
                    androidx.compose.foundation.pager.HorizontalPager(pager, Modifier.fillMaxSize()) { page ->
                        ZoomImage(File(l[page]))
                    }
                    if (l.size > 1) BasicText(
                        "${pager.currentPage + 1} / ${l.size}",
                        Modifier.align(Alignment.BottomCenter).padding(bottom = 32.dp), style = TextStyle(color = Color.White, fontSize = 15.sp),
                    )
                }
            }
            Box(Modifier.statusBarsPadding().padding(8.dp).size(48.dp).clickable(onClick = onClose), contentAlignment = Alignment.Center) { CloseIcon(Color.White) }
        }
    }
}

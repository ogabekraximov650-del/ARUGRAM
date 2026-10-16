package uz.arumediatv.com.ui.packs

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.media.MediaMetadataRetriever
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.PlayerView
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.PackInfo
import uz.arumediatv.com.data.PackKind
import uz.arumediatv.com.data.sniffImage
import uz.arumediatv.com.ui.profile.OverlayScaffold
import uz.arumediatv.com.ui.theme.AruColors
import java.io.File
import java.util.Locale
import kotlin.math.roundToInt

/** Tur bo'yicha eng uzun bo'lak (soniya); 0 — uzunlik cheklanmaydi (Flutter `packMaxSeconds`). */
fun packMaxSeconds(kind: String): Int = when (kind) { PackKind.EMOJI -> 8; PackKind.GIF -> 0; else -> 12 }

fun fmtMs(ms: Long): String {
    val s = ms / 1000.0
    val m = (s / 60).toInt()
    return String.format(Locale.US, "%d:%04.1f", m, s - m * 60).replace('.', ',')
}

private fun mb(b: Long) = String.format(Locale.US, "%.1f MB", b / 1048576.0)

/**
 * Elementni qo'shish oynasi (Flutter `PackAddScreen`): oldindan ko'rish, emoji belgisi, video uchun "Kesish" (bo'lak va ovoz),
 * hajm tekshiruvi (<= 5 MB) va yuborish (admin ko'rib chiqadi).
 */
@Composable
fun PackAddScreen(pack: PackInfo, file: File, onBack: () -> Unit, onSent: () -> Unit) {
    val scope = rememberCoroutineScope()
    val size = remember(file) { file.length() }
    val sniff = remember(file) { file.inputStream().use { i -> val b = ByteArray(32); val n = i.read(b); if (n <= 0) "" else sniffImage(b.copyOf(n)) } }
    val isVideo = sniff == "mp4" || sniff == "webm"
    val problem = when {
        sniff.isEmpty() -> "Fayl turi qo'llanmaydi (PNG, JPG, GIF, WebP, MP4, WebM)"
        size > uz.arumediatv.com.data.PACK_ITEM_MAX_BYTES -> "${mb(size)} — 5 MB dan katta, yuklab bo'lmaydi"
        else -> null
    }
    var duration by remember { mutableLongStateOf(0L) }
    var trimA by remember { mutableLongStateOf(0L) }
    var trimB by remember { mutableLongStateOf(0L) }
    var sound by remember { mutableStateOf(true) }
    var emoji by remember { mutableStateOf("") }
    var trimOpen by remember { mutableStateOf(false) }
    var emojiOpen by remember { mutableStateOf(false) }
    var progress by remember { mutableStateOf<Float?>(null) }
    var status by remember { mutableStateOf("") }
    var thumb by remember { mutableStateOf<Bitmap?>(null) }

    LaunchedEffect(file) {
        withContext(Dispatchers.IO) {
            if (isVideo) {
                val r = MediaMetadataRetriever()
                try {
                    r.setDataSource(file.absolutePath)
                    duration = r.extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION)?.toLongOrNull() ?: 0L
                    thumb = r.getFrameAtTime(0)
                } catch (_: Throwable) {
                } finally { runCatching { r.release() } }
                val max = packMaxSeconds(pack.kind) * 1000L
                trimA = 0
                trimB = if (duration <= 0) 0 else if (max <= 0 || duration < max) duration else max
            } else if (problem == null) thumb = BitmapFactory.decodeFile(file.absolutePath)
        }
        if (isVideo && problem == null && duration > 0) trimOpen = true
    }

    if (trimOpen) {
        PackVideoTrimScreen(file, pack.kind, duration, trimA, trimB, sound, onCancel = { trimOpen = false }) { a, b, snd -> trimA = a; trimB = b; sound = snd; trimOpen = false }
        return
    }
    BackHandler(enabled = progress == null, onBack = onBack)
    OverlayScaffold("${PackKind.single(pack.kind)} qo'shish", { if (progress == null) onBack() }) {
        Column(Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
            Text(
                if (pack.kind == PackKind.EMOJI || pack.kind == PackKind.STICKER) "Rasm yoki video (har biri 5 MB gacha). Video ko'pi bilan ${packMaxSeconds(pack.kind)} soniya: kerakli bo'lagini \"Kesish\" bilan tanlang. Admin tasdiqlagach \"${pack.title}\" to'plamida ko'rinadi."
                else "Rasm yoki video (har biri 5 MB gacha, uzunligi cheklanmaydi). Kerak bo'lsa bo'lagini \"Kesish\" bilan tanlang. Admin tasdiqlagach \"${pack.title}\" to'plamida ko'rinadi.",
                color = AruColors.TextDim, fontSize = 12.5.sp, lineHeight = 18.sp,
            )
            Spacer(Modifier.height(14.dp))
            val shape = RoundedCornerShape(16.dp)
            Box(Modifier.fillMaxWidth().aspectRatio(1.4f).clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape), contentAlignment = Alignment.Center) {
                thumb?.let { Image(it.asImageBitmap(), null, contentScale = ContentScale.Fit, modifier = Modifier.fillMaxSize()) }
                if (isVideo) Text("▶", color = Color.White, fontSize = 36.sp)
            }
            Spacer(Modifier.height(10.dp))
            Text(
                when {
                    isVideo -> "Video · ${mb(size)}"
                    sniff.isNotEmpty() -> "${sniff.uppercase()} · ${mb(size)}"
                    else -> mb(size)
                },
                color = Color.White, fontSize = 13.sp,
            )
            problem?.let { Text(it, color = Color(0xFFF87171), fontSize = 12.5.sp, modifier = Modifier.padding(top = 6.dp)) }
            if (isVideo && problem == null && duration > 0) {
                Row(Modifier.padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("Bo'lak: ${fmtMs(trimA)} – ${fmtMs(trimB)} (${String.format(Locale.US, "%.1f", (trimB - trimA) / 1000.0)} s)", color = AruColors.TextDim, fontSize = 12.5.sp, modifier = Modifier.weight(1f))
                    Text("Kesish", color = AruColors.Accent2, fontSize = 13.sp, fontWeight = FontWeight.Bold, modifier = Modifier.clickable { trimOpen = true }.padding(8.dp))
                }
            }
            if (pack.kind != PackKind.GIF) {
                Row(Modifier.padding(top = 10.dp).clip(RoundedCornerShape(12.dp)).background(AruColors.Card).clickable { emojiOpen = true }.padding(horizontal = 14.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(emoji.ifEmpty { "☺" }, fontSize = 22.sp, fontFamily = TgEmojiFont, color = Color.White)
                    Spacer(Modifier.width(10.dp))
                    Text(if (emoji.isEmpty()) "Emoji belgisini tanlash" else "Emoji belgisi", color = AruColors.TextDim, fontSize = 13.sp)
                }
            }
            Spacer(Modifier.weight(1f))
            progress?.let {
                Text(status.ifEmpty { "Yuklanmoqda ${(it * 100).toInt()}%" }, color = AruColors.TextDim, fontSize = 12.sp)
                LinearProgressIndicator(progress = { it }, color = AruColors.Accent2, trackColor = AruColors.CardAlt, modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp).height(3.dp))
            }
            val ok = problem == null && progress == null
            Box(
                Modifier.fillMaxWidth().padding(bottom = 20.dp).height(50.dp).clip(RoundedCornerShape(14.dp)).background(if (ok) AruColors.Accent else AruColors.Accent.copy(alpha = 0.35f)).clickable(enabled = ok) {
                    scope.launch {
                        progress = 0f
                        val err = AppServices.packs.addItem(pack, file, emoji, if (isVideo) trimA else 0, if (isVideo) trimB else 0, sound, onProgress = { progress = it })
                        progress = null
                        if (err == null) onSent() else status = err
                    }
                },
                contentAlignment = Alignment.Center,
            ) { Text(if (progress != null) "Yuklash tugashini kuting" else "Yuborish (1)", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.Bold) }
            if (progress == null && status.isNotEmpty()) Text(status, color = Color(0xFFF87171), fontSize = 12.sp, modifier = Modifier.padding(bottom = 12.dp))
        }
    }
    if (emojiOpen) {
        AlertDialog(
            onDismissRequest = { emojiOpen = false },
            confirmButton = { TextButton(onClick = { emoji = ""; emojiOpen = false }) { Text("Tozalash") } },
            text = {
                LazyVerticalGrid(GridCells.Fixed(7), Modifier.height(300.dp)) {
                    items(TG_EMOJI_GROUPS.first().emoji.take(140)) { e ->
                        Text(e, fontSize = 24.sp, fontFamily = TgEmojiFont, color = Color.White, modifier = Modifier.clickable { emoji = e; emojiOpen = false }.padding(4.dp))
                    }
                }
            },
            containerColor = AruColors.Surface,
        )
    }
}

/**
 * Videoni kesish oynasi (Flutter `PackVideoTrimScreen`): tepada aylanib turuvchi ko'rinish, pastda kadrlar tasmasi;
 * chetlarini yoki oynaning o'zini sudrab bo'lak tanlanadi (uzunlik tur bo'yicha cheklangan). Kesishni telefon emas, Actions bajaradi.
 */
@Composable
fun PackVideoTrimScreen(file: File, kind: String, durationMs: Long, start: Long, end: Long, sound0: Boolean, onCancel: () -> Unit, onDone: (Long, Long, Boolean) -> Unit) {
    BackHandler(onBack = onCancel)
    val ctx = LocalContext.current
    val maxMs = packMaxSeconds(kind) * 1000L
    var a by remember { mutableLongStateOf(start) }
    var b by remember { mutableLongStateOf(if (end > start) end else durationMs) }
    var sound by remember { mutableStateOf(sound0) }
    var frames by remember { mutableStateOf<List<Bitmap>>(emptyList()) }
    val exo = remember { ExoPlayer.Builder(ctx).build().apply { setMediaItem(MediaItem.fromUri(android.net.Uri.fromFile(file))); repeatMode = Player.REPEAT_MODE_OFF; prepare(); playWhenReady = true } }
    DisposableEffect(Unit) { onDispose { exo.release() } }
    LaunchedEffect(sound) { exo.volume = if (sound && kind == PackKind.GIF) 1f else 0f }
    // Bo'lak ichida aylantirish
    LaunchedEffect(a, b) {
        exo.seekTo(a)
        while (true) { delay(120); if (exo.currentPosition >= b || exo.currentPosition < a - 200) exo.seekTo(a) }
    }
    LaunchedEffect(file) {
        frames = withContext(Dispatchers.IO) {
            val r = MediaMetadataRetriever(); val out = ArrayList<Bitmap>()
            try {
                r.setDataSource(file.absolutePath)
                val n = 8
                for (i in 0 until n) {
                    val t = if (durationMs > 1) (durationMs - 1) * 1000L * i / (n - 1) else 0L
                    r.getFrameAtTime(t, MediaMetadataRetriever.OPTION_CLOSEST_SYNC)?.let { f -> out.add(Bitmap.createScaledBitmap(f, 160, (160f * f.height / f.width).toInt().coerceAtLeast(1), true)) }
                }
            } catch (_: Throwable) {} finally { runCatching { r.release() } }
            out
        }
    }
    OverlayScaffold("Videoni kesish", onCancel) {
        Column(Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
            AndroidView(
                factory = { c -> (android.view.LayoutInflater.from(c).inflate(uz.arumediatv.com.R.layout.aru_player_view, null) as PlayerView).apply { useController = false; player = exo } },
                modifier = Modifier.fillMaxWidth().aspectRatio(1.3f).clip(RoundedCornerShape(16.dp)).background(Color.Black),
            )
            Spacer(Modifier.height(14.dp))
            // Vaqt chizig'i: kadrlar + tanlangan oyna
            var width by remember { mutableStateOf(1) }
            val density = LocalDensity.current
            val handle = with(density) { 14.dp.toPx() }
            Box(Modifier.fillMaxWidth().height(64.dp).clip(RoundedCornerShape(10.dp)).background(AruColors.Card).onSizeChanged { width = it.width.coerceAtLeast(1) }) {
                Row(Modifier.fillMaxSize()) { frames.forEach { f -> Image(f.asImageBitmap(), null, contentScale = ContentScale.Crop, modifier = Modifier.weight(1f).fillMaxSize()) } }
                val total = durationMs.coerceAtLeast(1)
                fun px(ms: Long) = (ms.toFloat() / total * width)
                // Tashqarisi xira
                Box(Modifier.offset { IntOffset(0, 0) }.size(with(density) { px(a).toDp() }, 64.dp).background(Color.Black.copy(alpha = 0.6f)))
                Box(Modifier.offset { IntOffset(px(b).roundToInt(), 0) }.size(with(density) { (width - px(b)).coerceAtLeast(0f).toDp() }, 64.dp).background(Color.Black.copy(alpha = 0.6f)))
                // Oyna (suriladi)
                Box(
                    Modifier.offset { IntOffset(px(a).roundToInt(), 0) }.size(with(density) { (px(b) - px(a)).coerceAtLeast(1f).toDp() }, 64.dp)
                        .border(2.dp, AruColors.Accent2, RoundedCornerShape(6.dp))
                        .pointerInput(maxMs, durationMs) {
                            detectDragGestures { ch, d ->
                                ch.consume()
                                val dm = (d.x / width * total).toLong()
                                val len = b - a
                                val na = (a + dm).coerceIn(0, total - len)
                                a = na; b = na + len
                            }
                        },
                )
                // Chap va o'ng tutqichlar
                listOf(true, false).forEach { left ->
                    Box(
                        Modifier.offset { IntOffset((if (left) px(a) - handle / 2 else px(b) - handle / 2).roundToInt(), 0) }.size(14.dp, 64.dp).clip(RoundedCornerShape(4.dp)).background(AruColors.Accent2)
                            .pointerInput(left, maxMs, durationMs) {
                                detectDragGestures { ch, d ->
                                    ch.consume()
                                    val dm = (d.x / width * total).toLong()
                                    if (left) {
                                        var na = (a + dm).coerceIn(0, b - 500)
                                        if (maxMs > 0 && b - na > maxMs) na = b - maxMs
                                        a = na
                                    } else {
                                        var nb = (b + dm).coerceIn(a + 500, total)
                                        if (maxMs > 0 && nb - a > maxMs) nb = a + maxMs
                                        b = nb
                                    }
                                }
                            },
                    )
                }
            }
            Text(
                "${fmtMs(a)} – ${fmtMs(b)}  ·  ${String.format(Locale.US, "%.1f", (b - a) / 1000.0)} s" + if (maxMs > 0) "  ·  eng ko'pi ${maxMs / 1000} s" else "",
                color = Color.White, fontSize = 13.sp, modifier = Modifier.padding(top = 10.dp),
            )
            if (kind == PackKind.GIF) Row(Modifier.padding(top = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(if (sound) "Ovoz bilan" else "Ovozsiz", color = Color.White, fontSize = 14.sp, modifier = Modifier.weight(1f))
                Switch(sound, { sound = it }, colors = SwitchDefaults.colors(checkedThumbColor = Color.White, checkedTrackColor = AruColors.Accent))
            } else Text("Ovoz faqat GIF to'plamida saqlanadi", color = AruColors.TextFaint, fontSize = 12.sp, modifier = Modifier.padding(top = 10.dp))
            Spacer(Modifier.weight(1f))
            Box(Modifier.fillMaxWidth().padding(bottom = 20.dp).height(50.dp).clip(RoundedCornerShape(14.dp)).background(AruColors.Accent).clickable { onDone(a, b, sound) }, contentAlignment = Alignment.Center) {
                Text("Tayyor", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.Bold)
            }
        }
    }
}

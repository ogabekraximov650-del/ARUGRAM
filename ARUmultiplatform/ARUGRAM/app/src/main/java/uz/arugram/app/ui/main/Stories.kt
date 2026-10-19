package uz.arugram.app.ui.main

import android.net.Uri
import android.widget.VideoView
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import uz.arugram.app.core.RustCore
import uz.arugram.app.ui.components.CloseIcon
import uz.arugram.app.ui.theme.LocalTg
import java.io.File

/** Bitta hikoya (Rust `story_json`). */
data class StoryItemUi(val id: Int, val date: Long, val expire: Long, val kind: String, val caption: String, val out: Boolean)

/** Bir eganing faol hikoyalari (Rust `rust_tg_stories`). */
data class StoryPeer(
    val id: Long, val kind: String, val name: String, val hash: String, val photoId: String,
    val maxRead: Int, val stories: List<StoryItemUi>,
) {
    val spec: String get() = if (hash.isNotEmpty() && hash != "0") "$id@$hash" else "$id"

    /** Hali ko'rilmagan hikoya bormi (Telegram'da halqa ko'k, aks holda kulrang). */
    val unseen: Boolean get() = stories.any { it.id > maxRead }

    /** Avatar chizish uchun yengil `ChatRow`. */
    val row: ChatRow get() = ChatRow(id, kind, name, false, 0, false, false, 0, "", false, false, false, access = hash)

    /** Ko'rishni qaysi hikoyadan boshlash kerak: birinchi ko'rilmagan, aks holda birinchisi. */
    val firstUnseenIndex: Int get() = stories.indexOfFirst { it.id > maxRead }.let { if (it < 0) 0 else it }
}

fun parseStories(j: JSONObject): List<StoryPeer> = buildList {
    val arr = j.optJSONArray("peers") ?: return@buildList
    for (i in 0 until arr.length()) {
        val o = arr.getJSONObject(i)
        val sa = o.optJSONArray("stories") ?: continue
        val items = buildList {
            for (k in 0 until sa.length()) {
                val s = sa.getJSONObject(k)
                add(StoryItemUi(s.optInt("id"), s.optLong("date"), s.optLong("expire"), s.optString("kind"), s.optString("caption"), s.optBoolean("out")))
            }
        }
        if (items.isEmpty()) continue
        add(StoryPeer(o.optLong("id"), o.optString("kind"), o.optString("name"), o.optString("hash", "0"), o.optString("photo_id", "0"), o.optInt("max_read"), items))
    }
}

private val RingUnseen = Brush.sweepGradient(listOf(Color(0xFF34C76F), Color(0xFF3DA1FD), Color(0xFF34C76F)))
private val RingSeen = Brush.sweepGradient(listOf(Color(0xFFB0B4B8), Color(0xFFB0B4B8)))

/**
 * Chatlar ro'yxati tepasidagi hikoyalar qatori (DrKLO `DialogStoriesCell`): 70 dp kenglikdagi dumaloq avatarlar, atrofida ko'rilmagan
 * hikoyalar uchun rangli halqa, pastda ism (11 sp). Birinchisi — "Hikoyam" (o'zingizniki yoki yangi qo'shish).
 */
@Composable
fun StoriesRow(peers: List<StoryPeer>, meId: Long?, onOpen: (Int) -> Unit, onAdd: () -> Unit) {
    val c = LocalTg.current
    val ctx = LocalContext.current
    LaunchedEffect(peers.map { it.id }) {
        val users = peers.filter { it.kind == "user" && it.photoId != "0" }.map { Triple(it.id, it.hash, it.photoId) }
        if (users.isNotEmpty()) AvatarCache.ensureUsers(ctx, users)
        val others = peers.filter { it.kind != "user" }.map { it.id }
        if (others.isNotEmpty()) AvatarCache.ensure(ctx, others)
    }
    val mine = peers.indexOfFirst { it.id == meId }
    LazyRow(
        Modifier.fillMaxWidth().background(c.bar).padding(vertical = 8.dp),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        item(key = "mine") {
            val own = peers.getOrNull(mine)
            Column(Modifier.width(70.dp).clickable { if (own != null) onOpen(mine) else onAdd() }, horizontalAlignment = Alignment.CenterHorizontally) {
                Box(Modifier.size(62.dp), contentAlignment = Alignment.Center) {
                    if (own != null) Ring(own.unseen)
                    val av = ChatRow(meId ?: 0L, "user", "", false, 0, false, false, 0, "", false, false, false)
                    ChatAvatar(av, 52.dp)
                    Box(
                        Modifier.align(Alignment.BottomEnd).size(20.dp).clip(CircleShape).background(c.bar).padding(2.dp).clip(CircleShape).background(c.fab)
                            .clickable(onClick = onAdd),
                        contentAlignment = Alignment.Center,
                    ) { BasicText("+", style = TextStyle(color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold)) }
                }
                BasicText("Hikoyam", Modifier.padding(top = 3.dp), style = TextStyle(color = c.gray, fontSize = 11.sp), maxLines = 1)
            }
        }
        items(peers.withIndex().filter { it.index != mine }.toList(), key = { it.value.id }) { (idx, p) ->
            Column(Modifier.width(70.dp).clickable { onOpen(idx) }, horizontalAlignment = Alignment.CenterHorizontally) {
                Box(Modifier.size(62.dp), contentAlignment = Alignment.Center) {
                    Ring(p.unseen)
                    ChatAvatar(p.row, 52.dp)
                }
                BasicText(
                    p.name.substringBefore(' '), Modifier.padding(top = 3.dp),
                    style = TextStyle(color = if (p.unseen) c.text else c.gray, fontSize = 11.sp, textAlign = TextAlign.Center),
                    maxLines = 1, overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

@Composable
private fun Ring(unseen: Boolean) {
    Canvas(Modifier.size(62.dp)) {
        val w = 2.dp.toPx()
        drawCircle(brush = if (unseen) RingUnseen else RingSeen, radius = size.minDimension / 2f - w / 2f, style = Stroke(width = if (unseen) w else 1.5.dp.toPx()))
    }
}

/**
 * Hikoyalar ko'rgichi (DrKLO `StoryViewer`): qora to'liq ekran, tepada segmentli progress (rasm 5 soniya, video o'z davomiyligicha),
 * o'ng/chap tomonga bosish — keyingi/oldingi, bosib turish — to'xtatish, pastga surish — yopish. Ko'rilgan hikoya serverga belgilanadi.
 */
@Composable
fun StoryViewer(peers: List<StoryPeer>, start: Int, onClose: () -> Unit) {
    val ctx = LocalContext.current
    var pi by remember { mutableIntStateOf(start.coerceIn(0, (peers.size - 1).coerceAtLeast(0))) }
    var si by remember { mutableIntStateOf(peers.getOrNull(pi)?.firstUnseenIndex ?: 0) }
    val peer = peers.getOrNull(pi)
    val story = peer?.stories?.getOrNull(si)
    var file by remember { mutableStateOf<File?>(null) }
    var failed by remember { mutableStateOf(false) }
    var paused by remember { mutableStateOf(false) }
    var progress by remember { mutableFloatStateOf(0f) }
    var video by remember { mutableStateOf<VideoView?>(null) }

    fun next() {
        val p = peer ?: return onClose()
        if (si + 1 < p.stories.size) si++
        else if (pi + 1 < peers.size) { pi++; si = peers[pi].firstUnseenIndex }
        else onClose()
    }
    fun prev() {
        if (si > 0) si--
        else if (pi > 0) { pi--; si = 0 }
        else progress = 0f
    }

    LaunchedEffect(peer?.id, story?.id) {
        file = null; failed = false; progress = 0f; video = null
        val p = peer ?: return@LaunchedEffect
        val s = story ?: return@LaunchedEffect
        val dir = File(ctx.filesDir, "stories").apply { mkdirs() }
        val r = withContext(Dispatchers.IO) { RustCore.tgStoryFile(p.spec, s.id, dir.absolutePath) }
        val path = r.optString("path")
        if (path.isEmpty()) failed = true else file = File(path)
        withContext(Dispatchers.IO) { RustCore.tgStoryRead(p.spec, s.id) }
    }

    // Vaqt chizig'i: rasmda 5 soniya; videoda joriy pozitsiya / davomiylik. To'xtatilganda yoki yuklanmaganda yurmaydi.
    LaunchedEffect(peer?.id, story?.id, file) {
        if (file == null) return@LaunchedEffect
        if (story?.kind == "video") {
            while (true) {
                val v = video
                if (v != null && v.duration > 0) {
                    progress = (v.currentPosition.toFloat() / v.duration).coerceIn(0f, 1f)
                    if (progress >= 0.995f) { next(); return@LaunchedEffect }
                }
                androidx.compose.runtime.withFrameMillis { }
            }
        } else {
            var last = 0L
            while (progress < 1f) {
                val now = androidx.compose.runtime.withFrameMillis { it }
                if (last != 0L && !paused) progress += (now - last) / 5000f
                last = now
            }
            next()
        }
    }
    LaunchedEffect(paused, video) { video?.let { if (paused) it.pause() else if (!it.isPlaying) it.start() } }

    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        Box(
            Modifier.fillMaxSize().background(Color.Black)
                .pointerInput(peer?.id, story?.id) {
                    detectTapGestures(
                        onPress = { paused = true; tryAwaitRelease(); paused = false },
                        onTap = { o -> if (o.x < size.width / 3f) prev() else next() },
                    )
                }
                .pointerInput(Unit) {
                    var dy = 0f
                    detectVerticalDragGestures(onDragEnd = { dy = 0f }, onDragCancel = { dy = 0f }) { _, d ->
                        dy += d
                        if (dy > 220f) { dy = 0f; onClose() }
                    }
                },
        ) {
            val f = file
            when {
                failed -> BasicText("Hikoyani yuklab bo‘lmadi", Modifier.align(Alignment.Center), style = TextStyle(color = Color.White, fontSize = 16.sp))
                f == null -> CircularProgressIndicator(Modifier.align(Alignment.Center).size(40.dp), color = Color.White, strokeWidth = 3.dp)
                story?.kind == "video" -> AndroidView(
                    modifier = Modifier.fillMaxSize(),
                    factory = { c ->
                        VideoView(c).apply {
                            setVideoPath(f.path)
                            val self = this
                            setOnPreparedListener { it.isLooping = false; start(); video = self }
                        }
                    },
                    onRelease = { it.stopPlayback() },
                )
                else -> {
                    val bmp = remember(f.path) { MediaCache.decode(f)?.asImageBitmap() }
                    if (bmp != null) Image(bmp, null, Modifier.fillMaxSize(), contentScale = ContentScale.Fit)
                }
            }
            Column(Modifier.fillMaxWidth().statusBarsPadding().padding(horizontal = 8.dp, vertical = 8.dp)) {
                // Segmentli progress
                Row(Modifier.fillMaxWidth().height(2.dp), horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                    val n = peer?.stories?.size ?: 0
                    for (i in 0 until n) {
                        val v = when {
                            i < si -> 1f
                            i == si -> progress.coerceIn(0f, 1f)
                            else -> 0f
                        }
                        Canvas(Modifier.weight(1f).height(2.dp)) {
                            drawRoundRect(Color(0x66FFFFFF), size = Size(size.width, size.height), cornerRadius = CornerRadius(1.dp.toPx()))
                            drawRoundRect(Color.White, size = Size(size.width * v, size.height), cornerRadius = CornerRadius(1.dp.toPx()))
                        }
                    }
                }
                Row(Modifier.fillMaxWidth().padding(top = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                    if (peer != null) ChatAvatar(peer.row, 36.dp)
                    Column(Modifier.weight(1f).padding(start = 10.dp)) {
                        BasicText(peer?.name.orEmpty(), style = TextStyle(color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.Medium), maxLines = 1, overflow = TextOverflow.Ellipsis)
                        if (story != null) BasicText(chatTime(story.date), style = TextStyle(color = Color(0xB3FFFFFF), fontSize = 12.sp))
                    }
                    Box(Modifier.size(44.dp).clickable(onClick = onClose), contentAlignment = Alignment.Center) { CloseIcon(Color.White) }
                }
            }
            val cap = story?.caption.orEmpty()
            if (cap.isNotEmpty()) {
                BasicText(
                    cap,
                    Modifier.align(Alignment.BottomCenter).fillMaxWidth().background(Brush.verticalGradient(listOf(Color.Transparent, Color(0x99000000)))).padding(horizontal = 16.dp, vertical = 24.dp),
                    style = TextStyle(color = Color.White, fontSize = 16.sp),
                )
            }
        }
    }
}

/** Galereyadan tanlangan rasm/videoni hikoya qilib joylaydi. Qaytadi: xato matni yoki bo'sh (muvaffaqiyat). */
suspend fun postStory(ctx: android.content.Context, uri: Uri, caption: String, audience: String): String {
    val mime = ctx.contentResolver.getType(uri).orEmpty()
    val f = withContext(Dispatchers.IO) { copyUriToCache(ctx, uri) } ?: return "Fayl ochilmadi"
    val video = mime.startsWith("video/")
    var extra = ""
    if (video) {
        runCatching {
            val r = android.media.MediaMetadataRetriever()
            r.setDataSource(f.absolutePath)
            val dur = (r.extractMetadata(android.media.MediaMetadataRetriever.METADATA_KEY_DURATION)?.toLongOrNull() ?: 0L) / 1000.0
            val w = r.extractMetadata(android.media.MediaMetadataRetriever.METADATA_KEY_VIDEO_WIDTH)?.toIntOrNull() ?: 720
            val h = r.extractMetadata(android.media.MediaMetadataRetriever.METADATA_KEY_VIDEO_HEIGHT)?.toIntOrNull() ?: 1280
            r.release()
            extra = "$dur:$w:$h"
        }
    }
    val res = withContext(Dispatchers.IO) { RustCore.tgStorySend(if (video) "video" else "photo", f.absolutePath, caption, audience, extra) }
    f.delete()
    return res.optString("error")
}

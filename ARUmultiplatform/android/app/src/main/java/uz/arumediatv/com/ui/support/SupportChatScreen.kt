package uz.arumediatv.com.ui.support

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.media.MediaPlayer
import android.media.MediaRecorder
import android.net.Uri
import android.os.Build
import android.provider.OpenableColumns
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.R
import uz.arumediatv.com.data.ChatMessage
import uz.arumediatv.com.data.ChatStore
import uz.arumediatv.com.ui.common.AruIcons
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.packs.TgEmojiFont
import uz.arumediatv.com.ui.packs.TgMediaPanel
import uz.arumediatv.com.ui.season.AruPlayer
import uz.arumediatv.com.ui.season.PlayerSurface
import uz.arumediatv.com.ui.season.formatClock
import uz.arumediatv.com.ui.theme.AruColors
import java.io.File
import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Date
import java.util.Locale
import java.util.TimeZone
import kotlin.math.abs
import kotlin.math.roundToInt

// ── Telegram qorong'i mavzusi (Flutter `support_chat_screen.dart` ranglari) ──
private val TgPill = Color(0xF0241F1C)
private val TgIn = Color(0xFF2A2420)
private val TgOut = Color(0xFF7A4A2A)
private val TgOutMeta = Color(0xFFE2BE9C)

private val hm = ThreadLocal.withInitial { SimpleDateFormat("HH:mm", Locale.US).apply { timeZone = TimeZone.getTimeZone("GMT+5") } }
private val MONTHS = listOf("yanvar", "fevral", "mart", "aprel", "may", "iyun", "iyul", "avgust", "sentyabr", "oktyabr", "noyabr", "dekabr")

private fun cal(ms: Long): Calendar = Calendar.getInstance(TimeZone.getTimeZone("GMT+5")).apply { timeInMillis = ms }

private fun sameDay(a: Long, b: Long): Boolean = cal(a).let { x -> cal(b).let { y -> x[Calendar.YEAR] == y[Calendar.YEAR] && x[Calendar.DAY_OF_YEAR] == y[Calendar.DAY_OF_YEAR] } }

/** "Bugun" / "Kecha" / "12-sentyabr" (Flutter `tgDayLabel`). */
private fun dayLabel(ms: Long): String {
    val d = cal(ms)
    val now = cal(System.currentTimeMillis())
    fun num(c: Calendar) = c[Calendar.YEAR] * 366 + c[Calendar.DAY_OF_YEAR]
    val diff = num(now) - num(d)
    if (diff == 0) return "Bugun"
    if (diff == 1) return "Kecha"
    val s = "${d[Calendar.DAY_OF_MONTH]}-${MONTHS[d[Calendar.MONTH]]}"
    return if (d[Calendar.YEAR] == now[Calendar.YEAR]) s else "$s, ${d[Calendar.YEAR]}"
}

// ── Javob: xabar matni boshida `[re:<id>]` (Flutter `tgSplitReply`) ──
private val RE = Regex("^\\[re:([A-Za-z0-9_-]{1,40})\\]")
private fun splitReply(body: String): Pair<String?, String> = RE.find(body)?.let { it.groupValues[1] to body.substring(it.range.last + 1) } ?: (null to body)
private fun withReply(id: String?, text: String) = if (id.isNullOrEmpty()) text else "[re:$id]$text"

private val PACK_TOKEN = Regex("\\[pe:\\d{1,16}:\\d{1,9}:([^\\]]{0,16})\\]")
private fun plain(s: String) = PACK_TOKEN.replace(s) { it.groupValues[1] }

private fun snippet(m: ChatMessage): String {
    val body = splitReply(m.body).second
    if (m.hasMedia) {
        val label = when (m.mediaType) {
            "voice" -> "Ovozli xabar"; "round" -> "Video xabar"; "sticker" -> "Stiker"; "gif" -> "GIF"
            "file" -> m.body.ifEmpty { "Fayl" }; "video" -> "Video"; else -> "Rasm"
        }
        if (m.mediaType == "file" || m.mediaType == "voice" || body.isEmpty()) return label
        return "$label, ${plain(body)}"
    }
    return plain(body).replace('\n', ' ')
}

private val MIMES = mapOf(
    "jpg" to "image/jpeg", "jpeg" to "image/jpeg", "png" to "image/png", "webp" to "image/webp", "gif" to "image/gif", "heic" to "image/heic",
    "mp4" to "video/mp4", "mov" to "video/quicktime", "mkv" to "video/x-matroska", "webm" to "video/webm", "3gp" to "video/3gpp",
    "mp3" to "audio/mpeg", "m4a" to "audio/mp4", "ogg" to "audio/ogg", "wav" to "audio/wav", "flac" to "audio/flac",
    "pdf" to "application/pdf", "zip" to "application/zip", "txt" to "text/plain", "apk" to "application/vnd.android.package-archive",
)

/**
 * Admin bilan yozishma (Flutter `SupportChatScreen`, Telegram uslubi): suzuvchi sarlavha, kun ajratgichlari, xabarga javob (surib
 * yoki menyudan) va iqtibos, nusxa olish, qidiruv, tanlab o'chirish (admin), rasm/video/fayl, ovozli xabar (bosib turib; chapga
 * surilsa bekor, tepaga surilsa qulf), emoji/GIF/stiker paneli.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun SupportChatScreen(userId: Long?, title: String, onBack: () -> Unit, onPacks: () -> Unit = {}) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val store = remember(userId) { ChatStore(ctx.applicationContext, AppServices.auth, userId) }
    val items by store.items.collectAsStateWithLifecycle()
    val error by store.error.collectAsStateWithLifecycle()
    val loading by store.loading.collectAsStateWithLifecycle()
    val me by AppServices.auth.user.collectAsStateWithLifecycle()
    val adminSide = userId != null
    val canDelete = me?.isAdmin == true && adminSide
    var text by remember { mutableStateOf("") }
    var upProgress by remember { mutableStateOf<Float?>(null) }
    var upLabel by remember { mutableStateOf("") }
    var viewer by remember { mutableStateOf<ChatMessage?>(null) }
    var panelOpen by remember { mutableStateOf(false) }
    var replyTo by remember { mutableStateOf<ChatMessage?>(null) }
    var searching by remember { mutableStateOf(false) }
    var query by remember { mutableStateOf("") }
    var hit by remember { mutableStateOf(0) }
    var flash by remember { mutableStateOf<String?>(null) }
    var menuFor by remember { mutableStateOf<ChatMessage?>(null) }
    var moreMenu by remember { mutableStateOf(false) }
    var confirm by remember { mutableStateOf<Pair<String, () -> Unit>?>(null) }
    val selected = remember { mutableStateListOf<String>() }
    val listState = rememberLazyListState()
    val focus = LocalFocusManager.current
    val keyboard = LocalSoftwareKeyboardController.current
    fun toast(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_SHORT).show()
    fun nameOf(m: ChatMessage) = if (m.fromAdmin == adminSide) "Siz" else title.ifEmpty { "Admin" }
    val byId = remember(items) { items.associateBy { it.id } }
    val hits = remember(items, query) {
        val q = query.trim().lowercase()
        if (q.isEmpty()) emptyList() else items.reversed().filter { plain(splitReply(it.body).second).lowercase().contains(q) }.map { it.id }
    }

    LaunchedEffect(store) { store.loadFromDisk(); store.load() }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    DisposableEffect(store, lifecycle) {
        if (lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) store.startWatching()
        val ob = LifecycleEventObserver { _, e -> if (e == Lifecycle.Event.ON_RESUME) store.startWatching() else if (e == Lifecycle.Event.ON_PAUSE) store.stopWatching() }
        lifecycle.addObserver(ob)
        onDispose { lifecycle.removeObserver(ob); store.close(); VoicePlayer.stop() }
    }
    LaunchedEffect(items.size) { if (items.isNotEmpty() && !searching) listState.scrollToItem(items.lastIndex + 1) }

    fun goTo(id: String) {
        val i = items.indexOfFirst { it.id == id }
        if (i < 0) { toast("Asl xabar topilmadi"); return }
        scope.launch {
            listState.animateScrollToItem(i, scrollOffset = -200)
            flash = id
            delay(1200)
            if (flash == id) flash = null
        }
    }
    LaunchedEffect(hits, hit) { if (hits.isNotEmpty()) goTo(hits[hit.coerceIn(0, hits.lastIndex)]) }

    // ── Fayl yuborish ──
    fun sendUri(uri: Uri, type: String, body: String = "", ms: Long = 0) {
        scope.launch {
            val uid = me?.id ?: 0L
            val display = withContext(Dispatchers.IO) {
                try { ctx.contentResolver.query(uri, null, null, null, null)?.use { c -> if (c.moveToFirst()) c.getString(c.getColumnIndexOrThrow(OpenableColumns.DISPLAY_NAME)) else null } } catch (_: Exception) { null }
            }.orEmpty()
            var ext = display.substringAfterLast('.', "").lowercase()
            if (ext.isEmpty() || ext.length > 5 || !ext.all { it.isLetterOrDigit() }) ext = when (type) { "video" -> "mp4"; "image" -> "jpg"; else -> "bin" }
            val mime = MIMES[ext] ?: ctx.contentResolver.getType(uri) ?: "application/octet-stream"
            val name = "chat_${uid}_${System.currentTimeMillis()}.$ext"
            upProgress = 0f; upLabel = if (type == "file") display else ""
            val local = withContext(Dispatchers.IO) {
                try { File(ctx.cacheDir, name).also { f -> ctx.contentResolver.openInputStream(uri)?.use { i -> f.outputStream().use { o -> i.copyTo(o) } } } } catch (_: Exception) { null }
            }
            if (local == null) { upProgress = null; toast("Faylni o'qib bo'lmadi"); return@launch }
            val err = AppServices.telegram.uploadFile(local.absolutePath, name, mime, onProgress = { s, t -> if (t > 0) upProgress = s.toFloat() / t })
            if (err != null) toast(err) else store.send(withReply(replyTo?.id, if (type == "file") display else body), mediaFile = name, mediaType = type, mediaMs = ms)?.let(::toast)
            if (err == null) replyTo = null
            runCatching { local.delete() }
            upProgress = null
        }
    }
    val pickMedia = rememberLauncherForActivityResult(ActivityResultContracts.PickMultipleVisualMedia(10)) { l ->
        l.forEach { u -> sendUri(u, if ((ctx.contentResolver.getType(u) ?: "").startsWith("video")) "video" else "image") }
    }
    val pickFile = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { l -> l.forEach { u -> sendUri(u, "file") } }

    // ── Ovoz yozish ──
    var recording by remember { mutableStateOf(false) }
    var locked by remember { mutableStateOf(false) }
    var dragX by remember { mutableFloatStateOf(0f) }
    var recStart by remember { mutableLongStateOf(0L) }
    var recorder by remember { mutableStateOf<MediaRecorder?>(null) }
    var recFile by remember { mutableStateOf<File?>(null) }
    var recMs by remember { mutableLongStateOf(0L) }
    fun startRec(): Boolean {
        return try {
            val f = File(ctx.cacheDir, "voice_${System.currentTimeMillis()}.m4a")
            val r = if (Build.VERSION.SDK_INT >= 31) MediaRecorder(ctx) else @Suppress("DEPRECATION") MediaRecorder()
            r.setAudioSource(MediaRecorder.AudioSource.MIC); r.setOutputFormat(MediaRecorder.OutputFormat.MPEG_4); r.setAudioEncoder(MediaRecorder.AudioEncoder.AAC)
            r.setAudioEncodingBitRate(64_000); r.setAudioSamplingRate(44_100); r.setOutputFile(f.absolutePath)
            r.prepare(); r.start()
            VoicePlayer.stop()
            recorder = r; recFile = f; recStart = System.currentTimeMillis(); recording = true; locked = false; dragX = 0f
            true
        } catch (e: Exception) { toast("Yozib bo'lmadi: ${e.message ?: ""}"); recording = false; false }
    }
    fun stopRec(send: Boolean) {
        val r = recorder; val f = recFile
        recording = false; locked = false; dragX = 0f; recorder = null
        val ms = System.currentTimeMillis() - recStart
        try { r?.stop() } catch (_: Exception) {}
        runCatching { r?.release() }
        if (!send || f == null) { f?.delete(); return }
        if (ms < 1000) { f.delete(); toast("Juda qisqa"); return }
        scope.launch {
            val uid = me?.id ?: 0L
            val name = "chat_${uid}_${System.currentTimeMillis()}.m4a"
            upProgress = 0f; upLabel = ""
            val err = AppServices.telegram.uploadFile(f.absolutePath, name, "audio/mp4", onProgress = { s, t -> if (t > 0) upProgress = s.toFloat() / t })
            if (err != null) toast(err) else store.send(withReply(replyTo?.id, ""), mediaFile = name, mediaType = "voice", mediaMs = ms)?.let(::toast)
            if (err == null) replyTo = null
            runCatching { f.delete() }
            upProgress = null
        }
    }
    val micPerm = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok -> if (!ok) toast("Mikrofonga ruxsat berilmadi") }
    LaunchedEffect(recording) { while (recording) { recMs = System.currentTimeMillis() - recStart; delay(80) } }
    DisposableEffect(Unit) { onDispose { runCatching { recorder?.stop() }; runCatching { recorder?.release() } } }

    fun send() {
        val t = text.trim()
        if (t.isEmpty()) return
        val reply = replyTo
        text = ""; replyTo = null
        scope.launch { store.send(withReply(reply?.id, t))?.let { e -> toast(e); text = t } }
    }
    fun deleteSelected() {
        val ids = selected.toList()
        if (ids.isEmpty()) return
        confirm = (if (ids.size == 1) "Xabar butunlay o'chirilsinmi?" else "${ids.size} ta xabar butunlay o'chirilsinmi?") to {
            selected.clear()
            scope.launch { store.remove(ids)?.let(::toast) }
        }
    }
    val selecting = selected.isNotEmpty()
    BackHandler(enabled = selecting || searching || panelOpen) { if (selecting) selected.clear() else if (searching) { searching = false; query = "" } else panelOpen = false }
    BackHandler(enabled = !selecting && !searching && !panelOpen, onBack = onBack)

    Box(Modifier.fillMaxSize().background(Color(0xFF0C0A09))) {
        // Fon: gradient + naqsh (Flutter `TgChatBackground`)
        Box(Modifier.fillMaxSize().background(Brush.linearGradient(listOf(Color(0xFF6B3A1C).copy(alpha = 0.26f), Color(0xFF3E2A4A).copy(alpha = 0.22f), Color(0xFF2B3B4A).copy(alpha = 0.22f), Color(0xFF7A4A1E).copy(alpha = 0.26f)))))
        Image(painterResource(R.drawable.tg_pattern), null, contentScale = ContentScale.Crop, alpha = 0.10f, modifier = Modifier.fillMaxSize())

        Column(Modifier.fillMaxSize().statusBarsPadding().imePadding()) {
            // ── Sarlavha ──
            Row(Modifier.fillMaxWidth().padding(start = 10.dp, end = 10.dp, top = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                when {
                    selecting -> {
                        RoundBtn(Icons.Filled.Close) { selected.clear() }
                        Spacer(Modifier.width(8.dp))
                        Pill(Modifier.weight(1f).padding(horizontal = 20.dp)) { Text("${selected.size} ta tanlandi", color = Color.White, fontSize = 18.sp, fontWeight = FontWeight.SemiBold) }
                        Spacer(Modifier.width(8.dp))
                        Row(Modifier.height(56.dp).clip(RoundedCornerShape(28.dp)).background(TgPill), verticalAlignment = Alignment.CenterVertically) {
                            val all = items.isNotEmpty() && selected.size >= items.size
                            Box(Modifier.size(48.dp).clickable { if (all) selected.clear() else { selected.clear(); selected.addAll(items.map { it.id }) } }, contentAlignment = Alignment.Center) {
                                Text(if (all) "☐" else "☑", color = Color.White, fontSize = 22.sp)
                            }
                            Box(Modifier.size(48.dp).clickable { deleteSelected() }, contentAlignment = Alignment.Center) { Icon(AruIcons.DeleteOutline, null, tint = Color(0xFFFF5A5A), modifier = Modifier.size(24.dp)) }
                        }
                    }
                    searching -> {
                        RoundBtn(Icons.AutoMirrored.Filled.ArrowBack) { searching = false; query = ""; hit = 0 }
                        Spacer(Modifier.width(8.dp))
                        Pill(Modifier.weight(1f).padding(horizontal = 18.dp)) {
                            BasicTextField(
                                query, { query = it; hit = 0 }, singleLine = true, cursorBrush = SolidColor(AruColors.Accent),
                                textStyle = TextStyle(color = Color.White, fontSize = 17.sp, fontFamily = TgEmojiFont),
                                decorationBox = { inner -> Box { if (query.isEmpty()) Text("Qidiruv", color = Color.White.copy(alpha = 0.4f), fontSize = 17.sp); inner() } },
                                modifier = Modifier.fillMaxWidth(),
                            )
                        }
                    }
                    else -> {
                        RoundBtn(Icons.AutoMirrored.Filled.ArrowBack, onBack)
                        Spacer(Modifier.width(8.dp))
                        Row(Modifier.weight(1f).height(56.dp).clip(RoundedCornerShape(28.dp)).background(TgPill).padding(start = 6.dp, end = 18.dp), verticalAlignment = Alignment.CenterVertically) {
                            if (adminSide) Box(Modifier.size(44.dp).clip(CircleShape).background(AruColors.AccentTint), contentAlignment = Alignment.Center) {
                                Text(title.removePrefix("@").take(1).uppercase(), color = AruColors.Accent2, fontWeight = FontWeight.Bold, fontSize = 18.sp)
                            } else Image(painterResource(R.drawable.aru_mark), null, modifier = Modifier.size(44.dp).clip(CircleShape).background(Color.Black).padding(7.dp))
                            Spacer(Modifier.width(10.dp))
                            Column(Modifier.weight(1f), verticalArrangement = Arrangement.Center) {
                                Text(title, color = Color.White, fontSize = 19.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                Text(if (adminSide) "ID: $userId" else "yordam xizmati", color = Color.White.copy(alpha = 0.5f), fontSize = 15.sp, maxLines = 1)
                            }
                        }
                        Spacer(Modifier.width(8.dp))
                        Box {
                            RoundBtn(AruIcons.MoreVert) { moreMenu = true }
                            DropdownMenu(moreMenu, { moreMenu = false }, containerColor = Color(0xFF2A2420)) {
                                DropdownMenuItem(text = { Text("Qidiruv") }, onClick = { moreMenu = false; searching = true })
                                if (canDelete) {
                                    DropdownMenuItem(text = { Text("Tanlash") }, onClick = { moreMenu = false; items.lastOrNull()?.let { selected.add(it.id) } })
                                    DropdownMenuItem(text = { Text("Tarixni tozalash", color = Color(0xFFFF5A5A)) }, onClick = {
                                        moreMenu = false
                                        if (items.isNotEmpty()) confirm = "Butun yozishma o'chirilsinmi?" to { scope.launch { store.remove(items.map { it.id })?.let(::toast) } }
                                    })
                                }
                            }
                        }
                    }
                }
            }
            if (searching) Row(Modifier.padding(start = 10.dp, end = 10.dp, top = 6.dp, bottom = 2.dp).fillMaxWidth().clip(RoundedCornerShape(18.dp)).background(TgPill).padding(horizontal = 14.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(if (query.isBlank()) "" else if (hits.isEmpty()) "Hech narsa topilmadi" else "${hit + 1} / ${hits.size}", color = Color.White.copy(alpha = 0.7f), fontSize = 14.sp, modifier = Modifier.weight(1f))
                Text("▲", color = Color.White, fontSize = 18.sp, modifier = Modifier.clickable { if (hits.isNotEmpty()) hit = (hit + 1).coerceAtMost(hits.lastIndex) }.padding(10.dp))
                Text("▼", color = Color.White, fontSize = 18.sp, modifier = Modifier.clickable { if (hits.isNotEmpty()) hit = (hit - 1).coerceAtLeast(0) }.padding(10.dp))
            }

            // ── Xabarlar ──
            Box(Modifier.weight(1f).fillMaxWidth()) {
                when {
                    loading && items.isEmpty() -> LoadingBox()
                    items.isEmpty() && upProgress == null -> EmptyState(AruIcons.SupportAgent, error ?: if (adminSide) "Bu odam hali yozmagan" else "Savolingiz bormi? Yozing — admin javob beradi.")
                    else -> LazyColumn(Modifier.fillMaxSize(), state = listState, contentPadding = PaddingValues(start = 12.dp, end = 12.dp, top = 14.dp, bottom = 10.dp)) {
                        itemsIndexed(items, key = { _, m -> m.id }) { i, m ->
                            val mine = m.fromAdmin == adminSide
                            val prev = items.getOrNull(i - 1)
                            val next = items.getOrNull(i + 1)
                            val newDay = prev == null || !sameDay(prev.createdAt, m.createdAt)
                            val lastOfGroup = next == null || next.fromAdmin != m.fromAdmin || !sameDay(next.createdAt, m.createdAt)
                            val topNear = !newDay && prev != null && prev.fromAdmin == m.fromAdmin
                            if (newDay) DateChip(dayLabel(m.createdAt))
                            val rid = splitReply(m.body).first
                            Box {
                                SwipeReply(enabled = !selecting && !m.pending, onReply = { replyTo = m; focus.clearFocus() }) {
                                    Bubble(
                                        m, mine, topNear = topNear, bottomNear = !lastOfGroup, selected = m.id in selected || flash == m.id,
                                        quote = rid?.let { id -> byId[id]?.let { o -> nameOf(o) to snippet(o) } ?: ("Xabar" to "o'chirilgan") },
                                        onQuote = { rid?.let(::goTo) },
                                        onTap = { if (selecting) { if (m.id in selected) selected.remove(m.id) else selected.add(m.id) } else if (m.hasMedia && m.mediaType != "voice") viewer = m },
                                        onLong = { if (!selecting) menuFor = m },
                                    )
                                }
                                DropdownMenu(menuFor?.id == m.id, { menuFor = null }, containerColor = Color(0xFF26272B)) {
                                    val t = splitReply(m.body).second
                                    if (!m.pending) DropdownMenuItem(text = { Text("Javob berish") }, onClick = { menuFor = null; replyTo = m })
                                    if (!m.hasMedia && t.isNotEmpty()) DropdownMenuItem(text = { Text("Nusxa olish") }, onClick = {
                                        menuFor = null
                                        (ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(ClipData.newPlainText("xabar", plain(t)))
                                        toast("Nusxa olindi")
                                    })
                                    if (canDelete) {
                                        DropdownMenuItem(text = { Text("Tanlash") }, onClick = { menuFor = null; selected.add(m.id) })
                                        DropdownMenuItem(text = { Text("O'chirish", color = Color(0xFFFF5A5A)) }, onClick = { menuFor = null; selected.clear(); selected.add(m.id); deleteSelected() })
                                    }
                                }
                            }
                        }
                        item(key = "up") {
                            upProgress?.let { p -> UploadingBubble(upLabel, p) }
                        }
                    }
                }
            }

            // ── Yozish ──
            Column(Modifier.fillMaxWidth()) {
                replyTo?.let { r ->
                    if (!recording) Row(
                        Modifier.padding(start = 10.dp, end = 10.dp, top = 6.dp).fillMaxWidth().clip(RoundedCornerShape(18.dp)).background(TgPill).padding(start = 14.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Box(Modifier.width(3.dp).height(34.dp).background(AruColors.Accent2, RoundedCornerShape(2.dp)))
                        Column(Modifier.weight(1f).padding(horizontal = 10.dp)) {
                            Text(nameOf(r), color = AruColors.Accent2, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1)
                            Text(snippet(r), color = Color.White, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                        Box(Modifier.size(40.dp).clip(CircleShape).clickable { replyTo = null }, contentAlignment = Alignment.Center) { Icon(Icons.Filled.Close, null, tint = Color.White.copy(alpha = 0.6f), modifier = Modifier.size(20.dp)) }
                    }
                }
                Row(Modifier.fillMaxWidth().padding(start = 10.dp, end = 10.dp, top = 6.dp, bottom = 6.dp), verticalAlignment = Alignment.Bottom) {
                    if (recording) {
                        Row(Modifier.weight(1f).height(48.dp).clip(RoundedCornerShape(24.dp)).background(TgPill), verticalAlignment = Alignment.CenterVertically) {
                            Spacer(Modifier.width(14.dp))
                            Box(Modifier.size(10.dp).clip(CircleShape).background(Color(0xFFE5484D)))
                            Spacer(Modifier.width(10.dp))
                            Text(String.format(Locale.US, "%d:%02d,%02d", recMs / 60000, (recMs / 1000) % 60, (recMs % 1000) / 10), color = Color.White, fontSize = 15.sp)
                            Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                                if (locked) Text("BEKOR QILISH", color = AruColors.Accent2, fontSize = 15.sp, fontWeight = FontWeight.Bold, modifier = Modifier.clickable { stopRec(false) }.padding(10.dp))
                                else Text("‹ Bekor qilish uchun suring", color = Color.White.copy(alpha = (1 + dragX / 300f).coerceIn(0f, 1f) * 0.6f), fontSize = 14.sp, modifier = Modifier.offset { IntOffset(dragX.roundToInt(), 0) })
                            }
                        }
                    } else {
                        Row(Modifier.weight(1f).heightIn(min = 48.dp).clip(RoundedCornerShape(24.dp)).background(TgPill), verticalAlignment = Alignment.Bottom) {
                            Box(Modifier.size(48.dp).clip(CircleShape).clickable {
                                if (panelOpen) { panelOpen = false; keyboard?.show() } else { panelOpen = true; focus.clearFocus(); keyboard?.hide() }
                            }, contentAlignment = Alignment.Center) { Icon(if (panelOpen) AruIcons.Keyboard else AruIcons.Emoticon, null, tint = Color.White.copy(alpha = 0.55f), modifier = Modifier.size(26.dp)) }
                            BasicTextField(
                                text, { text = it.take(2000) }, maxLines = 6, cursorBrush = SolidColor(AruColors.Accent),
                                textStyle = TextStyle(color = Color.White, fontSize = 15.sp, fontFamily = TgEmojiFont),
                                decorationBox = { inner -> Box { if (text.isEmpty()) Text("Xabar", color = Color.White.copy(alpha = 0.38f), fontSize = 15.sp); inner() } },
                                modifier = Modifier.weight(1f).padding(vertical = 14.dp),
                            )
                            var attach by remember { mutableStateOf(false) }
                            Box {
                                Box(Modifier.size(width = 42.dp, height = 48.dp).clickable(enabled = upProgress == null) { attach = true }, contentAlignment = Alignment.Center) {
                                    Icon(AruIcons.AttachFile, null, tint = Color.White.copy(alpha = if (upProgress == null) 0.55f else 0.25f), modifier = Modifier.size(23.dp))
                                }
                                DropdownMenu(attach, { attach = false }, containerColor = Color(0xFF2A2420)) {
                                    DropdownMenuItem(text = { Text("Rasm yoki video") }, onClick = { attach = false; pickMedia.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageAndVideo)) })
                                    DropdownMenuItem(text = { Text("Fayl") }, onClick = { attach = false; pickFile.launch(arrayOf("*/*")) })
                                }
                            }
                        }
                    }
                    Spacer(Modifier.width(8.dp))
                    RecordButton(
                        hasText = text.isNotBlank() && !recording, locked = locked, recording = recording,
                        onSend = { send() },
                        onStart = {
                            keyboard?.hide(); focus.clearFocus()
                            if (androidx.core.content.ContextCompat.checkSelfPermission(ctx, Manifest.permission.RECORD_AUDIO) != android.content.pm.PackageManager.PERMISSION_GRANTED) { micPerm.launch(Manifest.permission.RECORD_AUDIO); false }
                            else startRec()
                        },
                        onStop = { stopRec(it) },
                        onLock = { locked = true },
                        onDrag = { dragX = it },
                    )
                }
                if (panelOpen) TgMediaPanel(
                    onEmoji = { text += it },
                    onPick = { pick ->
                        if (pick.kind == uz.arumediatv.com.data.PackKind.EMOJI) text += uz.arumediatv.com.data.packEmojiToken(pick.pack, pick.item, pick.emoji)
                        else scope.launch { store.send(withReply(replyTo?.id, ""), mediaFile = pick.ref, mediaType = if (pick.kind == uz.arumediatv.com.data.PackKind.GIF) "gif" else "sticker")?.let(::toast); replyTo = null }
                    },
                    onBackspace = {
                        val m = Regex("\\[pe:\\d{1,16}:\\d{1,9}:[^\\]]{0,16}\\]$").find(text)
                        text = if (m != null) text.removeRange(m.range) else {
                            val bi = java.text.BreakIterator.getCharacterInstance(); bi.setText(text)
                            val last = bi.preceding(text.length)
                            if (text.isEmpty() || last < 0) "" else text.substring(0, last)
                        }
                    },
                    onManage = onPacks,
                    modifier = Modifier.fillMaxWidth().height(330.dp),
                )
            }
        }
    }
    viewer?.let { MediaViewer(it) { viewer = null } }
    confirm?.let { (msg, act) ->
        AlertDialog(
            onDismissRequest = { confirm = null },
            text = { Text(msg) },
            confirmButton = { TextButton(onClick = { confirm = null; act() }) { Text("O'chirish", color = Color(0xFFF87171)) } },
            dismissButton = { TextButton(onClick = { confirm = null }) { Text("Bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
}

@Composable
private fun RoundBtn(icon: androidx.compose.ui.graphics.vector.ImageVector, onClick: () -> Unit) {
    Box(Modifier.size(56.dp).clip(CircleShape).background(TgPill).clickable(onClick = onClick), contentAlignment = Alignment.Center) { Icon(icon, null, tint = Color.White, modifier = Modifier.size(26.dp)) }
}

@Composable
private fun Pill(modifier: Modifier, content: @Composable () -> Unit) {
    Box(modifier.height(56.dp).clip(RoundedCornerShape(28.dp)).background(TgPill), contentAlignment = Alignment.CenterStart) { content() }
}

@Composable
private fun DateChip(label: String) {
    Box(Modifier.fillMaxWidth().padding(vertical = 8.dp), contentAlignment = Alignment.Center) {
        Text(label, color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.Medium, modifier = Modifier.clip(RoundedCornerShape(14.dp)).background(Color.Black.copy(alpha = 0.35f)).padding(horizontal = 12.dp, vertical = 4.dp))
    }
}

/** Xabarni o'ngga surish — javob berish (Flutter `TgSwipeReply`). */
@Composable
private fun SwipeReply(enabled: Boolean, onReply: () -> Unit, content: @Composable () -> Unit) {
    var dx by remember { mutableFloatStateOf(0f) }
    val trigger = with(LocalDensity.current) { 64.dp.toPx() }
    Box(
        Modifier.pointerInput(enabled) {
            if (!enabled) return@pointerInput
            detectHorizontalDragGestures(
                onDragEnd = { if (dx >= trigger) onReply(); dx = 0f },
                onDragCancel = { dx = 0f },
            ) { _, d -> dx = (dx + d).coerceIn(0f, trigger * 1.4f) }
        },
    ) {
        if (dx > 20f) Icon(AruIcons.Reply, null, tint = Color.White.copy(alpha = (dx / trigger).coerceIn(0f, 1f)), modifier = Modifier.align(Alignment.CenterStart).padding(start = 12.dp).size(22.dp))
        Box(Modifier.offset { IntOffset(dx.roundToInt(), 0) }) { content() }
    }
}

/** Ovoz yozish tugmasi: matn bo'lsa — yuborish; bo'lmasa bosib turiladi (chapga surish — bekor, tepaga — qulf). */
@Composable
private fun RecordButton(
    hasText: Boolean, locked: Boolean, recording: Boolean,
    onSend: () -> Unit, onStart: () -> Boolean, onStop: (Boolean) -> Unit, onLock: () -> Unit, onDrag: (Float) -> Unit,
) {
    val d = LocalDensity.current
    val cancelAt = with(d) { 110.dp.toPx() }
    val lockAt = with(d) { 70.dp.toPx() }
    val ctx = LocalContext.current
    Box(
        Modifier.size(48.dp).clip(CircleShape).background(AruColors.Accent)
            .pointerInput(hasText, locked) {
                if (hasText || locked) {
                    awaitEachGesture { awaitFirstDown(); waitUp(); if (hasText) onSend() else onStop(true) }
                    return@pointerInput
                }
                awaitEachGesture {
                    awaitFirstDown()
                    if (!onStart()) { waitUp(); return@awaitEachGesture }
                    var x = 0f; var y = 0f; var cancelled = false; var lockedNow = false
                    while (true) {
                        val ev = awaitPointerEvent(PointerEventPass.Main)
                        val ch = ev.changes.firstOrNull() ?: break
                        if (!ch.pressed) break
                        val delta = ch.positionChange()
                        x += delta.x; y += delta.y
                        onDrag(x.coerceAtMost(0f))
                        if (!lockedNow && y < -lockAt) { lockedNow = true; onLock(); break }
                        if (x < -cancelAt) { cancelled = true; break }
                    }
                    if (lockedNow) return@awaitEachGesture
                    onDrag(0f)
                    onStop(!cancelled)
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        if (hasText || locked) Icon(Icons.AutoMirrored.Filled.Send, "Yuborish", tint = Color.White, modifier = Modifier.size(22.dp))
        else Icon(AruIcons.Mic, "Ovoz", tint = Color.White, modifier = Modifier.size(24.dp))
    }
}

private suspend fun androidx.compose.ui.input.pointer.AwaitPointerEventScope.waitUp() {
    while (true) {
        val e = awaitPointerEvent()
        if (e.changes.none { it.pressed }) return
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun Bubble(
    m: ChatMessage, mine: Boolean, topNear: Boolean, bottomNear: Boolean, selected: Boolean,
    quote: Pair<String, String>?, onQuote: () -> Unit, onTap: () -> Unit, onLong: () -> Unit,
) {
    val big = 18.dp; val small = 5.dp
    val shape = RoundedCornerShape(
        topStart = if (!mine && topNear) small else big, topEnd = if (mine && topNear) small else big,
        bottomStart = if (mine) big else if (bottomNear) small else 4.dp, bottomEnd = if (!mine) big else if (bottomNear) small else 4.dp,
    )
    val bare = (m.mediaType == "sticker" || m.mediaType == "gif") && splitReply(m.body).second.isEmpty()
    val body = splitReply(m.body).second
    Row(Modifier.fillMaxWidth().background(if (selected) Color(0x33FFFFFF) else Color.Transparent).padding(top = if (topNear) 2.dp else 6.dp), horizontalArrangement = if (mine) Arrangement.End else Arrangement.Start) {
        Column(
            Modifier.widthIn(max = 300.dp)
                .then(if (bare) Modifier else Modifier.clip(shape).background(if (mine) TgOut else TgIn))
                .combinedClickable(onClick = onTap, onLongClick = onLong).padding(if (bare) 0.dp else 8.dp),
        ) {
            if (quote != null) Row(Modifier.padding(bottom = 4.dp).clip(RoundedCornerShape(6.dp)).background(Color.White.copy(alpha = 0.08f)).clickable(onClick = onQuote).padding(end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.width(3.dp).height(36.dp).background(if (mine) Color.White else AruColors.Accent2))
                Column(Modifier.padding(start = 8.dp, top = 3.dp, bottom = 3.dp)) {
                    Text(quote.first, color = if (mine) Color.White else AruColors.Accent2, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1)
                    Text(quote.second, color = Color.White, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
            if (m.hasMedia) MediaBlock(m)
            if (m.mediaType != "file" && body.isNotEmpty()) uz.arumediatv.com.ui.packs.RichPackText(body, Color.White, 15.sp, lineHeight = 21.sp)
            Row(Modifier.align(Alignment.End).padding(top = 2.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(hm.get()!!.format(Date(m.createdAt)), color = if (mine) TgOutMeta else Color.White.copy(alpha = 0.55f), fontSize = 11.sp)
                if (mine) Text(if (m.pending) "  🕓" else if (m.seen) "  ✓✓" else "  ✓", color = TgOutMeta, fontSize = 11.sp)
            }
        }
    }
}

@Composable
private fun UploadingBubble(label: String, progress: Float) {
    Row(Modifier.fillMaxWidth().padding(top = 6.dp), horizontalArrangement = Arrangement.End) {
        Column(Modifier.widthIn(max = 260.dp).clip(RoundedCornerShape(18.dp)).background(TgOut).padding(12.dp)) {
            Text(if (label.isEmpty()) "Yuborilmoqda..." else label, color = Color.White, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Spacer(Modifier.height(8.dp))
            LinearProgressIndicator(progress = { progress }, color = Color.White, trackColor = Color.White.copy(alpha = 0.25f), modifier = Modifier.width(200.dp).height(3.dp))
        }
    }
}

@Composable
private fun MediaBlock(m: ChatMessage) {
    val ctx = LocalContext.current
    when (m.mediaType) {
        "image" -> TgImage(m.mediaUrl, Modifier.size(width = 230.dp, height = 230.dp).clip(RoundedCornerShape(12.dp)).background(Color.Black.copy(alpha = 0.25f)), maxPx = 640)
        "video" -> Box(Modifier.size(width = 230.dp, height = 150.dp).clip(RoundedCornerShape(12.dp)).background(Color.Black), contentAlignment = Alignment.Center) {
            Box(Modifier.size(48.dp).clip(CircleShape).background(Color.Black.copy(alpha = 0.5f)), contentAlignment = Alignment.Center) { Icon(Icons.Filled.PlayArrow, "Video", tint = Color.White, modifier = Modifier.size(32.dp)) }
            if (m.mediaMs > 0) Text(formatClock(m.mediaMs), color = Color.White, fontSize = 11.sp, modifier = Modifier.align(Alignment.BottomEnd).padding(8.dp))
        }
        "round" -> Box(Modifier.size(200.dp).clip(CircleShape).background(Color.Black), contentAlignment = Alignment.Center) {
            Icon(Icons.Filled.PlayArrow, "Video", tint = Color.White, modifier = Modifier.size(44.dp))
        }
        "voice" -> VoiceRow(m)
        "file" -> {
            val scope = rememberCoroutineScope()
            Row(Modifier.clickable {
                scope.launch {
                    val u = AppServices.telegram.playUrl(m.mediaUrl)
                    if (u == null) Toast.makeText(ctx, "Fayl ochilmadi", Toast.LENGTH_SHORT).show()
                    else try {
                        val ext = m.body.substringAfterLast('.', "").lowercase()
                        ctx.startActivity(Intent.createChooser(Intent(Intent.ACTION_VIEW).setDataAndType(Uri.parse(u), MIMES[ext] ?: "*/*"), "Faylni ochish").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                    } catch (e: Exception) { Toast.makeText(ctx, "Fayl ochilmadi", Toast.LENGTH_SHORT).show() }
                }
            }.padding(vertical = 2.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(46.dp).clip(CircleShape).background(Color.White.copy(alpha = 0.2f)), contentAlignment = Alignment.Center) { Icon(AruIcons.AttachFile, null, tint = Color.White, modifier = Modifier.size(24.dp)) }
                Spacer(Modifier.width(10.dp))
                Text(m.body.ifEmpty { "Fayl" }, color = Color.White, fontSize = 14.sp, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.widthIn(max = 200.dp))
            }
        }
        else -> {
            val ref = uz.arumediatv.com.data.Names.fileNameOf(m.mediaUrl)
            if (uz.arumediatv.com.data.parsePackRef(ref) != null) uz.arumediatv.com.ui.packs.PackRefImage(ref, Modifier.size(if (m.mediaType == "gif") 190.dp else 140.dp))
            else TgImage(m.mediaUrl, Modifier.size(width = 150.dp, height = 150.dp), maxPx = 400, contentScale = ContentScale.Fit)
        }
    }
}

@Composable
private fun VoiceRow(m: ChatMessage) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val playing by VoicePlayer.playingId.collectAsStateWithLifecycle()
    val on = playing == m.id
    Row(Modifier.padding(bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(
            Modifier.size(42.dp).clip(CircleShape).background(Color.White.copy(alpha = 0.2f)).clickable {
                if (on) VoicePlayer.stop() else scope.launch {
                    val u = AppServices.telegram.playUrl(m.mediaUrl)
                    if (u == null) Toast.makeText(ctx, "Ovozni ochib bo'lmadi", Toast.LENGTH_SHORT).show() else VoicePlayer.play(m.id, u)
                }
            },
            contentAlignment = Alignment.Center,
        ) { Icon(if (on) AruIcons.Pause else Icons.Filled.PlayArrow, null, tint = Color.White, modifier = Modifier.size(24.dp)) }
        Spacer(Modifier.width(10.dp))
        // To'lqin chiziqlari (xabar id'sidan barqaror)
        Row(horizontalArrangement = Arrangement.spacedBy(2.dp), verticalAlignment = Alignment.CenterVertically) {
            val seed = m.id.hashCode()
            repeat(22) { i -> val h = 6 + abs((seed * (i + 3) * 2654435761L).toInt() % 18); Box(Modifier.width(3.dp).height(h.dp).clip(RoundedCornerShape(2.dp)).background(Color.White.copy(alpha = if (on) 0.95f else 0.55f))) }
        }
        Spacer(Modifier.width(8.dp))
        Text(formatClock(m.mediaMs), color = Color.White.copy(alpha = 0.8f), fontSize = 12.sp)
    }
}

/** Ovozli xabarlarni ijro etuvchi (bitta vaqtda bittasi). */
object VoicePlayer {
    private var mp: MediaPlayer? = null
    private val _playingId = kotlinx.coroutines.flow.MutableStateFlow<String?>(null)
    val playingId: kotlinx.coroutines.flow.StateFlow<String?> = _playingId

    fun play(id: String, url: String) {
        stop()
        try {
            val p = MediaPlayer()
            p.setDataSource(url)
            p.setOnPreparedListener { it.start() }
            p.setOnCompletionListener { stop() }
            p.setOnErrorListener { _, _, _ -> stop(); true }
            p.prepareAsync()
            mp = p
            _playingId.value = id
        } catch (_: Exception) {
            stop()
        }
    }

    fun stop() {
        runCatching { mp?.stop() }
        runCatching { mp?.release() }
        mp = null
        _playingId.value = null
    }
}

/** Rasm yoki videoni to'liq ekranda ko'rish. */
@Composable
private fun MediaViewer(m: ChatMessage, onClose: () -> Unit) {
    val ctx = LocalContext.current
    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Box(Modifier.fillMaxSize().background(Color.Black).clickable(onClick = onClose), contentAlignment = Alignment.Center) {
            if (m.mediaType == "image" || m.mediaType == "sticker" || m.mediaType == "gif") {
                TgImage(m.mediaUrl, Modifier.fillMaxSize(), maxPx = 1600, contentScale = androidx.compose.ui.layout.ContentScale.Fit)
            } else {
                val player = remember { AruPlayer(ctx) }
                DisposableEffect(Unit) { onDispose { player.release() } }
                LaunchedEffect(m.id) {
                    val src = uz.arumediatv.com.data.PlaySource.resolve(m.mediaUrl, AppServices.telegram)
                    if (src != null) player.open(src.uri, 0, true)
                }
                PlayerSurface(player, Modifier.fillMaxSize())
            }
            Icon(Icons.Filled.Close, "Yopish", tint = Color.White, modifier = Modifier.align(Alignment.TopEnd).padding(20.dp).size(30.dp).clickable(onClick = onClose))
        }
    }
}

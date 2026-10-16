package uz.arumediatv.com.ui.support

import android.Manifest
import android.content.Context
import android.media.MediaPlayer
import android.media.MediaRecorder
import android.net.Uri
import android.os.Build
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
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
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
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
import uz.arumediatv.com.data.ChatMessage
import uz.arumediatv.com.data.ChatStore
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.profile.OverlayScaffold
import uz.arumediatv.com.ui.season.AruPlayer
import uz.arumediatv.com.ui.season.PlayerSurface
import uz.arumediatv.com.ui.season.formatClock
import uz.arumediatv.com.ui.theme.AruColors
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

private val hm = ThreadLocal.withInitial { SimpleDateFormat("HH:mm", Locale.US).apply { timeZone = TimeZone.getTimeZone("GMT+5") } }
private val dmy = ThreadLocal.withInitial { SimpleDateFormat("d MMM", Locale("uz")).apply { timeZone = TimeZone.getTimeZone("GMT+5") } }

/**
 * Admin bilan yozishma. `userId == null` — o'z suhbatim (foydalanuvchi); admin boshqa odamning suhbatini ochsa `userId` beriladi.
 * Matn, rasm, video, ovozli xabar; uzoq bosib tanlash va o'chirish.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun SupportChatScreen(userId: Long?, title: String, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val store = remember(userId) { ChatStore(ctx.applicationContext, AppServices.auth, userId) }
    val items by store.items.collectAsStateWithLifecycle()
    val error by store.error.collectAsStateWithLifecycle()
    val loading by store.loading.collectAsStateWithLifecycle()
    val me by AppServices.auth.user.collectAsStateWithLifecycle()
    var text by remember { mutableStateOf("") }
    var upProgress by remember { mutableStateOf<Float?>(null) }
    var viewer by remember { mutableStateOf<ChatMessage?>(null) }
    val selected = remember { mutableStateListOf<String>() }
    val listState = rememberLazyListState()
    val adminSide = userId != null
    fun toast(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_SHORT).show()

    LaunchedEffect(store) {
        store.loadFromDisk()
        store.load()
    }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    DisposableEffect(store, lifecycle) {
        if (lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) store.startWatching()
        val ob = LifecycleEventObserver { _, e ->
            if (e == Lifecycle.Event.ON_RESUME) store.startWatching() else if (e == Lifecycle.Event.ON_PAUSE) store.stopWatching()
        }
        lifecycle.addObserver(ob)
        onDispose { lifecycle.removeObserver(ob); store.close(); VoicePlayer.stop() }
    }
    LaunchedEffect(items.size) { if (items.isNotEmpty()) listState.scrollToItem(items.lastIndex) }

    // ── Fayl yuborish ──
    fun sendFile(uri: Uri, type: String, ext: String, mime: String) {
        scope.launch {
            val uid = me?.id ?: 0L
            val name = "chat_${uid}_${System.currentTimeMillis()}.$ext"
            upProgress = 0f
            val local = withContext(Dispatchers.IO) {
                try {
                    val f = File(ctx.cacheDir, name)
                    ctx.contentResolver.openInputStream(uri)?.use { i -> f.outputStream().use { o -> i.copyTo(o) } }
                    f
                } catch (_: Exception) { null }
            }
            if (local == null) { upProgress = null; toast("Faylni o'qib bo'lmadi"); return@launch }
            val err = AppServices.telegram.uploadFile(local.absolutePath, name, mime, onProgress = { sent, total -> if (total > 0) upProgress = sent.toFloat() / total })
            if (err != null) toast(err) else store.send("", mediaFile = name, mediaType = type)?.let(::toast)
            runCatching { local.delete() }
            upProgress = null
        }
    }
    val pickImage = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { u -> if (u != null) sendFile(u, "image", "jpg", "image/jpeg") }
    val pickVideo = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { u -> if (u != null) sendFile(u, "video", "mp4", "video/mp4") }

    // ── Ovoz yozish ──
    var recording by remember { mutableStateOf(false) }
    var recStart by remember { mutableLongStateOf(0L) }
    var recorder by remember { mutableStateOf<MediaRecorder?>(null) }
    var recFile by remember { mutableStateOf<File?>(null) }
    var recSecs by remember { mutableLongStateOf(0L) }
    fun startRec() {
        try {
            val f = File(ctx.cacheDir, "voice_${System.currentTimeMillis()}.m4a")
            val r = if (Build.VERSION.SDK_INT >= 31) MediaRecorder(ctx) else @Suppress("DEPRECATION") MediaRecorder()
            r.setAudioSource(MediaRecorder.AudioSource.MIC)
            r.setOutputFormat(MediaRecorder.OutputFormat.MPEG_4)
            r.setAudioEncoder(MediaRecorder.AudioEncoder.AAC)
            r.setAudioEncodingBitRate(64_000)
            r.setAudioSamplingRate(44_100)
            r.setOutputFile(f.absolutePath)
            r.prepare(); r.start()
            VoicePlayer.stop()
            recorder = r; recFile = f; recStart = System.currentTimeMillis(); recording = true
        } catch (e: Exception) {
            toast("Yozib bo'lmadi"); recording = false
        }
    }
    fun stopRec(send: Boolean) {
        val r = recorder; val f = recFile
        recording = false; recorder = null
        val ms = System.currentTimeMillis() - recStart
        try { r?.stop() } catch (_: Exception) {}
        runCatching { r?.release() }
        if (!send || f == null || ms < 700) { f?.delete(); return }
        scope.launch {
            val uid = me?.id ?: 0L
            val name = "chat_${uid}_${System.currentTimeMillis()}.m4a"
            upProgress = 0f
            val err = AppServices.telegram.uploadFile(f.absolutePath, name, "audio/mp4", onProgress = { s, t -> if (t > 0) upProgress = s.toFloat() / t })
            if (err != null) toast(err) else store.send("", mediaFile = name, mediaType = "voice", mediaMs = ms)?.let(::toast)
            runCatching { f.delete() }
            upProgress = null
        }
    }
    val micPerm = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok -> if (ok) startRec() else toast("Mikrofonga ruxsat berilmadi") }
    LaunchedEffect(recording) { while (recording) { recSecs = (System.currentTimeMillis() - recStart) / 1000; delay(250) } }
    DisposableEffect(Unit) { onDispose { runCatching { recorder?.stop() }; runCatching { recorder?.release() } } }

    OverlayScaffold(if (selected.isNotEmpty()) "${selected.size} ta tanlandi" else title, if (selected.isNotEmpty()) ({ selected.clear() }) else onBack) {
        Column(Modifier.fillMaxSize().imePadding()) {
            Box(Modifier.weight(1f).fillMaxWidth()) {
                when {
                    items.isEmpty() && loading -> LoadingBox()
                    items.isEmpty() -> EmptyState(text = error ?: if (adminSide) "Suhbat bo'sh" else "Admin bilan yozishma shu yerda.\nSavolingizni yozing")
                    else -> LazyColumn(Modifier.fillMaxSize(), state = listState, contentPadding = androidx.compose.foundation.layout.PaddingValues(12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        items(items, key = { it.id }) { m ->
                            val mine = m.fromAdmin == adminSide
                            Bubble(
                                m, mine, selected = m.id in selected,
                                onTap = { if (selected.isNotEmpty()) { if (m.id in selected) selected.remove(m.id) else selected.add(m.id) } else if (m.hasMedia && m.mediaType != "voice") viewer = m },
                                onLong = { if (m.id in selected) selected.remove(m.id) else selected.add(m.id) },
                            )
                        }
                    }
                }
                if (selected.isNotEmpty()) {
                    Row(Modifier.align(Alignment.BottomCenter).padding(12.dp).clip(RoundedCornerShape(16.dp)).background(AruColors.Surface).border(1.dp, AruColors.BorderBright, RoundedCornerShape(16.dp)).padding(horizontal = 18.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Filled.Delete, null, tint = Color(0xFFF87171), modifier = Modifier.size(20.dp))
                        Spacer(Modifier.width(8.dp))
                        Text("O'chirish", color = Color(0xFFF87171), fontWeight = FontWeight.SemiBold, modifier = Modifier.clickable {
                            val ids = selected.toList(); selected.clear()
                            scope.launch { store.remove(ids)?.let(::toast) }
                        })
                    }
                }
            }
            upProgress?.let { LinearProgressIndicator(progress = { it }, color = AruColors.Accent2, trackColor = AruColors.CardAlt, modifier = Modifier.fillMaxWidth().height(3.dp)) }
            // ── Yozish maydoni ──
            Row(Modifier.fillMaxWidth().background(AruColors.Surface).padding(horizontal = 10.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                var menu by remember { mutableStateOf(false) }
                Box {
                    Box(Modifier.size(42.dp).clip(CircleShape).background(AruColors.CardAlt).clickable { menu = true }, contentAlignment = Alignment.Center) {
                        Icon(Icons.Filled.Add, "Biriktirish", tint = Color.White, modifier = Modifier.size(22.dp))
                    }
                    DropdownMenu(menu, { menu = false }, containerColor = AruColors.Surface) {
                        DropdownMenuItem(text = { Text("Rasm") }, onClick = { menu = false; pickImage.launch("image/*") })
                        DropdownMenuItem(text = { Text("Video") }, onClick = { menu = false; pickVideo.launch("video/*") })
                    }
                }
                Spacer(Modifier.width(8.dp))
                if (recording) {
                    Text("● ${formatClock(recSecs * 1000)}  Yozilmoqda...", color = Color(0xFFF87171), fontSize = 14.sp, modifier = Modifier.weight(1f))
                    Text("Bekor", color = AruColors.TextDim, modifier = Modifier.clickable { stopRec(false) }.padding(10.dp))
                } else {
                    OutlinedTextField(
                        value = text, onValueChange = { text = it.take(2000) }, placeholder = { Text("Xabar yozing...", color = AruColors.TextFaint) },
                        maxLines = 4, shape = RoundedCornerShape(20.dp),
                        colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White, focusedBorderColor = AruColors.Accent, unfocusedBorderColor = AruColors.BorderBright),
                        modifier = Modifier.weight(1f),
                    )
                }
                Spacer(Modifier.width(8.dp))
                val hasText = text.isNotBlank()
                Box(
                    Modifier.size(46.dp).clip(CircleShape).background(AruColors.Accent).clickable {
                        when {
                            recording -> stopRec(true)
                            hasText -> { val t = text; text = ""; scope.launch { store.send(t)?.let { e -> toast(e); text = t } } }
                            else -> if (androidx.core.content.ContextCompat.checkSelfPermission(ctx, Manifest.permission.RECORD_AUDIO) == android.content.pm.PackageManager.PERMISSION_GRANTED) startRec() else micPerm.launch(Manifest.permission.RECORD_AUDIO)
                        }
                    },
                    contentAlignment = Alignment.Center,
                ) {
                    if (hasText || recording) Icon(Icons.AutoMirrored.Filled.Send, "Yuborish", tint = Color.White, modifier = Modifier.size(22.dp))
                    else Text("🎤", fontSize = 20.sp)
                }
            }
        }
    }
    viewer?.let { MediaViewer(it) { viewer = null } }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun Bubble(m: ChatMessage, mine: Boolean, selected: Boolean, onTap: () -> Unit, onLong: () -> Unit) {
    val shape = RoundedCornerShape(topStart = 18.dp, topEnd = 18.dp, bottomStart = if (mine) 18.dp else 4.dp, bottomEnd = if (mine) 4.dp else 18.dp)
    Row(Modifier.fillMaxWidth().background(if (selected) Color(0x33C2410C) else Color.Transparent).padding(vertical = 1.dp), horizontalArrangement = if (mine) Arrangement.End else Arrangement.Start) {
        Column(
            Modifier.widthIn(max = 290.dp).clip(shape).background(if (mine) AruColors.Accent else AruColors.Card)
                .combinedClickable(onClick = onTap, onLongClick = onLong).padding(horizontal = 12.dp, vertical = 8.dp),
        ) {
            if (m.hasMedia) MediaBlock(m)
            if (m.body.isNotEmpty()) Text(m.body, color = Color.White, fontSize = 15.sp, lineHeight = 21.sp)
            Row(Modifier.align(Alignment.End).padding(top = 2.dp)) {
                Text(
                    hm.get()!!.format(Date(m.createdAt)) + if (mine && !m.pending) (if (m.seen) "  ✓✓" else "  ✓") else if (m.pending) "  …" else "",
                    color = Color.White.copy(alpha = 0.6f), fontSize = 10.sp,
                )
            }
        }
    }
}

@Composable
private fun MediaBlock(m: ChatMessage) {
    when (m.mediaType) {
        "image" -> TgImage(m.mediaUrl, Modifier.size(width = 220.dp, height = 220.dp).clip(RoundedCornerShape(12.dp)).background(Color.Black.copy(alpha = 0.25f)), maxPx = 640)
        "video", "round" -> Box(Modifier.size(width = 220.dp, height = 140.dp).clip(RoundedCornerShape(12.dp)).background(Color.Black), contentAlignment = Alignment.Center) {
            Icon(Icons.Filled.PlayArrow, "Video", tint = Color.White, modifier = Modifier.size(44.dp))
            if (m.mediaMs > 0) Text(formatClock(m.mediaMs), color = Color.White, fontSize = 11.sp, modifier = Modifier.align(Alignment.BottomEnd).padding(8.dp))
        }
        "voice" -> VoiceRow(m)
        else -> TgImage(m.mediaUrl, Modifier.size(width = 150.dp, height = 150.dp), maxPx = 400, contentScale = androidx.compose.ui.layout.ContentScale.Fit)
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
            Modifier.size(38.dp).clip(CircleShape).background(Color.White.copy(alpha = 0.2f)).clickable {
                if (on) VoicePlayer.stop() else scope.launch {
                    val u = AppServices.telegram.playUrl(m.mediaUrl)
                    if (u == null) Toast.makeText(ctx, "Ovozni ochib bo'lmadi", Toast.LENGTH_SHORT).show() else VoicePlayer.play(m.id, u)
                }
            },
            contentAlignment = Alignment.Center,
        ) { Text(if (on) "❚❚" else "▶", color = Color.White, fontSize = 14.sp) }
        Spacer(Modifier.width(10.dp))
        Text("Ovozli xabar · ${formatClock(m.mediaMs)}", color = Color.White, fontSize = 13.sp)
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

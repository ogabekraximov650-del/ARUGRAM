package uz.arugram.app.ui.main

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.media.MediaMetadataRetriever
import android.media.MediaRecorder
import android.os.Build
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import kotlinx.coroutines.delay
import uz.arugram.app.R
import uz.arugram.app.ui.components.CloseIcon
import uz.arugram.app.ui.components.SendIcon
import uz.arugram.app.ui.components.TgIcon
import uz.arugram.app.ui.theme.LocalTg
import java.io.File

private val Blue = Color(0xFF229AF0)  // chat_messagePanelSend = TELEGRAM_COLOR

/** Ovoz yozgich (Telegram ovozli xabari — OGG/Opus; Android 10+). */
class VoiceRecorder(private val ctx: Context) {
    private var rec: MediaRecorder? = null
    private var file: File? = null
    private var startedAt = 0L
    private val amps = ArrayList<Int>()

    val supported get() = Build.VERSION.SDK_INT >= 29

    fun start(): Boolean {
        if (!supported) return false
        return runCatching {
            val f = File(ctx.cacheDir, "send").apply { mkdirs() }.let { File(it, "voice_${System.currentTimeMillis()}.ogg") }
            @Suppress("DEPRECATION")
            val r = if (Build.VERSION.SDK_INT >= 31) MediaRecorder(ctx) else MediaRecorder()
            r.setAudioSource(MediaRecorder.AudioSource.MIC)
            r.setOutputFormat(MediaRecorder.OutputFormat.OGG)
            r.setAudioEncoder(MediaRecorder.AudioEncoder.OPUS)
            r.setAudioSamplingRate(48000)
            r.setAudioEncodingBitRate(32000)
            r.setOutputFile(f.absolutePath)
            r.prepare()
            r.start()
            rec = r
            file = f
            startedAt = System.currentTimeMillis()
            amps.clear()
            true
        }.getOrDefault(false)
    }

    /** Yozish paytida chaqiriladi (har ~100 ms): ovoz balandligini to'lqin shakli uchun eslab qoladi. */
    fun sample() {
        val a = runCatching { rec?.maxAmplitude ?: 0 }.getOrDefault(0)
        amps.add(a)
    }

    /** 100 ta 5 bitli qiymat (little-endian) hex ko'rinishida — Telegram `DocumentAttributeAudio.waveform`. */
    fun waveformHex(): String {
        if (amps.isEmpty()) return ""
        val peak = (amps.maxOrNull() ?: 1).coerceAtLeast(1)
        val vals = IntArray(100) { i ->
            val a = i * amps.size / 100
            val b = ((i + 1) * amps.size / 100).coerceAtLeast(a + 1).coerceAtMost(amps.size)
            var mx = 0
            for (k in a until b) mx = maxOf(mx, amps[k])
            (mx * 31 / peak).coerceIn(0, 31)
        }
        val out = ByteArray((vals.size * 5 + 7) / 8 + 1)
        for ((i, x) in vals.withIndex()) {
            val bit = i * 5
            val byte = bit / 8
            val shift = bit % 8
            val w = (x and 31) shl shift
            out[byte] = (out[byte].toInt() or (w and 0xFF)).toByte()
            if (w shr 8 != 0) out[byte + 1] = (out[byte + 1].toInt() or (w shr 8)).toByte()
        }
        return out.copyOf((vals.size * 5 + 7) / 8).joinToString("") { "%02x".format(it) }
    }

    fun elapsed(): Double = if (rec == null) 0.0 else (System.currentTimeMillis() - startedAt) / 1000.0

    /** To'xtatadi; (fayl, soniya) yoki juda qisqa bo'lsa null. */
    fun stop(): Pair<File, Double>? {
        val r = rec ?: return null
        val secs = elapsed()
        val f = file
        rec = null
        file = null
        runCatching { r.stop() }
        runCatching { r.release() }
        if (f == null || secs < 0.7) {
            f?.delete()
            return null
        }
        return f to secs
    }

    fun cancel() {
        val r = rec
        val f = file
        rec = null
        file = null
        runCatching { r?.stop() }
        runCatching { r?.release() }
        f?.delete()
    }
}

private val Emojis = ("😀 😃 😄 😁 😆 😅 😂 🤣 😊 😇 🙂 🙃 😉 😌 😍 🥰 😘 😗 😙 😚 😋 😛 😝 😜 🤪 🤨 🧐 🤓 😎 🤩 🥳 😏 😒 😞 😔 😟 😕 🙁 ☹ 😣 😖 😫 😩 🥺 😢 😭 😤 😠 😡 🤬 🤯 😳 🥵 🥶 😱 😨 😰 😥 😓 🤗 🤔 🤭 🤫 🤥 😶 😐 😑 😬 🙄 😯 😦 😧 😮 😲 🥱 😴 🤤 😪 😵 🤐 🥴 🤢 🤮 🤧 😷 🤒 🤕 🤑 🤠 😈 👿 👹 👺 🤡 💩 👻 💀 ☠ 👽 👾 🤖 🎃 " +
    "👋 🤚 🖐 ✋ 🖖 👌 🤌 🤏 ✌ 🤞 🤟 🤘 🤙 👈 👉 👆 👇 ☝ 👍 👎 ✊ 👊 🤛 🤜 👏 🙌 👐 🤲 🤝 🙏 ✍ 💅 💪 🦾 👀 👅 👄 " +
    "❤ 🧡 💛 💚 💙 💜 🖤 🤍 🤎 💔 ❣ 💕 💞 💓 💗 💖 💘 💝 💟 💯 💢 💥 💫 💦 💨 🔥 ✨ ⭐ 🌟 ⚡ 🌈 ☀ ☁ ❄ " +
    "🐶 🐱 🐭 🐹 🐰 🦊 🐻 🐼 🐨 🐯 🦁 🐮 🐷 🐸 🐵 🐔 🐧 🐦 🦆 🦅 🦉 🐺 🐴 🦄 🐝 🦋 🐌 🐞 🐢 🐍 🐙 🐬 🐳 🦈 " +
    "🍎 🍊 🍋 🍌 🍉 🍇 🍓 🍒 🍑 🍍 🥝 🍅 🥑 🍔 🍟 🍕 🌭 🥪 🌮 🍣 🍜 🍩 🍪 🎂 🍰 🍫 🍬 🍭 ☕ 🍺 🍷 🥂 " +
    "⚽ 🏀 🏈 ⚾ 🎾 🏐 🎱 🏓 🥊 🎮 🎯 🎲 🎸 🎹 🎤 🎧 🎬 🎉 🎊 🎁 🏆 🥇 " +
    "🚗 🚕 🚌 🚀 ✈ 🚁 🚢 🏠 🏝 🌍 🗽 📱 💻 ⌚ 📷 💡 📚 ✏ 📌 🔒 🔑 💰 💎 ✅ ❌ ❓ ❗ ⚠ ➕ ➖ ✔").split(" ")

/**
 * Xabar yozish paneli (DrKLO `ChatActivityEnterView`): chapda emoji, o'rtada matn maydoni (6 qatorgacha), o'ngda — skrepka va mikrofon
 * (matn bo'lsa — ko'k "yuborish"). Tepada javob/tahrir paneli. Ovoz yozilayotganda — qizil nuqta, vaqt, "Bekor qilish" va yuborish.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ChatInputBar(
    broadcast: Boolean = false,
    draft: TextFieldValue,
    onDraft: (TextFieldValue) -> Unit,
    replyTo: MsgRow?,
    editing: MsgRow?,
    canWrite: Boolean,
    onCancelContext: () -> Unit,
    onSend: () -> Unit,
    onSendFile: (File, String, String, String) -> Unit,
    onSendVoice: (File, Double, String) -> Unit,
    onSendSticker: (Sticker) -> Unit,
    onFormat: () -> Unit = {},
    hasCommands: Boolean = false,
    onCommands: () -> Unit = {},
    onSendEx: (silent: Boolean, schedule: Long) -> Unit = { _, _ -> },
    onSendSpecial: (kind: String, payload: String) -> Unit = { _, _ -> },
) {
    val c = LocalTg.current
    val ctx = LocalContext.current
    val kb = LocalSoftwareKeyboardController.current
    var attachMenu by remember { mutableStateOf(false) }
    var emojiPanel by remember { mutableStateOf(false) }
    var panelTab by remember { mutableStateOf(0) }
    var recording by remember { mutableStateOf(false) }
    var recSeconds by remember { mutableStateOf(0.0) }
    val recorder = remember { VoiceRecorder(ctx) }
    var sendMenu by remember { mutableStateOf(false) }
    var pollForm by remember { mutableStateOf(false) }
    var pendingFile by remember { mutableStateOf<Triple<File, String, String>?>(null) }
    var toast by remember { mutableStateOf<String?>(null) }

    // Joylashuv: oxirgi ma'lum joy (GPS yoki tarmoq) yuboriladi.
    fun sendLocation() {
        val lm = ctx.getSystemService(Context.LOCATION_SERVICE) as? android.location.LocationManager
        val loc = runCatching {
            listOf(android.location.LocationManager.GPS_PROVIDER, android.location.LocationManager.NETWORK_PROVIDER, android.location.LocationManager.PASSIVE_PROVIDER)
                .mapNotNull { runCatching { lm?.getLastKnownLocation(it) }.getOrNull() }.maxByOrNull { it.time }
        }.getOrNull()
        if (loc == null) toast = "Joylashuv aniqlanmadi — qurilmada geolokatsiyani yoqing" else onSendSpecial("geo", "${loc.latitude},${loc.longitude}")
    }
    val locationPermission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok -> if (ok) sendLocation() else toast = "Joylashuvga ruxsat berilmadi" }
    // Kontakt: telefon raqamlar ro'yxatidan tanlanadi (qo'shimcha ruxsat kerak emas).
    val contactPicker = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) { res ->
        val uri = res.data?.data ?: return@rememberLauncherForActivityResult
        runCatching {
            ctx.contentResolver.query(uri, arrayOf(android.provider.ContactsContract.CommonDataKinds.Phone.NUMBER, android.provider.ContactsContract.CommonDataKinds.Phone.DISPLAY_NAME), null, null, null)?.use { cur ->
                if (cur.moveToFirst()) {
                    val number = cur.getString(0).orEmpty().replace("|", "")
                    val name = cur.getString(1).orEmpty().replace("|", "")
                    onSendSpecial("contact", "$number|$name|")
                }
            }
        }
    }
    // Rejalashtirish: avval sana, so'ng vaqt.
    fun pickSchedule() {
        val now = java.util.Calendar.getInstance()
        android.app.DatePickerDialog(ctx, { _, y, m, d ->
            android.app.TimePickerDialog(ctx, { _, hh, mm ->
                val t = java.util.Calendar.getInstance().apply { set(y, m, d, hh, mm, 0); set(java.util.Calendar.MILLISECOND, 0) }
                if (t.timeInMillis <= System.currentTimeMillis() + 10_000) toast = "Kelajakdagi vaqtni tanlang" else onSendEx(false, t.timeInMillis / 1000)
            }, now.get(java.util.Calendar.HOUR_OF_DAY), now.get(java.util.Calendar.MINUTE), true).show()
        }, now.get(java.util.Calendar.YEAR), now.get(java.util.Calendar.MONTH), now.get(java.util.Calendar.DAY_OF_MONTH)).show()
    }

    fun pick(uri: Uri?, kind: String) {
        if (uri == null) return
        val f = copyUriToCache(ctx, uri) ?: return
        var extra = ""
        if (kind == "video") {
            runCatching {
                val r = MediaMetadataRetriever()
                r.setDataSource(f.path)
                val dur = (r.extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION)?.toLongOrNull() ?: 0L) / 1000.0
                val w = r.extractMetadata(MediaMetadataRetriever.METADATA_KEY_VIDEO_WIDTH) ?: "0"
                val h = r.extractMetadata(MediaMetadataRetriever.METADATA_KEY_VIDEO_HEIGHT) ?: "0"
                r.release()
                extra = "$dur:$w:$h"
            }
        }
        // Rasm/video/fayl: avval izoh so'raladi (Telegram'dagi "izoh qo'shish" qatori).
        pendingFile = Triple(f, kind, extra)
    }
    val galleryPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) pick(uri, if (ctx.contentResolver.getType(uri).orEmpty().startsWith("video/")) "video" else "photo")
    }
    val filePicker = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { pick(it, "file") }
    val permission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok ->
        if (ok && recorder.start()) recording = true
    }

    LaunchedEffect(recording) { while (recording) { recSeconds = recorder.elapsed(); recorder.sample(); delay(100) } }

    if (attachMenu) {
        AttachSheet(
            onPhoto = { uri -> attachMenu = false; pick(uri, "photo") },
            onGallery = { attachMenu = false; galleryPicker.launch(arrayOf("image/*", "video/*")) },
            onFile = { attachMenu = false; filePicker.launch("*/*") },
            onLocation = {
                attachMenu = false
                if (ContextCompat.checkSelfPermission(ctx, Manifest.permission.ACCESS_FINE_LOCATION) == PackageManager.PERMISSION_GRANTED) sendLocation()
                else locationPermission.launch(Manifest.permission.ACCESS_FINE_LOCATION)
            },
            onContact = { attachMenu = false; contactPicker.launch(android.content.Intent(android.content.Intent.ACTION_PICK, android.provider.ContactsContract.CommonDataKinds.Phone.CONTENT_URI)) },
            onPoll = { attachMenu = false; pollForm = true },
            onDismiss = { attachMenu = false },
        )
    }

    if (sendMenu) {
        PopupMenu(
            listOf(
                MenuItem(R.drawable.input_notify_off, "Ovozsiz yuborish") { onSendEx(true, 0) },
                MenuItem(R.drawable.input_schedule, "Rejalashtirish") { pickSchedule() },
            ),
            onDismiss = { sendMenu = false },
        )
    }
    pendingFile?.let { (f, kind, extra) ->
        TgForm(
            when (kind) { "photo" -> "Rasm yuborish"; "video" -> "Video yuborish"; else -> "Fayl yuborish" },
            listOf("Izoh (ixtiyoriy)"), "Yuborish",
            onConfirm = { v -> pendingFile = null; onSendFile(f, kind, extra, v[0].trim()) },
            onDismiss = { pendingFile = null; f.delete() },
        )
    }
    if (pollForm) {
        TgForm("Yangi so‘rovnoma", listOf("Savol", "Variant 1", "Variant 2", "Variant 3 (ixtiyoriy)", "Variant 4 (ixtiyoriy)"), "Yuborish", onConfirm = { v ->
            pollForm = false
            val opts = v.drop(1).map { it.trim().replace("|", "") }.filter { it.isNotEmpty() }
            if (v[0].isBlank() || opts.size < 2) toast = "Savol va kamida 2 ta variant kiriting"
            else onSendSpecial("poll", (listOf(v[0].trim().replace("|", ""), "0", "0", "0") + opts).joinToString("|"))
        }, onDismiss = { pollForm = false })
    }
    LaunchedEffect(toast) { if (toast != null) { delay(2500); toast = null } }
    var silentMode by remember { androidx.compose.runtime.mutableStateOf(false) }
    val needBg = emojiPanel || recording || replyTo != null || editing != null || !draft.selection.collapsed
    Column(Modifier.fillMaxWidth().then(if (needBg) Modifier.background(c.bar) else Modifier).navigationBarsPadding().imePadding()) {
        // Matn bezaklari: belgilangan qismni belgilar bilan o'raydi (`**qalin**`, `__kursiv__`, `==tagiga chizilgan==`, `~~o'chirilgan~~`, `` `kod` ``, `||spoyler||`).
        val sel = draft.selection
        if (canWrite && !sel.collapsed && !recording) {
            Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 2.dp), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                listOf("B" to "**", "I" to "__", "U" to "==", "S" to "~~", "<>" to "`", "▒" to "||").forEach { (label, mark) ->
                    Box(
                        Modifier.clip(CircleShape).clickable {
                            val t = draft.text
                            val a = sel.min
                            val b = sel.max
                            onFormat()
                            onDraft(TextFieldValue(t.substring(0, a) + mark + t.substring(a, b) + mark + t.substring(b), TextRange(a + mark.length, b + mark.length)))
                        }.padding(horizontal = 14.dp, vertical = 6.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        BasicText(label, style = TextStyle(color = Blue, fontSize = 15.sp, fontWeight = if (label == "B") FontWeight.Bold else FontWeight.Medium, fontStyle = if (label == "I") androidx.compose.ui.text.font.FontStyle.Italic else null, textDecoration = when (label) { "U" -> androidx.compose.ui.text.style.TextDecoration.Underline; "S" -> androidx.compose.ui.text.style.TextDecoration.LineThrough; else -> null }))
                    }
                }
            }
        }
        if (toast != null) BasicText(toast.orEmpty(), Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp), style = TextStyle(color = c.red, fontSize = 13.sp))
        if (!canWrite) {
            Box(Modifier.fillMaxWidth().height(48.dp), contentAlignment = Alignment.Center) {
                BasicText("Bu yerda xabar yozib bo‘lmaydi", style = TextStyle(color = c.gray, fontSize = 15.sp))
            }
            return@Column
        }
        // Javob / tahrir paneli
        val ctxMsg = editing ?: replyTo
        if (ctxMsg != null) {
            Row(Modifier.fillMaxWidth().padding(start = 12.dp, end = 4.dp, top = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                TgIcon(if (editing != null) R.drawable.msg_edit else R.drawable.input_reply, Blue, size = 24.dp)
                Box(Modifier.padding(start = 10.dp).size(2.dp, 34.dp).background(Blue))
                Column(Modifier.weight(1f).padding(start = 8.dp)) {
                    BasicText(if (editing != null) "Tahrirlash" else "Javob: ${ctxMsg.sender.ifEmpty { if (ctxMsg.out) "Siz" else "" }}".trimEnd(' ', ':'), style = TextStyle(color = Blue, fontSize = 14.sp, fontWeight = FontWeight.Medium), maxLines = 1)
                    BasicText(ctxMsg.text.ifEmpty { ctxMsg.media }, style = TextStyle(color = c.text, fontSize = 14.sp), maxLines = 1)
                }
                Box(Modifier.size(40.dp).clickable(onClick = onCancelContext), contentAlignment = Alignment.Center) { CloseIcon(c.gray, size = 20.dp) }
            }
        }
        if (recording) {
            Row(Modifier.fillMaxWidth().height(52.dp).padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.padding(start = 8.dp).size(12.dp).clip(CircleShape).background(Color(0xFFE53935)))
                BasicText(formatDuration(recSeconds), Modifier.padding(start = 10.dp), style = TextStyle(color = c.text, fontSize = 17.sp))
                Box(Modifier.weight(1f))
                BasicText("Bekor qilish", Modifier.clickable { recorder.cancel(); recording = false }.padding(12.dp), style = TextStyle(color = Blue, fontSize = 15.sp, fontWeight = FontWeight.Medium))
                Box(Modifier.size(44.dp).clip(CircleShape).background(Blue).clickable {
                    recording = false
                    val wave = recorder.waveformHex()
                    recorder.stop()?.let { (f, s) -> onSendVoice(f, s, wave) }
                }, contentAlignment = Alignment.Center) { SendIcon(Color.White) }
            }
        } else {
            val icoCol = if (c.dark) c.gray else Color(0xFF8E959B)
            // DrKLO yangi uslub: yozish paneli — yumaloq shaffof pilyul; o'ng chetida ko'k doira (mikrofon / yuborish).
            Row(
                Modifier.padding(horizontal = 8.dp, vertical = 8.dp).fillMaxWidth().heightIn(min = 48.dp).clip(androidx.compose.foundation.shape.RoundedCornerShape(24.dp))
                    .background(if (c.dark) Color(0xFF1F1F1F) else c.bar),
                verticalAlignment = Alignment.Bottom,
            ) {
                Box(Modifier.size(48.dp).clickable {
                    emojiPanel = !emojiPanel
                    if (emojiPanel) kb?.hide() else kb?.show()
                }, contentAlignment = Alignment.Center) { TgIcon(if (emojiPanel) R.drawable.input_keyboard else R.drawable.input_smile, icoCol) }
                Box(Modifier.weight(1f).padding(top = 13.dp, bottom = 12.dp)) {
                    // DrKLO: `chat_messagePanelHint` 0xFF858A84, kursor 0xFF54A1DB, yozuv 18 sp.
                    if (draft.text.isEmpty()) BasicText(if (broadcast) "Xabar tarqatish" else "Xabar", style = TextStyle(color = if (c.dark) c.hint else Color(0xFF858A84), fontSize = 18.sp))
                    BasicTextField(
                        value = draft, onValueChange = onDraft, maxLines = 6,
                        textStyle = TextStyle(color = c.text, fontSize = 18.sp),
                        cursorBrush = SolidColor(if (c.dark) c.inputActive else Color(0xFF54A1DB)),
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                if (draft.text.isBlank()) {
                    if (hasCommands) Box(Modifier.size(48.dp).clickable(onClick = onCommands), contentAlignment = Alignment.Center) {
                        BasicText("/", style = TextStyle(color = icoCol, fontSize = 26.sp, fontWeight = FontWeight.Medium))
                    }
                    if (broadcast) Box(Modifier.size(48.dp).clickable { silentMode = !silentMode }, contentAlignment = Alignment.Center) {
                        TgIcon(if (silentMode) R.drawable.input_notify_off else R.drawable.input_notify_on, icoCol)
                    }
                    Box(Modifier.size(48.dp).clickable { attachMenu = true }, contentAlignment = Alignment.Center) { TgIcon(R.drawable.input_attach, icoCol) }
                }
                Box(Modifier.padding(4.dp).size(40.dp).clip(CircleShape).background(if (c.dark) c.fab else Blue)
                    .combinedClickable(
                        onClick = {
                            if (draft.text.isNotBlank()) { if (broadcast && silentMode) onSendEx(true, 0) else onSend() }
                            else if (recorder.supported) {
                                if (ContextCompat.checkSelfPermission(ctx, Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) {
                                    if (recorder.start()) recording = true
                                } else permission.launch(Manifest.permission.RECORD_AUDIO)
                            }
                        },
                        onLongClick = { if (draft.text.isNotBlank()) sendMenu = true },
                    ), contentAlignment = Alignment.Center) {
                    if (draft.text.isBlank()) TgIcon(R.drawable.input_mic, Color.White, size = 22.dp) else SendIcon(Color.White)
                }
            }
            if (emojiPanel) {
                if (panelTab == 0) {
                    LazyVerticalGrid(GridCells.Adaptive(44.dp), Modifier.fillMaxWidth().height(260.dp).padding(horizontal = 6.dp)) {
                        items(Emojis.filter { it.isNotBlank() }) { e ->
                            Box(Modifier.size(44.dp).clickable {
                                val t = draft.text
                                val sel = draft.selection
                                val nt = t.replaceRange(sel.start, sel.end, e)
                                onDraft(TextFieldValue(nt, TextRange(sel.start + e.length)))
                            }, contentAlignment = Alignment.Center) { BasicText(e, style = TextStyle(fontSize = 26.sp)) }
                        }
                    }
                } else if (panelTab == 1) {
                    StickerPanel(onPick = onSendSticker)
                } else {
                    GifPanel(onPick = { ref -> onSendSticker(Sticker(ref, "video/mp4", "")) })
                }
                // Pastki tab paneli (DrKLO `bottomTabContainer`, 40 dp): markazda Emoji / GIF / Stikerlar belgilari, o'ngda o'chirish tugmasi.
                Box(Modifier.fillMaxWidth().height(40.dp)) {
                    Row(Modifier.align(Alignment.Center), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                        listOf(0 to R.drawable.smiles_tab_smiles, 2 to R.drawable.smiles_tab_gif, 1 to R.drawable.smiles_tab_stickers).forEach { (i, icon) ->
                            Box(Modifier.size(48.dp, 40.dp).clickable { panelTab = i }, contentAlignment = Alignment.Center) {
                                TgIcon(icon, if (panelTab == i) c.inputActive else (if (c.dark) c.gray else Color(0xFF8E959B)), size = 24.dp)
                            }
                        }
                    }
                    Box(
                        Modifier.align(Alignment.CenterEnd).size(48.dp, 40.dp).clickable {
                            val sel = draft.selection
                            val t = draft.text
                            if (!sel.collapsed) onDraft(TextFieldValue(t.removeRange(sel.start, sel.end), TextRange(sel.start)))
                            else if (sel.start > 0) {
                                val cut = if (sel.start >= 2 && Character.isLowSurrogate(t[sel.start - 1])) 2 else 1
                                onDraft(TextFieldValue(t.removeRange(sel.start - cut, sel.start), TextRange(sel.start - cut)))
                            }
                        },
                        contentAlignment = Alignment.Center,
                    ) { TgIcon(R.drawable.smiles_tab_clear, if (c.dark) c.gray else Color(0xFF8E959B), size = 24.dp) }
                }
            }
        }
    }
}

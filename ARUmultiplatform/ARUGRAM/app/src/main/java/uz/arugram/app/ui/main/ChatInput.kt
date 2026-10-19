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

private val Blue = Color(0xFF3390EC)

/** Ovoz yozgich (Telegram ovozli xabari — OGG/Opus; Android 10+). */
class VoiceRecorder(private val ctx: Context) {
    private var rec: MediaRecorder? = null
    private var file: File? = null
    private var startedAt = 0L

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
            true
        }.getOrDefault(false)
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
    draft: TextFieldValue,
    onDraft: (TextFieldValue) -> Unit,
    replyTo: MsgRow?,
    editing: MsgRow?,
    canWrite: Boolean,
    onCancelContext: () -> Unit,
    onSend: () -> Unit,
    onSendFile: (File, String, String) -> Unit,
    onSendVoice: (File, Double) -> Unit,
    onSendSticker: (Sticker) -> Unit,
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
        onSendFile(f, kind, extra)
    }
    val imagePicker = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { pick(it, "photo") }
    val videoPicker = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { pick(it, "video") }
    val filePicker = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { pick(it, "file") }
    val permission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok ->
        if (ok && recorder.start()) recording = true
    }

    LaunchedEffect(recording) { while (recording) { recSeconds = recorder.elapsed(); delay(200) } }

    if (attachMenu) {
        PopupMenu(
            listOf(
                MenuItem(R.drawable.msg_photos, "Rasm") { imagePicker.launch("image/*") },
                MenuItem(R.drawable.input_video, "Video") { videoPicker.launch("video/*") },
                MenuItem(R.drawable.files_internal, "Fayl") { filePicker.launch("*/*") },
                MenuItem(R.drawable.msg_location, "Joylashuv") {
                    if (ContextCompat.checkSelfPermission(ctx, Manifest.permission.ACCESS_FINE_LOCATION) == PackageManager.PERMISSION_GRANTED) sendLocation()
                    else locationPermission.launch(Manifest.permission.ACCESS_FINE_LOCATION)
                },
                MenuItem(R.drawable.msg_contacts, "Kontakt") {
                    contactPicker.launch(android.content.Intent(android.content.Intent.ACTION_PICK, android.provider.ContactsContract.CommonDataKinds.Phone.CONTENT_URI))
                },
                MenuItem(R.drawable.msg_select, "So‘rovnoma") { pollForm = true },
            ),
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
    if (pollForm) {
        TgForm("Yangi so‘rovnoma", listOf("Savol", "Variant 1", "Variant 2", "Variant 3 (ixtiyoriy)", "Variant 4 (ixtiyoriy)"), "Yuborish", onConfirm = { v ->
            pollForm = false
            val opts = v.drop(1).map { it.trim().replace("|", "") }.filter { it.isNotEmpty() }
            if (v[0].isBlank() || opts.size < 2) toast = "Savol va kamida 2 ta variant kiriting"
            else onSendSpecial("poll", (listOf(v[0].trim().replace("|", ""), "0", "0", "0") + opts).joinToString("|"))
        }, onDismiss = { pollForm = false })
    }
    LaunchedEffect(toast) { if (toast != null) { delay(2500); toast = null } }
    Column(Modifier.fillMaxWidth().background(c.bar).navigationBarsPadding().imePadding()) {
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
                    recorder.stop()?.let { (f, s) -> onSendVoice(f, s) }
                }, contentAlignment = Alignment.Center) { SendIcon(Color.White) }
            }
        } else {
            Row(Modifier.fillMaxWidth().heightIn(min = 48.dp), verticalAlignment = Alignment.Bottom) {
                Box(Modifier.size(48.dp).clickable {
                    emojiPanel = !emojiPanel
                    if (emojiPanel) kb?.hide() else kb?.show()
                }, contentAlignment = Alignment.Center) { TgIcon(if (emojiPanel) R.drawable.input_keyboard else R.drawable.input_smile, c.gray) }
                Box(Modifier.weight(1f).padding(vertical = 12.dp)) {
                    if (draft.text.isEmpty()) BasicText("Xabar", style = TextStyle(color = c.hint, fontSize = 18.sp))
                    BasicTextField(
                        value = draft, onValueChange = onDraft, maxLines = 6,
                        textStyle = TextStyle(color = c.text, fontSize = 18.sp),
                        cursorBrush = SolidColor(c.inputActive),
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                if (draft.text.isBlank()) {
                    Box(Modifier.size(48.dp).clickable { attachMenu = true }, contentAlignment = Alignment.Center) { TgIcon(R.drawable.input_attach, c.gray) }
                    if (recorder.supported) {
                        Box(Modifier.size(48.dp).clickable {
                            if (ContextCompat.checkSelfPermission(ctx, Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) {
                                if (recorder.start()) recording = true
                            } else {
                                permission.launch(Manifest.permission.RECORD_AUDIO)
                            }
                        }, contentAlignment = Alignment.Center) { TgIcon(R.drawable.input_mic, c.gray) }
                    }
                } else {
                    Box(Modifier.size(48.dp).combinedClickable(onClick = onSend, onLongClick = { sendMenu = true }), contentAlignment = Alignment.Center) { SendIcon(Blue) }
                }
            }
            if (emojiPanel) {
                Row(Modifier.fillMaxWidth().height(40.dp), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
                    listOf("Emoji", "Stikerlar").forEachIndexed { i, t ->
                        BasicText(
                            t, Modifier.clickable { panelTab = i }.padding(horizontal = 20.dp, vertical = 8.dp),
                            style = TextStyle(color = if (panelTab == i) Blue else c.gray, fontSize = 15.sp, fontWeight = FontWeight.Medium),
                        )
                    }
                }
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
                } else {
                    StickerPanel(onPick = onSendSticker)
                }
            }
        }
    }
}

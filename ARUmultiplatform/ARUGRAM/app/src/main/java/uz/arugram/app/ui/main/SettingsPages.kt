package uz.arugram.app.ui.main

import android.content.Context
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import uz.arugram.app.R
import uz.arugram.app.core.RustCore
import uz.arugram.app.ui.components.BackArrowIcon
import uz.arugram.app.ui.theme.LocalThemeCtl
import uz.arugram.app.ui.theme.LocalTg
import java.io.File
import java.util.Calendar

private val Blue = Color(0xFF3390EC)

fun settingsPrefs(ctx: Context) = ctx.getSharedPreferences("settings", Context.MODE_PRIVATE)

@Composable
private fun Page(title: String, onBack: () -> Unit, content: @Composable () -> Unit) {
    val c = LocalTg.current
    BackHandler(onBack = onBack)
    Column(Modifier.fillMaxSize().background(c.grayBg).statusBarsPadding()) {
        Row(Modifier.fillMaxWidth().height(56.dp).background(c.bar), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(56.dp).clickable(onClick = onBack), contentAlignment = Alignment.Center) { BackArrowIcon(c.text) }
            BasicText(title, style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold))
        }
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState())) { content() }
    }
}

@Composable
private fun Card(content: @Composable () -> Unit) {
    val c = LocalTg.current
    // DrKLO `UniversalRecyclerView.setSections`: chetdan 12 dp, 16 dp burchakli bo'lim (karta).
    Column(Modifier.padding(start = 12.dp, end = 12.dp, top = 6.dp).fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(if (c.dark) c.bg else c.bar)) { content() }
}

@Composable
private fun Hint(text: String) {
    val c = LocalTg.current
    BasicText(text, Modifier.padding(start = 30.dp, end = 30.dp, top = 10.dp, bottom = 8.dp), style = TextStyle(color = c.gray, fontSize = 14.sp, lineHeight = 19.sp))
}

@Composable
private fun TextRow(title: String, value: String? = null, subtitle: String? = null, danger: Boolean = false, onClick: (() -> Unit)? = null) {
    val c = LocalTg.current
    Row(
        Modifier.fillMaxWidth().heightIn(min = 50.dp).then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier).padding(horizontal = 18.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            BasicText(title, style = TextStyle(color = if (danger) c.red else c.text, fontSize = 16.sp))
            if (subtitle != null) BasicText(subtitle, Modifier.padding(top = 2.dp), style = TextStyle(color = c.gray, fontSize = 13.sp))
        }
        if (value != null) BasicText(value, style = TextStyle(color = c.value, fontSize = 16.sp))
    }
}

@Composable
private fun SwitchRow(title: String, subtitle: String? = null, checked: Boolean, onChange: (Boolean) -> Unit) {
    val c = LocalTg.current
    Row(Modifier.fillMaxWidth().heightIn(min = 50.dp).clickable { onChange(!checked) }.padding(start = 18.dp, end = 12.dp, top = 6.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            BasicText(title, style = TextStyle(color = c.text, fontSize = 16.sp))
            if (subtitle != null) BasicText(subtitle, Modifier.padding(top = 2.dp), style = TextStyle(color = c.gray, fontSize = 13.sp))
        }
        Switch(checked, onChange, colors = SwitchDefaults.colors(checkedTrackColor = Blue, checkedThumbColor = Color.White))
    }
}

@Composable
private fun RadioRow(title: String, selected: Boolean, enabled: Boolean = true, onClick: () -> Unit) {
    val c = LocalTg.current
    Row(Modifier.fillMaxWidth().then(if (enabled) Modifier.clickable(onClick = onClick) else Modifier).padding(horizontal = 18.dp, vertical = 14.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(20.dp).clip(CircleShape).background(if (selected) Blue else c.input), contentAlignment = Alignment.Center) {
            Box(Modifier.size(if (selected) 8.dp else 16.dp).clip(CircleShape).background(if (selected) Color.White else c.bar))
        }
        BasicText(title, Modifier.padding(start = 16.dp), style = TextStyle(color = if (enabled) c.text else c.hint, fontSize = 16.sp))
    }
}

// ── Akkaunt ──
@Composable
fun AccountPage(onBack: () -> Unit) {
    val c = LocalTg.current
    val scope = rememberCoroutineScope()
    var p by remember { mutableStateOf<JSONObject?>(null) }
    var edit by remember { mutableStateOf<String?>(null) }
    var msg by remember { mutableStateOf<String?>(null) }
    fun load() { scope.launch { p = withContext(Dispatchers.IO) { RustCore.tgProfile() } } }
    LaunchedEffect(Unit) { load() }
    val first = p?.optString("first").orEmpty()
    val last = p?.optString("last").orEmpty()
    LaunchedEffect(msg) { if (msg != null) { kotlinx.coroutines.delay(2500); msg = null } }
    Box(Modifier.fillMaxSize()) {
    Page("Akkaunt", onBack) {
        if (p == null) Box(Modifier.fillMaxWidth().padding(40.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(32.dp), color = c.inputActive, strokeWidth = 3.dp) }
        else Card {
            TextRow("Telefon", p?.optString("phone")?.let { "+$it" })
            TextRow("Ism", "$first $last".trim().ifEmpty { "—" }) { edit = "name" }
            TextRow("Foydalanuvchi nomi", p?.optString("username")?.let { if (it.isEmpty()) "yo‘q" else "@$it" }) { edit = "username" }
            TextRow("Bio", p?.optString("about")?.let { if (it.isEmpty()) "yo‘q" else it.take(30) }) { edit = "about" }
        }
        Hint("Ismingiz, foydalanuvchi nomingiz va bio’ngiz boshqa foydalanuvchilarga ko‘rinadi.")
    }
    Bulletin(msg, Modifier.align(Alignment.BottomCenter).padding(bottom = 40.dp))
    }
    fun apply(r: JSONObject) {
        val err = r.optString("error")
        msg = if (err.isEmpty()) "Saqlandi" else err
        load()
    }
    when (edit) {
        "name" -> TgPrompt("Ism", first, "Keyingi", onConfirm = { f ->
            edit = "last:$f"
        }, onDismiss = { edit = null })
        "username" -> TgPrompt("Foydalanuvchi nomi", p?.optString("username").orEmpty(), "Saqlash", onConfirm = { v ->
            edit = null
            scope.launch { apply(withContext(Dispatchers.IO) { RustCore.tgUpdateUsername(v) }) }
        }, onDismiss = { edit = null })
        "about" -> TgPrompt("Bio", p?.optString("about").orEmpty(), "Saqlash", onConfirm = { v ->
            edit = null
            scope.launch { apply(withContext(Dispatchers.IO) { RustCore.tgUpdateProfile(first, last, v.take(70)) }) }
        }, onDismiss = { edit = null }, singleLine = false)
        null -> {}
        else -> if (edit!!.startsWith("last:")) {
            val f = edit!!.removePrefix("last:")
            TgPrompt("Familiya", last, "Saqlash", onConfirm = { l ->
                edit = null
                scope.launch { apply(withContext(Dispatchers.IO) { RustCore.tgUpdateProfile(f, l, p?.optString("about").orEmpty()) }) }
            }, onDismiss = { edit = null })
        }
    }
}

// ── Chat sozlamalari (mavzu) ──
@Composable
fun ChatSettingsPage(onBack: () -> Unit) {
    val ctl = LocalThemeCtl.current
    val ctx = LocalContext.current
    val prefs = remember { settingsPrefs(ctx) }
    var anim by remember { mutableStateOf(prefs.getBoolean("animations", true)) }
    Page("Chat sozlamalari", onBack) {
        Hint("Mavzu")
        Card {
            RadioRow("Tizim bo‘yicha", ctl.mode == 0) { ctl.set(0) }
            RadioRow("Kunduzgi", ctl.mode == 1) { ctl.set(1) }
            RadioRow("Tungi", ctl.mode == 2) { ctl.set(2) }
        }
        Card {
            SwitchRow("Animatsiyalar", "Suhbat va ro‘yxatlardagi silliq o‘tishlar", anim) { anim = it; prefs.edit().putBoolean("animations", it).apply() }
        }
        Hint("Xabarlar matni o‘lchami")
        Card {
            var size by remember { mutableStateOf(prefs.getInt("text_size", 16).toFloat()) }
            Row(Modifier.fillMaxWidth().padding(horizontal = 18.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                androidx.compose.material3.Slider(
                    value = size, onValueChange = { size = it; prefs.edit().putInt("text_size", it.toInt()).apply() }, valueRange = 12f..28f, modifier = Modifier.weight(1f),
                    colors = androidx.compose.material3.SliderDefaults.colors(thumbColor = Blue, activeTrackColor = Blue),
                )
                BasicText("${size.toInt()}", Modifier.padding(start = 12.dp), style = TextStyle(color = LocalTg.current.text, fontSize = 16.sp))
            }
        }
        Hint("Suhbat foni")
        Card {
            var sel by remember { mutableStateOf(prefs.getInt("wallpaper", 0)) }
            val picker = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.GetContent()) { uri ->
                if (uri != null) {
                    val f = copyUriToCache(ctx, uri)
                    if (f != null) {
                        f.copyTo(File(ctx.filesDir, "wallpaper.jpg"), overwrite = true)
                        f.delete()
                        sel = WALLPAPER_IMAGE
                        prefs.edit().putInt("wallpaper", WALLPAPER_IMAGE).apply()
                    }
                }
            }
            WallpaperPresets.forEachIndexed { i, p ->
                RadioRow(p.first, sel == i) { sel = i; prefs.edit().putInt("wallpaper", i).apply() }
            }
            RadioRow("Galereyadan tanlash…", sel == WALLPAPER_IMAGE) { picker.launch("image/*") }
        }
    }
}

// ── Maxfiylik ──
@Composable
fun PrivacyPage(onBack: () -> Unit, onDevices: () -> Unit) {
    val ctx = LocalContext.current
    var enabled by remember { mutableStateOf(Passcode.enabled(ctx)) }
    var mode by remember { mutableStateOf<String?>(null) }
    when (mode) {
        "set" -> { PasscodeSetup(onDone = { enabled = true; mode = null }, onCancel = { mode = null }); return }
        "off" -> { PasscodeConfirm("Parolni o‘chirish uchun joriy parolni kiriting", onOk = { Passcode.disable(ctx); enabled = false; mode = null }, onCancel = { mode = null }); return }
        "change" -> { PasscodeConfirm("Joriy parolni kiriting", onOk = { mode = "set" }, onCancel = { mode = null }); return }
    }
    Page("Maxfiylik va xavfsizlik", onBack) {
        Card {
            TextRow("Faol seanslar", subtitle = "Hisobingiz kirilgan qurilmalar", onClick = onDevices)
            TextRow("Parol-qulf", subtitle = if (enabled) "Yoqilgan — ilova 1 daqiqadan keyin qulflanadi" else "Ilovani PIN bilan qulflash", value = if (enabled) "Yoqilgan" else "O‘chirilgan") {
                mode = if (enabled) "off" else "set"
            }
            if (enabled) TextRow("Parolni o‘zgartirish") { mode = "change" }
            TextRow("Ikki bosqichli tekshiruv", subtitle = "Kirishda qo‘shimcha parol so‘raladi", value = "Telegramda")
        }
        Hint("Kim sizni ko‘ra olishi")
        PrivacyRules()
        Hint("Parol-qulf faqat shu qurilmadagi ilovani himoyalaydi. Quyidagi sozlamalar Telegram serverida saqlanadi (istisnolar Telegram’ning rasmiy ilovasida o‘zgartiriladi).")
    }
}

// ── Bildirishnomalar ──
@Composable
fun NotificationsPage(onBack: () -> Unit) {
    val ctx = LocalContext.current
    val prefs = remember { settingsPrefs(ctx) }
    var enabled by remember { mutableStateOf(prefs.getBoolean("notify", true)) }
    var preview by remember { mutableStateOf(prefs.getBoolean("notify_preview", true)) }
    var groups by remember { mutableStateOf(prefs.getBoolean("notify_groups", true)) }
    var vibrate by remember { mutableStateOf(prefs.getBoolean("notify_vibrate", true)) }
    val permission = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.RequestPermission()) { }
    Page("Bildirishnomalar", onBack) {
        Card {
            SwitchRow("Xabar bildirishnomalari", "Ilova yopiq bo‘lganda ham yangi xabarlar haqida xabar beradi", enabled) {
                enabled = it
                prefs.edit().putBoolean("notify", it).apply()
                if (it && android.os.Build.VERSION.SDK_INT >= 33) permission.launch(android.Manifest.permission.POST_NOTIFICATIONS)
                if (it) uz.arugram.app.notify.NotifyService.start(ctx) else uz.arugram.app.notify.NotifyService.stop(ctx)
            }
            SwitchRow("Xabar matnini ko‘rsatish", null, preview) { preview = it; prefs.edit().putBoolean("notify_preview", it).apply() }
            SwitchRow("Guruh va kanallar", null, groups) { groups = it; prefs.edit().putBoolean("notify_groups", it).apply() }
            SwitchRow("Tebranish", null, vibrate) { vibrate = it; prefs.edit().putBoolean("notify_vibrate", it).apply() }
        }
        Hint("Bildirishnomalar ilovaning o‘z ulanishi orqali keladi (doimiy “ARUGRAM ishlayapti” xabari bilan). Muayyan suhbatni jim qilish — suhbat menyusida.")
    }
}

// ── Ma'lumotlar va xotira ──
@Composable
fun DataPage(onBack: () -> Unit) {
    val ctx = LocalContext.current
    val prefs = remember { settingsPrefs(ctx) }
    var size by remember { mutableStateOf(0L) }
    var autoPhoto by remember { mutableStateOf(prefs.getBoolean("auto_photo", true)) }
    var autoVoice by remember { mutableStateOf(prefs.getBoolean("auto_voice", false)) }
    fun dirSize(f: File): Long = if (f.isDirectory) (f.listFiles()?.sumOf { dirSize(it) } ?: 0L) else f.length()
    fun calc() { size = dirSize(File(ctx.filesDir, "media")) + dirSize(File(ctx.filesDir, "avatars")) + dirSize(File(ctx.cacheDir, "send")) }
    LaunchedEffect(Unit) { withContext(Dispatchers.IO) { calc() } }
    var confirm by remember { mutableStateOf(false) }
    Page("Ma’lumotlar va xotira", onBack) {
        Card {
            TextRow("Xotiradagi kesh", formatSize(size))
            TextRow("Keshni tozalash", danger = true) { confirm = true }
        }
        Card {
            SwitchRow("Rasmlarni avto-yuklash", null, autoPhoto) { autoPhoto = it; prefs.edit().putBoolean("auto_photo", it).apply() }
            SwitchRow("Ovozli xabarlarni avto-yuklash", null, autoVoice) { autoVoice = it; prefs.edit().putBoolean("auto_voice", it).apply() }
        }
        Hint("Kesh — yuklab olingan rasm, video, ovoz va avatarlar. Tozalagach ular kerak bo‘lganda qayta yuklanadi.")
    }
    if (confirm) {
        TgAlert("Keshni tozalash", "Yuklab olingan fayllar (${formatSize(size)}) qurilmadan o‘chiriladi.", "Tozalash", danger = true, onConfirm = {
            confirm = false
            File(ctx.filesDir, "media").deleteRecursively()
            File(ctx.filesDir, "avatars").deleteRecursively()
            File(ctx.cacheDir, "send").deleteRecursively()
            MediaCache.clear()
            AvatarCache.clear()
            calc()
        }, onDismiss = { confirm = false })
    }
}

// ── Qurilmalar ──
@Composable
fun DevicesPage(onBack: () -> Unit) {
    val c = LocalTg.current
    val scope = rememberCoroutineScope()
    var list by remember { mutableStateOf<List<JSONObject>?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var kill by remember { mutableStateOf<JSONObject?>(null) }
    fun load() {
        scope.launch {
            val j = withContext(Dispatchers.IO) { RustCore.tgSessions() }
            val err = j.optString("error")
            if (err.isNotEmpty()) { error = err; list = emptyList(); return@launch }
            val a = j.optJSONArray("sessions")
            list = buildList { for (i in 0 until (a?.length() ?: 0)) add(a!!.getJSONObject(i)) }
        }
    }
    LaunchedEffect(Unit) { load() }
    Page("Qurilmalar", onBack) {
        val l = list
        if (l == null) Box(Modifier.fillMaxWidth().padding(40.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(32.dp), color = c.inputActive, strokeWidth = 3.dp) }
        else {
            if (error != null) Hint(error.orEmpty())
            val cur = l.firstOrNull { it.optBoolean("current") }
            if (cur != null) {
                Hint("Bu qurilma")
                Card { SessionRow(cur, null) }
            }
            val others = l.filter { !it.optBoolean("current") }
            if (others.isNotEmpty()) {
                Hint("Faol seanslar")
                Card { others.forEach { s -> SessionRow(s) { kill = s } } }
                Hint("Seansni bosib, uni tugatishingiz mumkin.")
            }
        }
    }
    kill?.let { s ->
        TgAlert("Seansni tugatish", "${s.optString("device")} (${s.optString("app")}) qurilmasidagi seans tugatiladi.", "Tugatish", danger = true, onConfirm = {
            kill = null
            scope.launch { withContext(Dispatchers.IO) { RustCore.tgTerminate(s.optString("hash")) }; load() }
        }, onDismiss = { kill = null })
    }
}

@Composable
private fun SessionRow(s: JSONObject, onClick: (() -> Unit)?) {
    val c = LocalTg.current
    val d = Calendar.getInstance().apply { timeInMillis = s.optLong("date") * 1000 }
    val whenText = "%02d.%02d.%02d %02d:%02d".format(d.get(Calendar.DAY_OF_MONTH), d.get(Calendar.MONTH) + 1, d.get(Calendar.YEAR) % 100, d.get(Calendar.HOUR_OF_DAY), d.get(Calendar.MINUTE))
    Column(Modifier.fillMaxWidth().then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier).padding(horizontal = 18.dp, vertical = 10.dp)) {
        BasicText(s.optString("device").ifEmpty { "Noma’lum qurilma" }, style = TextStyle(color = c.text, fontSize = 16.sp, fontWeight = FontWeight.Medium))
        BasicText(s.optString("app"), style = TextStyle(color = c.text, fontSize = 14.sp))
        BasicText("${s.optString("ip")} · ${s.optString("country")} · $whenText", style = TextStyle(color = c.gray, fontSize = 13.sp))
    }
}

// ── Chat jildlari ──
@Composable
fun FoldersPage(onBack: () -> Unit, folders: List<Folder>, onChanged: () -> Unit = {}) {
    var edit by remember { mutableStateOf<Folder?>(null) }
    var creating by remember { mutableStateOf(false) }
    Page("Chat jildlari", onBack) {
        Card {
            folders.forEach { f -> TextRow(f.title, subtitle = f.summary()) { edit = f } }
            TextRow("Yangi jild yaratish") { creating = true }
        }
        Hint("Jildlar suhbatlar ro‘yxati tepasida ko‘rinadi. Jildni bosib tahrirlash yoki o‘chirish mumkin.")
    }
    if (creating || edit != null) {
        FolderEditor(edit, onDismiss = { creating = false; edit = null }, onSaved = { creating = false; edit = null; onChanged() })
    }
}

// ── Quvvat tejash ──
@Composable
fun PowerPage(onBack: () -> Unit) {
    val ctx = LocalContext.current
    val prefs = remember { settingsPrefs(ctx) }
    var on by remember { mutableStateOf(prefs.getBoolean("power", false)) }
    Page("Quvvat tejash", onBack) {
        Card { SwitchRow("Quvvat tejash rejimi", "Orqa fonda yangilanishlar kamayadi", on) { on = it; prefs.edit().putBoolean("power", it).apply() } }
        Hint("Yoqilganda ilova orqa fonda xabarlarni kamroq tekshiradi (batareyani tejaydi, bildirishnomalar biroz kechikishi mumkin).")
    }
}

// ── Til ──
@Composable
fun LanguagePage(onBack: () -> Unit) {
    Page("Til", onBack) {
        Card {
            RadioRow("O‘zbekcha", true) {}
            RadioRow("English (tez orada)", false, enabled = false) {}
        }
        Hint("Ilova interfeysi hozircha o‘zbek tilida.")
    }
}

// ── Qo'ng'iroqlar tarixi ──
@Composable
fun CallsPage(onBack: () -> Unit) {
    val c = LocalTg.current
    var list by remember { mutableStateOf<List<JSONObject>?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) {
        val j = withContext(Dispatchers.IO) { RustCore.tgCalls() }
        error = j.optString("error").ifEmpty { null }
        val a = j.optJSONArray("calls")
        list = buildList { for (i in 0 until (a?.length() ?: 0)) add(a!!.getJSONObject(i)) }
    }
    Page("Qo‘ng‘iroqlar", onBack) {
        val l = list
        if (l == null) Box(Modifier.fillMaxWidth().padding(40.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(32.dp), color = c.inputActive, strokeWidth = 3.dp) }
        else {
            if (error != null) Hint(error.orEmpty())
            if (l.isEmpty() && error == null) Hint("Hozircha qo‘ng‘iroqlar yo‘q.")
            else Card {
                l.forEach { o ->
                    val out = o.optBoolean("out")
                    val missed = o.optBoolean("missed")
                    val dur = o.optInt("duration")
                    val kind = when {
                        missed && out -> "Bekor qilingan"
                        missed -> "O‘tkazib yuborilgan"
                        out -> "Chiquvchi"
                        else -> "Kiruvchi"
                    } + (if (o.optBoolean("video")) " video" else "") + (if (dur > 0) " (%d:%02d)".format(dur / 60, dur % 60) else "")
                    val row = ChatRow(o.optLong("user"), "user", o.optString("name"), false, 0, false, false, 0, "", false, false, false, access = o.optString("hash", "0"))
                    Row(Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        ChatAvatar(row, 44.dp)
                        Column(Modifier.weight(1f).padding(start = 12.dp)) {
                            BasicText(o.optString("name"), style = TextStyle(color = if (missed && !out) c.red else c.text, fontSize = 16.sp, fontWeight = FontWeight.Medium), maxLines = 1)
                            BasicText(kind, Modifier.padding(top = 2.dp), style = TextStyle(color = c.gray, fontSize = 13.sp))
                        }
                        BasicText(chatTime(o.optLong("date")), style = TextStyle(color = c.gray, fontSize = 13.sp))
                    }
                }
            }
            Hint("Ovozli va video qo‘ng‘iroq qilish hozircha qo‘llab-quvvatlanmaydi — bu yerda faqat qo‘ng‘iroqlar tarixi ko‘rinadi.")
        }
    }
}

// ── Telegram Premium ──
@Composable
fun PremiumPage(onBack: () -> Unit) {
    val c = LocalTg.current
    val ctx = LocalContext.current
    var text by remember { mutableStateOf<String?>(null) }
    var options by remember { mutableStateOf<List<JSONObject>>(emptyList()) }
    var error by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) {
        val j = withContext(Dispatchers.IO) { RustCore.tgPremium() }
        error = j.optString("error").ifEmpty { null }
        text = j.optString("text")
        val a = j.optJSONArray("options")
        options = buildList { for (i in 0 until (a?.length() ?: 0)) add(a!!.getJSONObject(i)) }
    }
    Page("Telegram Premium", onBack) {
        Column(Modifier.fillMaxWidth().background(c.bar).padding(horizontal = 24.dp, vertical = 24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            BasicText("★", style = TextStyle(color = Color(0xFF7C6BFF), fontSize = 56.sp))
            BasicText("Telegram Premium", Modifier.padding(top = 8.dp), style = TextStyle(color = c.text, fontSize = 22.sp, fontWeight = FontWeight.Bold))
            BasicText(
                "Cheksiz imkoniyatlar: 4 GB gacha fayllar, tezroq yuklash, ovozni matnga aylantirish, qo‘shimcha reaksiyalar, maxsus stikerlar va boshqalar.",
                Modifier.padding(top = 8.dp), style = TextStyle(color = c.gray, fontSize = 15.sp, lineHeight = 20.sp),
            )
        }
        if (error != null) Hint(error.orEmpty())
        text?.takeIf { it.isNotEmpty() }?.let { Hint(it) }
        if (options.isNotEmpty()) Card {
            options.forEach { o ->
                val months = o.optInt("months")
                val title = if (months >= 12 && months % 12 == 0) "${months / 12} yil" else "$months oy"
                val price = "%.2f %s".format(o.optLong("amount") / 100.0, o.optString("currency"))
                TextRow(title, value = price, subtitle = if (o.optBoolean("current")) "Hozirgi obuna" else null) { openUrl(ctx, o.optString("url")) }
            }
        }
        Card { TextRow("Premium’ni Telegram botida olish") { openUrl(ctx, "https://t.me/PremiumBot") } }
        Hint("To‘lov Telegram’ning rasmiy @PremiumBot orqali amalga oshiriladi. Obuna faollashgach bu hisob ARUGRAM’da ham Premium imkoniyatlaridan foydalanadi.")
    }
}

// ── Stars ──
@Composable
fun StarsPage(onBack: () -> Unit) {
    val c = LocalTg.current
    val ctx = LocalContext.current
    var balance by remember { mutableStateOf<Long?>(null) }
    var history by remember { mutableStateOf<List<JSONObject>>(emptyList()) }
    var error by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) {
        val j = withContext(Dispatchers.IO) { RustCore.tgStars() }
        error = j.optString("error").ifEmpty { null }
        if (error == null) balance = j.optLong("balance")
        val a = j.optJSONArray("history")
        history = buildList { for (i in 0 until (a?.length() ?: 0)) add(a!!.getJSONObject(i)) }
    }
    Page("Mening Stars’im", onBack) {
        Column(Modifier.fillMaxWidth().background(c.bar).padding(vertical = 24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            BasicText("Balans", style = TextStyle(color = c.gray, fontSize = 14.sp))
            BasicText("★ ${balance ?: "—"}", Modifier.padding(top = 4.dp), style = TextStyle(color = Color(0xFFF2A100), fontSize = 34.sp, fontWeight = FontWeight.Bold))
        }
        if (error != null) Hint(error.orEmpty())
        Card { TextRow("Stars sotib olish") { openUrl(ctx, "https://t.me/PremiumBot") } }
        if (history.isNotEmpty()) {
            Hint("Amallar tarixi")
            Card {
                history.forEach { o ->
                    val amt = o.optLong("amount")
                    TextRow(
                        o.optString("title").ifEmpty { if (amt >= 0) "Kirim" else "Chiqim" },
                        value = (if (amt > 0) "+" else "") + amt,
                        subtitle = chatTime(o.optLong("date")) + if (o.optBoolean("refund")) " · qaytarilgan" else "",
                    )
                }
            }
        }
    }
}

/** Maxfiylik qoidalari (`account.getPrivacy` / `setPrivacy`): har biri "Hamma / Kontaktlarim / Hech kim". */
@Composable
private fun PrivacyRules() {
    val scope = rememberCoroutineScope()
    val items = listOf(
        "Telefon raqamim" to "phone", "So‘nggi marta onlayn" to "last_seen", "Profil rasmim" to "photo", "Bio" to "about",
        "Guruhlarga taklif qilish" to "invite", "Qo‘ng‘iroqlar" to "call", "Uzatilgan xabarlar" to "forwards", "Ovozli xabarlar" to "voice",
    )
    val values = remember { androidx.compose.runtime.mutableStateMapOf<String, String>() }
    var editing by remember { mutableStateOf<Pair<String, String>?>(null) }
    LaunchedEffect(Unit) {
        for ((_, k) in items) {
            val j = withContext(Dispatchers.IO) { RustCore.tgPrivacyGet(k) }
            values[k] = j.optString("value").ifEmpty { "?" }
        }
    }
    fun label(v: String?) = when (v) { "all" -> "Hamma"; "contacts" -> "Kontaktlarim"; "nobody" -> "Hech kim"; null -> "..."; else -> "—" }
    Card { items.forEach { (title, k) -> TextRow(title, value = label(values[k])) { editing = title to k } } }
    editing?.let { (title, k) ->
        val c = LocalTg.current
        androidx.compose.ui.window.Dialog(onDismissRequest = { editing = null }) {
            Column(Modifier.clip(androidx.compose.foundation.shape.RoundedCornerShape(12.dp)).background(if (c.dark) Color(0xFF1C2229) else Color.White).padding(vertical = 12.dp)) {
                BasicText(title, Modifier.padding(horizontal = 24.dp, vertical = 8.dp), style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold))
                listOf("all" to "Hamma", "contacts" to "Kontaktlarim", "nobody" to "Hech kim").forEach { (v, name) ->
                    RadioRow(name, values[k] == v) {
                        editing = null
                        values[k] = v
                        scope.launch { withContext(Dispatchers.IO) { RustCore.tgPrivacySet(k, v) } }
                    }
                }
            }
        }
    }
}

// ── Stikerlar ──
@Composable
fun StickersPage(onBack: () -> Unit) {
    val c = LocalTg.current
    val scope = rememberCoroutineScope()
    var name by remember { mutableStateOf("") }
    var msg by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { StickerStore.loadSets() }
    Page("Stikerlar", onBack) {
        Card {
            Row(Modifier.fillMaxWidth().padding(horizontal = 18.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.weight(1f)) {
                    if (name.isEmpty()) BasicText("To‘plam nomi yoki t.me/addstickers/... havolasi", style = TextStyle(color = c.hint, fontSize = 15.sp))
                    androidx.compose.foundation.text.BasicTextField(
                        name, { name = it }, singleLine = true, textStyle = TextStyle(color = c.text, fontSize = 16.sp),
                        cursorBrush = androidx.compose.ui.graphics.SolidColor(c.inputActive), modifier = Modifier.fillMaxWidth(),
                    )
                }
                BasicText(
                    if (busy) "..." else "Qo‘shish",
                    Modifier.clickable(enabled = !busy && name.isNotBlank()) {
                        busy = true
                        scope.launch {
                            val r = withContext(Dispatchers.IO) { RustCore.tgStickerInstall(name) }
                            busy = false
                            msg = r.optString("error").ifEmpty { "Stiker to‘plami qo‘shildi" }
                            if (r.optString("error").isEmpty()) { name = ""; StickerStore.loadSets(force = true) }
                        }
                    }.padding(start = 12.dp, top = 8.dp, bottom = 8.dp),
                    style = TextStyle(color = Blue, fontSize = 15.sp, fontWeight = FontWeight.Bold),
                )
            }
        }
        msg?.let { Hint(it) }
        val sets = StickerStore.sets
        Hint("Mening stikerlarim")
        Card {
            if (sets == null) Box(Modifier.fillMaxWidth().padding(20.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(26.dp), color = c.inputActive, strokeWidth = 3.dp) }
            else if (sets.isEmpty()) TextRow("Hozircha stiker to‘plami yo‘q")
            else sets.forEach { s ->
                Row(Modifier.fillMaxWidth().padding(horizontal = 18.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        BasicText(s.title, style = TextStyle(color = c.text, fontSize = 16.sp), maxLines = 1)
                        BasicText("${s.count} ta stiker", style = TextStyle(color = c.gray, fontSize = 13.sp))
                    }
                    BasicText(
                        "O‘chirish",
                        Modifier.clickable {
                            scope.launch {
                                val r = withContext(Dispatchers.IO) { RustCore.tgStickerUninstall(s.id, s.hash) }
                                msg = r.optString("error").ifEmpty { "To‘plam olib tashlandi" }
                                StickerStore.loadSets(force = true)
                            }
                        }.padding(8.dp),
                        style = TextStyle(color = c.red, fontSize = 14.sp, fontWeight = FontWeight.Medium),
                    )
                }
            }
        }
    }
}

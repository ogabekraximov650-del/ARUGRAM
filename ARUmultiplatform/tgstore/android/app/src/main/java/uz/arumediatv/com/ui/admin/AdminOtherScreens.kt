package uz.arumediatv.com.ui.admin

import android.graphics.BitmapFactory
import android.util.Base64
import android.widget.Toast
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.core.RustCore
import uz.arumediatv.com.data.AppStats
import uz.arumediatv.com.data.SeriesPoint
import uz.arumediatv.com.data.StatBlock
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.profile.OverlayScaffold
import uz.arumediatv.com.ui.profile.formatBytes
import uz.arumediatv.com.ui.season.formatCount
import uz.arumediatv.com.ui.season.formatHours
import uz.arumediatv.com.ui.season.timeAgo
import uz.arumediatv.com.ui.theme.AruColors

private val card = RoundedCornerShape(16.dp)

@Composable
private fun Pill(text: String, danger: Boolean = false, primary: Boolean = false, onClick: () -> Unit) {
    Text(
        text, color = if (danger) Color(0xFFF87171) else Color.White, fontSize = 12.sp, fontWeight = FontWeight.SemiBold,
        modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(if (danger) Color(0x22F87171) else if (primary) AruColors.Accent else AruColors.CardAlt).clickable(onClick = onClick).padding(horizontal = 12.dp, vertical = 8.dp),
    )
}

// ══════════════════════ MAJBURIY KANALLAR ══════════════════════

/** Bepul ko'rish uchun majburiy kanallar: ro'yxat, limit o'zgartirish, o'chirish, qo'shish. */
@Composable
fun AdminChannelsScreen(onBack: () -> Unit, onAdd: () -> Unit) {
    val ctx = LocalContext.current
    val api = AppServices.admin
    val scope = rememberCoroutineScope()
    var items by remember { mutableStateOf<List<JSONObject>>(emptyList()) }
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var limitFor by remember { mutableStateOf<JSONObject?>(null) }
    var delFor by remember { mutableStateOf<JSONObject?>(null) }
    fun toast(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_SHORT).show()
    suspend fun load() {
        val r = api.get("/api/admin/channels")
        val a = r.json?.optJSONArray("items")
        if (r.ok && a != null) { items = (0 until a.length()).mapNotNull { a.optJSONObject(it) }; error = null } else error = api.errorOf(r, "Yuklanmadi")
        loading = false
    }
    suspend fun post(b: JSONObject): String? { val r = api.post("/api/admin/channels", b); if (r.ok) { load(); return null }; return api.errorOf(r, "Xato") }
    LaunchedEffect(Unit) { load() }

    OverlayScaffold("Majburiy kanallar", onBack) {
        Box(Modifier.fillMaxSize()) {
            when {
                loading -> LoadingBox()
                items.isEmpty() -> EmptyState(text = error ?: "Kanal yo'q")
                else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 100.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    items(items.size) { i ->
                        val c = items[i]
                        val need = c.optInt("need")
                        Column(Modifier.fillMaxWidth().clip(card).background(AruColors.Card).border(1.dp, AruColors.Border, card).padding(14.dp)) {
                            Text(c.optString("title"), color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.Bold)
                            Text("${if (c.optString("kind") == "private") "Yopiq" else "Ochiq"} · ${c.optInt("joined")} / ${if (need > 0) "$need" else "cheksiz"} · ${if (c.optBoolean("active")) "faol" else "to'ldi"}", color = AruColors.TextDim, fontSize = 12.sp)
                            Text(c.optString("url"), color = AruColors.TextFaint, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Row(Modifier.padding(top = 10.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                Pill("Limit") { limitFor = c }
                                Pill("O'chirish", danger = true) { delFor = c }
                            }
                        }
                    }
                }
            }
            Text("+ Kanal qo'shish", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 14.sp, modifier = Modifier.align(Alignment.BottomCenter).padding(16.dp).clip(RoundedCornerShape(16.dp)).background(AruColors.Accent).clickable(onClick = onAdd).padding(horizontal = 22.dp, vertical = 14.dp))
        }
    }
    limitFor?.let { c ->
        var t by remember(c) { mutableStateOf(if (c.optInt("need") > 0) c.optInt("need").toString() else "") }
        AlertDialog(
            onDismissRequest = { limitFor = null },
            title = { Text("Limit (0 — cheksiz)") },
            text = { OutlinedTextField(t, { t = it.filter(Char::isDigit).take(9) }, singleLine = true, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White)) },
            confirmButton = { TextButton(onClick = { limitFor = null; scope.launch { toast(post(JSONObject().put("op", "set").put("id", c.optLong("id")).put("need", t.toIntOrNull() ?: 0)) ?: "Saqlandi") } }) { Text("Saqlash") } },
            dismissButton = { TextButton(onClick = { limitFor = null }) { Text("Bekor") } },
            containerColor = AruColors.Surface,
        )
    }
    delFor?.let { c ->
        AlertDialog(
            onDismissRequest = { delFor = null },
            title = { Text("Kanalni o'chirish") }, text = { Text("«${c.optString("title")}» ro'yxatdan olib tashlanadi.") },
            confirmButton = { TextButton(onClick = { delFor = null; scope.launch { toast(post(JSONObject().put("op", "del").put("id", c.optLong("id"))) ?: "O'chirildi") } }) { Text("O'chirish", color = Color(0xFFF87171)) } },
            dismissButton = { TextButton(onClick = { delFor = null }) { Text("Bekor") } },
            containerColor = AruColors.Surface,
        )
    }
}

/** Kanal qo'shish: avval admin Telegram hisobi bilan tekshiriladi (nomi, rasmi, obunachilar), keyin botni admin qilib qo'shiladi. */
@Composable
fun AdminChannelAddScreen(onBack: () -> Unit, onDone: () -> Unit) {
    val api = AppServices.admin
    val scope = rememberCoroutineScope()
    var input by remember { mutableStateOf("") }
    var info by remember { mutableStateOf<JSONObject?>(null) }
    var kind by remember { mutableStateOf("public") }
    var unlimited by remember { mutableStateOf(true) }
    var need by remember { mutableStateOf("") }
    var checking by remember { mutableStateOf(false) }
    var adding by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val photo = remember(info) { info?.optString("photo")?.takeIf { it.isNotEmpty() && it != "null" }?.let { runCatching { BitmapFactory.decodeByteArray(Base64.decode(it, Base64.DEFAULT), 0, Base64.decode(it, Base64.DEFAULT).size)?.asImageBitmap() }.getOrNull() } }

    OverlayScaffold("Kanal qo'shish", onBack) {
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            OutlinedTextField(
                input, { input = it }, label = { Text("@username, -100... ID yoki t.me/+havola") }, singleLine = true, modifier = Modifier.fillMaxWidth(),
                colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White),
            )
            Spacer(Modifier.height(10.dp))
            Pill(if (checking) "Tekshirilmoqda..." else "Tekshirish", primary = true) {
                if (input.isBlank() || checking) return@Pill
                if (!AppServices.telegram.state.value.authorized) { error = "Telegram hisobingiz ulanmagan — kanalni tekshirib bo'lmaydi."; return@Pill }
                checking = true; error = null; info = null
                scope.launch {
                    val j = withContext(Dispatchers.IO) { RustCore.tgChannelInfo(input.trim()) }
                    checking = false
                    if (j.optBoolean("ok")) { info = j; kind = if (j.optString("username").isNotEmpty()) "public" else "private" } else error = j.optString("error").ifEmpty { "Kanal topilmadi" }
                }
            }
            error?.let { Text(it, color = Color(0xFFF87171), fontSize = 13.sp, modifier = Modifier.padding(top = 10.dp)) }
            info?.let { j ->
                Spacer(Modifier.height(14.dp))
                Row(Modifier.fillMaxWidth().clip(card).background(AruColors.Card).padding(14.dp), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(56.dp).clip(CircleShape).background(AruColors.CardAlt)) { if (photo != null) Image(photo, null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop) }
                    Spacer(Modifier.width(12.dp))
                    Column {
                        Text(j.optString("title"), color = Color.White, fontWeight = FontWeight.Bold, fontSize = 15.sp)
                        val un = j.optString("username")
                        Text("${if (un.isNotEmpty() && un != "null") "@$un · " else ""}${j.optInt("members")} a'zo", color = AruColors.TextDim, fontSize = 12.sp)
                        if (j.optString("about").isNotEmpty() && j.optString("about") != "null") Text(j.optString("about"), color = AruColors.TextFaint, fontSize = 11.sp, maxLines = 3, overflow = TextOverflow.Ellipsis)
                    }
                }
                Spacer(Modifier.height(12.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf("public" to "Ochiq", "private" to "Yopiq").forEach { (k, l) -> Text(l, color = if (kind == k) Color.White else AruColors.TextDim, fontSize = 13.sp, modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(if (kind == k) AruColors.Accent else AruColors.CardAlt).clickable { kind = k }.padding(horizontal = 16.dp, vertical = 8.dp)) }
                }
                Row(Modifier.fillMaxWidth().padding(top = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("Cheksiz", color = Color.White, fontSize = 14.sp, modifier = Modifier.weight(1f))
                    Switch(unlimited, { unlimited = it }, colors = SwitchDefaults.colors(checkedThumbColor = Color.White, checkedTrackColor = AruColors.Accent))
                }
                if (!unlimited) OutlinedTextField(need, { need = it.filter(Char::isDigit).take(9) }, label = { Text("Limit (masalan 1000)") }, singleLine = true, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White))
                Spacer(Modifier.height(14.dp))
                Pill(if (adding) "Qo'shilmoqda..." else "Qo'shish", primary = true) {
                    if (adding) return@Pill
                    val n = if (unlimited) 0 else need.toIntOrNull() ?: 0
                    if (!unlimited && n <= 0) { error = "Limitni yozing yoki «Cheksiz»ni yoqing"; return@Pill }
                    adding = true; error = null
                    scope.launch {
                        val id = j.optLong("id")
                        var target = if (id != 0L) "$id" else input.trim()
                        var adminErr = ""
                        val a = withContext(Dispatchers.IO) { RustCore.tgMakeBotAdmin(target) }
                        if (a.optBoolean("ok") && a.has("chat_id")) target = a.optLong("chat_id").toString() else adminErr = a.optString("error")
                        val r = api.post("/api/admin/channels", JSONObject().put("op", "add").put("kind", kind).put("input", target).put("need", n))
                        adding = false
                        if (r.ok) onDone() else error = api.errorOf(r, "Xato") + if (adminErr.isNotEmpty()) "\n(Botni admin qilib bo'lmadi: $adminErr)" else ""
                    }
                }
            }
            Spacer(Modifier.height(40.dp))
        }
    }
}

// ══════════════════════ TO'PLAMLARNI KO'RIB CHIQISH ══════════════════════

/** Foydalanuvchilar yuborgan emoji/GIF/stiker rasmlarini tasdiqlash yoki rad etish. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AdminPacksScreen(onBack: () -> Unit) {
    val ctx = LocalContext.current
    val store = AppServices.packs
    val scope = rememberCoroutineScope()
    var ops by remember { mutableStateOf<List<JSONObject>?>(null) }
    var rejectFor by remember { mutableStateOf<JSONObject?>(null) }
    var approveAll by remember { mutableStateOf(false) }
    fun toast(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_SHORT).show()
    suspend fun load() { ops = store.adminPending() }
    LaunchedEffect(Unit) { load() }

    OverlayScaffold("To'plamlarni ko'rib chiqish", onBack) {
        val list = ops
        when {
            list == null -> LoadingBox()
            list.isEmpty() -> EmptyState(text = "Kutayotgan rasm yo'q")
            else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 32.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                item { Row { Text("${list.size} ta kutmoqda", color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.weight(1f)); Text("Hammasini tasdiqlash", color = AruColors.Accent2, fontSize = 12.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.clickable { approveAll = true }) } }
                items(list, key = { it.optLong("id") }) { o ->
                    var bytes by remember(o) { mutableStateOf<ByteArray?>(null) }
                    LaunchedEffect(o) { bytes = store.stagingBytes(o.optString("file")) }
                    Row(Modifier.fillMaxWidth().clip(card).background(AruColors.Card).border(1.dp, AruColors.Border, card).padding(10.dp), verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.size(84.dp).clip(RoundedCornerShape(12.dp)).background(AruColors.CardAlt)) {
                            bytes?.let { b -> val bmp = remember(b) { BitmapFactory.decodeByteArray(b, 0, b.size)?.asImageBitmap() }; if (bmp != null) Image(bmp, null, Modifier.fillMaxSize(), contentScale = ContentScale.Fit) }
                        }
                        Spacer(Modifier.width(12.dp))
                        Column(Modifier.weight(1f)) {
                            Text(o.optString("title"), color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text("${o.optString("kind")} · ${o.optString("owner")} · ${formatBytes(o.optLong("size"))}", color = AruColors.TextDim, fontSize = 12.sp)
                            if (o.optString("emoji").isNotEmpty()) Text(o.optString("emoji"), fontSize = 18.sp)
                            Row(Modifier.padding(top = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                Pill("Tasdiqlash", primary = true) { scope.launch { toast(store.adminReview(listOf(o.optLong("id")), true) ?: "Tasdiqlandi"); load() } }
                                Pill("Rad etish", danger = true) { rejectFor = o }
                            }
                        }
                    }
                }
            }
        }
    }
    rejectFor?.let { o ->
        var reason by remember(o) { mutableStateOf("") }
        AlertDialog(
            onDismissRequest = { rejectFor = null },
            title = { Text("Nima uchun rad etiladi?") },
            text = {
                Column {
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        listOf("Nomaqbul kontent", "Sifati past", "Mualliflik huquqi buzilgan", "Noto'g'ri to'plamga yuborilgan").forEach { r -> Text(r, color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(AruColors.CardAlt).clickable { reason = r }.padding(horizontal = 10.dp, vertical = 7.dp)) }
                    }
                    OutlinedTextField(reason, { reason = it.take(120) }, label = { Text("Yoki o'zingiz yozing") }, colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White))
                }
            },
            confirmButton = { TextButton(onClick = { rejectFor = null; scope.launch { toast(store.adminReview(listOf(o.optLong("id")), false, reason.trim()) ?: "Rad etildi"); load() } }) { Text("Rad etish", color = Color(0xFFF87171)) } },
            dismissButton = { TextButton(onClick = { rejectFor = null }) { Text("Bekor") } },
            containerColor = AruColors.Surface,
        )
    }
    if (approveAll) {
        AlertDialog(
            onDismissRequest = { approveAll = false },
            title = { Text("Hammasini tasdiqlash") }, text = { Text("Hamma ${ops?.size ?: 0} ta rasm ko'rib chiqilib, tasdiqlansinmi?") },
            confirmButton = { TextButton(onClick = { approveAll = false; scope.launch { val ids = ops.orEmpty().map { it.optLong("id") }; toast(store.adminReview(ids, true) ?: "Tasdiqlandi"); load() } }) { Text("Ha") } },
            dismissButton = { TextButton(onClick = { approveAll = false }) { Text("Yo'q") } },
            containerColor = AruColors.Surface,
        )
    }
}

// ══════════════════════ UMUMIY STATISTIKA ══════════════════════

private val RANGES = listOf("24h" to "24 soat", "7d" to "7 kun", "30d" to "30 kun", "all" to "Hammasi")

/** Umumiy raqamlar (kunlik/haftalik/oylik/jami) va har biri uchun grafik (`/api/stats/series`). */
@Composable
fun AdminStatsScreen(onBack: () -> Unit) {
    val api = AppServices.admin
    var stats by remember { mutableStateOf<AppStats?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) {
        val r = api.get("/api/stats")
        if (r.ok && r.json != null) stats = AppStats.from(r.json!!) else error = api.errorOf(r, "Yuklanmadi")
    }
    OverlayScaffold("Umumiy statistika", onBack) {
        val s = stats
        if (s == null) { if (error != null) EmptyState(text = error!!) else LoadingBox(); return@OverlayScaffold }
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Column(Modifier.fillMaxWidth().clip(card).background(AruColors.Card).padding(16.dp)) {
                Text("Kontent", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.Bold)
                Text("Anime: ${s.anime} · Bo'lim: ${s.seasons} · Qism: ${s.episodes}", color = AruColors.TextDim, fontSize = 13.sp, modifier = Modifier.padding(top = 6.dp))
                Text("Yangi qismlar: bugun ${s.newEpisodes.daily} · hafta ${s.newEpisodes.weekly} · oy ${s.newEpisodes.monthly}", color = AruColors.TextFaint, fontSize = 12.sp, modifier = Modifier.padding(top = 4.dp))
            }
            StatCard("Foydalanuvchilar", s.users, "users") { "${formatCount(it)} ta" }
            StatCard("Anime ko'rishlar", s.animeViews, "anime_views") { "${formatCount(it)} ta" }
            StatCard("Bo'lim ko'rishlar", s.seasonViews, "season_views") { "${formatCount(it)} ta" }
            StatCard("Qism ko'rishlar", s.views, "views") { "${formatCount(it)} ta" }
            StatCard("Ko'rish vaqti", s.watch, "watch_ms") { "${formatHours(it)} soat" }
            StatCard("Trafik sarfi", s.traffic, "traffic") { formatBytes(it) }
            Text("Vaqt mintaqasi: UTC+5 (Toshkent)", color = AruColors.TextFaint, fontSize = 11.sp, modifier = Modifier.padding(bottom = 32.dp))
        }
    }
}

@Composable
private fun StatCard(title: String, b: StatBlock, metric: String, fmt: (Long) -> String) {
    var open by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxWidth().clip(card).background(AruColors.Card).border(1.dp, AruColors.Border, card).clickable { open = !open }.padding(16.dp)) {
        Text(title, color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.Bold)
        Text(fmt(b.total), color = AruColors.Accent2, fontSize = 24.sp, fontWeight = FontWeight.Black)
        Row(Modifier.fillMaxWidth().padding(top = 6.dp), horizontalArrangement = Arrangement.SpaceBetween) {
            listOf("Bugun" to b.daily, "Hafta" to b.weekly, "Oy" to b.monthly).forEach { (l, v) -> Column { Text(fmt(v), color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.SemiBold); Text(l, color = AruColors.TextFaint, fontSize = 11.sp) } }
        }
        if (open) TrendChart(metric, fmt)
    }
}

/** Ustunli grafik: davrni tanlash, nuqtaga bosganda qiymat ko'rinadi. */
@Composable
private fun TrendChart(metric: String, fmt: (Long) -> String) {
    val api = AppServices.admin
    var range by remember { mutableStateOf("7d") }
    var points by remember(metric, range) { mutableStateOf<List<SeriesPoint>?>(null) }
    var touch by remember(metric, range) { mutableStateOf(-1) }
    LaunchedEffect(metric, range) {
        points = null
        val r = api.get("/api/stats/series?metric=${java.net.URLEncoder.encode(metric, "UTF-8")}&range=$range")
        val a = r.json?.optJSONArray("points")
        points = if (r.ok && a != null) (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { SeriesPoint(it.optString("t"), it.optLong("v")) } else emptyList()
    }
    Column(Modifier.padding(top = 12.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            RANGES.forEach { (k, l) -> Text(l, color = if (range == k) Color.White else AruColors.TextDim, fontSize = 11.sp, modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(if (range == k) AruColors.Accent else AruColors.CardAlt).clickable { range = k }.padding(horizontal = 10.dp, vertical = 6.dp)) }
        }
        val pts = points
        when {
            pts == null -> Box(Modifier.fillMaxWidth().height(120.dp), contentAlignment = Alignment.Center) { Text("Yuklanmoqda...", color = AruColors.TextFaint, fontSize = 12.sp) }
            pts.isEmpty() -> Box(Modifier.fillMaxWidth().height(120.dp), contentAlignment = Alignment.Center) { Text("Ma'lumot yo'q", color = AruColors.TextFaint, fontSize = 12.sp) }
            else -> {
                val max = (pts.maxOf { it.v }).coerceAtLeast(1)
                Canvas(
                    Modifier.fillMaxWidth().height(120.dp).padding(top = 10.dp).clickable { },
                ) {
                    val bw = size.width / pts.size
                    pts.forEachIndexed { i, p ->
                        val h = size.height * (p.v.toFloat() / max)
                        drawRect(if (i == touch) Color(0xFFFFC93C) else Color(0xFFE2620F), topLeft = Offset(i * bw + bw * 0.12f, size.height - h), size = Size(bw * 0.76f, h.coerceAtLeast(1f)))
                    }
                }
                Row(Modifier.fillMaxWidth().padding(top = 6.dp).clip(RoundedCornerShape(8.dp)).clickable { touch = if (touch >= 0 && touch < pts.size - 1) touch + 1 else 0 }.padding(vertical = 4.dp)) {
                    val p = pts.getOrNull(touch)
                    Text(p?.let { "${it.t.replace('T', ' ')} → ${fmt(it.v)}" } ?: "Eng yuqori: ${fmt(max)} · bosing: nuqtalar bo'ylab", color = AruColors.TextDim, fontSize = 11.sp)
                }
            }
        }
    }
}

// ══════════════════════ ILOVA VA XAVFSIZLIK ══════════════════════

/** Eng kichik ruxsat etilgan versiya (eskilari ishlamaydi) va ilova imzosi (`X-App-Sig`) sozlamalari. */
@Composable
fun AdminAppScreen(onBack: () -> Unit) {
    val ctx = LocalContext.current
    val api = AppServices.admin
    val scope = rememberCoroutineScope()
    var cfg by remember { mutableStateOf<JSONObject?>(null) }
    var version by remember { mutableStateOf("") }
    var error by remember { mutableStateOf<String?>(null) }
    fun toast(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_SHORT).show()
    suspend fun load() { val r = api.get("/api/admin/app"); if (r.ok && r.json != null) { cfg = r.json; version = r.json!!.optString("min_version") } else error = api.errorOf(r, "Yuklanmadi") }
    suspend fun post(b: JSONObject, done: String) { val r = api.post("/api/admin/app", b); toast(if (r.ok) done else api.errorOf(r, "Saqlanmadi")); load() }
    LaunchedEffect(Unit) { load() }

    OverlayScaffold("Ilova va xavfsizlik", onBack) {
        val c = cfg
        if (c == null) { if (error != null) EmptyState(text = error!!) else LoadingBox(); return@OverlayScaffold }
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Column(Modifier.fillMaxWidth().clip(card).background(AruColors.Card).padding(16.dp)) {
                Text("Eng kichik versiya", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.Bold)
                Text("Bundan eski ilovalar yangilanmaguncha ishlamaydi. Masalan 0.1.0 yoki 0.1.0+9. Bo'sh — cheklov yo'q.", color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.padding(vertical = 6.dp))
                OutlinedTextField(version, { version = it.take(20) }, singleLine = true, colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White), modifier = Modifier.fillMaxWidth())
                Spacer(Modifier.height(8.dp))
                Pill("Saqlash", primary = true) { scope.launch { post(JSONObject().put("min_version", version.trim()), if (version.isBlank()) "O'chirildi" else "Saqlandi") } }
            }
            Column(Modifier.fillMaxWidth().clip(card).background(AruColors.Card).padding(16.dp)) {
                Text("Ilova imzosi", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.Bold)
                Text(if (c.optBoolean("gate_on")) "Himoya YOQILGAN: faqat tanilgan imzoli ilovalar kiradi." else "Himoya o'chiq.", color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.padding(vertical = 6.dp))
                Text("Ishonchli imzolar: ${c.optInt("sig_count")} · bu qurilma: ${if (c.optBoolean("my_sig_trusted")) "ishonchli" else "ishonchsiz"}", color = AruColors.TextFaint, fontSize = 12.sp)
                Row(Modifier.padding(top = 10.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Pill("Shu imzoga ishonish") { scope.launch { post(JSONObject().put("trust_me", true), "Qo'shildi") } }
                    Pill("Faqat shu imzo") { scope.launch { post(JSONObject().put("only_me", true), "Saqlandi") } }
                    Pill("Tozalash", danger = true) { scope.launch { post(JSONObject().put("clear", true), "Tozalandi") } }
                }
            }
            Spacer(Modifier.height(32.dp))
        }
    }
}

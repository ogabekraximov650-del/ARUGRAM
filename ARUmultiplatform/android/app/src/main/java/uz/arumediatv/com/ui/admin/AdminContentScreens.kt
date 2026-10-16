package uz.arumediatv.com.ui.admin

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.Episode
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.profile.OverlayScaffold
import uz.arumediatv.com.ui.profile.formatBytes
import uz.arumediatv.com.ui.theme.AruColors
import java.io.File

/** Janrlar ro'yxati (alifbo tartibida; bazada bo'limga bir nechta janr). */
val GENRES = listOf(
    "Bolalar uchun", "Detektiv", "Drama", "Ekshen", "Etti", "Fantastika", "Fantaziya", "G'ayrioddiy", "Garem", "Harbiy", "Iblislar", "Ish",
    "Jang san'ati", "Jangari", "Komediya", "Kosmos", "Kundalik hayot", "Maktab", "Mashinalar", "Mexa", "Musiqiy", "O'yinlar", "O'zga dunyo",
    "Psixologik", "Qo'rqinchli", "Romantika", "Sarguzasht", "Sehr", "Shafqatsizlik", "Sir", "Sport", "Super kuch", "Syodze", "Syonen", "Tarixiy", "Tragediya", "Triller",
)

private val card = RoundedCornerShape(16.dp)

@Composable
private fun Btn(text: String, danger: Boolean = false, primary: Boolean = false, onClick: () -> Unit) {
    Text(
        text, color = if (danger) Color(0xFFF87171) else Color.White, fontSize = 12.sp, fontWeight = FontWeight.SemiBold,
        modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(if (danger) Color(0x22F87171) else if (primary) AruColors.Accent else AruColors.CardAlt).clickable(onClick = onClick).padding(horizontal = 12.dp, vertical = 8.dp),
    )
}

@Composable
private fun Field(value: String, onChange: (String) -> Unit, label: String, modifier: Modifier = Modifier, number: Boolean = false, minLines: Int = 1) {
    OutlinedTextField(
        value = value, onValueChange = onChange, label = { Text(label) }, minLines = minLines, singleLine = minLines == 1,
        keyboardOptions = KeyboardOptions(keyboardType = if (number) KeyboardType.Number else KeyboardType.Text),
        colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White, focusedBorderColor = AruColors.Accent, unfocusedBorderColor = AruColors.BorderBright),
        modifier = modifier.fillMaxWidth().padding(vertical = 4.dp),
    )
}

/** Tanlangan fayl: nom, hajm va Rust o'qiy oladigan yo'l (`/proc/self/fd/N` — nusxa ko'chirilmaydi, katta videolar uchun). */
suspend fun <T> withPickedFile(ctx: Context, uri: Uri, block: suspend (path: String, name: String, size: Long) -> T): T? = withContext(Dispatchers.IO) {
    val pfd = try { ctx.contentResolver.openFileDescriptor(uri, "r") } catch (_: Exception) { null } ?: return@withContext null
    try {
        var name = "file"
        ctx.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c -> if (c.moveToFirst()) name = c.getString(0) ?: name }
        block("/proc/self/fd/${pfd.fd}", name, pfd.statSize)
    } finally {
        runCatching { pfd.close() }
    }
}

private fun fileNameOfUrl(url: String): String {
    val i = url.indexOf("/api/image/")
    return if (i < 0) url else url.substring(i + "/api/image/".length)
}

/** Rasmni Telegram'ga yuklaydi va fayl NOMINI qaytaradi (`<prefix>_<vaqt>.jpg`). */
private suspend fun uploadPhoto(ctx: Context, uri: Uri, prefix: String): Pair<String?, String?> = withPickedFile(ctx, uri) { path, _, _ ->
    val name = "${prefix}_${System.currentTimeMillis()}.jpg"
    val err = AppServices.telegram.uploadFile(path, name, "image/jpeg")
    if (err == null) name to null else null to err
} ?: (null to "Faylni o'qib bo'lmadi")

// ══════════════════════ ANIME RO'YXATI ══════════════════════

/** Anime boshqaruvi: ro'yxat, qo'shish, tahrirlash, o'chirish; bosilsa bo'limlar ochiladi. */
@Composable
fun AdminAnimeScreen(onBack: () -> Unit, onOpenAnime: (JSONObject) -> Unit, onEdit: (JSONObject?) -> Unit) {
    val ctx = LocalContext.current
    val api = AppServices.admin
    val scope = rememberCoroutineScope()
    var items by remember { mutableStateOf<List<JSONObject>?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var delFor by remember { mutableStateOf<JSONObject?>(null) }
    fun toast(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_SHORT).show()
    suspend fun load() {
        val (code, text) = withContext(Dispatchers.IO) { uz.arumediatv.com.data.Http.client.callText("${uz.arumediatv.com.BuildConfig.API_BASE}/api/anime") }
        if (code == 200) items = try { val a = JSONArray(text); (0 until a.length()).mapNotNull { a.optJSONObject(it) } } catch (_: Exception) { emptyList() } else error = "Yuklanmadi ($code)"
    }
    LaunchedEffect(Unit) { load() }
    OverlayScaffold("Anime boshqaruvi", onBack) {
        Box(Modifier.fillMaxSize()) {
            val list = items
            when {
                list == null -> if (error != null) EmptyState(text = error!!) else LoadingBox()
                list.isEmpty() -> EmptyState(text = "Anime yo'q")
                else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 100.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    items(list, key = { it.optLong("id") }) { a ->
                        Row(Modifier.fillMaxWidth().clip(card).background(AruColors.Card).border(1.dp, AruColors.Border, card).clickable { onOpenAnime(a) }.padding(10.dp), verticalAlignment = Alignment.CenterVertically) {
                            TgImage(a.optString("photo_url"), Modifier.size(56.dp).clip(RoundedCornerShape(10.dp)).background(AruColors.CardAlt), maxPx = 200)
                            Spacer(Modifier.width(12.dp))
                            Column(Modifier.weight(1f)) {
                                Text(a.optString("name"), color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                Text("ID ${a.optLong("id")} · ${a.optString("janri")}", color = AruColors.TextDim, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                            Btn("Tahrir") { onEdit(a) }
                            Spacer(Modifier.width(6.dp))
                            Btn("O'chirish", danger = true) { delFor = a }
                        }
                    }
                }
            }
            Text("+ Anime qo'shish", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 14.sp, modifier = Modifier.align(Alignment.BottomCenter).padding(16.dp).clip(RoundedCornerShape(16.dp)).background(AruColors.Accent).clickable { onEdit(null) }.padding(horizontal = 22.dp, vertical = 14.dp))
        }
    }
    delFor?.let { a ->
        AlertDialog(
            onDismissRequest = { delFor = null }, title = { Text("Animeni o'chirish") }, text = { Text("«${a.optString("name")}» va uning hamma bo'limlari, qismlari o'chiriladi.") },
            confirmButton = { TextButton(onClick = { delFor = null; scope.launch { val r = api.delete("/api/anime/${a.optLong("id")}"); toast(if (r.ok) "O'chirildi" else api.errorOf(r, "O'chirilmadi")); load() } }) { Text("O'chirish", color = Color(0xFFF87171)) } },
            dismissButton = { TextButton(onClick = { delFor = null }) { Text("Bekor") } }, containerColor = AruColors.Surface,
        )
    }
}

/** Anime qo'shish / tahrirlash (rasm Telegram'ga yuklanadi, bazaga faqat fayl nomi yoziladi). */
@Composable
fun AdminAnimeEditScreen(initial: JSONObject?, onBack: () -> Unit, onDone: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val api = AppServices.admin
    var name by remember { mutableStateOf(initial?.optString("name").orEmpty()) }
    var davlat by remember { mutableStateOf(initial?.optString("davlat").orEmpty()) }
    var studiya by remember { mutableStateOf(initial?.optString("studiya").orEmpty()) }
    var janri by remember { mutableStateOf(initial?.optString("janri").orEmpty()) }
    var tavsif by remember { mutableStateOf(initial?.optString("tavsif").orEmpty()) }
    var photo by remember { mutableStateOf(fileNameOfUrl(initial?.optString("photo_url").orEmpty())) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { uri ->
        if (uri != null) scope.launch { busy = true; val (n, e) = uploadPhoto(ctx, uri, "anime"); busy = false; if (n != null) { photo = n; error = null } else error = "Rasm yuklashda xato: $e" }
    }
    OverlayScaffold(if (initial == null) "Anime qo'shish" else "Animeni tahrirlash", onBack) {
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            Box(Modifier.fillMaxWidth().height(160.dp).clip(card).background(AruColors.Card).clickable(enabled = !busy) { pick.launch("image/*") }, contentAlignment = Alignment.Center) {
                if (photo.isNotEmpty()) TgImage(photo, Modifier.fillMaxSize(), maxPx = 640) else Text("Rasm tanlash", color = AruColors.TextDim)
                if (busy) LinearProgressIndicator(color = AruColors.Accent2, modifier = Modifier.align(Alignment.BottomCenter).fillMaxWidth())
            }
            Spacer(Modifier.height(10.dp))
            Field(name, { name = it }, "Nomi"); Field(davlat, { davlat = it }, "Davlat"); Field(studiya, { studiya = it }, "Studiya"); Field(janri, { janri = it }, "Janri"); Field(tavsif, { tavsif = it }, "Tavsif", minLines = 4)
            error?.let { Text(it, color = Color(0xFFF87171), fontSize = 13.sp) }
            Spacer(Modifier.height(10.dp))
            Btn(if (busy) "Saqlanmoqda..." else "Saqlash", primary = true) {
                if (busy || name.isBlank()) return@Btn
                busy = true
                scope.launch {
                    val b = JSONObject().put("photo_url", photo).put("name", name).put("davlat", davlat).put("studiya", studiya).put("janri", janri).put("tavsif", tavsif)
                    val r = if (initial == null) api.post("/api/anime", b) else withContext(Dispatchers.IO) { uz.arumediatv.com.data.Http.client.callJson("${uz.arumediatv.com.BuildConfig.API_BASE}/api/anime/${initial.optLong("id")}", "PUT", b) }
                    busy = false
                    if (r.ok) onDone() else error = api.errorOf(r, "Saqlashda xato")
                }
            }
            Spacer(Modifier.height(40.dp))
        }
    }
}

// ══════════════════════ BO'LIMLAR ══════════════════════

/** Animening bo'limlari: ro'yxat, qo'shish, tahrirlash, o'chirish; bosilsa qismlar. */
@Composable
fun AdminSeasonsScreen(anime: JSONObject, onBack: () -> Unit, onOpenSeason: (JSONObject) -> Unit, onEdit: (JSONObject?) -> Unit) {
    val ctx = LocalContext.current
    val api = AppServices.admin
    val scope = rememberCoroutineScope()
    val animeId = anime.optLong("id")
    var items by remember { mutableStateOf<List<JSONObject>?>(null) }
    var delFor by remember { mutableStateOf<JSONObject?>(null) }
    fun toast(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_SHORT).show()
    suspend fun load() {
        val (code, text) = withContext(Dispatchers.IO) { uz.arumediatv.com.data.Http.client.callText("${uz.arumediatv.com.BuildConfig.API_BASE}/api/seasons/anime/$animeId") }
        items = if (code == 200) try { val a = JSONArray(text); (0 until a.length()).mapNotNull { a.optJSONObject(it) } } catch (_: Exception) { emptyList() } else emptyList()
    }
    LaunchedEffect(Unit) { load() }
    OverlayScaffold(anime.optString("name"), onBack) {
        Box(Modifier.fillMaxSize()) {
            val list = items
            when {
                list == null -> LoadingBox()
                list.isEmpty() -> EmptyState(text = "Bo'lim yo'q")
                else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 100.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    items(list, key = { it.optLong("season_id") }) { s ->
                        Row(Modifier.fillMaxWidth().clip(card).background(AruColors.Card).border(1.dp, AruColors.Border, card).clickable { onOpenSeason(s) }.padding(10.dp), verticalAlignment = Alignment.CenterVertically) {
                            TgImage(s.optString("photo_url"), Modifier.size(56.dp).clip(RoundedCornerShape(10.dp)).background(AruColors.CardAlt), maxPx = 200)
                            Spacer(Modifier.width(12.dp))
                            Column(Modifier.weight(1f)) {
                                Text(s.optString("nomi").ifEmpty { "${s.optInt("bolim_id")}-bo'lim" }, color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                Text("${s.optInt("bolim_id")}-bo'lim · ${s.optString("turi")} · ${s.optString("holati")}", color = AruColors.TextDim, fontSize = 12.sp, maxLines = 1)
                            }
                            Btn("Tahrir") { onEdit(s) }
                            Spacer(Modifier.width(6.dp))
                            Btn("O'chirish", danger = true) { delFor = s }
                        }
                    }
                }
            }
            Text("+ Bo'lim qo'shish", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 14.sp, modifier = Modifier.align(Alignment.BottomCenter).padding(16.dp).clip(RoundedCornerShape(16.dp)).background(AruColors.Accent).clickable { onEdit(null) }.padding(horizontal = 22.dp, vertical = 14.dp))
        }
    }
    delFor?.let { s ->
        AlertDialog(
            onDismissRequest = { delFor = null }, title = { Text("Bo'limni o'chirish") }, text = { Text("Bo'lim va uning hamma qismlari o'chiriladi.") },
            confirmButton = { TextButton(onClick = { delFor = null; scope.launch { val r = api.delete("/api/seasons/$animeId/${s.optLong("season_id")}"); toast(if (r.ok) "O'chirildi" else api.errorOf(r, "O'chirilmadi")); load() } }) { Text("O'chirish", color = Color(0xFFF87171)) } },
            dismissButton = { TextButton(onClick = { delFor = null }) { Text("Bekor") } }, containerColor = AruColors.Surface,
        )
    }
}

/** Bo'lim qo'shish / tahrirlash: raqam, nom, studio, tarjimon, yil, janrlar, tur, holat, yosh, tavsif, rasm. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AdminSeasonEditScreen(animeId: Long, initial: JSONObject?, onBack: () -> Unit, onDone: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val api = AppServices.admin
    var bolim by remember { mutableStateOf(initial?.optInt("bolim_id")?.takeIf { it > 0 }?.toString().orEmpty()) }
    var nomi by remember { mutableStateOf(initial?.optString("nomi").orEmpty()) }
    var studio by remember { mutableStateOf(initial?.optString("studio").orEmpty()) }
    var tarjimon by remember { mutableStateOf(initial?.optString("tarjimon").orEmpty()) }
    var yili by remember { mutableStateOf(initial?.optString("yili").orEmpty()) }
    var tavsif by remember { mutableStateOf(initial?.optString("tavsif").orEmpty()) }
    var davlat by remember { mutableStateOf(initial?.optString("davlat").orEmpty()) }
    var rejissyor by remember { mutableStateOf(initial?.optString("rejissyor").orEmpty()) }
    var yosh by remember { mutableStateOf(initial?.optInt("yosh")?.takeIf { it > 0 }?.toString().orEmpty()) }
    var turi by remember { mutableStateOf(initial?.optString("turi")?.takeIf { it in listOf("TV", "FILM", "OVA") } ?: "TV") }
    var holati by remember { mutableStateOf(initial?.optString("holati")?.takeIf { it in listOf("Davom etmoqda", "Tugallangan") } ?: "Davom etmoqda") }
    val genres = remember { mutableStateListOf<String>().also { l -> initial?.optString("janri").orEmpty().split(',').map { it.trim() }.filter { it.isNotEmpty() }.forEach(l::add) } }
    var photo by remember { mutableStateOf(fileNameOfUrl(initial?.optString("photo_url").orEmpty())) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { uri ->
        if (uri != null) scope.launch { busy = true; val (n, e) = uploadPhoto(ctx, uri, "season"); busy = false; if (n != null) { photo = n; error = null } else error = "Rasm yuklashda xato: $e" }
    }
    OverlayScaffold(if (initial == null) "Yangi bo'lim qo'shish" else "Bo'limni tahrirlash", onBack) {
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            Box(Modifier.fillMaxWidth().height(160.dp).clip(card).background(AruColors.Card).clickable(enabled = !busy) { pick.launch("image/*") }, contentAlignment = Alignment.Center) {
                if (photo.isNotEmpty()) TgImage(photo, Modifier.fillMaxSize(), maxPx = 640) else Text("Rasm tanlash", color = AruColors.TextDim)
                if (busy) LinearProgressIndicator(color = AruColors.Accent2, modifier = Modifier.align(Alignment.BottomCenter).fillMaxWidth())
            }
            Spacer(Modifier.height(10.dp))
            Field(bolim, { bolim = it.filter(Char::isDigit).take(4) }, "Bo'lim raqami (N-bo'lim)", number = true)
            Field(nomi, { nomi = it }, "Nomi"); Field(studio, { studio = it }, "Studio"); Field(tarjimon, { tarjimon = it }, "Tarjimon"); Field(yili, { yili = it.take(4) }, "Yili", number = true)
            Field(davlat, { davlat = it }, "Davlat"); Field(rejissyor, { rejissyor = it }, "Rejissyor"); Field(yosh, { yosh = it.filter(Char::isDigit).take(2) }, "Yosh chegarasi (masalan 13)", number = true)
            Text("Turi", color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.padding(top = 8.dp, bottom = 4.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) { listOf("TV", "FILM", "OVA").forEach { t -> Chip(t, turi == t) { turi = t } } }
            Text("Holati", color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.padding(top = 8.dp, bottom = 4.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) { listOf("Davom etmoqda", "Tugallangan").forEach { t -> Chip(t, holati == t) { holati = t } } }
            Text("Janrlar", color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.padding(top = 8.dp, bottom = 4.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) { GENRES.forEach { g -> Chip(g, g in genres) { if (g in genres) genres.remove(g) else genres.add(g) } } }
            Field(tavsif, { tavsif = it }, "Tavsif", minLines = 4)
            error?.let { Text(it, color = Color(0xFFF87171), fontSize = 13.sp) }
            Spacer(Modifier.height(10.dp))
            Btn(if (busy) "Saqlanmoqda..." else "Saqlash", primary = true) {
                if (busy) return@Btn
                busy = true; error = null
                scope.launch {
                    val g = genres.sorted()
                    val b = JSONObject().put("anime_id", animeId).put("bolim_id", bolim.toIntOrNull() ?: 0).put("photo_url", photo).put("nomi", nomi).put("studio", studio)
                        .put("tarjimon", tarjimon).put("yili", yili).put("janrlar", JSONArray(g)).put("janri", g.joinToString(", ")).put("turi", turi).put("holati", holati)
                        .put("tavsif", tavsif).put("davlat", davlat.trim()).put("rejissyor", rejissyor.trim()).put("yosh", yosh.trim().toIntOrNull() ?: 0)
                    val r = if (initial == null) api.post("/api/seasons", b) else withContext(Dispatchers.IO) { uz.arumediatv.com.data.Http.client.callJson("${uz.arumediatv.com.BuildConfig.API_BASE}/api/seasons/$animeId/${initial.optLong("season_id")}", "PUT", b) }
                    busy = false
                    if (r.ok) onDone() else error = api.errorOf(r, "Saqlashda xato")
                }
            }
            Spacer(Modifier.height(40.dp))
        }
    }
}

@Composable
private fun Chip(text: String, on: Boolean, onClick: () -> Unit) {
    Text(
        text, color = if (on) Color.White else AruColors.TextDim, fontSize = 12.sp,
        modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(if (on) AruColors.Accent else AruColors.CardAlt).clickable(onClick = onClick).padding(horizontal = 12.dp, vertical = 7.dp),
    )
}

// ══════════════════════ QISMLAR ══════════════════════

/** Bo'limning barcha qismlari (yashirinlari ham): qo'shish, tahrirlash, o'chirish, kanalga joylash. */
@Composable
fun AdminEpisodesScreen(animeId: Long, seasonId: Long, title: String, onBack: () -> Unit, onEdit: (JSONObject?) -> Unit) {
    val ctx = LocalContext.current
    val api = AppServices.admin
    val scope = rememberCoroutineScope()
    var items by remember { mutableStateOf<List<JSONObject>?>(null) }
    var delFor by remember { mutableStateOf<JSONObject?>(null) }
    var postFor by remember { mutableStateOf<JSONObject?>(null) }
    fun toast(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_SHORT).show()
    suspend fun load() {
        val (code, text) = withContext(Dispatchers.IO) { uz.arumediatv.com.data.Http.client.callText("${uz.arumediatv.com.BuildConfig.API_BASE}/api/epizods/$animeId/$seasonId?all=1", AppServices.auth.sessionToken) }
        items = if (code == 200) try { val a = JSONArray(text); (0 until a.length()).mapNotNull { a.optJSONObject(it) }.reversed() } catch (_: Exception) { emptyList() } else emptyList()
    }
    LaunchedEffect(Unit) { load() }
    OverlayScaffold(title, onBack) {
        Box(Modifier.fillMaxSize()) {
            val list = items
            when {
                list == null -> LoadingBox()
                list.isEmpty() -> EmptyState(text = "Qism yo'q")
                else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 100.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    items(list, key = { it.optLong("epizod_id") }) { e ->
                        val ep = Episode.from(e)
                        Column(Modifier.fillMaxWidth().clip(card).background(AruColors.Card).border(1.dp, AruColors.Border, card).padding(12.dp)) {
                            Text("${ep.number}-qism${if (ep.name.isNotBlank()) " — ${ep.name}" else ""}", color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold)
                            Text(ep.qualities.joinToString(" · ").ifEmpty { "Fayl yo'q" } + if (e.optString("origin_video").isNotEmpty()) " · asl video" else "", color = AruColors.TextDim, fontSize = 12.sp)
                            Row(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                Btn("Tahrir") { onEdit(e) }
                                Btn("Kanalga") { postFor = e }
                                Btn("O'chirish", danger = true) { delFor = e }
                            }
                        }
                    }
                }
            }
            Text("+ Qism qo'shish", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 14.sp, modifier = Modifier.align(Alignment.BottomCenter).padding(16.dp).clip(RoundedCornerShape(16.dp)).background(AruColors.Accent).clickable { onEdit(null) }.padding(horizontal = 22.dp, vertical = 14.dp))
        }
    }
    delFor?.let { e ->
        AlertDialog(
            onDismissRequest = { delFor = null }, title = { Text("Qismni o'chirish") }, text = { Text("${e.optInt("epizod_number")}-qism va barcha fayl havolalari o'chiriladi.") },
            confirmButton = { TextButton(onClick = { delFor = null; scope.launch { val r = api.delete("/api/epizods/$animeId/$seasonId/${e.optLong("epizod_id")}"); toast(if (r.ok) "O'chirildi" else api.errorOf(r, "O'chirilmadi")); load() } }) { Text("O'chirish", color = Color(0xFFF87171)) } },
            dismissButton = { TextButton(onClick = { delFor = null }) { Text("Bekor") } }, containerColor = AruColors.Surface,
        )
    }
    postFor?.let { e ->
        AlertDialog(
            onDismissRequest = { postFor = null }, title = { Text("Kanalga joylash") }, text = { Text("${e.optInt("epizod_number")}-qism kanalga post qilib joylansinmi?") },
            confirmButton = { TextButton(onClick = { postFor = null; scope.launch { val r = api.post("/api/epizods/$animeId/$seasonId/${e.optLong("epizod_id")}/announce"); toast(if (r.ok) "Joylandi" else api.errorOf(r, "Joylanmadi")) } }) { Text("Joylash") } },
            dismissButton = { TextButton(onClick = { postFor = null }) { Text("Yo'q") } }, containerColor = AruColors.Surface,
        )
    }
}

private fun normalizeIntro(raw: String): String {
    val t = raw.trim()
    if (t.isEmpty()) return ""
    if (t.contains(':')) return t
    val digits = t.filter { it.isDigit() }
    val n = digits.toLongOrNull() ?: return ""
    if (n == 0L) return ""
    if (digits.length <= 2) return "0:${digits.padStart(2, '0')}"
    return "${digits.substring(0, digits.length - 2)}:${digits.takeLast(2)}"
}

private fun introText(raw: String?): String {
    val t = raw?.trim().orEmpty()
    if (t.isEmpty() || t == "0" || t == "null") return ""
    if (t.contains(':')) return t
    val s = t.toIntOrNull() ?: return ""
    return if (s <= 0) "" else "${s / 60}:${(s % 60).toString().padStart(2, '0')}"
}

private class QState(val label: String, var url: String = "", var size: Long = 0, var key: String = "")

/** Qism qo'shish / tahrirlash: raqam, nom, asl video (kodlash navbatiga), tayyor sifatlar (360p–1080p), intro vaqtlari (5 ta). */
@Composable
fun AdminEpisodeEditScreen(animeId: Long, seasonId: Long, initial: JSONObject?, onBack: () -> Unit, onDone: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val api = AppServices.admin
    var number by remember { mutableStateOf(initial?.optInt("epizod_number")?.toString().orEmpty()) }
    var name by remember { mutableStateOf(initial?.optString("epizod_name").orEmpty().takeIf { it != "null" }.orEmpty()) }
    val intro = remember { List(10) { i -> mutableStateOf(introText(initial?.optString("intro_${i + 1}"))) } }
    val qs = remember {
        listOf("360p", "480p", "720p", "1080p").map { q ->
            QState(q, fileNameOfUrl(initial?.optString("url_$q").orEmpty()), initial?.optLong("size_$q") ?: 0)
        }
    }
    var tick by remember { mutableStateOf(0) }
    var origin by remember { mutableStateOf(initial?.optString("origin_video").orEmpty().takeIf { it != "null" }.orEmpty()) }
    var originNew by remember { mutableStateOf(false) }
    var upLabel by remember { mutableStateOf<String?>(null) }
    var upProgress by remember { mutableStateOf(0f) }
    var saving by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var job by remember { mutableStateOf<JSONObject?>(null) }
    fun toast(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_SHORT).show()
    val epId = initial?.optLong("epizod_id") ?: 0L

    LaunchedEffect(Unit) {
        if (epId > 0) {
            val r = api.get("/api/encode/status?anime_id=$animeId&season_id=$seasonId")
            val a = r.json?.optJSONArray("items")
            job = (0 until (a?.length() ?: 0)).mapNotNull { a?.optJSONObject(it) }.firstOrNull { it.optLong("epizod_id") == epId }
        }
    }

    var pending by remember { mutableStateOf<String?>(null) } // "origin" yoki sifat nomi
    val pickVideo = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { uri ->
        val target = pending
        if (uri == null || target == null) return@rememberLauncherForActivityResult
        if (!AppServices.telegram.state.value.authorized) { toast("Video Telegram orqali yuklanadi: Profil -> Telegram'ni ulash"); return@rememberLauncherForActivityResult }
        scope.launch {
            upLabel = if (target == "origin") "Asl video" else target; upProgress = 0f
            val uid = AppServices.auth.user.value?.id ?: 0L
            val res = withPickedFile(ctx, uri) { path, display, size ->
                val ext = display.substringAfterLast('.', "mp4").lowercase().ifEmpty { "mp4" }
                val mime = if (ext == "mkv") "video/x-matroska" else "video/mp4"
                val fname = if (target == "origin") "orig_${uid}_${animeId}_${seasonId}_${System.currentTimeMillis()}.$ext" else "ep_${animeId}_${seasonId}_${target}_${System.currentTimeMillis()}.$ext"
                val err = AppServices.telegram.uploadFile(path, fname, mime, onProgress = { s, t -> if (t > 0) upProgress = s.toFloat() / t })
                Triple(err, fname, size)
            }
            upLabel = null
            if (res == null) { toast("Faylni o'qib bo'lmadi"); return@launch }
            val (err, fname, size) = res
            if (err != null) { toast("Yuklanmadi — $err"); return@launch }
            if (target == "origin") { origin = fname; originNew = true; job = null; toast("Asl video yuklandi — «Saqlash»dan keyin kodlash navbatiga qo'yiladi") }
            else { qs.first { it.label == target }.apply { url = fname; this.size = size; key = AppServices.telegram.keyFor(fname) }; tick++; toast("$target yuklandi — endi «Saqlash»ni bosing") }
        }
    }

    OverlayScaffold(if (initial == null) "Qism qo'shish" else "Qismni tahrirlash", onBack) {
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            error?.let { Text(it, color = Color(0xFFF87171), fontSize = 13.sp, modifier = Modifier.padding(bottom = 8.dp)) }
            Field(number, { number = it.filter(Char::isDigit).take(5) }, "Qism raqami (0 dan boshlash)", number = true)
            Field(name, { name = it }, "Qism nomi (ixtiyoriy)")
            upLabel?.let { l -> Column(Modifier.padding(vertical = 8.dp)) { Text("$l yuklanmoqda ${(upProgress * 100).toInt()}%", color = AruColors.TextDim, fontSize = 12.sp); LinearProgressIndicator(progress = { upProgress }, color = AruColors.Accent2, trackColor = AruColors.CardAlt, modifier = Modifier.fillMaxWidth()) } }

            Text("ASL VIDEO (avto-kodlash)", color = AruColors.Accent2, fontSize = 12.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(top = 14.dp, bottom = 6.dp))
            Row(Modifier.fillMaxWidth().clip(card).background(AruColors.Card).padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(if (origin.isEmpty()) "Yuklanmagan" else origin, color = Color.White, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    job?.let { Text("Kodlash: ${it.optString("state")} ${it.optString("quality")}", color = AruColors.TextDim, fontSize = 11.sp) }
                }
                Btn(if (origin.isEmpty()) "Tanlash" else "Almashtirish") { pending = "origin"; pickVideo.launch("video/*") }
                if (origin.isNotEmpty() && epId > 0) {
                    Spacer(Modifier.width(6.dp))
                    Btn("O'chirish", danger = true) {
                        scope.launch {
                            val r = api.post("/api/encode/delete", JSONObject().put("anime_id", animeId).put("season_id", seasonId).put("epizod_id", epId).put("origin", origin))
                            if (r.ok) { origin = ""; originNew = false; job = null; toast("Asl video o'chirildi") } else toast(api.errorOf(r, "O'chirilmadi"))
                        }
                    }
                }
            }

            Text("TAYYOR SIFATLAR", color = AruColors.Accent2, fontSize = 12.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(top = 14.dp, bottom = 6.dp))
            qs.forEach { q ->
                key(tick) {
                    Row(Modifier.fillMaxWidth().padding(vertical = 4.dp).clip(card).background(AruColors.Card).padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text(q.label, color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold)
                            Text(if (q.url.isEmpty()) "Yo'q" else "${q.url} · ${formatBytes(q.size)}", color = AruColors.TextDim, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                        Btn(if (q.url.isEmpty()) "Yuklash" else "Almashtirish") { pending = q.label; pickVideo.launch("video/*") }
                        if (q.url.isNotEmpty()) {
                            Spacer(Modifier.width(6.dp))
                            Btn("O'chirish", danger = true) {
                                scope.launch {
                                    if (epId > 0) { val r = api.delete("/api/epizods/$animeId/$seasonId/$epId/${q.label}"); if (!r.ok) { toast(api.errorOf(r, "O'chirilmadi")); return@launch } }
                                    q.url = ""; q.size = 0; q.key = ""; tick++
                                }
                            }
                        }
                    }
                }
            }

            Text("INTRO VAQTLARI (masalan 1:30 yoki 130)", color = AruColors.Accent2, fontSize = 12.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(top = 14.dp, bottom = 6.dp))
            for (row in 0 until 5) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Field(intro[row * 2].value, { intro[row * 2].value = it.take(8) }, "${row + 1}-boshi", Modifier.weight(1f))
                    Field(intro[row * 2 + 1].value, { intro[row * 2 + 1].value = it.take(8) }, "${row + 1}-oxiri", Modifier.weight(1f))
                }
            }
            Spacer(Modifier.height(14.dp))
            Btn(if (saving) "Saqlanmoqda..." else "Saqlash", primary = true) {
                if (saving) return@Btn
                saving = true; error = null
                scope.launch {
                    val b = JSONObject().put("anime_id", animeId).put("season_id", seasonId).put("epizod_number", number.toIntOrNull() ?: 0).put("epizod_name", name)
                    qs.forEach { q -> b.put("url_${q.label}", q.url).put("size_${q.label}", q.size).put("key_${q.label}", q.key) }
                    intro.forEachIndexed { i, s -> b.put("intro_${i + 1}", normalizeIntro(s.value)) }
                    val r = if (initial != null) withContext(Dispatchers.IO) { uz.arumediatv.com.data.Http.client.callJson("${uz.arumediatv.com.BuildConfig.API_BASE}/api/epizods/$animeId/$seasonId/$epId", "PUT", b) } else api.post("/api/epizods", b)
                    if (r.ok) {
                        if (originNew && origin.isNotEmpty()) {
                            val id = if (epId > 0) epId else r.json?.optLong("epizod_id") ?: 0L
                            val q = api.post("/api/encode/queue", JSONObject().put("anime_id", animeId).put("season_id", seasonId).put("epizod_id", id).put("origin", origin).put("key", AppServices.telegram.keyFor(origin)))
                            toast(if (q.ok) "Kodlash navbatiga qo'yildi" else "Qism saqlandi, lekin navbatga qo'yilmadi: ${api.errorOf(q, "xato")}")
                        }
                        saving = false
                        onDone()
                    } else { saving = false; error = api.errorOf(r, "Saqlashda xato") }
                }
            }
            Spacer(Modifier.height(40.dp))
        }
    }
}

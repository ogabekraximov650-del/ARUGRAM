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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.imePadding
import androidx.compose.ui.graphics.vector.ImageVector
import uz.arumediatv.com.ui.common.AruIcons
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
import uz.arumediatv.com.data.callJson
import uz.arumediatv.com.data.callText
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

// ══════════════════════ BOSHQARISH OYNALARI (Flutter: anime/season/epizod_management_screen) ══════════════════════

/** Ro'yxatlar diskda saqlanadi: ekran darhol to'ladi, keyin yangilanadi (Flutter `DiskCache` bilan bir xil g'oya). */
private object AdminListCache {
    private fun file(ctx: Context, key: String) = File(ctx.filesDir, "admin_cache_${key.replace(Regex("[^A-Za-z0-9_]"), "_")}.json")
    fun read(ctx: Context, key: String): List<JSONObject>? = try {
        val f = file(ctx, key)
        if (!f.exists()) null else { val a = JSONArray(f.readText()); (0 until a.length()).mapNotNull { a.optJSONObject(it) } }
    } catch (_: Exception) { null }
    fun write(ctx: Context, key: String, text: String) { runCatching { file(ctx, key).writeText(text) } }
}

private fun parseRows(text: String): List<JSONObject> = try { val a = JSONArray(text); (0 until a.length()).mapNotNull { a.optJSONObject(it) } } catch (_: Exception) { emptyList() }

private val glassRow = RoundedCornerShape(18.dp)
private val white10 = Color(0x1AFFFFFF)
private val white50 = Color(0x80FFFFFF)

/** Flutter bilan bir xil oyna: tepada yumaloq sarlavha kartasi (orqaga + o'ziga xos tarkib), pastki o'ngda "+" tugmasi, pastda xabar. */
@Composable
private fun MgmtScaffold(onBack: () -> Unit, onAdd: () -> Unit, snack: SnackbarHostState, snackOk: Boolean = false, header: @Composable RowScope.() -> Unit, content: @Composable BoxScope.() -> Unit) {
    BackHandler(onBack = onBack)
    val hs = RoundedCornerShape(20.dp)
    val bs = RoundedCornerShape(14.dp)
    Column(Modifier.fillMaxSize().background(AruColors.Bg).statusBarsPadding().imePadding()) {
        Row(
            Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 8.dp).fillMaxWidth().clip(hs).background(AruColors.Card).border(1.dp, AruColors.Border, hs).padding(horizontal = 14.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(Modifier.clip(bs).background(AruColors.CardAlt).border(1.dp, AruColors.Border, bs).clickable(onClick = onBack).padding(8.dp)) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, "Orqaga", tint = Color.White, modifier = Modifier.size(24.dp))
            }
            Spacer(Modifier.width(12.dp))
            header()
        }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            content()
            val fab = RoundedCornerShape(18.dp)
            Box(
                Modifier.align(Alignment.BottomEnd).navigationBarsPadding().padding(end = 16.dp, bottom = 16.dp).size(56.dp).clip(fab).background(Color(0x2EFFFFFF)).border(1.dp, AruColors.Border, fab).clickable(onClick = onAdd),
                contentAlignment = Alignment.Center,
            ) { Icon(Icons.Filled.Add, "Qo'shish", tint = Color.White, modifier = Modifier.size(28.dp)) }
            SnackbarHost(snack, Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(start = 16.dp, end = 88.dp, bottom = 16.dp)) { d ->
                Snackbar(d, containerColor = if (snackOk) Color(0xCC2E7D32) else AruColors.CardAlt, contentColor = Color.White)
            }
        }
    }
}

@Composable
private fun MgmtLoading() {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator(color = Color(0x99FFFFFF), strokeWidth = 3.dp) }
}

@Composable
private fun MgmtEmpty(icon: ImageVector, title: String, hint: String? = null) {
    Column(Modifier.fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
        Icon(icon, null, tint = Color(0x3DFFFFFF), modifier = Modifier.size(52.dp))
        Spacer(Modifier.height(12.dp))
        Text(title, color = Color(0x73FFFFFF), fontSize = 14.sp)
        if (hint != null) { Spacer(Modifier.height(4.dp)); Text(hint, color = Color(0x4DFFFFFF), fontSize = 12.sp) }
    }
}

@Composable
private fun MgmtPhoto(url: String, size: Int, radius: Int, px: Int) {
    Box(Modifier.size(size.dp).clip(RoundedCornerShape(radius.dp)).background(white10), contentAlignment = Alignment.Center) {
        if (url.isNotEmpty() && url != "null") TgImage(url, Modifier.fillMaxSize(), maxPx = px)
        else Icon(AruIcons.Movie, null, tint = Color(0x8AFFFFFF), modifier = Modifier.size((size / 3).dp))
    }
}

@Composable
private fun MgmtIcon(icon: ImageVector, tint: Color, size: Int = 20, onClick: () -> Unit) {
    IconButton(onClick = onClick) { Icon(icon, null, tint = tint, modifier = Modifier.size(size.dp)) }
}

/** Flutter `AlertDialog(backgroundColor: cardAlt, 20 radius)` bilan bir xil tasdiqlash oynasi. */
@Composable
private fun ConfirmDialog(title: String, text: String, confirm: String, danger: Boolean, onResult: (Boolean) -> Unit) {
    AlertDialog(
        onDismissRequest = { onResult(false) }, containerColor = AruColors.CardAlt, shape = RoundedCornerShape(20.dp),
        title = { Text(title, color = Color.White) }, text = { Text(text, color = Color(0xB3FFFFFF)) },
        dismissButton = { TextButton(onClick = { onResult(false) }) { Text("Yo'q", color = white50) } },
        confirmButton = { Button(onClick = { onResult(true) }, colors = ButtonDefaults.buttonColors(containerColor = if (danger) Color(0xFFF44336) else AruColors.Accent, contentColor = Color.White)) { Text(confirm) } },
    )
}

/** Animelarni boshqarish: ro'yxat (diskdan darhol), qo'shish, tahrirlash, o'chirish; bosilsa bo'limlar ochiladi. */
@Composable
fun AdminAnimeScreen(onBack: () -> Unit, onOpenAnime: (JSONObject) -> Unit, onEdit: (JSONObject?) -> Unit) {
    val ctx = LocalContext.current
    val api = AppServices.admin
    val scope = rememberCoroutineScope()
    val snack = remember { SnackbarHostState() }
    var snackOk by remember { mutableStateOf(false) }
    var items by remember { mutableStateOf(AdminListCache.read(ctx, "admin_anime") ?: emptyList()) }
    var loading by remember { mutableStateOf(items.isEmpty()) }
    var error by remember { mutableStateOf<String?>(null) }
    var delFor by remember { mutableStateOf<JSONObject?>(null) }
    suspend fun load() {
        loading = items.isEmpty(); error = null
        val (code, text) = try { withContext(Dispatchers.IO) { uz.arumediatv.com.data.Http.client.callText("${uz.arumediatv.com.BuildConfig.API_BASE}/api/anime") } } catch (e: Exception) { -1 to (e.message ?: "") }
        if (code == 200) { items = parseRows(text); AdminListCache.write(ctx, "admin_anime", text) } else error = if (code < 0) "Xato: $text" else "Xato: Animelar yuklab olib bo'lmadi"
        loading = false
    }
    LaunchedEffect(Unit) { load() }
    MgmtScaffold(onBack, { onEdit(null) }, snack, snackOk, header = {
        Text("Animelarni boshqarish", color = Color.White, fontSize = 20.sp, fontWeight = FontWeight.Bold, modifier = Modifier.weight(1f))
    }) {
        when {
            loading -> MgmtLoading()
            error != null -> Column(Modifier.fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
                Icon(AruIcons.ErrorOutline, null, tint = Color(0xFFF44336), modifier = Modifier.size(48.dp))
                Spacer(Modifier.height(12.dp)); Text(error!!, color = Color(0xB3FFFFFF), fontSize = 14.sp); Spacer(Modifier.height(16.dp))
                Button(onClick = { scope.launch { load() } }, colors = ButtonDefaults.buttonColors(containerColor = AruColors.Accent, contentColor = Color.White)) { Text("Qayta urinish") }
            }
            items.isEmpty() -> MgmtEmpty(AruIcons.Movie, "Hali anime qo'shilmagan")
            else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 120.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                items(items, key = { it.optLong("id") }) { a ->
                    Row(Modifier.fillMaxWidth().clip(glassRow).background(AruColors.Card).border(1.dp, AruColors.Border, glassRow).clickable { onOpenAnime(a) }.padding(horizontal = 12.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                        MgmtPhoto(a.optString("photo_url"), 76, 14, 228)
                        Spacer(Modifier.width(14.dp))
                        Column(Modifier.weight(1f)) {
                            Text(a.optString("name"), color = Color.White, fontWeight = FontWeight.SemiBold, fontSize = 15.sp)
                            Text(a.optString("janri").takeIf { it != "null" }.orEmpty(), color = white50, fontSize = 12.sp)
                        }
                        MgmtIcon(Icons.Filled.Edit, Color(0x8AFFFFFF)) { onEdit(a) }
                        MgmtIcon(AruIcons.DeleteOutline, Color(0xFFFF5252)) { delFor = a }
                    }
                }
            }
        }
    }
    delFor?.let { a ->
        ConfirmDialog("Animeni o'chirish?", "\"${a.optString("name")}\" animeni rostanxam o'chirmoqchisiz?", "Ha, o'chirish", danger = true) { yes ->
            delFor = null
            if (yes) scope.launch {
                val r = api.delete("/api/anime/${a.optLong("id")}")
                snackOk = r.ok
                if (r.ok) load()
                snack.showSnackbar(if (r.ok) "Anime o'chirildi" else "Xato: ${api.errorOf(r, "O'chirishda xato")}")
            }
        }
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

/** Bitta animening bo'limlarini boshqarish: tepada anime rasmi va nomi doim ko'rinadi; bosilsa qismlar ochiladi. */
@Composable
fun AdminSeasonsScreen(anime: JSONObject, onBack: () -> Unit, onOpenSeason: (JSONObject) -> Unit, onEdit: (JSONObject?) -> Unit) {
    val ctx = LocalContext.current
    val api = AppServices.admin
    val scope = rememberCoroutineScope()
    val animeId = anime.optLong("id")
    val snack = remember { SnackbarHostState() }
    val key = "admin_seasons_$animeId"
    var items by remember { mutableStateOf(AdminListCache.read(ctx, key) ?: emptyList()) }
    var loading by remember { mutableStateOf(items.isEmpty()) }
    var error by remember { mutableStateOf<String?>(null) }
    var delFor by remember { mutableStateOf<JSONObject?>(null) }
    suspend fun load() {
        loading = items.isEmpty(); error = null
        val (code, text) = try { withContext(Dispatchers.IO) { uz.arumediatv.com.data.Http.client.callText("${uz.arumediatv.com.BuildConfig.API_BASE}/api/seasons/anime/$animeId") } } catch (e: Exception) { -1 to (e.message ?: "") }
        if (code == 200) { items = parseRows(text); AdminListCache.write(ctx, key, text) } else error = if (code < 0) "Xato: $text" else "Xato: Bo'limlar yuklab olib bo'lmadi"
        loading = false
    }
    LaunchedEffect(Unit) { load() }
    MgmtScaffold(onBack, { onEdit(null) }, snack, header = {
        MgmtPhoto(anime.optString("photo_url"), 48, 12, 144)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(anime.optString("name"), color = Color.White, fontSize = 17.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text("Bo'limni boshqarish", color = white50, fontSize = 12.sp)
        }
    }) {
        when {
            loading -> MgmtLoading()
            error != null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { Text(error!!, color = Color(0xB3FFFFFF), fontSize = 14.sp) }
            items.isEmpty() -> MgmtEmpty(AruIcons.VideoLibrary, "Hali bo'lim qo'shilmagan", "Pastdagi + tugmasi orqali qo'sh")
            else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 120.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                items(items, key = { it.optLong("season_id") }) { s ->
                    val bolim = s.optInt("bolim_id").takeIf { it > 0 } ?: s.optInt("season_id")
                    Row(Modifier.fillMaxWidth().clip(glassRow).background(AruColors.Card).border(1.dp, AruColors.Border, glassRow).clickable { onOpenSeason(s) }.padding(horizontal = 12.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                        MgmtPhoto(s.optString("photo_url"), 64, 12, 192)
                        Spacer(Modifier.width(14.dp))
                        Column(Modifier.weight(1f)) {
                            Text("$bolim-bo'lim — ${s.optString("nomi").takeIf { it != "null" }.orEmpty()}", color = Color.White, fontWeight = FontWeight.SemiBold, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Spacer(Modifier.height(3.dp))
                            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                                listOf(s.optString("turi"), s.optString("holati")).filter { it.isNotEmpty() && it != "null" }.forEach { t ->
                                    Text(t, color = AruColors.Accent, fontSize = 10.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.clip(RoundedCornerShape(6.dp)).background(AruColors.Accent.copy(alpha = 0.18f)).padding(horizontal = 6.dp, vertical = 2.dp))
                                }
                            }
                            Spacer(Modifier.height(2.dp))
                            Text("Epizodlarni ko'rish →", color = AruColors.Accent.copy(alpha = 0.8f), fontSize = 11.sp)
                        }
                        MgmtIcon(Icons.Filled.Edit, Color(0x8AFFFFFF)) { onEdit(s) }
                        MgmtIcon(AruIcons.DeleteOutline, Color(0xFFFF5252)) { delFor = s }
                    }
                }
            }
        }
    }
    delFor?.let { s ->
        ConfirmDialog("Bo'limni o'chirish?", "\"${s.optString("nomi").takeIf { it != "null" }.orEmpty()}\" bo'limini rostanxam o'chirmoqchisiz?", "Ha, o'chirish", danger = true) { yes ->
            delFor = null
            if (yes) scope.launch {
                val r = api.delete("/api/seasons/$animeId/${s.optLong("season_id")}")
                if (r.ok) load() else snack.showSnackbar("Xato: ${api.errorOf(r, "O'chirishda xato")}")
            }
        }
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

/** Epizodlarni boshqarish (faqat admin): hamma qism (yashirinlari ham); karta bosilsa tahrirlash oynasi. */
@Composable
fun AdminEpisodesScreen(anime: JSONObject, season: JSONObject, onBack: () -> Unit, onEdit: (JSONObject?) -> Unit) {
    val ctx = LocalContext.current
    val api = AppServices.admin
    val scope = rememberCoroutineScope()
    val animeId = anime.optLong("id")
    val seasonId = season.optLong("season_id")
    val snack = remember { SnackbarHostState() }
    val key = "eps_${animeId}_$seasonId"
    var items by remember { mutableStateOf(AdminListCache.read(ctx, key)?.reversed() ?: emptyList()) }
    var loading by remember { mutableStateOf(items.isEmpty()) }
    var error by remember { mutableStateOf<String?>(null) }
    var delFor by remember { mutableStateOf<JSONObject?>(null) }
    var postFor by remember { mutableStateOf<JSONObject?>(null) }
    suspend fun load() {
        loading = items.isEmpty(); error = null
        val (code, text) = try { withContext(Dispatchers.IO) { uz.arumediatv.com.data.Http.client.callText("${uz.arumediatv.com.BuildConfig.API_BASE}/api/epizods/$animeId/$seasonId?all=1", AppServices.auth.sessionToken) } } catch (e: Exception) { -1 to (e.message ?: "") }
        if (code == 200) { items = parseRows(text).reversed(); AdminListCache.write(ctx, key, text) } else error = if (code < 0) "Xato: $text" else "Xato: Epizodlar yuklab olib bo'lmadi"
        loading = false
    }
    LaunchedEffect(Unit) { load() }
    val bolimId = season.optInt("bolim_id").takeIf { it > 0 } ?: season.optInt("season_id")
    val seasonName = season.optString("nomi").takeIf { it != "null" }.orEmpty()
    MgmtScaffold(onBack, { onEdit(null) }, snack, header = {
        MgmtPhoto(season.optString("photo_url"), 48, 12, 144)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text("$bolimId-bo'lim${if (seasonName.isNotEmpty()) " — $seasonName" else ""}", color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text("Epizodlarni boshqarish", color = white50, fontSize = 12.sp)
        }
    }) {
        when {
            loading -> MgmtLoading()
            error != null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { Text(error!!, color = Color(0xB3FFFFFF), fontSize = 14.sp) }
            items.isEmpty() -> MgmtEmpty(AruIcons.PlayCircleOutline, "Hali epizod qo'shilmagan", "Pastdagi + tugmasi orqali qo'sh")
            else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 120.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                items(items, key = { it.optLong("epizod_id") }) { e ->
                    val num = if (e.has("epizod_number")) e.optInt("epizod_number").toString() else "?"
                    val name = e.optString("epizod_name").takeIf { it != "null" }.orEmpty()
                    val shape = RoundedCornerShape(16.dp)
                    Row(Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).clickable { onEdit(e) }.padding(horizontal = 14.dp, vertical = 14.dp), verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.size(44.dp).clip(RoundedCornerShape(12.dp)).background(AruColors.Accent.copy(alpha = 0.12f)).border(1.dp, AruColors.Accent.copy(alpha = 0.25f), RoundedCornerShape(12.dp)), contentAlignment = Alignment.Center) {
                            Text(num, color = AruColors.Accent, fontWeight = FontWeight.ExtraBold, fontSize = 15.sp)
                        }
                        Spacer(Modifier.width(14.dp))
                        Column(Modifier.weight(1f)) {
                            Text(name.ifEmpty { "Epizod $num" }, color = Color.White, fontWeight = FontWeight.SemiBold, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Spacer(Modifier.height(3.dp))
                            Text(qualitiesLine(e), color = white50, fontSize = 11.sp)
                        }
                        MgmtIcon(Icons.AutoMirrored.Filled.Send, Color(0x8AFFFFFF)) { postFor = e }
                        Icon(Icons.Filled.Edit, null, tint = Color(0x62FFFFFF), modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(4.dp))
                        MgmtIcon(AruIcons.DeleteOutline, Color(0xFFFF5252)) { delFor = e }
                    }
                }
            }
        }
    }
    delFor?.let { e ->
        val label = "Ep ${if (e.has("epizod_number")) e.optInt("epizod_number").toString() else "?"}${e.optString("epizod_name").takeIf { it.isNotEmpty() && it != "null" }?.let { " — $it" }.orEmpty()}"
        ConfirmDialog("Epizodni o'chirish?", "\"$label\" epizodini rostanxam o'chirmoqchisiz?\nBarcha fayl linklari ham o'chadi.", "Ha, o'chirish", danger = true) { yes ->
            delFor = null
            if (yes) scope.launch {
                val r = api.delete("/api/epizods/$animeId/$seasonId/${e.optLong("epizod_id")}")
                if (r.ok) load() else snack.showSnackbar("Xato: ${api.errorOf(r, "O'chirishda xato")}")
            }
        }
    }
    postFor?.let { e ->
        ConfirmDialog("Kanalga joylash?", "${if (e.has("epizod_number")) e.optInt("epizod_number").toString() else "?"}-qism kanalga post qilib joylansinmi?", "Ha, joylash", danger = false) { yes ->
            postFor = null
            if (yes) scope.launch {
                val r = api.post("/api/epizods/$animeId/$seasonId/${e.optLong("epizod_id")}/announce")
                snack.showSnackbar(if (r.ok) "Kanalga joylandi" else "Joylashda xato (${r.code})")
            }
        }
    }
}

/** Flutter `_AdminEpizodCard._qualities` bilan bir xil: kodlanguncha ASL video "Original — tayyorlanmoqda". */
private fun qualitiesLine(e: JSONObject): String {
    val o = e.optString("origin_video").takeIf { it != "null" }.orEmpty()
    fun isOrigin(u: String) = o.isNotEmpty() && (u == o || u.endsWith("/$o"))
    val q = mutableListOf<String>()
    for (k in listOf("360p", "480p", "720p", "1080p")) {
        val u = e.optString("url_$k").takeIf { it != "null" }.orEmpty()
        if (u.isNotEmpty() && !isOrigin(u)) q.add(k)
    }
    if (o.isNotEmpty()) q.add("Original \u2014 tayyorlanmoqda")
    return if (q.isEmpty()) "Fayl yo'q" else q.joinToString(" \u2022 ")
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

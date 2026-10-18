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
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.ui.text.TextStyle
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

// ══════════════════════ QO'SHISH / TAHRIRLASH OYNALARI (Flutter: add_anime / add_season / add_epizod_screen) ══════════════════════

private val white60 = Color(0x99FFFFFF)

private fun jstr(o: JSONObject?, key: String): String = o?.optString(key).orEmpty().let { if (it == "null") "" else it }

/** Sarlavha kartasi (orqaga + nom) va aylantiriladigan forma (padding 16/8/16/32) — Flutter bilan bir xil. */
@Composable
private fun FormScaffold(title: String, titleSp: Int, onBack: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    BackHandler(onBack = onBack)
    val hs = RoundedCornerShape(20.dp)
    val bs = RoundedCornerShape(14.dp)
    Column(Modifier.fillMaxSize().background(AruColors.Bg).statusBarsPadding().imePadding()) {
        Row(
            Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 8.dp).fillMaxWidth().clip(hs).background(AruColors.Card).border(1.dp, AruColors.Border, hs).padding(horizontal = 18.dp, vertical = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(Modifier.clip(bs).background(AruColors.CardAlt).border(1.dp, AruColors.Border, bs).clickable(onClick = onBack).padding(8.dp)) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, "Orqaga", tint = Color.White, modifier = Modifier.size(24.dp))
            }
            Spacer(Modifier.width(14.dp))
            Text(title, color = Color.White, fontSize = titleSp.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Column(Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()).padding(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 32.dp), content = content)
    }
}

@Composable
private fun GlassCard(radius: Int, padding: PaddingValues, modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    val sh = RoundedCornerShape(radius.dp)
    Column(modifier.fillMaxWidth().clip(sh).background(AruColors.Card).border(1.dp, AruColors.Border, sh).padding(padding), content = content)
}

/** Rasm tanlash kartasi: 20 yumaloq karta, ichida 16 yumaloq oq 8% quti (balandlik 200/180). */
@Composable
private fun PhotoBox(height: Int, photo: String, busy: Boolean, onClick: () -> Unit) {
    GlassCard(20, PaddingValues(16.dp), Modifier.clickable(enabled = !busy, onClick = onClick)) {
        Box(Modifier.fillMaxWidth().height(height.dp).clip(RoundedCornerShape(16.dp)).background(Color(0x14FFFFFF)), contentAlignment = Alignment.Center) {
            if (photo.isNotEmpty()) TgImage(photo, Modifier.fillMaxSize(), maxPx = 1080)
            else Column(horizontalAlignment = Alignment.CenterHorizontally) {
                Icon(AruIcons.ImageOutline, null, tint = Color(0x8AFFFFFF), modifier = Modifier.size(if (height >= 200) 48.dp else 44.dp))
                Spacer(Modifier.height(8.dp))
                Text("Rasm tanlang (ixtiyoriy)", color = white60, fontSize = 14.sp)
            }
            if (busy) LinearProgressIndicator(color = AruColors.Accent, trackColor = Color(0x1AFFFFFF), modifier = Modifier.align(Alignment.BottomCenter).fillMaxWidth())
        }
    }
}

@Composable
private fun ErrorBox(msg: String) {
    val sh = RoundedCornerShape(12.dp)
    Text(msg, color = Color(0xFFFF5252), fontSize = 12.sp, modifier = Modifier.fillMaxWidth().clip(sh).background(Color(0x33F44336)).border(1.dp, Color(0x80F44336), sh).padding(12.dp))
}

/** Prefiks ikonkali, hint matnli glass maydon (Flutter `_buildTextField`). */
@Composable
private fun GlassField(value: String, onChange: (String) -> Unit, hint: String, icon: ImageVector, number: Boolean = false, maxLines: Int = 1, trailing: (@Composable () -> Unit)? = null, modifier: Modifier = Modifier) {
    val sh = RoundedCornerShape(14.dp)
    Box(modifier.fillMaxWidth().clip(sh).background(AruColors.Card).border(1.dp, AruColors.Border, sh)) {
        TextField(
            value = value, onValueChange = onChange, modifier = Modifier.fillMaxWidth(),
            placeholder = { Text(hint, color = Color(0x66FFFFFF)) },
            leadingIcon = { Icon(icon, null, tint = Color(0x8AFFFFFF)) },
            trailingIcon = trailing,
            singleLine = maxLines == 1, minLines = maxLines, maxLines = maxLines,
            textStyle = TextStyle(color = Color.White, fontSize = 16.sp),
            keyboardOptions = KeyboardOptions(keyboardType = if (number) KeyboardType.Number else KeyboardType.Text),
            colors = TextFieldDefaults.colors(
                focusedContainerColor = Color.Transparent, unfocusedContainerColor = Color.Transparent, disabledContainerColor = Color.Transparent,
                focusedIndicatorColor = Color.Transparent, unfocusedIndicatorColor = Color.Transparent, disabledIndicatorColor = Color.Transparent,
                cursorColor = Color.White, focusedTextColor = Color.White, unfocusedTextColor = Color.White,
            ),
        )
    }
}

@Composable
private fun SectionLabel(text: String) {
    Text(text, color = white60, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.fillMaxWidth())
}

/** "Bekor" va "Saqlash" tugmalari yonma-yon (har biri 14 yumaloq). */
@Composable
private fun FormButtons(cancel: String, ok: String, busy: Boolean, onCancel: () -> Unit, onOk: () -> Unit) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Button(
            onClick = onCancel, enabled = !busy, modifier = Modifier.weight(1f).height(44.dp), shape = RoundedCornerShape(14.dp),
            colors = ButtonDefaults.buttonColors(containerColor = Color(0x1AFFFFFF), contentColor = Color.White, disabledContainerColor = Color(0x0DFFFFFF), disabledContentColor = white50),
        ) { Text(cancel) }
        Button(
            onClick = onOk, enabled = !busy, modifier = Modifier.weight(1f).height(44.dp), shape = RoundedCornerShape(14.dp),
            colors = ButtonDefaults.buttonColors(containerColor = AruColors.Accent, contentColor = Color.White, disabledContainerColor = AruColors.Accent.copy(alpha = 0.5f), disabledContentColor = Color.White),
        ) { if (busy) CircularProgressIndicator(color = Color.White, strokeWidth = 2.dp, modifier = Modifier.size(20.dp)) else Text(ok) }
    }
}

/** Anime qo'shish / tahrirlash (rasm Telegram'ga yuklanadi, bazaga faqat fayl nomi yoziladi). Hech qaysi maydon majburiy emas. */
@Composable
fun AdminAnimeEditScreen(initial: JSONObject?, onBack: () -> Unit, onDone: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val api = AppServices.admin
    var name by remember { mutableStateOf(jstr(initial, "name")) }
    var davlat by remember { mutableStateOf(jstr(initial, "davlat")) }
    var studiya by remember { mutableStateOf(jstr(initial, "studiya")) }
    var janri by remember { mutableStateOf(jstr(initial, "janri")) }
    var tavsif by remember { mutableStateOf(jstr(initial, "tavsif")) }
    var photo by remember { mutableStateOf(fileNameOfUrl(jstr(initial, "photo_url"))) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { uri ->
        if (uri != null) scope.launch { busy = true; val (n, e) = uploadPhoto(ctx, uri, "anime"); busy = false; if (n != null) { photo = n; error = null } else error = "Rasm yuklashda xato: $e" }
    }
    FormScaffold(if (initial == null) "Anime qo'shish" else "Animeni tahrirlash", 20, onBack) {
        PhotoBox(200, photo, busy) { pick.launch("image/*") }
        Spacer(Modifier.height(16.dp))
        error?.let { ErrorBox(it); Spacer(Modifier.height(16.dp)) }
        GlassField(name, { name = it }, "Anime nomi (ixtiyoriy)", AruIcons.Movie); Spacer(Modifier.height(12.dp))
        GlassField(davlat, { davlat = it }, "Davlat (ixtiyoriy)", AruIcons.LocationOn); Spacer(Modifier.height(12.dp))
        GlassField(studiya, { studiya = it }, "Studiya (ixtiyoriy)", AruIcons.Business); Spacer(Modifier.height(12.dp))
        GlassField(janri, { janri = it }, "Janri (ixtiyoriy)", AruIcons.LabelOutline); Spacer(Modifier.height(12.dp))
        GlassField(tavsif, { tavsif = it }, "Tavsif (ixtiyoriy)", AruIcons.DescriptionOutline, maxLines = 4)
        Spacer(Modifier.height(24.dp))
        FormButtons("Bekor", "Saqlash", busy, onBack) {
            busy = true; error = null
            scope.launch {
                val b = JSONObject().put("photo_url", photo).put("name", name).put("davlat", davlat).put("studiya", studiya).put("janri", janri).put("tavsif", tavsif)
                val r = if (initial == null) api.post("/api/anime", b) else withContext(Dispatchers.IO) { uz.arumediatv.com.data.Http.client.callJson("${uz.arumediatv.com.BuildConfig.API_BASE}/api/anime/${initial.optLong("id")}", "PUT", b) }
                busy = false
                if (r.ok) onDone() else error = "Xato: ${api.errorOf(r, "Saqlashda xato")}"
            }
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

/** Bo'lim qo'shish / tahrirlash: raqam, nom, studiya, tarjimon, davlat, rejissyor, yil, yosh, janrlar, tur, holat, tavsif, rasm. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AdminSeasonEditScreen(animeId: Long, initial: JSONObject?, onBack: () -> Unit, onDone: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val api = AppServices.admin
    var bolim by remember { mutableStateOf(initial?.optInt("bolim_id")?.takeIf { it > 0 }?.toString().orEmpty()) }
    var nomi by remember { mutableStateOf(jstr(initial, "nomi")) }
    var studio by remember { mutableStateOf(jstr(initial, "studio")) }
    var tarjimon by remember { mutableStateOf(jstr(initial, "tarjimon")) }
    var yili by remember { mutableStateOf(jstr(initial, "yili")) }
    var tavsif by remember { mutableStateOf(jstr(initial, "tavsif")) }
    var davlat by remember { mutableStateOf(jstr(initial, "davlat")) }
    var rejissyor by remember { mutableStateOf(jstr(initial, "rejissyor")) }
    var yosh by remember { mutableStateOf(initial?.optInt("yosh")?.takeIf { it > 0 }?.toString().orEmpty()) }
    var turi by remember { mutableStateOf(jstr(initial, "turi").takeIf { it in listOf("TV", "FILM", "OVA") } ?: "TV") }
    var holati by remember { mutableStateOf(jstr(initial, "holati").takeIf { it in listOf("Davom etmoqda", "Tugallangan") } ?: "Davom etmoqda") }
    val genres = remember { mutableStateListOf<String>().also { l -> jstr(initial, "janri").split(',').map { it.trim() }.filter { it.isNotEmpty() }.forEach { if (it !in l) l.add(it) } } }
    var photo by remember { mutableStateOf(fileNameOfUrl(jstr(initial, "photo_url"))) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { uri ->
        if (uri != null) scope.launch { busy = true; val (n, e) = uploadPhoto(ctx, uri, "season"); busy = false; if (n != null) { photo = n; error = null } else error = "Rasm yuklashda xato: $e" }
    }
    FormScaffold(if (initial == null) "Yangi bo'lim qo'shish" else "Bo'limni tahrirlash", 19, onBack) {
        PhotoBox(180, photo, busy) { pick.launch("image/*") }
        Spacer(Modifier.height(16.dp))
        error?.let { ErrorBox(it); Spacer(Modifier.height(16.dp)) }
        GlassField(bolim, { bolim = it.filter(Char::isDigit) }, "Bo'lim raqami (N-bo'lim)", AruIcons.Tag, number = true); Spacer(Modifier.height(12.dp))
        GlassField(nomi, { nomi = it }, "Nomi", AruIcons.Movie); Spacer(Modifier.height(12.dp))
        GlassField(studio, { studio = it }, "Studiya", AruIcons.Business); Spacer(Modifier.height(12.dp))
        GlassField(tarjimon, { tarjimon = it }, "Tarjimon", AruIcons.Translate); Spacer(Modifier.height(12.dp))
        GlassField(davlat, { davlat = it }, "Davlat", AruIcons.LocationOn); Spacer(Modifier.height(12.dp))
        GlassField(rejissyor, { rejissyor = it }, "Rejissyor", AruIcons.VideocamOutline); Spacer(Modifier.height(12.dp))
        GlassField(yili, { yili = it }, "Yili", AruIcons.CalendarToday, number = true); Spacer(Modifier.height(12.dp))
        // Yosh chegarasi — butun bo'lim uchun bir marta; yozilgan raqam o'ngda "18+" ko'rinishida turadi.
        GlassField(yosh, { yosh = it.filter(Char::isDigit).take(2) }, "Yosh chegarasi (masalan 18)", AruIcons.ShieldOutline, number = true, trailing = if (yosh.trim().isEmpty()) null else ({
            Text("${yosh.trim()}+", color = AruColors.Accent, fontSize = 15.sp, fontWeight = FontWeight.ExtraBold, modifier = Modifier.padding(end = 4.dp))
        }))
        Spacer(Modifier.height(12.dp))
        SectionLabel("Janri"); Spacer(Modifier.height(8.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            GENRES.forEach { g -> Chip(g, g in genres) { if (g in genres) genres.remove(g) else genres.add(g) } }
        }
        Spacer(Modifier.height(16.dp))
        SectionLabel("Turi"); Spacer(Modifier.height(8.dp))
        Segmented(listOf("TV", "FILM", "OVA"), turi) { turi = it }
        Spacer(Modifier.height(16.dp))
        SectionLabel("Holat"); Spacer(Modifier.height(8.dp))
        Segmented(listOf("Davom etmoqda", "Tugallangan"), holati) { holati = it }
        Spacer(Modifier.height(16.dp))
        GlassField(tavsif, { tavsif = it }, "Tavsif", AruIcons.DescriptionOutline, maxLines = 4)
        Spacer(Modifier.height(24.dp))
        FormButtons("Bekor qilish", if (initial == null) "Qo'shish" else "Saqlash", busy, onBack) {
            busy = true; error = null
            scope.launch {
                val g = genres.sorted()
                val b = JSONObject().put("anime_id", animeId).put("bolim_id", bolim.toIntOrNull() ?: 0).put("photo_url", photo).put("nomi", nomi).put("studio", studio)
                    .put("tarjimon", tarjimon).put("yili", yili).put("janrlar", JSONArray(g)).put("janri", g.joinToString(", ")).put("turi", turi).put("holati", holati)
                    .put("tavsif", tavsif).put("davlat", davlat.trim()).put("rejissyor", rejissyor.trim()).put("yosh", yosh.trim().toIntOrNull() ?: 0)
                val r = if (initial == null) api.post("/api/seasons", b) else withContext(Dispatchers.IO) { uz.arumediatv.com.data.Http.client.callJson("${uz.arumediatv.com.BuildConfig.API_BASE}/api/seasons/$animeId/${initial.optLong("season_id")}", "PUT", b) }
                busy = false
                if (r.ok) onDone() else error = "Xato: ${api.errorOf(r, "Saqlashda xato")}"
            }
        }
    }
}

/** Janr tugmasi: tanlansa apelsin hoshiyali. */
@Composable
private fun Chip(text: String, on: Boolean, onClick: () -> Unit) {
    val sh = RoundedCornerShape(12.dp)
    Text(
        text, color = if (on) Color.White else Color(0x99FFFFFF), fontSize = 13.sp, fontWeight = if (on) FontWeight.SemiBold else FontWeight.Medium,
        modifier = Modifier.clip(sh).background(if (on) AruColors.Accent.copy(alpha = 0.22f) else AruColors.Card).border(1.dp, if (on) AruColors.Accent else AruColors.Border, sh).clickable(onClick = onClick).padding(horizontal = 14.dp, vertical = 9.dp),
    )
}

/** Teng kenglikdagi tanlov tugmalari (Tur / Holat). */
@Composable
private fun Segmented(options: List<String>, selected: String, onSelect: (String) -> Unit) {
    val sh = RoundedCornerShape(12.dp)
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        options.forEach { opt ->
            val on = opt == selected
            Box(
                Modifier.weight(1f).clip(sh).background(if (on) AruColors.Accent else Color(0x0FFFFFFF)).border(1.dp, if (on) AruColors.Accent else Color(0x1FFFFFFF), sh).clickable { onSelect(opt) }.padding(vertical = 12.dp),
                contentAlignment = Alignment.Center,
            ) { Text(opt, color = if (on) Color.White else white60, fontWeight = if (on) FontWeight.Bold else FontWeight.Medium, fontSize = 13.sp) }
        }
    }
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

private fun mb(bytes: Long) = "%.2f MB".format(bytes / (1024.0 * 1024.0))

private fun progressLine(sent: Long, total: Long) = "${"%.1f".format(if (total > 0) sent * 100.0 / total else 0.0)}% (${mb(sent)} / ${mb(total)})"

/** Kodlash holati — bir qator matn (Flutter `_encodeStatusText`). */
private fun encodeStatusText(job: JSONObject?, originNew: Boolean): String? {
    if (job == null) return if (originNew) "Saqlangach kodlash navbatiga qo'yiladi" else null
    val done = job.optString("done").split(',').filter { it.isNotEmpty() }.joinToString(", ")
    return when (job.optString("state")) {
        "queued" -> if (done.isEmpty()) "Navbatda" else "Navbatda (tayyor: $done)"
        "running" -> if (done.isEmpty()) "Kodlanmoqda..." else "Kodlanmoqda... tayyor: $done"
        "done" -> "Tayyor: $done"
        "error" -> "Xato: ${job.optString("error")}"
        else -> null
    }
}

/** O'chirish tugmasi: 10 yumaloq glass quti ichida qizil ikonka. */
@Composable
private fun TrashBox(onClick: () -> Unit) {
    val sh = RoundedCornerShape(10.dp)
    Box(Modifier.clip(sh).background(AruColors.Card).border(1.dp, AruColors.Border, sh).clickable(onClick = onClick).padding(8.dp)) {
        Icon(AruIcons.DeleteOutline, null, tint = Color(0xFFFF5252), modifier = Modifier.size(20.dp))
    }
}

/** Apelsin ikonkali kengaytirilgan yuklash tugmasi (12 yumaloq). */
@Composable
private fun UploadButton(icon: ImageVector, text: String, onClick: () -> Unit) {
    val sh = RoundedCornerShape(12.dp)
    Row(Modifier.fillMaxWidth().clip(sh).background(AruColors.Card).border(1.dp, AruColors.Border, sh).clickable(onClick = onClick).padding(vertical = 14.dp), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
        Icon(icon, null, tint = AruColors.Accent, modifier = Modifier.size(20.dp))
        Spacer(Modifier.width(8.dp))
        Text(text, color = AruColors.Accent, fontWeight = FontWeight.SemiBold, fontSize = 13.sp)
    }
}

@Composable
private fun UploadProgress(progress: Float, text: String) {
    LinearProgressIndicator(progress = { progress }, color = AruColors.Accent, trackColor = Color(0x1AFFFFFF), modifier = Modifier.fillMaxWidth().height(6.dp).clip(RoundedCornerShape(6.dp)))
    Spacer(Modifier.height(8.dp))
    Text(text, color = Color(0xB3FFFFFF), fontSize = 12.sp)
}

/** Bitta sifat kartasi (Flutter `_QualityCard`). */
@Composable
private fun QualityCard(label: String, hasFile: Boolean, sizeText: String, uploading: Boolean, progress: Float, progressText: String, onUpload: () -> Unit, onDelete: () -> Unit) {
    GlassCard(16, PaddingValues(14.dp)) {
        Text("$label sifat", color = Color.White, fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
        Spacer(Modifier.height(10.dp))
        when {
            uploading -> UploadProgress(progress, progressText)
            hasFile -> Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(AruIcons.CheckCircle, null, tint = Color(0xFF69F0AE), modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text(sizeText, color = Color(0xCCFFFFFF), fontSize = 13.sp, modifier = Modifier.weight(1f))
                TrashBox(onDelete)
            }
            else -> UploadButton(AruIcons.UploadFile, "$label fayl yuklash", onUpload)
        }
    }
}

/** Qism qo'shish / tahrirlash: raqam, nom, asl video (kodlash navbatiga), tayyor sifatlar (360p–1080p), intro vaqtlari (5 ta). */
@Composable
fun AdminEpisodeEditScreen(animeId: Long, seasonId: Long, initial: JSONObject?, onBack: () -> Unit, onDone: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val api = AppServices.admin
    var number by remember { mutableStateOf(if (initial != null && initial.has("epizod_number")) initial.optInt("epizod_number").toString() else "") }
    var name by remember { mutableStateOf(jstr(initial, "epizod_name")) }
    val intro = remember { List(10) { i -> mutableStateOf(introText(initial?.optString("intro_${i + 1}"))) } }
    val originInit = jstr(initial, "origin_video")
    val qs = remember {
        listOf("360p", "480p", "720p", "1080p").map { q ->
            val u = fileNameOfUrl(jstr(initial, "url_$q"))
            // Kodlanguncha worker ASL videoni bo'sh sifat o'rnida beradi — u sifat emas, "Asl video" bo'limida ko'rinadi.
            if (originInit.isNotEmpty() && u == originInit) QState(q) else QState(q, u, initial?.optLong("size_$q") ?: 0)
        }
    }
    var tick by remember { mutableStateOf(0) }
    var origin by remember { mutableStateOf(originInit) }
    var originNew by remember { mutableStateOf(false) }
    var upTarget by remember { mutableStateOf<String?>(null) }
    var upSent by remember { mutableStateOf(0L) }
    var upTotal by remember { mutableStateOf(0L) }
    var saving by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var job by remember { mutableStateOf<JSONObject?>(null) }
    var delQuality by remember { mutableStateOf<QState?>(null) }
    var delOrigin by remember { mutableStateOf(false) }
    fun toast(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_LONG).show()
    val epId = initial?.optLong("epizod_id") ?: 0L

    suspend fun loadStatus() {
        if (epId > 0) {
            val r = api.get("/api/encode/status?anime_id=$animeId&season_id=$seasonId")
            val a = r.json?.optJSONArray("items")
            job = (0 until (a?.length() ?: 0)).mapNotNull { a?.optJSONObject(it) }.firstOrNull { it.optLong("epizod_id") == epId }
        }
    }
    LaunchedEffect(Unit) { loadStatus() }

    var pending by remember { mutableStateOf<String?>(null) } // "origin" yoki sifat nomi
    val pickVideo = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { uri ->
        val target = pending
        if (uri == null || target == null) return@rememberLauncherForActivityResult
        if (!AppServices.telegram.state.value.authorized) { toast("Asl video Telegram orqali yuklanadi: Profil → \"Telegram'ni ulash\""); return@rememberLauncherForActivityResult }
        scope.launch {
            upTarget = target; upSent = 0; upTotal = 0
            val uid = AppServices.auth.user.value?.id ?: 0L
            val res = withPickedFile(ctx, uri) { path, display, size ->
                val ext = display.substringAfterLast('.', "mp4").lowercase().ifEmpty { "mp4" }
                val mime = if (ext == "mkv") "video/x-matroska" else "video/mp4"
                val fname = if (target == "origin") "orig_${uid}_${animeId}_${seasonId}_${System.currentTimeMillis()}.$ext" else "ep_${animeId}_${seasonId}_${target}_${System.currentTimeMillis()}.$ext"
                upTotal = size
                val err = AppServices.telegram.uploadFile(path, fname, mime, onProgress = { s, t -> upSent = s; if (t > 0) upTotal = t })
                Triple(err, fname, size)
            }
            upTarget = null
            if (res == null) { toast("Faylni o'qib bo'lmadi"); return@launch }
            val (err, fname, size) = res
            if (target == "origin") {
                if (err != null) { toast("Asl video yuklanmadi — $err"); return@launch }
                origin = fname; originNew = true; job = null
                toast("Asl video yuklandi — \"Saqlash\"dan keyin kodlash navbatiga qo'yiladi")
            } else {
                if (err != null) { toast("$target: Telegram'ga yuklanmadi — $err"); return@launch }
                qs.first { it.label == target }.apply { url = fname; this.size = size; key = AppServices.telegram.keyFor(fname) }; tick++
                toast("$target Telegram kanaliga yuklandi — endi \"Saqlash\"ni bosing")
            }
        }
    }

    suspend fun queueEncode(id: Long): String? {
        val q = api.post("/api/encode/queue", JSONObject().put("anime_id", animeId).put("season_id", seasonId).put("epizod_id", id).put("origin", origin).put("key", AppServices.telegram.keyFor(origin)))
        return if (q.ok) null else api.errorOf(q, "xato")
    }

    FormScaffold(if (initial == null) "Epizod qo'shish" else "Epizodni tahrirlash", 19, onBack) {
        error?.let { ErrorBox(it); Spacer(Modifier.height(16.dp)) }
        GlassField(number, { number = it.filter(Char::isDigit).take(5) }, "Epizod raqami (0 dan boshlash)", AruIcons.FormatListNumbered, number = true); Spacer(Modifier.height(12.dp))
        GlassField(name, { name = it }, "Epizod nomi (ixtiyoriy)", AruIcons.TitleIcon)
        Spacer(Modifier.height(20.dp))

        // ── Asl video (avto-kodlash) ──
        val status = encodeStatusText(job, originNew)
        val failed = job?.optString("state") == "error"
        GlassCard(16, PaddingValues(14.dp)) {
            Text("Asl video (avto-kodlash)", color = Color.White, fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
            Spacer(Modifier.height(4.dp))
            Text("Bitta asl video yuklang — 1080p/720p/480p/360p (manbadan baland emas) H.265 bilan o'zi tayyorlanadi va yoziladi.", color = Color(0x8CFFFFFF), fontSize = 12.sp)
            Spacer(Modifier.height(10.dp))
            if (upTarget == "origin") UploadProgress(if (upTotal > 0) upSent.toFloat() / upTotal else 0f, progressLine(upSent, upTotal))
            else {
                if (origin.isNotEmpty()) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(if (failed) AruIcons.ErrorOutline else AruIcons.CheckCircle, null, tint = if (failed) Color(0xFFFF5252) else Color(0xFF69F0AE), modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(8.dp))
                        Text(status ?: "Asl video yuklangan", color = Color(0xCCFFFFFF), fontSize = 13.sp, modifier = Modifier.weight(1f))
                        TrashBox { delOrigin = true }
                    }
                    Spacer(Modifier.height(10.dp))
                }
                UploadButton(AruIcons.MovieFilter, if (origin.isEmpty()) "Original video yuklash" else "Boshqa asl video yuklash") { pending = "origin"; pickVideo.launch("video/*") }
                if (failed && !originNew && epId > 0) {
                    Spacer(Modifier.height(8.dp))
                    Row(Modifier.fillMaxWidth().clickable { scope.launch { val err = queueEncode(epId); toast(if (err == null) "Qayta navbatga qo'yildi" else "Xato: $err"); loadStatus() } }.padding(vertical = 8.dp), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
                        Icon(AruIcons.Refresh, null, tint = AruColors.Accent, modifier = Modifier.size(18.dp)); Spacer(Modifier.width(8.dp)); Text("Qayta urinish", color = AruColors.Accent, fontSize = 14.sp)
                    }
                }
            }
        }
        Spacer(Modifier.height(12.dp))

        // ── 4 ta sifat ──
        qs.forEach { q ->
            key(tick) {
                val up = upTarget == q.label
                QualityCard(q.label, q.url.isNotEmpty(), formatBytes(q.size), up, if (upTotal > 0) upSent.toFloat() / upTotal else 0f, progressLine(upSent, upTotal), onUpload = { pending = q.label; pickVideo.launch("video/*") }, onDelete = { delQuality = q })
            }
            Spacer(Modifier.height(12.dp))
        }
        Spacer(Modifier.height(8.dp))

        // ── O'tkazib yuboriladigan joylar ──
        GlassCard(16, PaddingValues(start = 16.dp, top = 14.dp, end = 16.dp, bottom = 16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(AruIcons.FastForward, null, tint = Color(0xB3FFFFFF), modifier = Modifier.size(18.dp)); Spacer(Modifier.width(8.dp))
                Text("O'tkazib yuboriladigan joylar", color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold)
            }
            Spacer(Modifier.height(4.dp))
            Text("Chapga boshlanish, o'ngga tugash vaqti: 5:14 va 6:44.\nVideo 5:14 ga kelganda pleyerda \"Introni o'tkazish\" tugmasi chiqadi. Bo'sh qatorlar e'tiborga olinmaydi.", color = Color(0x73FFFFFF), fontSize = 11.5.sp, lineHeight = 15.5.sp)
            Spacer(Modifier.height(12.dp))
            for (row in 0 until 5) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text("${row + 1}", color = Color(0x59FFFFFF), fontSize = 12.sp, modifier = Modifier.width(20.dp))
                    IntroField(intro[row * 2].value, { intro[row * 2].value = it }, "boshi", Modifier.weight(1f))
                    Spacer(Modifier.width(10.dp))
                    IntroField(intro[row * 2 + 1].value, { intro[row * 2 + 1].value = it }, "oxiri", Modifier.weight(1f))
                }
                if (row != 4) Spacer(Modifier.height(8.dp))
            }
        }
        Spacer(Modifier.height(12.dp))

        FormButtons("Bekor qilish", if (initial == null) "Qo'shish" else "Saqlash", saving, onBack) {
            saving = true; error = null
            scope.launch {
                val b = JSONObject().put("anime_id", animeId).put("season_id", seasonId).put("epizod_number", number.toIntOrNull() ?: 0).put("epizod_name", name)
                qs.forEach { q -> b.put("url_${q.label}", q.url).put("size_${q.label}", q.size).put("key_${q.label}", q.key) }
                intro.forEachIndexed { i, s -> b.put("intro_${i + 1}", normalizeIntro(s.value)) }
                val r = if (initial != null) withContext(Dispatchers.IO) { uz.arumediatv.com.data.Http.client.callJson("${uz.arumediatv.com.BuildConfig.API_BASE}/api/epizods/$animeId/$seasonId/$epId", "PUT", b) } else api.post("/api/epizods", b)
                if (r.ok) {
                    if (originNew && origin.isNotEmpty()) {
                        val id = if (epId > 0) epId else r.json?.optLong("epizod_id") ?: 0L
                        val err = if (id == 0L) "qism raqami olinmadi" else queueEncode(id)
                        toast(if (err == null) "Kodlash navbatiga qo'yildi — GitHub Actions'da \"Avto-kodlash\" ni ishga tushiring" else "Qism saqlandi, lekin kodlash navbatiga qo'yilmadi: $err")
                    }
                    saving = false
                    onDone()
                } else { saving = false; error = "Xato: ${api.errorOf(r, "Saqlashda xato")}" }
            }
        }
    }

    delQuality?.let { q ->
        ConfirmDialog("${q.label} faylni o'chirish?", "Fayl B2dan ham, bazadan ham o'chiriladi. Davom etasizmi?", "Ha, o'chirish", danger = true) { yes ->
            delQuality = null
            if (yes) scope.launch {
                if (epId > 0) {
                    val r = api.delete("/api/epizods/$animeId/$seasonId/$epId/${q.label}")
                    if (!r.ok) { toast("O'chirishda xato: ${api.errorOf(r, "xato")}"); return@launch }
                }
                q.url = ""; q.size = 0; q.key = ""; tick++
            }
        }
    }
    if (delOrigin) {
        ConfirmDialog("Asl videoni o'chirish?", "Asl video Telegram'dan ham, bazadan ham o'chiriladi va kodlash navbatdan olinadi. Tayyor bo'lgan sifatlar qoladi. Davom etasizmi?", "Ha, o'chirish", danger = true) { yes ->
            delOrigin = false
            if (yes) scope.launch {
                val r = api.post("/api/encode/delete", JSONObject().put("anime_id", animeId).put("season_id", seasonId).put("epizod_id", epId).put("origin", origin))
                if (r.ok) { origin = ""; originNew = false; job = null; toast("Asl video o'chirildi") } else toast("Asl video o'chirilmadi: ${api.errorOf(r, "xato")}")
            }
        }
    }
}

/** Vaqt maydoni (Flutter `_introField`): oq 6% fon, 10 yumaloq, faqat raqam va ikki nuqta. */
@Composable
private fun IntroField(value: String, onChange: (String) -> Unit, hint: String, modifier: Modifier) {
    val sh = RoundedCornerShape(10.dp)
    Box(modifier.clip(sh).background(Color(0x0FFFFFFF)).border(1.dp, Color(0x1AFFFFFF), sh).padding(horizontal = 12.dp, vertical = 11.dp)) {
        androidx.compose.foundation.text.BasicTextField(
            value = value, onValueChange = { onChange(it.filter { c -> c.isDigit() || c == ':' }.take(8)) }, singleLine = true,
            textStyle = TextStyle(color = Color.White, fontSize = 14.sp), cursorBrush = androidx.compose.ui.graphics.SolidColor(Color.White),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Text), modifier = Modifier.fillMaxWidth(),
            decorationBox = { inner -> if (value.isEmpty()) Text(hint, color = Color(0x4DFFFFFF), fontSize = 13.sp); inner() },
        )
    }
}

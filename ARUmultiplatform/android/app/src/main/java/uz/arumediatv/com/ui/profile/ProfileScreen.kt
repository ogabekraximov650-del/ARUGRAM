package uz.arumediatv.com.ui.profile

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.widget.Toast
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import uz.arumediatv.com.data.MyStats
import uz.arumediatv.com.data.SyncResult
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ExitToApp
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.Phone
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.BuildConfig
import uz.arumediatv.com.core.RustCore
import uz.arumediatv.com.data.AppUser
import uz.arumediatv.com.ui.common.LocalBarPad
import uz.arumediatv.com.ui.common.AruIcons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Notifications
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.theme.AruColors

/**
 * Profil: hisob kartasi (ID nusxalanadi), shaxsiy statistika, Sozlamalar, Telegram holati, qurilmalar, kesh, ilova haqida,
 * hisobni o'chirish va chiqish. Chiqishda navbatdagi yozuvlar avval serverga saqlanadi (`SyncQueue`).
 */
@Composable
fun ProfileScreen(onLogin: () -> Unit, onEdit: () -> Unit, onSessions: () -> Unit, onSettings: () -> Unit, onBilling: () -> Unit, onOpenStat: (StatKind) -> Unit, onStorage: () -> Unit, onSupport: () -> Unit, onAdmin: () -> Unit, onPacks: () -> Unit) {
    val auth = AppServices.auth
    val tg = AppServices.telegram
    val user by auth.user.collectAsStateWithLifecycle()
    val tgState by tg.state.collectAsStateWithLifecycle()
    val stats by AppServices.myStats.state.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    val ctx = LocalContext.current

    var confirmLogout by remember { mutableStateOf(false) }
    var unsavedWarn by remember { mutableStateOf(false) }
    var deleteStep by remember { mutableStateOf(0) } // 0 yo'q, 1 birinchi, 2 ikkinchi tasdiq
    var showAbout by remember { mutableStateOf(false) }
    var cacheBytes by remember { mutableStateOf(0L) }
    var busy by remember { mutableStateOf<String?>(null) }
    var error by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(user) {
        cacheBytes = withContext(Dispatchers.IO) { AppServices.images.diskBytes() + AppServices.videoCache.diskBytes() + AppServices.thumbs.diskBytes() }
        if (user != null) AppServices.myStats.load()
    }

    suspend fun wipeLocal() {
        tg.logout(); runCatching { uz.arumediatv.com.AppServices.tgStickers.clear() }
        AppServices.images.clearDisk()
        AppServices.library.clear()
        AppServices.myStats.clear()
        AppServices.sync.clear()
        AppServices.billing.clear()
        AppServices.unread.clear()
    }

    fun toast(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_SHORT).show()
    var avatarBusy by remember { mutableStateOf(false) }
    val pickAvatar = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.GetContent()) { uri ->
        if (uri != null) {
            avatarBusy = true
            scope.launch {
                val err = uz.arumediatv.com.data.uploadAvatar(ctx, uri, auth, tg)
                avatarBusy = false
                toast(err ?: "Profil rasmi yangilandi")
            }
        }
    }

    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState())
            .padding(start = 16.dp, end = 16.dp, top = LocalBarPad.current.calculateTopPadding()),
    ) {
        uz.arumediatv.com.ui.common.PageHeader("Profil", Modifier.padding(horizontal = 4.dp))
        Spacer(Modifier.height(8.dp))
        val u = user
        if (u == null) {
            GuestCard(onLogin)
        } else {
            HeroCard(u, onEdit = onEdit, avatarBusy = avatarBusy, onAvatar = { pickAvatar.launch("image/*") }, onCopyId = {
                (ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(ClipData.newPlainText("ID", u.id.toString()))
                toast("ID nusxalandi: ${u.id}")
            })
            Spacer(Modifier.height(16.dp))
            BillingButton(onBilling)
            Spacer(Modifier.height(12.dp))
            StatsGrid(stats, cacheBytes, onOpenStat, onStorage)
            Spacer(Modifier.height(12.dp))
            TrafficCard(stats.traffic)
        }

        Spacer(Modifier.height(16.dp))
        val unread by AppServices.unread.count.collectAsStateWithLifecycle()
        Group {
            Tile(Icons.Filled.Notifications, "Bildirishnoma", onClick = { toast("«Bildirishnoma» bo'limi tez orada qo'shiladi") })
            Divider2()
            Tile(Icons.Filled.Settings, "Sozlamalar", onClick = { if (u == null) onLogin() else onSettings() })
            Divider2()
            if (!tgState.authorized) {
                Tile(Icons.AutoMirrored.Filled.Send, "Telegram'ni ulash", onClick = onLogin)
                Divider2()
            }
            Tile(AruIcons.Devices, "Qurilmalar", onClick = { if (u == null) onLogin() else onSessions() })
            Divider2()
            Tile(AruIcons.Emoticon, "Emoji, GIF va stikerlar", onClick = { if (u == null) onLogin() else onPacks() })
            Divider2()
            Tile(AruIcons.SupportAgent, "Admin bilan bog'lanish", dot = unread > 0, onClick = { if (u == null) onLogin() else onSupport() })
            Divider2()
            Tile(Icons.Filled.Info, "Ilova haqida", onClick = { showAbout = true })
        }

        // Admin paneli: faqat admin build'da VA server admin deb belgilagan hisobda (haqiqiy to'siq serverda).
        if (BuildConfig.IS_ADMIN && u != null && u.isAdmin) {
            Spacer(Modifier.height(16.dp))
            Group { Tile(Icons.Filled.Lock, "Admin paneli", onClick = onAdmin) }
        }

        if (u != null) {
            Spacer(Modifier.height(16.dp))
            DangerTile(Icons.Filled.Delete, "Accountni o'chirish") { deleteStep = 1 }
            Spacer(Modifier.height(16.dp))
            DangerTile(Icons.Filled.ExitToApp, "Accountdan chiqish") { confirmLogout = true }
        }
        Spacer(Modifier.height(32.dp + LocalBarPad.current.calculateBottomPadding()))
    }

    // ── Chiqish ──
    if (confirmLogout) {
        AlertDialog(
            onDismissRequest = { if (busy == null) confirmLogout = false },
            title = { Text("Chiqish") },
            text = { Text("Hisobdan chiqasizmi? Avval saqlanmagan o'zgarishlar serverga yuboriladi.") },
            confirmButton = {
                TextButton(
                    enabled = busy == null,
                    onClick = {
                        busy = "Ma'lumotlar saqlanmoqda"
                        scope.launch {
                            val r = withContext(Dispatchers.IO) { AppServices.sync.flush(force = true) }
                            if (r == SyncResult.OFFLINE && AppServices.sync.pendingCount > 0) {
                                busy = null
                                unsavedWarn = true
                            } else {
                                auth.logout()
                                wipeLocal()
                                busy = null
                                confirmLogout = false
                            }
                        }
                    },
                ) { Text("Chiqish", color = Color(0xFFF87171)) }
            },
            dismissButton = { TextButton(enabled = busy == null, onClick = { confirmLogout = false }) { Text("Bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
    if (unsavedWarn) {
        AlertDialog(
            onDismissRequest = { unsavedWarn = false },
            title = { Text("Internet yo'q") },
            text = { Text("Saqlanmagan o'zgarishlar serverga yuborilmadi. Hozir chiqsangiz ular yo'qoladi.") },
            confirmButton = {
                TextButton(onClick = {
                    unsavedWarn = false
                    confirmLogout = false
                    scope.launch {
                        auth.logout()
                        wipeLocal()
                    }
                }) { Text("Baribir chiqish", color = Color(0xFFF87171)) }
            },
            dismissButton = { TextButton(onClick = { unsavedWarn = false; confirmLogout = false }) { Text("Qolish") } },
            containerColor = AruColors.Surface,
        )
    }

    // ── Hisobni o'chirish: ikki bosqichli tasdiq ──
    if (deleteStep == 1) {
        AlertDialog(
            onDismissRequest = { deleteStep = 0 },
            title = { Text("Accountni o'chirish") },
            text = { Text("Hisobingiz, tarix, sevimlilar, baholar va izohlaringiz o'chiriladi. Bu amalni qaytarib bo'lmaydi.") },
            confirmButton = { TextButton(onClick = { deleteStep = 2 }) { Text("Davom etish", color = Color(0xFFF87171)) } },
            dismissButton = { TextButton(onClick = { deleteStep = 0 }) { Text("Bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
    if (deleteStep == 2) {
        AlertDialog(
            onDismissRequest = { deleteStep = 0 },
            title = { Text("Ishonchingiz komilmi?") },
            text = { Text("Account butunlay o'chiriladi.") },
            confirmButton = {
                TextButton(onClick = {
                    deleteStep = 0
                    busy = "Ma'lumotlar tozalanmoqda"
                    scope.launch {
                        val err = auth.deleteAccount()
                        if (err == null) wipeLocal() else error = err
                        busy = null
                    }
                }) { Text("Ha, o'chirilsin", color = Color(0xFFF87171)) }
            },
            dismissButton = { TextButton(onClick = { deleteStep = 0 }) { Text("Yo'q, bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
    error?.let { msg ->
        AlertDialog(
            onDismissRequest = { error = null },
            title = { Text("Bajarib bo'lmadi") },
            text = { Text(msg) },
            confirmButton = { TextButton(onClick = { error = null }) { Text("Yopish") } },
            containerColor = AruColors.Surface,
        )
    }
    busy?.let { label ->
        AlertDialog(
            onDismissRequest = {},
            title = { Text(label) },
            text = { LinearProgressIndicator(color = AruColors.Accent2, trackColor = AruColors.CardAlt, modifier = Modifier.fillMaxWidth()) },
            confirmButton = {},
            containerColor = AruColors.Surface,
        )
    }
    if (showAbout) {
        AlertDialog(
            onDismissRequest = { showAbout = false },
            title = { Text("ARUMEDIATV") },
            text = {
                Text(
                    "Versiya: ${BuildConfig.VERSION_NAME} (${BuildConfig.APP_VERSION})\n" +
                        "Server: ${BuildConfig.API_BASE.removePrefix("https://")}\n" +
                        "Yadro: ${if (RustCore.available) RustCore.version().ifEmpty { "yuklangan" } else (RustCore.loadError ?: "yuklanmagan")}\n" +
                        "Kotlin + Jetpack Compose + Rust",
                )
            },
            confirmButton = { TextButton(onClick = { showAbout = false }) { Text("Yopish") } },
            containerColor = AruColors.Surface,
        )
    }
}

@Composable
private fun GuestCard(onLogin: () -> Unit) {
    Card2 {
        Column(Modifier.fillMaxWidth().padding(20.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Avatar(null, 72.dp)
            Spacer(Modifier.height(12.dp))
            Text("Mehmon", color = Color.White, fontSize = 18.sp, fontWeight = FontWeight.Bold)
            Text("Telegram hisobingiz bilan kiring", color = AruColors.TextDim, fontSize = 13.sp)
            Spacer(Modifier.height(16.dp))
            Box(
                Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(14.dp))
                    .background(AruColors.Accent)
                    .clickable(onClick = onLogin)
                    .padding(vertical = 14.dp),
                contentAlignment = Alignment.Center,
            ) { Text("Kirish", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 15.sp) }
        }
    }
}

@Composable
private fun HeroCard(u: AppUser, onEdit: () -> Unit, onCopyId: () -> Unit, onAvatar: () -> Unit, avatarBusy: Boolean) {
    val shape = RoundedCornerShape(20.dp)
    val billing by AppServices.billing.state.collectAsStateWithLifecycle()
    Box(Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape)) {
        Row(Modifier.fillMaxWidth().padding(18.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(96.dp).clickable(onClick = onAvatar)) {
                Avatar(u, 96.dp)
                if (avatarBusy) Box(Modifier.size(96.dp).clip(CircleShape).background(Color.Black.copy(alpha = 0.55f)), contentAlignment = Alignment.Center) {
                    androidx.compose.material3.CircularProgressIndicator(color = Color.White, strokeWidth = 2.4.dp, modifier = Modifier.size(26.dp))
                }
                if (billing.active) Row(
                    Modifier.align(Alignment.TopCenter).padding(top = 3.dp).clip(RoundedCornerShape(20.dp))
                        .background(Brush.linearGradient(listOf(AruColors.Gold, AruColors.Accent2))).padding(horizontal = 6.dp, vertical = 2.5.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(AruIcons.WorkspacePremium, null, tint = AruColors.AccentTint, modifier = Modifier.size(11.dp))
                    Spacer(Modifier.width(2.dp))
                    Text("PREMIUM", color = AruColors.AccentTint, fontSize = 8.sp, fontWeight = FontWeight.Black, letterSpacing = 0.2.sp)
                }
                Box(
                    Modifier.align(Alignment.BottomEnd).clip(CircleShape).background(AruColors.Card).padding(2.dp).clip(CircleShape).background(AruColors.Accent).padding(5.dp),
                ) { Icon(AruIcons.PhotoCamera, null, tint = Color.White, modifier = Modifier.size(13.dp)) }
            }
            Spacer(Modifier.width(30.dp))
            Column(Modifier.weight(1f).padding(end = 34.dp)) {
                Text(u.fullName, color = Color.White, fontSize = 21.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (u.username.isNotEmpty()) Text("@${u.username}", color = AruColors.TextDim, fontSize = 15.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 4.dp))
                Row(
                    Modifier.padding(top = 8.dp).clip(RoundedCornerShape(100.dp)).background(Color.White.copy(alpha = 0.08f))
                        .border(1.dp, Color.White.copy(alpha = 0.20f), RoundedCornerShape(100.dp)).clickable(onClick = onCopyId).padding(horizontal = 14.dp, vertical = 7.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text("ID: ${u.id}", color = Color.White.copy(alpha = 0.78f), fontSize = 14.5.sp, fontWeight = FontWeight.SemiBold)
                    Spacer(Modifier.width(9.dp))
                    Icon(AruIcons.Copy, null, tint = Color.White.copy(alpha = 0.6f), modifier = Modifier.size(16.dp))
                }
                Text("Balans: ${u.balance}", color = Color.White.copy(alpha = 0.62f), fontSize = 14.5.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(top = 6.dp))
            }
        }
        Box(
            Modifier.align(Alignment.TopEnd).padding(18.dp).clip(CircleShape).background(Color.White.copy(alpha = 0.10f))
                .border(1.dp, Color.White.copy(alpha = 0.18f), CircleShape).clickable(onClick = onEdit).padding(8.dp),
        ) { Icon(Icons.Filled.Edit, "Tahrirlash", tint = Color.White.copy(alpha = 0.85f), modifier = Modifier.size(17.dp)) }
    }
}

@Composable
private fun BillingButton(onClick: () -> Unit) {
    val st by AppServices.billing.state.collectAsStateWithLifecycle()
    var now by remember { mutableStateOf(System.currentTimeMillis()) }
    LaunchedEffect(st.active) { while (st.active) { now = System.currentTimeMillis(); kotlinx.coroutines.delay(1000) } }
    val shape = RoundedCornerShape(18.dp)
    val until = remember(st.until) { java.text.SimpleDateFormat("dd.MM.yyyy HH:mm", java.util.Locale.US).format(java.util.Date(st.until)) }
    Row(
        Modifier.fillMaxWidth().clip(shape).background(Brush.horizontalGradient(listOf(AruColors.Accent, AruColors.Accent2))).clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(AruIcons.WorkspacePremium, null, tint = Color.White, modifier = Modifier.size(20.dp))
        Spacer(Modifier.width(10.dp))
        Column(Modifier.weight(1f)) {
            Text("Obuna olish va Balans to'ldirish", color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(if (st.active) "Obuna $until gacha" else "Obuna yo'q", color = Color.White.copy(alpha = 0.85f), fontSize = 11.5.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, modifier = Modifier.padding(top = 3.dp))
            if (st.active) Text(uz.arumediatv.com.data.formatLeft(st.until - now), color = Color.White, fontSize = 12.5.sp, fontWeight = FontWeight.Bold, maxLines = 1)
        }
        Spacer(Modifier.width(6.dp))
        Icon(AruIcons.ChevronRight, null, tint = Color.White.copy(alpha = 0.7f), modifier = Modifier.size(20.dp))
    }
}

@Composable
private fun StatsGrid(s: MyStats, storageBytes: Long, onOpen: (StatKind) -> Unit, onStorage: () -> Unit) {
    fun fmt(n: Long) = uz.arumediatv.com.ui.season.formatCount(n)
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            StatBox(AruIcons.Movie, "Anime", fmt(s.animes), Modifier.weight(1f), null)
            StatBox(AruIcons.PlayCircleOutline, "Qismlar", fmt(s.episodes), Modifier.weight(1f)) { onOpen(StatKind.EPISODES) }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            StatBox(AruIcons.GridView, "Bo'limlar", fmt(s.seasons), Modifier.weight(1f)) { onOpen(StatKind.SEASONS) }
            StatBox(AruIcons.Bookmark, "Sevimlilar", fmt(s.favorites), Modifier.weight(1f)) { onOpen(StatKind.FAVORITES) }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            StatBox(Icons.Filled.Star, "Baholangan", fmt(s.rated), Modifier.weight(1f)) { onOpen(StatKind.RATED) }
            StatBox(AruIcons.Comment, "Izohlar", fmt(s.comments), Modifier.weight(1f)) { onOpen(StatKind.COMMENTS) }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            StatBox(AruIcons.Schedule, "Tomosha vaqti", "${uz.arumediatv.com.ui.season.formatHours(s.watchMs)} soat", Modifier.weight(1f), null)
            StatBox(AruIcons.SdStorage, "Xotira", formatBytes(storageBytes).replace('.', ','), Modifier.weight(1f), onStorage)
        }
    }
}

/** Trafik: umumiy hajm, ranglangan chiziq va toifalar (Flutter `_TrafficBox`). */
@Composable
private fun TrafficCard(syncedBytes: Long) {
    val totals by AppServices.traffic.totals.collectAsStateWithLifecycle()
    val pending = remember(totals) { AppServices.trafficOrNull()?.pendingBytes ?: 0L }
    val rowsRaw = listOf("Videolar" to (totals["video"] ?: 0L), "Ma'lumotlar" to (totals["api"] ?: 0L)).filter { it.second > 0 }.sortedByDescending { it.second }
    val known = rowsRaw.sumOf { it.second }
    val total = maxOf(syncedBytes + pending, known)
    val rows = buildList {
        addAll(rowsRaw)
        val rest = total - known
        if (rest > 0 && known > 0) add("Oldingi hisob" to rest) else if (isEmpty() && total > 0) add("Oldingi hisob" to total)
    }
    fun colorOf(l: String) = when (l) { "Videolar" -> AruColors.Accent; "Ma'lumotlar" -> Color(0xFF4ADE80); else -> AruColors.TextFaint }
    val shape = RoundedCornerShape(18.dp)
    Column(Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).padding(start = 16.dp, end = 16.dp, top = 14.dp, bottom = 14.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(AruIcons.CloudDownload, null, tint = AruColors.Accent, modifier = Modifier.size(15.dp))
            Spacer(Modifier.width(6.dp))
            Text("Trafik", color = Color.White.copy(alpha = 0.55f), fontSize = 12.sp)
            Spacer(Modifier.weight(1f))
            Text(formatBytes(total).replace('.', ','), color = Color.White, fontSize = 17.sp, fontWeight = FontWeight.ExtraBold)
        }
        Spacer(Modifier.height(12.dp))
        Row(Modifier.fillMaxWidth().height(7.dp).clip(RoundedCornerShape(4.dp)).background(Color.White.copy(alpha = 0.10f))) {
            val sum = rows.sumOf { it.second }.coerceAtLeast(1).toFloat()
            rows.forEach { (l, b) -> Box(Modifier.weight((b / sum).coerceAtLeast(0.001f)).height(7.dp).background(colorOf(l))) }
        }
        Spacer(Modifier.height(12.dp))
        if (rows.isEmpty()) Text("Hozircha trafik sarflanmagan", color = Color.White.copy(alpha = 0.4f), fontSize = 12.sp)
        rows.forEachIndexed { i, (l, b) ->
            if (i > 0) Spacer(Modifier.height(7.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(7.dp).clip(CircleShape).background(colorOf(l)))
                Spacer(Modifier.width(7.dp))
                Text(l, color = Color.White.copy(alpha = 0.72f), fontSize = 12.5.sp, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                Spacer(Modifier.width(8.dp))
                Text(formatBytes(b).replace('.', ','), color = Color.White, fontSize = 12.5.sp, fontWeight = FontWeight.Bold)
                Spacer(Modifier.width(10.dp))
                Text(String.format(java.util.Locale.US, "%.2f%%", if (total > 0) b * 100.0 / total else 0.0), color = Color.White.copy(alpha = 0.5f), fontSize = 12.sp, textAlign = androidx.compose.ui.text.style.TextAlign.End, modifier = Modifier.width(58.dp))
            }
        }
    }
}

@Composable
private fun StatBox(icon: ImageVector, label: String, value: String, modifier: Modifier, onClick: (() -> Unit)?) {
    val shape = RoundedCornerShape(18.dp)
    Column(
        modifier.clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape)
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier).padding(horizontal = 14.dp, vertical = 14.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(icon, null, tint = AruColors.Accent, modifier = Modifier.size(16.dp))
            Spacer(Modifier.width(6.dp))
            Text(label, color = Color.White.copy(alpha = 0.55f), fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Text(value, color = Color.White, fontSize = 17.sp, fontWeight = FontWeight.ExtraBold, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 8.dp))
    }
}

@Composable
private fun Tile(icon: ImageVector, title: String, dot: Boolean = false, onClick: () -> Unit) {
    Row(Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        // Flutterdagidek: to'q apelsin yumaloq kvadrat ichida apelsin belgi.
        Box(Modifier.size(42.dp).clip(RoundedCornerShape(14.dp)).background(AruColors.Accent.copy(alpha = 0.14f)), contentAlignment = Alignment.Center) {
            Icon(icon, null, tint = AruColors.Accent, modifier = Modifier.size(22.dp))
        }
        Spacer(Modifier.width(16.dp))
        Text(title, color = Color.White, fontSize = 16.sp, modifier = Modifier.weight(1f))
        if (dot) { Box(Modifier.size(9.dp).clip(CircleShape).background(AruColors.Accent)); Spacer(Modifier.width(6.dp)) }
        Icon(AruIcons.ChevronRight, null, tint = Color.White.copy(alpha = 0.38f), modifier = Modifier.size(24.dp))
    }
}

@Composable
private fun DangerTile(icon: ImageVector, label: String, onClick: () -> Unit) {
    val shape = RoundedCornerShape(20.dp)
    Row(
        Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).clickable(onClick = onClick).padding(horizontal = 20.dp, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.clip(RoundedCornerShape(12.dp)).background(AruColors.Accent.copy(alpha = 0.14f)).padding(8.dp)) { Icon(icon, null, tint = AruColors.Accent, modifier = Modifier.size(22.dp)) }
        Spacer(Modifier.width(14.dp))
        Text(label, color = AruColors.Accent, fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
    }
}

@Composable
fun Avatar(u: AppUser?, size: androidx.compose.ui.unit.Dp) {
    Box(
        Modifier.size(size).clip(CircleShape).background(AruColors.AccentTint).border(1.dp, AruColors.BorderBright, CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        val initials = u?.initials ?: "?"
        TgImage(
            url = u?.photoUrl.orEmpty(),
            modifier = Modifier.fillMaxSize(),
            maxPx = 256,
            placeholder = {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text(initials, color = AruColors.Accent2, fontSize = (size.value / 2.4f).sp, fontWeight = FontWeight.Bold)
                }
            },
        )
    }
}

@Composable
private fun Pill(text: String, highlight: Boolean = false) {
    Text(
        text,
        color = if (highlight) AruColors.Gold else AruColors.TextDim,
        fontSize = 11.sp,
        fontWeight = FontWeight.SemiBold,
        modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(AruColors.CardAlt).padding(horizontal = 8.dp, vertical = 3.dp),
    )
}

@Composable
private fun Card2(content: @Composable () -> Unit) {
    val shape = RoundedCornerShape(20.dp)
    Box(Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape)) { content() }
}

@Composable
private fun Group(content: @Composable () -> Unit) {
    val shape = RoundedCornerShape(20.dp)
    Column(Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape)) { content() }
}

@Composable
private fun Divider2() {
    Box(Modifier.fillMaxWidth().height(1.dp).background(Color.White.copy(alpha = 0.12f)))
}

@Composable
private fun Row2(
    icon: ImageVector,
    title: String,
    subtitle: String,
    onClick: (() -> Unit)? = null,
    danger: Boolean = false,
    action: String? = null,
) {
    val tint = if (danger) Color(0xFFF87171) else AruColors.Accent2
    Row(
        Modifier
            .fillMaxWidth()
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(42.dp).clip(RoundedCornerShape(14.dp)).background((if (danger) Color(0xFFF87171) else AruColors.Accent).copy(alpha = 0.14f)), contentAlignment = Alignment.Center) {
            Icon(icon, null, tint = if (danger) tint else AruColors.Accent, modifier = Modifier.size(22.dp))
        }
        Spacer(Modifier.width(14.dp))
        Column(Modifier.weight(1f)) {
            Text(title, color = if (danger) Color(0xFFF87171) else Color.White, fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
            Text(subtitle, color = AruColors.TextDim, fontSize = 12.sp)
        }
        if (action != null) Text(action, color = AruColors.Accent2, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
    }
}

fun formatBytes(b: Long): String =
    when {
        b < 1024 -> "$b B"
        b < 1024 * 1024 -> "${b / 1024} KB"
        b < 1024L * 1024 * 1024 -> String.format(java.util.Locale.US, "%.1f MB", b / 1048576.0)
        else -> String.format(java.util.Locale.US, "%.2f GB", b / 1073741824.0)
    }

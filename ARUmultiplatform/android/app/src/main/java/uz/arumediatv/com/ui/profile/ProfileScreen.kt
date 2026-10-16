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
import uz.arumediatv.com.ui.common.PageHeader
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.theme.AruColors

/**
 * Profil: hisob kartasi (ID nusxalanadi), shaxsiy statistika, Sozlamalar, Telegram holati, qurilmalar, kesh, ilova haqida,
 * hisobni o'chirish va chiqish. Chiqishda navbatdagi yozuvlar avval serverga saqlanadi (`SyncQueue`).
 */
@Composable
fun ProfileScreen(onLogin: () -> Unit, onEdit: () -> Unit, onSessions: () -> Unit, onSettings: () -> Unit, onBilling: () -> Unit, onOpenStat: (StatKind) -> Unit, onStorage: () -> Unit, onSupport: () -> Unit, onAdmin: () -> Unit) {
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
        cacheBytes = withContext(Dispatchers.IO) { AppServices.images.diskBytes() }
        if (user != null) AppServices.myStats.load()
    }

    suspend fun wipeLocal() {
        tg.logout()
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
            .padding(start = 16.dp, end = 16.dp, top = 4.dp + LocalBarPad.current.calculateTopPadding()),
    ) {
        PageHeader("Profil", Modifier.padding(horizontal = 4.dp))
        Spacer(Modifier.height(8.dp))

        val u = user
        if (u == null) {
            GuestCard(onLogin)
        } else {
            HeroCard(u, onEdit = onEdit, avatarBusy = avatarBusy, onAvatar = { pickAvatar.launch("image/*") }, onCopyId = {
                (ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(ClipData.newPlainText("ID", u.id.toString()))
                toast("ID nusxalandi: ${u.id}")
            })
            Spacer(Modifier.height(12.dp))
            BillingButton(onBilling)
            Spacer(Modifier.height(12.dp))
            StatsGrid(stats, onOpenStat)
        }

        Spacer(Modifier.height(16.dp))
        Group {
            if (u != null) {
                Row2(Icons.Filled.Settings, "Sozlamalar", "Maxfiylik: statistikani boshqalardan yashirish", onClick = onSettings)
                Divider2()
            }
            Row2(
                Icons.Filled.Phone, "Telegram",
                when {
                    tgState.error != null && !tgState.started -> "Ishga tushmadi"
                    tgState.authorized -> "Ulangan — rasm va video ko'rinadi"
                    tgState.configUnreachable -> "Server bilan bog'lanib bo'lmadi"
                    else -> "Ulanmagan"
                },
                onClick = if (!tgState.authorized) onLogin else null,
                action = if (!tgState.authorized) "Ulash" else null,
            )
            if (u != null) {
                Divider2()
                Row2(Icons.Filled.Person, "Qurilmalar", "Hisobga kirgan qurilmalar", onClick = onSessions)
            }
        }

        Spacer(Modifier.height(16.dp))
        Group {
            Row2(
                Icons.Filled.Delete, "Rasmlar keshi", formatBytes(cacheBytes),
                onClick = {
                    AppServices.images.clearDisk()
                    cacheBytes = 0
                },
                action = "Tozalash",
            )
            Divider2()
            Row2(Icons.Filled.Info, "Xotira", "Yuklab olingan videolar, kesh, hajm", onClick = onStorage)
            Divider2()
            val unread by AppServices.unread.count.collectAsStateWithLifecycle()
            Row2(
                Icons.Filled.Phone, "Admin bilan bog'lanish", if (unread > 0) "Yangi xabar bor" else "Savol va takliflar",
                onClick = { if (u == null) onLogin() else onSupport() }, action = if (unread > 0) "●" else null,
            )
            Divider2()
            Row2(Icons.Filled.Refresh, "Hisobni yangilash", "Serverdan qayta o'qish", onClick = {
                if (u != null) scope.launch {
                    auth.refresh()
                    AppServices.myStats.load(force = true)
                }
            })
            Divider2()
            Row2(Icons.Filled.Info, "Ilova haqida", "Versiya ${BuildConfig.VERSION_NAME}", onClick = { showAbout = true })
        }

        // Admin paneli: faqat admin build'da VA server admin deb belgilagan hisobda (haqiqiy to'siq serverda).
        if (BuildConfig.IS_ADMIN && u != null && u.isAdmin) {
            Spacer(Modifier.height(16.dp))
            Group {
                Row2(Icons.Filled.Lock, "Admin paneli", "Foydalanuvchilar, anime, shikoyatlar, kanallar", onClick = onAdmin)
            }
        }

        if (u != null) {
            Spacer(Modifier.height(16.dp))
            Group {
                Row2(Icons.Filled.Delete, "Accountni o'chirish", "Hisob va barcha ma'lumotlar o'chadi", onClick = { deleteStep = 1 }, danger = true)
                Divider2()
                Row2(Icons.Filled.ExitToApp, "Accountdan chiqish", "Bu qurilmadagi hisobdan chiqish", onClick = { confirmLogout = true }, danger = true)
            }
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
    val shape = RoundedCornerShape(24.dp)
    Box(
        Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(Brush.linearGradient(listOf(AruColors.AccentTint, AruColors.Card)))
            .border(1.dp, AruColors.Accent.copy(alpha = 0.25f), shape),
    ) {
        Row(Modifier.fillMaxWidth().padding(18.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.clip(CircleShape).background(Brush.linearGradient(listOf(AruColors.Accent, AruColors.Accent2))).padding(3.dp).clickable(onClick = onAvatar)) {
                Avatar(u, 76.dp)
                if (avatarBusy) Box(Modifier.size(76.dp).clip(CircleShape).background(Color.Black.copy(alpha = 0.55f)), contentAlignment = Alignment.Center) {
                    androidx.compose.material3.CircularProgressIndicator(color = Color.White, strokeWidth = 2.dp, modifier = Modifier.size(26.dp))
                }
            }
            Spacer(Modifier.width(16.dp))
            Column(Modifier.weight(1f).padding(end = 30.dp)) {
                Text(u.fullName, color = Color.White, fontSize = 21.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (u.username.isNotEmpty()) Text("@${u.username}", color = AruColors.TextDim, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Spacer(Modifier.height(8.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        "ID ${u.id}", color = AruColors.TextDim, fontSize = 12.sp, fontWeight = FontWeight.SemiBold,
                        modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(Color.White.copy(alpha = 0.08f)).clickable(onClick = onCopyId).padding(horizontal = 10.dp, vertical = 4.dp),
                    )
                    if (u.isAdmin) {
                        Spacer(Modifier.width(8.dp))
                        Pill("Admin", highlight = true)
                    }
                }
                Spacer(Modifier.height(6.dp))
                Text("Balans: ${u.balance}", color = AruColors.Gold, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
            }
        }
        Box(
            Modifier.align(Alignment.TopEnd).padding(10.dp).size(34.dp).clip(CircleShape).background(Color.White.copy(alpha = 0.08f)).clickable(onClick = onEdit),
            contentAlignment = Alignment.Center,
        ) { Icon(Icons.Filled.Edit, "Tahrirlash", tint = Color.White.copy(alpha = 0.8f), modifier = Modifier.size(18.dp)) }
    }
}

@Composable
private fun BillingButton(onClick: () -> Unit) {
    val st by AppServices.billing.state.collectAsStateWithLifecycle()
    val shape = RoundedCornerShape(18.dp)
    Row(
        Modifier.fillMaxWidth().clip(shape).background(Brush.linearGradient(listOf(AruColors.Accent, AruColors.Accent2))).clickable(onClick = onClick).padding(horizontal = 18.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text("Obuna olish va balans to'ldirish", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.Bold)
            Text(if (st.active) "Obuna faol · ${st.leftMs / 86_400_000 + 1} kun qoldi" else "Obuna yo'q", color = Color.White.copy(alpha = 0.8f), fontSize = 12.sp)
        }
    }
}

@Composable
private fun StatsGrid(s: MyStats, onOpen: (StatKind) -> Unit) {
    val hours = s.watchHours
    val hoursText = if (hours >= 10) "${hours.toInt()} soat" else String.format(java.util.Locale.US, "%.1f soat", hours)
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            StatBox("Anime", s.animes.toString(), Modifier.weight(1f)) {}
            StatBox("Qism ›", s.episodes.toString(), Modifier.weight(1f)) { onOpen(StatKind.EPISODES) }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            StatBox("Tomosha", hoursText, Modifier.weight(1f)) {}
            StatBox("Trafik", formatBytes(s.traffic), Modifier.weight(1f)) {}
        }
    }
}

@Composable
private fun StatBox(label: String, value: String, modifier: Modifier, onClick: () -> Unit) {
    val shape = RoundedCornerShape(18.dp)
    Column(modifier.clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 14.dp)) {
        Text(value, color = AruColors.Accent2, fontSize = 20.sp, fontWeight = FontWeight.Black, maxLines = 1)
        Text(label, color = AruColors.TextDim, fontSize = 12.sp)
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
    val shape = RoundedCornerShape(18.dp)
    Column(Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape)) { content() }
}

@Composable
private fun Divider2() {
    Box(Modifier.fillMaxWidth().height(1.dp).background(AruColors.Border))
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
        Box(Modifier.size(36.dp).clip(RoundedCornerShape(10.dp)).background(AruColors.AccentTint), contentAlignment = Alignment.Center) {
            Icon(icon, null, tint = tint, modifier = Modifier.size(20.dp))
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

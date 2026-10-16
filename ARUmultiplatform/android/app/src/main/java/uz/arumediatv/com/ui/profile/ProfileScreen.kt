package uz.arumediatv.com.ui.profile

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
import uz.arumediatv.com.ui.common.PageHeader
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.theme.AruColors

/** Profil (Flutter `ProfileScreen` ga mos): hisob kartasi, Telegram holati, tahrirlash, qurilmalar, kesh, ilova haqida, chiqish. */
@Composable
fun ProfileScreen(onLogin: () -> Unit, onEdit: () -> Unit, onSessions: () -> Unit) {
    val auth = AppServices.auth
    val tg = AppServices.telegram
    val user by auth.user.collectAsStateWithLifecycle()
    val tgState by tg.state.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()

    var confirmLogout by remember { mutableStateOf(false) }
    var showAbout by remember { mutableStateOf(false) }
    var cacheBytes by remember { mutableStateOf(0L) }
    var busy by remember { mutableStateOf(false) }

    LaunchedEffect(user) {
        cacheBytes = withContext(Dispatchers.IO) { AppServices.images.diskBytes() }
    }

    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 4.dp)) {
        PageHeader("Profil", Modifier.padding(horizontal = 4.dp))
        Spacer(Modifier.height(8.dp))

        val u = user
        if (u == null) GuestCard(onLogin) else UserCard(u)

        Spacer(Modifier.height(16.dp))
        Group {
            Row2(
                Icons.Filled.Phone, "Telegram",
                when {
                    tgState.error != null && !tgState.started -> "Ishga tushmadi"
                    tgState.authorized -> "Ulangan — rasm va video ko'rinadi"
                    tgState.configUnreachable -> "Server bilan bog'lanib bo'lmadi"
                    else -> "Ulanmagan"
                },
            )
            if (u != null) {
                Divider2()
                Row2(Icons.Filled.Edit, "Profilni tahrirlash", "Ism va username", onClick = onEdit)
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
            Row2(Icons.Filled.Settings, "Hisobni yangilash", "Serverdan qayta o'qish", onClick = {
                if (u != null) scope.launch { auth.refresh() }
            })
            Divider2()
            Row2(Icons.Filled.Info, "Ilova haqida", "Versiya ${BuildConfig.VERSION_NAME}", onClick = { showAbout = true })
        }

        if (u != null) {
            Spacer(Modifier.height(16.dp))
            Group {
                Row2(Icons.Filled.ExitToApp, "Chiqish", "Bu qurilmadagi hisobdan chiqish", onClick = { confirmLogout = true }, danger = true)
            }
        }
        Spacer(Modifier.height(32.dp))
    }

    if (confirmLogout) {
        AlertDialog(
            onDismissRequest = { if (!busy) confirmLogout = false },
            title = { Text("Chiqish") },
            text = { Text("Hisobdan chiqasizmi? Rasm va videolarni ko'rish uchun qayta kirish kerak bo'ladi.") },
            confirmButton = {
                TextButton(
                    enabled = !busy,
                    onClick = {
                        busy = true
                        scope.launch {
                            auth.logout()
                            tg.logout()
                            AppServices.images.clearDisk()
                            busy = false
                            confirmLogout = false
                        }
                    },
                ) { Text(if (busy) "Chiqilmoqda..." else "Chiqish", color = Color(0xFFF87171)) }
            },
            dismissButton = { TextButton(enabled = !busy, onClick = { confirmLogout = false }) { Text("Bekor qilish") } },
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
private fun UserCard(u: AppUser) {
    Card2 {
        Row(Modifier.fillMaxWidth().padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
            Avatar(u, 64.dp)
            Spacer(Modifier.width(14.dp))
            Column(Modifier.weight(1f)) {
                Text(u.fullName, color = Color.White, fontSize = 18.sp, fontWeight = FontWeight.Bold, maxLines = 1)
                if (u.username.isNotEmpty()) Text("@${u.username}", color = AruColors.TextDim, fontSize = 13.sp)
                Spacer(Modifier.height(6.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Pill("ID ${u.id}")
                    Spacer(Modifier.width(8.dp))
                    Pill("Balans: ${u.balance}", highlight = true)
                    if (u.isAdmin) {
                        Spacer(Modifier.width(8.dp))
                        Pill("Admin", highlight = true)
                    }
                }
            }
        }
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

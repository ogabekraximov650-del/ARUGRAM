package uz.arumediatv.com.ui.admin

import androidx.compose.foundation.background
import uz.arumediatv.com.ui.common.AruIcons
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.material3.Icon
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.Icons
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.statusBarsPadding
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
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.ChatThread
import uz.arumediatv.com.data.ChatThreadsStore
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.profile.OverlayScaffold
import uz.arumediatv.com.ui.season.timeAgo
import uz.arumediatv.com.ui.theme.AruColors

/** Admin paneli bo'limlari (tartib va yozuvlar Flutter `AdminScreen` dagidek; faqat Foydalanuvchilar va Suhbatlar alohida). */
enum class AdminSection(val title: String, val hint: String, val icon: ImageVector) {
    ANIME("Animelarni boshqarish", "Qo'shish, tahrirlash, o'chirish", AruIcons.Movie),
    USERS("Foydalanuvchilar", "Balans, obuna, bloklash", AruIcons.People),
    CHATS("Suhbatlar", "Foydalanuvchilar yozishmalari", AruIcons.Chat),
    REPORTS("Shikoyatlar", "Izohlarga kelgan shikoyatlar", AruIcons.Flag),
    PACKS("Emoji, GIF va stikerlar", "Yuklangan rasmlarni ko'rib chiqish", AruIcons.Emoticon),
    CHANNELS("Majburiy obunalar", "Kanallar, limit va statistika", AruIcons.Campaign),
    STATS("Umumiy statistika", "Kontent, foydalanuvchilar, ko'rishlar, trafik", AruIcons.Insights),
    ENCODE("Kodlash navbati", "Kodlanayotgan va navbatdagi qismlar, log", AruIcons.Movie),
    APP("Ilova va xavfsizlik", "Eng past versiya, ulanish kaliti", AruIcons.Security),
}

/** Admin paneli (faqat admin build'da va server admin deb belgilagan hisobda). */
@Composable
fun AdminHubScreen(onBack: () -> Unit, onOpen: (AdminSection) -> Unit) {
    val unread by AppServices.unread.adminCount.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { AppServices.unread.refresh(force = true) }
    BackHandler(onBack = onBack)
    Column(Modifier.fillMaxSize().background(AruColors.Bg).statusBarsPadding()) {
        // ── Sarlavha ──
        val hs = RoundedCornerShape(20.dp)
        Row(
            Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 8.dp).fillMaxWidth().clip(hs).background(AruColors.Card).border(1.dp, AruColors.Border, hs)
                .padding(horizontal = 18.dp, vertical = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            val bs = RoundedCornerShape(14.dp)
            Box(Modifier.clip(bs).background(AruColors.CardAlt).border(1.dp, AruColors.Border, bs).clickable(onClick = onBack).padding(8.dp)) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, "Orqaga", tint = Color.White, modifier = Modifier.size(24.dp))
            }
            Spacer(Modifier.width(14.dp))
            Text("Admin paneli", color = Color.White, fontSize = 22.sp, fontWeight = FontWeight.Bold, modifier = Modifier.weight(1f))
            Icon(AruIcons.AdminPanel, null, tint = Color.White.copy(alpha = 0.54f), modifier = Modifier.size(24.dp))
        }
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 32.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            AdminSection.entries.forEach { s ->
                val shape = RoundedCornerShape(20.dp)
                val badge = if (s == AdminSection.CHATS) unread else 0
                Row(
                    Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).clickable { onOpen(s) }
                        .padding(horizontal = 20.dp, vertical = 18.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(Modifier.clip(RoundedCornerShape(14.dp)).background(AruColors.Accent2.copy(alpha = 0.14f)).padding(12.dp)) {
                        Icon(s.icon, null, tint = AruColors.Accent2, modifier = Modifier.size(26.dp))
                    }
                    Spacer(Modifier.width(16.dp))
                    Column(Modifier.weight(1f)) {
                        Text(s.title, color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
                        Spacer(Modifier.height(3.dp))
                        Text(
                            if (s == AdminSection.CHATS && unread > 0) "$unread ta o'qilmagan xabar" else s.hint,
                            color = Color.White.copy(alpha = 0.55f), fontSize = 12.sp,
                        )
                    }
                    if (badge > 0) {
                        Text(
                            "$badge", color = Color.White, fontSize = 11.5.sp, fontWeight = FontWeight.ExtraBold, textAlign = TextAlign.Center,
                            modifier = Modifier.defaultMinSize(minWidth = 22.dp).clip(RoundedCornerShape(11.dp)).background(AruColors.Accent2).padding(horizontal = 7.dp, vertical = 3.dp),
                        )
                        Spacer(Modifier.width(6.dp))
                    }
                    Icon(AruIcons.ChevronRight, null, tint = Color.White.copy(alpha = 0.38f), modifier = Modifier.size(24.dp))
                }
            }
        }
    }
}

/** Admin: barcha suhbatlar. Bosilsa yozishma ochiladi; uzoq bosilsa o'chirish so'raladi. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun AdminThreadsScreen(onBack: () -> Unit, onOpenThread: (ChatThread) -> Unit) {
    val store = remember { ChatThreadsStore(AppServices.auth) }
    val items by store.items.collectAsStateWithLifecycle()
    val err by store.error.collectAsStateWithLifecycle()
    var loaded by remember { mutableStateOf(false) }
    var del by remember { mutableStateOf<ChatThread?>(null) }
    val scope = rememberCoroutineScope()
    LaunchedEffect(Unit) { store.load(); loaded = true }

    OverlayScaffold("Suhbatlar", onBack) {
        when {
            !loaded -> LoadingBox()
            items.isEmpty() -> EmptyState(text = err.ifEmpty { "Suhbatlar yo'q" })
            else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 16.dp, vertical = 4.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(items, key = { it.userId }) { t ->
                    val shape = RoundedCornerShape(16.dp)
                    Row(
                        Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).combinedClickable(onClick = { onOpenThread(t) }, onLongClick = { del = t }).padding(12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Box(Modifier.size(44.dp).clip(CircleShape).background(AruColors.AccentTint), contentAlignment = Alignment.Center) {
                            TgImage(t.photoUrl, Modifier.fillMaxSize(), maxPx = 128, placeholder = { Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { Text(t.name.removePrefix("@").take(1).uppercase(), color = AruColors.Accent2, fontWeight = FontWeight.Bold) } })
                        }
                        Spacer(Modifier.width(12.dp))
                        Column(Modifier.weight(1f)) {
                            Text(t.name, color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text((if (t.lastFromAdmin) "Siz: " else "") + t.lastBody.ifEmpty { "Fayl" }, color = AruColors.TextDim, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                        Column(horizontalAlignment = Alignment.End) {
                            Text(timeAgo(t.lastAt), color = AruColors.TextFaint, fontSize = 10.sp)
                            if (t.unread > 0) Text("${t.unread}", color = Color.White, fontSize = 11.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(top = 4.dp).clip(CircleShape).background(AruColors.Accent).padding(horizontal = 8.dp, vertical = 2.dp))
                        }
                    }
                }
            }
        }
    }
    del?.let { t ->
        AlertDialog(
            onDismissRequest = { del = null },
            title = { Text("Suhbatni o'chirish") },
            text = { Text("${t.name} bilan butun yozishma o'chiriladi.") },
            confirmButton = { TextButton(onClick = { del = null; scope.launch { store.remove(t.userId) } }) { Text("O'chirish", color = Color(0xFFF87171)) } },
            dismissButton = { TextButton(onClick = { del = null }) { Text("Bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
}

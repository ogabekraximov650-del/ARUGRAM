package uz.arumediatv.com.ui.season

import android.widget.Toast
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.Comment
import uz.arumediatv.com.data.CommentSort
import uz.arumediatv.com.data.CommentsStore
import uz.arumediatv.com.data.Season
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.theme.AruColors

fun timeAgo(ms: Long): String {
    if (ms <= 0) return ""
    val m = (System.currentTimeMillis() - ms) / 60_000
    return when {
        m < 1 -> "hozir"
        m < 60 -> "$m daq oldin"
        m < 60 * 24 -> "${m / 60} soat oldin"
        m < 60 * 24 * 7 -> "${m / (60 * 24)} kun oldin"
        else -> formatYmd(ms)
    }
}

/** Izohlar: tartiblash, yozish, javob, layk, o'chirish, shikoyat, jonli yangilanish. */
@Composable
fun CommentsTab(season: Season, onOpenProfile: (Long) -> Unit, onLogin: () -> Unit, onPacks: () -> Unit = {}) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val store = remember(season.key) { CommentsStore(ctx.applicationContext, AppServices.auth, season.animeId, season.seasonId) }
    val st by store.state.collectAsStateWithLifecycle()
    val user by AppServices.auth.user.collectAsStateWithLifecycle()
    var text by remember { mutableStateOf("") }
    var replyTo by remember { mutableStateOf<Comment?>(null) }
    var sending by remember { mutableStateOf(false) }
    var menuFor by remember { mutableStateOf<Comment?>(null) }
    var reportFor by remember { mutableStateOf<Comment?>(null) }
    var deleteFor by remember { mutableStateOf<Comment?>(null) }
    var accepted by remember { mutableStateOf(false) }
    var picker by remember { mutableStateOf(false) }

    LaunchedEffect(store) {
        store.loadFromDisk()
        store.load()
    }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    DisposableEffect(store, lifecycle) {
        if (lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) store.startWatching()
        val ob = LifecycleEventObserver { _, e ->
            if (e == Lifecycle.Event.ON_RESUME) store.startWatching() else if (e == Lifecycle.Event.ON_PAUSE) store.stopWatching()
        }
        lifecycle.addObserver(ob)
        onDispose { lifecycle.removeObserver(ob); store.close() }
    }
    LaunchedEffect(accepted) {
        if (accepted) {
            kotlinx.coroutines.delay(10_000)
            accepted = false
        }
    }
    fun toast(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_SHORT).show()

    val listState = rememberLazyListState()
    val nearEnd by remember { derivedStateOf { val l = listState.layoutInfo; l.totalItemsCount > 0 && (l.visibleItemsInfo.lastOrNull()?.index ?: 0) >= l.totalItemsCount - 3 } }
    LaunchedEffect(nearEnd) { if (nearEnd) store.loadMore() }

    Column(Modifier.fillMaxSize().imePadding()) {
        // Tartib
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            CommentSort.entries.forEach { s ->
                val on = st.sort == s
                Text(
                    s.label, color = if (on) Color.White else AruColors.TextDim, fontSize = 12.sp, fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(if (on) AruColors.Accent else AruColors.Card).clickable { store.setSort(s) }.padding(horizontal = 12.dp, vertical = 6.dp),
                )
            }
            Spacer(Modifier.weight(1f))
            Text("${st.total} ta", color = AruColors.TextFaint, fontSize = 12.sp)
        }
        if (accepted) {
            Text(
                "Shikoyatingiz qabul qilindi. Tez orada tekshirib chiqamiz.",
                color = Color(0xFF4ADE80), fontSize = 12.sp,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 6.dp).fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(Color(0x2022C55E)).padding(12.dp),
            )
        }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            when {
                st.loading && st.items.isEmpty() -> LoadingBox()
                st.items.isEmpty() -> EmptyState(Icons.Filled.Menu, st.error ?: "Hali izoh yo'q — birinchi bo'ling")
                else -> LazyColumn(Modifier.fillMaxSize(), state = listState, contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 12.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                    items(st.items, key = { it.id }) { c ->
                        Column {
                            CommentRow(
                                c, mine = user?.id == c.userId, admin = user?.isAdmin == true,
                                onProfile = { onOpenProfile(c.userId) },
                                onLike = { scope.launch { store.toggleLike(c.id)?.let(::toast) } },
                                onReply = { if (user == null) onLogin() else replyTo = c },
                                onMenu = { menuFor = c },
                                onDelete = { deleteFor = c },
                            )
                            if (c.replyCount > 0) {
                                val open = st.replies.containsKey(c.id)
                                Text(
                                    if (open) "Javoblarni yashirish" else "${c.replyCount} ta javobni ko'rish",
                                    color = AruColors.Accent2, fontSize = 12.sp, fontWeight = FontWeight.SemiBold,
                                    modifier = Modifier.padding(start = 46.dp, top = 6.dp).clickable { store.toggleReplies(c.id) },
                                )
                                if (c.id in st.loadingReplies) CircularProgressIndicator(color = AruColors.Accent, strokeWidth = 2.dp, modifier = Modifier.padding(start = 46.dp, top = 6.dp).size(18.dp))
                                st.replies[c.id]?.forEach { r ->
                                    Box(Modifier.padding(start = 40.dp, top = 10.dp)) {
                                        CommentRow(
                                            r, mine = user?.id == r.userId, admin = user?.isAdmin == true,
                                            onProfile = { onOpenProfile(r.userId) },
                                            onLike = { scope.launch { store.toggleLike(r.id)?.let(::toast) } },
                                            onReply = { if (user == null) onLogin() else replyTo = c },
                                            onMenu = { menuFor = r },
                                            onDelete = { deleteFor = r },
                                            small = true,
                                        )
                                    }
                                }
                            }
                        }
                    }
                    if (st.loadingMore) item { LoadingBox(Modifier.height(60.dp)) }
                }
            }
        }

        // Yozish maydoni
        if (user == null) {
            Text(
                "Izoh yozish uchun hisobga kiring", color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.SemiBold,
                modifier = Modifier.padding(16.dp).fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(AruColors.Accent).clickable(onClick = onLogin).padding(vertical = 13.dp),
                textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            )
        } else {
            Column(Modifier.fillMaxWidth().background(AruColors.Surface).padding(horizontal = 12.dp, vertical = 8.dp)) {
                replyTo?.let { r ->
                    Row(Modifier.fillMaxWidth().padding(bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text("Javob: ${r.name}", color = AruColors.Accent2, fontSize = 12.sp, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Icon(Icons.Filled.Close, "Bekor qilish", tint = AruColors.TextDim, modifier = Modifier.size(18.dp).clickable { replyTo = null })
                    }
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(42.dp).clip(CircleShape).background(AruColors.CardAlt).clickable { picker = true }, contentAlignment = Alignment.Center) {
                        Text("☺", color = Color.White, fontSize = 20.sp)
                    }
                    Spacer(Modifier.width(6.dp))
                    OutlinedTextField(
                        value = text, onValueChange = { text = it.take(1000) }, placeholder = { Text("Izoh yozing...", color = AruColors.TextFaint) },
                        maxLines = 4, shape = RoundedCornerShape(18.dp),
                        colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White, focusedBorderColor = AruColors.Accent, unfocusedBorderColor = AruColors.BorderBright),
                        modifier = Modifier.weight(1f),
                    )
                    Spacer(Modifier.width(8.dp))
                    Box(
                        Modifier.size(46.dp).clip(CircleShape).background(if (text.isBlank() || sending) AruColors.CardAlt else AruColors.Accent)
                            .clickable(enabled = text.isNotBlank() && !sending) {
                                sending = true
                                scope.launch {
                                    val parent = replyTo?.id.orEmpty()
                                    val err = store.add(text, parentId = parent)
                                    sending = false
                                    if (err == null) { text = ""; replyTo = null; if (parent.isEmpty()) listState.scrollToItem(0) } else toast(err)
                                }
                            },
                        contentAlignment = Alignment.Center,
                    ) { Icon(Icons.AutoMirrored.Filled.Send, "Yuborish", tint = Color.White, modifier = Modifier.size(22.dp)) }
                }
            }
        }
    }

    // ── Stiker / GIF / emoji tanlash ──
    if (picker) {
        uz.arumediatv.com.ui.packs.PackPickerSheet(
            onDismiss = { picker = false },
            onManage = onPacks,
            onPick = { pick ->
                if (pick.kind == uz.arumediatv.com.data.PackKind.EMOJI) text += uz.arumediatv.com.data.packEmojiToken(pick.pack, pick.item, pick.emoji)
                else scope.launch {
                    val parent = replyTo?.id.orEmpty()
                    val err = store.add("", parentId = parent, mediaFile = pick.ref, mediaType = if (pick.kind == uz.arumediatv.com.data.PackKind.GIF) "gif" else "sticker")
                    if (err == null) replyTo = null else toast(err)
                }
            },
        )
    }

    // ── Menyu: shikoyat ──
    menuFor?.let { c ->
        AlertDialog(
            onDismissRequest = { menuFor = null },
            title = { Text(c.name) },
            text = { Text(commentPreview(c), maxLines = 4, overflow = TextOverflow.Ellipsis) },
            confirmButton = { TextButton(onClick = { reportFor = c; menuFor = null }) { Text("Shikoyat qilish", color = Color(0xFFF87171)) } },
            dismissButton = { TextButton(onClick = { menuFor = null }) { Text("Yopish") } },
            containerColor = AruColors.Surface,
        )
    }
    reportFor?.let { c ->
        var reason by remember { mutableStateOf("") }
        var busy by remember { mutableStateOf(false) }
        AlertDialog(
            onDismissRequest = { if (!busy) reportFor = null },
            title = { Text("Shikoyat") },
            text = {
                OutlinedTextField(
                    value = reason, onValueChange = { reason = it.take(2000) }, label = { Text("Sababini yozing (kamida 10 belgi)") }, minLines = 3,
                    colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White),
                )
            },
            confirmButton = {
                TextButton(enabled = !busy, onClick = {
                    busy = true
                    scope.launch {
                        val err = store.report(c.id, reason)
                        busy = false
                        if (err == null) { reportFor = null; accepted = true } else toast(err)
                    }
                }) { Text(if (busy) "Yuborilmoqda..." else "Yuborish") }
            },
            dismissButton = { TextButton(enabled = !busy, onClick = { reportFor = null }) { Text("Bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
    deleteFor?.let { c ->
        AlertDialog(
            onDismissRequest = { deleteFor = null },
            title = { Text("Izohni o'chirish") },
            text = { Text("Izoh (va javoblari) butunlay o'chiriladi.") },
            confirmButton = { TextButton(onClick = { deleteFor = null; scope.launch { store.remove(c.id)?.let(::toast) } }) { Text("O'chirish", color = Color(0xFFF87171)) } },
            dismissButton = { TextButton(onClick = { deleteFor = null }) { Text("Bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
}

fun commentPreview(c: Comment): String = c.body.ifEmpty { when (c.mediaType) { "sticker" -> "Stiker"; "gif" -> "GIF"; else -> "" } }

@Composable
private fun CommentRow(
    c: Comment, mine: Boolean, admin: Boolean, small: Boolean = false,
    onProfile: () -> Unit, onLike: () -> Unit, onReply: () -> Unit, onMenu: () -> Unit, onDelete: () -> Unit,
) {
    val av = if (small) 28.dp else 34.dp
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
        Box(Modifier.size(av).clip(CircleShape).background(AruColors.AccentTint).clickable(onClick = onProfile), contentAlignment = Alignment.Center) {
            TgImage(
                url = c.photoUrl, modifier = Modifier.fillMaxSize(), maxPx = 120,
                placeholder = { Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { Text(c.name.removePrefix("@").take(1).uppercase(), color = AruColors.Accent2, fontWeight = FontWeight.Bold, fontSize = 13.sp) } },
            )
        }
        Spacer(Modifier.width(10.dp))
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(c.name, color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.clickable(onClick = onProfile))
                Spacer(Modifier.width(8.dp))
                Text(timeAgo(c.createdAt), color = AruColors.TextFaint, fontSize = 11.sp)
            }
            if (c.mediaFile.isNotEmpty() && (c.mediaType == "sticker" || c.mediaType == "gif")) {
                uz.arumediatv.com.ui.packs.PackRefImage(c.mediaFile, Modifier.padding(top = 4.dp).size(if (c.mediaType == "sticker") 130.dp else 180.dp))
            }
            if (c.body.isNotEmpty()) uz.arumediatv.com.ui.packs.RichPackText(c.body, Color.White.copy(alpha = 0.88f), 14.sp, Modifier.padding(top = 2.dp), 20.sp)
            Row(Modifier.padding(top = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                Row(Modifier.clickable(onClick = onLike), verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Filled.Favorite, "Layk", tint = if (c.liked) AruColors.Accent2 else Color.White.copy(alpha = 0.35f), modifier = Modifier.size(16.dp))
                    if (c.likes > 0) Text(" ${c.likes}", color = AruColors.TextDim, fontSize = 12.sp)
                }
                if (!c.isReply) Text("Javob", color = AruColors.TextDim, fontSize = 12.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.clickable(onClick = onReply))
                if (mine || admin) Icon(Icons.Filled.Delete, "O'chirish", tint = AruColors.TextFaint, modifier = Modifier.size(16.dp).clickable(onClick = onDelete))
                if (!mine) Icon(Icons.Filled.Warning, "Shikoyat", tint = AruColors.TextFaint, modifier = Modifier.size(16.dp).clickable(onClick = onMenu))
            }
        }
    }
}

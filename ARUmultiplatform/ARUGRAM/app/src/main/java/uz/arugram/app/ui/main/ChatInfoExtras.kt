package uz.arugram.app.ui.main

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicText
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uz.arugram.app.R
import uz.arugram.app.core.RustCore
import uz.arugram.app.ui.components.TgIcon
import uz.arugram.app.ui.theme.LocalTg

/** Guruh/kanal a'zolari ro'yxati (DrKLO `ProfileActivity` a'zolar bo'limi); a'zo bosilsa shaxsiy suhbat ochiladi. */
@Composable
@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
fun MembersSection(chat: ChatRow, onOpen: (ChatRow) -> Unit) {
    val c = LocalTg.current
    var members by remember { mutableStateOf<List<ChatRow>?>(null) }
    var roles by remember { mutableStateOf<Map<Long, String>>(emptyMap()) }
    var error by remember { mutableStateOf<String?>(null) }
    var actionFor by remember { mutableStateOf<ChatRow?>(null) }
    var reload by remember { mutableStateOf(0) }
    val scope = rememberCoroutineScope()
    LaunchedEffect(chat.id, reload) {
        val j = withContext(Dispatchers.IO) { RustCore.tgMembers(chat.spec) }
        val err = j.optString("error")
        if (err.isNotEmpty()) { error = err; members = emptyList(); return@LaunchedEffect }
        val a = j.optJSONArray("members")
        val rs = HashMap<Long, String>()
        members = buildList {
            for (i in 0 until (a?.length() ?: 0)) {
                val o = a!!.getJSONObject(i)
                rs[o.optLong("id")] = o.optString("role")
                add(ChatRow(o.optLong("id"), "user", o.optString("name"), false, 0, false, false, 0, "", o.optBoolean("bot"), false, false, access = o.optString("hash", "0")))
            }
        }
        roles = rs
    }
    Column(Modifier.fillMaxWidth().background(c.bar).padding(top = 6.dp)) {
        BasicText("A’zolar", Modifier.padding(start = 18.dp, top = 8.dp, bottom = 6.dp), style = TextStyle(color = c.value, fontSize = 15.sp, fontWeight = FontWeight.Medium))
        val l = members
        if (l == null) Box(Modifier.fillMaxWidth().padding(20.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(26.dp), color = c.inputActive, strokeWidth = 3.dp) }
        else if (l.isEmpty()) BasicText(error ?: "A’zolar ro‘yxati ko‘rinmaydi", Modifier.padding(18.dp), style = TextStyle(color = c.gray, fontSize = 14.sp))
        else l.forEach { m ->
            Row(Modifier.fillMaxWidth().height(56.dp).combinedClickable(onClick = { onOpen(m) }, onLongClick = { actionFor = m }).padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                ChatAvatar(m, 40.dp)
                BasicText(m.name, Modifier.padding(start = 12.dp).weight(1f), style = TextStyle(color = c.text, fontSize = 16.sp), maxLines = 1, overflow = TextOverflow.Ellipsis)
                val role = roles[m.id].orEmpty()
                if (role.isNotEmpty()) BasicText(role, style = TextStyle(color = c.gray, fontSize = 13.sp))
            }
        }
    }
    val ctx = LocalContext.current
    actionFor?.let { m ->
        MemberActions(chat, m, onDone = { msg -> actionFor = null; reload++; android.widget.Toast.makeText(ctx, msg, android.widget.Toast.LENGTH_SHORT).show() }, onDismiss = { actionFor = null })
    }
}

/** A'zo ustida uzoq bosganda: chiqarish / bloklash / admin qilish (huquq yetmasa server xato qaytaradi). */
@Composable
private fun MemberActions(chat: ChatRow, m: ChatRow, onDone: (String) -> Unit, onDismiss: () -> Unit) {
    val scope = rememberCoroutineScope()
    fun act(a: String, ok: String) = scope.launch {
        val r = withContext(Dispatchers.IO) { RustCore.tgMemberAct(chat.spec, "${m.id}@${m.access}", a) }
        onDone(r.optString("error").ifEmpty { ok })
    }
    PopupMenu(
        listOf(
            MenuItem(R.drawable.msg_leave, "Guruhdan chiqarish", danger = true) { act("kick", "A’zo chiqarildi") },
            MenuItem(R.drawable.msg_block, "Bloklash", danger = true) { act("ban", "A’zo bloklandi") },
            MenuItem(R.drawable.msg_block, "Blokdan chiqarish") { act("unban", "Blok olib tashlandi") },
            MenuItem(R.drawable.msg_addbot, "Admin qilish") { act("admin", "Admin qilindi") },
            MenuItem(R.drawable.msg_addbot, "Adminlikdan olish") { act("demote", "Adminlik olib tashlandi") },
        ),
        onDismiss,
    )
}

/** Ulashilgan media (DrKLO `SharedMediaLayout`): Media / Fayllar / Havolalar / Ovozli yorliqlari. */
@Composable
fun SharedMediaSection(chat: ChatRow, onOpenMedia: (MsgRow) -> Unit) {
    val c = LocalTg.current
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val tabs = listOf("Media" to "photovideo", "Fayllar" to "docs", "Havolalar" to "links", "Ovozli" to "voice")
    var tab by remember { mutableStateOf(0) }
    var items by remember { mutableStateOf<List<MsgRow>?>(null) }
    LaunchedEffect(chat.id, tab) {
        items = null
        val j = withContext(Dispatchers.IO) { RustCore.tgShared(chat.spec, tabs[tab].second) }
        val a = j.optJSONArray("messages")
        items = buildList { for (i in 0 until (a?.length() ?: 0)) add(parseMsg(a!!.getJSONObject(i))) }
    }
    Column(Modifier.fillMaxWidth().background(c.bar).padding(top = 6.dp)) {
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState())) {
            tabs.forEachIndexed { i, (title, _) ->
                BasicText(
                    title, Modifier.clickable { tab = i }.padding(horizontal = 16.dp, vertical = 12.dp),
                    style = TextStyle(color = if (tab == i) Color(0xFF3390EC) else c.gray, fontSize = 15.sp, fontWeight = FontWeight.Medium),
                )
            }
        }
        val l = items
        when {
            l == null -> Box(Modifier.fillMaxWidth().padding(20.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(26.dp), color = c.inputActive, strokeWidth = 3.dp) }
            l.isEmpty() -> BasicText("Bu yerda hozircha hech narsa yo‘q", Modifier.padding(18.dp), style = TextStyle(color = c.gray, fontSize = 14.sp))
            tab == 0 -> l.chunked(3).forEach { rowItems ->
                Row(Modifier.fillMaxWidth()) {
                    rowItems.forEach { m ->
                        val k = MediaCache.key(chat.id, m.id)
                        LaunchedEffect(m.id) { if (m.kind == "photo") MediaCache.photo(ctx, chat, m.id) }
                        Box(Modifier.weight(1f).aspectRatio(1f).padding(1.dp).background(Color(0xFF2B2B2B)).clickable { onOpenMedia(m) }) {
                            val img = MediaCache.photos[k]
                            if (img != null) Image(img, null, Modifier.matchParentSize(), contentScale = ContentScale.Crop)
                            else if (m.kind == "video") TgIcon(R.drawable.ic_play, Color.White, Modifier.align(Alignment.Center), 28.dp)
                        }
                    }
                    repeat(3 - rowItems.size) { Box(Modifier.weight(1f)) }
                }
            }
            else -> l.forEach { m ->
                Column(
                    Modifier.fillMaxWidth().clickable {
                        if (tab == 2) {
                            val url = Regex("""https?://\S+""").find(m.text)?.value
                            if (url != null) openUrl(ctx, url)
                        } else scope.launch { MediaCache.file(ctx, chat, m.id)?.let { f -> if (m.kind == "voice" || m.kind == "audio") VoicePlayer.toggle(MediaCache.key(chat.id, m.id), f.file) else openFile(ctx, f.file, f.mime) } }
                    }.padding(horizontal = 18.dp, vertical = 8.dp),
                ) {
                    val title = when (tab) { 1 -> m.name.ifEmpty { "Fayl" }; 2 -> m.text.lines().firstOrNull().orEmpty(); else -> "Ovozli xabar  ${formatDuration(m.dur)}" }
                    BasicText(title, style = TextStyle(color = c.text, fontSize = 15.sp), maxLines = 2, overflow = TextOverflow.Ellipsis)
                    BasicText(if (tab == 1) "${formatSize(m.size)} · ${dateLabel(m.date)}" else dateLabel(m.date), style = TextStyle(color = c.gray, fontSize = 13.sp))
                }
            }
        }
    }
}

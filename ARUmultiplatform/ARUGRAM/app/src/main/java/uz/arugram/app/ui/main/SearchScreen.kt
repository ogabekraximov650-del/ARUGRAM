package uz.arugram.app.ui.main

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import uz.arugram.app.core.RustCore
import uz.arugram.app.ui.components.BackArrowIcon
import uz.arugram.app.ui.components.CloseIcon
import uz.arugram.app.ui.theme.LocalTg

/**
 * Global qidiruv (DrKLO `DialogsSearchAdapter`): yuqorida o'zimdagi suhbatlar (nom bo'yicha), pastda Telegram serveridagi odamlar, guruh va kanallar
 * (`contacts.search`, @foydalanuvchi_nomi bo'yicha ham). Natija bosilganda suhbat ochiladi.
 */
@Composable
fun SearchScreen(chats: List<ChatRow>, onOpen: (ChatRow) -> Unit, onBack: () -> Unit) {
    val c = LocalTg.current
    BackHandler(onBack = onBack)
    var query by remember { mutableStateOf("") }
    var remote by remember { mutableStateOf<List<ChatRow>>(emptyList()) }
    var searching by remember { mutableStateOf(false) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    LaunchedEffect(query) {
        if (query.trim().length < 2) { remote = emptyList(); searching = false; return@LaunchedEffect }
        searching = true
        delay(450)
        val j = withContext(Dispatchers.IO) { RustCore.tgSearch(query.trim()) }
        val arr = j.optJSONArray("results")
        remote = buildList {
            for (i in 0 until (arr?.length() ?: 0)) {
                val o = arr!!.getJSONObject(i)
                add(
                    ChatRow(
                        id = o.optLong("id"), kind = o.optString("kind"), name = o.optString("name"), pinned = false, unread = 0, muted = false, out = false, date = 0,
                        text = o.optString("username").let { if (it.isNotEmpty()) "@$it" else "" }, bot = o.optBoolean("bot"), self = false, verified = false,
                        access = o.optString("hash", "0"),
                    ),
                )
            }
        }
        searching = false
    }
    val local = if (query.isBlank()) emptyList() else chats.filter { it.name.contains(query.trim(), ignoreCase = true) }
    val localIds = local.map { it.id }.toSet()

    Column(Modifier.fillMaxSize().background(c.bg)) {
        Row(Modifier.fillMaxWidth().background(c.bar).statusBarsPadding().height(56.dp + 0.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(56.dp).clickable(onClick = onBack), contentAlignment = Alignment.Center) { BackArrowIcon(c.text) }
            Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
                if (query.isEmpty()) BasicText("Qidirish", style = TextStyle(color = c.hint, fontSize = 18.sp))
                BasicTextField(query, { query = it }, singleLine = true, textStyle = TextStyle(color = c.text, fontSize = 18.sp), cursorBrush = SolidColor(c.inputActive), modifier = Modifier.fillMaxWidth().focusRequester(focus))
            }
            if (query.isNotEmpty()) Box(Modifier.size(56.dp).clickable { query = "" }, contentAlignment = Alignment.Center) { CloseIcon(c.text) }
        }
        LazyColumn(Modifier.fillMaxSize()) {
            if (local.isNotEmpty()) {
                item { SectionTitle("Suhbatlar") }
                items(local, key = { "l${it.id}" }) { r -> ResultRow(r, onOpen) }
            }
            val rem = remote.filter { it.id !in localIds }
            if (rem.isNotEmpty()) {
                item { SectionTitle("Global qidiruv") }
                items(rem, key = { "r${it.id}" }) { r -> ResultRow(r, onOpen) }
            }
            if (query.isNotBlank() && local.isEmpty() && rem.isEmpty() && !searching) {
                item { BasicText("Hech narsa topilmadi", Modifier.fillMaxWidth().padding(32.dp), style = TextStyle(color = c.gray, fontSize = 16.sp, textAlign = androidx.compose.ui.text.style.TextAlign.Center)) }
            }
        }
    }
}

@Composable
private fun SectionTitle(text: String) {
    val c = LocalTg.current
    Box(Modifier.fillMaxWidth().background(c.grayBg).padding(start = 16.dp, top = 8.dp, bottom = 8.dp)) {
        BasicText(text, style = TextStyle(color = c.value, fontSize = 14.sp, fontWeight = FontWeight.Medium))
    }
}

@Composable
private fun ResultRow(r: ChatRow, onOpen: (ChatRow) -> Unit) {
    val c = LocalTg.current
    Row(Modifier.fillMaxWidth().height(60.dp).clickable { onOpen(r) }.padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        ChatAvatar(r, 44.dp)
        Column(Modifier.padding(start = 12.dp)) {
            BasicText(r.name, style = TextStyle(color = c.text, fontSize = 16.sp, fontWeight = FontWeight.Medium), maxLines = 1, overflow = TextOverflow.Ellipsis)
            val sub = if (r.text.isNotEmpty()) r.text else when (r.kind) { "channel" -> "kanal"; "group" -> "guruh"; else -> "" }
            if (sub.isNotEmpty()) BasicText(sub, style = TextStyle(color = c.gray, fontSize = 14.sp), maxLines = 1)
        }
    }
}

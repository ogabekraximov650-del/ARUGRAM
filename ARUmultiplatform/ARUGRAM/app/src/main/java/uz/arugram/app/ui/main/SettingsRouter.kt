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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uz.arugram.app.core.RustCore
import uz.arugram.app.ui.components.BackArrowIcon
import uz.arugram.app.ui.theme.LocalTg

/** Sozlamalar ichki sahifalarini nomi bo'yicha ochadi. */
@Composable
fun SettingsRouter(page: String, onBack: () -> Unit, onNavigate: (String) -> Unit, folders: List<Folder>) {
    val c = LocalTg.current
    Box(Modifier.fillMaxSize().background(c.grayBg)) {
        when (page) {
            "account" -> AccountPage(onBack)
            "chat" -> ChatSettingsPage(onBack)
            "privacy" -> PrivacyPage(onBack) { onNavigate("devices") }
            "notifications" -> NotificationsPage(onBack)
            "data" -> DataPage(onBack)
            "folders" -> FoldersPage(onBack, folders)
            "devices" -> DevicesPage(onBack)
            "power" -> PowerPage(onBack)
            "language" -> LanguagePage(onBack)
            "newchannel" -> NewChannelScreen(onBack)
            else -> onBack()
        }
    }
}

/** Yangi guruh (DrKLO `GroupCreateActivity`): kontaktlardan a'zolarni belgilash + nom. */
@Composable
fun NewGroupScreen(onBack: () -> Unit, onDone: () -> Unit, target: ChatRow? = null) {
    val c = LocalTg.current
    val scope = rememberCoroutineScope()
    BackHandler(onBack = onBack)
    var contacts by remember { mutableStateOf<List<Triple<Long, String, String>>?>(null) }
    val picked = remember { mutableStateListOf<Long>() }
    var title by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var msg by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) {
        val j = withContext(Dispatchers.IO) { RustCore.tgContacts() }
        val arr = j.optJSONArray("contacts")
        contacts = buildList { for (i in 0 until (arr?.length() ?: 0)) { val o = arr!!.getJSONObject(i); add(Triple(o.optLong("id"), o.optString("name"), o.optString("hash"))) } }.sortedBy { it.second.lowercase() }
    }
    LaunchedEffect(msg) { if (msg != null) { delay(3000); msg = null } }
    Column(Modifier.fillMaxSize().background(c.bg).statusBarsPadding()) {
        Row(Modifier.fillMaxWidth().height(56.dp).background(c.bar), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(56.dp).clickable(onClick = onBack), contentAlignment = Alignment.Center) { BackArrowIcon(c.text) }
            BasicText(if (target == null) "Yangi guruh" else "A’zo qo‘shish", Modifier.weight(1f), style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold))
            BasicText(
                if (busy) "..." else if (target == null) "Yaratish" else "Qo‘shish",
                Modifier.clickable(enabled = !busy) {
                    busy = true
                    val members = contacts.orEmpty().filter { it.first in picked }.joinToString(",") { "${it.first}@${it.third}" }
                    scope.launch {
                        val r = withContext(Dispatchers.IO) { if (target == null) RustCore.tgCreateGroup(title, members) else RustCore.tgAddMembers(target.spec, members) }
                        busy = false
                        val err = r.optString("error")
                        if (err.isEmpty()) onDone() else msg = err
                    }
                }.padding(16.dp),
                style = TextStyle(color = Color(0xFF3390EC), fontSize = 16.sp, fontWeight = FontWeight.Bold),
            )
        }
        if (target == null) Box(Modifier.fillMaxWidth().background(c.bar).padding(horizontal = 18.dp, vertical = 12.dp)) {
            if (title.isEmpty()) BasicText("Guruh nomi", style = TextStyle(color = c.hint, fontSize = 18.sp))
            BasicTextField(title, { title = it }, singleLine = true, textStyle = TextStyle(color = c.text, fontSize = 18.sp), cursorBrush = SolidColor(c.inputActive), modifier = Modifier.fillMaxWidth())
        }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            LazyColumn(Modifier.fillMaxSize()) {
                items(contacts.orEmpty(), key = { it.first }) { (id, name, hash) ->
                    val row = ChatRow(id, "user", name, false, 0, false, false, 0, "", false, false, false, access = hash)
                    val on = id in picked
                    Row(Modifier.fillMaxWidth().height(58.dp).clickable { if (on) picked.remove(id) else picked.add(id) }.padding(horizontal = 14.dp), verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.size(22.dp).clip(CircleShape).background(if (on) Color(0xFF3390EC) else c.input), contentAlignment = Alignment.Center) {
                            if (on) BasicText("✓", style = TextStyle(color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold))
                        }
                        Box(Modifier.padding(start = 12.dp)) { ChatAvatar(row, 40.dp) }
                        BasicText(name, Modifier.padding(start = 12.dp), style = TextStyle(color = c.text, fontSize = 16.sp), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
            Bulletin(msg, Modifier.align(Alignment.BottomCenter).padding(bottom = 24.dp))
        }
    }
}

/** Yangi kanal (DrKLO `ChannelCreateActivity`): nom va tavsif. */
@Composable
fun NewChannelScreen(onBack: () -> Unit) {
    val c = LocalTg.current
    val scope = rememberCoroutineScope()
    BackHandler(onBack = onBack)
    var title by remember { mutableStateOf("") }
    var about by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var msg by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(msg) { if (msg != null) { delay(3000); msg = null } }
    Box(Modifier.fillMaxSize().background(c.grayBg)) {
        Column(Modifier.fillMaxSize().statusBarsPadding()) {
            Row(Modifier.fillMaxWidth().height(56.dp).background(c.bar), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(56.dp).clickable(onClick = onBack), contentAlignment = Alignment.Center) { BackArrowIcon(c.text) }
                BasicText("Yangi kanal", Modifier.weight(1f), style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold))
                BasicText(
                    if (busy) "..." else "Yaratish",
                    Modifier.clickable(enabled = !busy) {
                        busy = true
                        scope.launch {
                            val r = withContext(Dispatchers.IO) { RustCore.tgCreateChannel(title, about) }
                            busy = false
                            val err = r.optString("error")
                            if (err.isEmpty()) onBack() else msg = err
                        }
                    }.padding(16.dp),
                    style = TextStyle(color = Color(0xFF3390EC), fontSize = 16.sp, fontWeight = FontWeight.Bold),
                )
            }
            Column(Modifier.padding(top = 8.dp).fillMaxWidth().background(c.bar).padding(horizontal = 18.dp, vertical = 14.dp)) {
                Box {
                    if (title.isEmpty()) BasicText("Kanal nomi", style = TextStyle(color = c.hint, fontSize = 18.sp))
                    BasicTextField(title, { title = it }, singleLine = true, textStyle = TextStyle(color = c.text, fontSize = 18.sp), cursorBrush = SolidColor(c.inputActive), modifier = Modifier.fillMaxWidth())
                }
                Box(Modifier.padding(top = 18.dp)) {
                    if (about.isEmpty()) BasicText("Tavsif (ixtiyoriy)", style = TextStyle(color = c.hint, fontSize = 16.sp))
                    BasicTextField(about, { about = it }, textStyle = TextStyle(color = c.text, fontSize = 16.sp), cursorBrush = SolidColor(c.inputActive), modifier = Modifier.fillMaxWidth())
                }
            }
        }
        Bulletin(msg, Modifier.align(Alignment.BottomCenter).padding(bottom = 24.dp))
    }
}

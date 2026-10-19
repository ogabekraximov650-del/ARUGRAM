package uz.arugram.app.ui.main

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
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
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.verticalScroll
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uz.arugram.app.R
import uz.arugram.app.core.RustCore
import uz.arugram.app.ui.components.BackArrowIcon
import uz.arugram.app.ui.components.TgIcon
import uz.arugram.app.ui.theme.LocalTg

/**
 * Suhbat / foydalanuvchi ma'lumoti (DrKLO `ProfileActivity`): katta avatar, nom, holat; "Ma'lumot" bo'limida telefon, @nom, bio; amallar —
 * bildirishnoma, xabar yozish, suhbatni o'chirish.
 */
@Composable
fun ChatInfo(chat: ChatRow, onBack: () -> Unit, onWrite: () -> Unit, onDeleted: () -> Unit, onOpenChat: (ChatRow) -> Unit = {}, onAddMembers: () -> Unit = {}) {
    val c = LocalTg.current
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    BackHandler(onBack = onBack)
    var info by remember { mutableStateOf<org.json.JSONObject?>(null) }
    var confirmDelete by remember { mutableStateOf(false) }
    var muted by remember(chat.id) { mutableStateOf(chat.muted) }
    var blocked by remember(chat.id) { mutableStateOf(false) }
    var viewer by remember { mutableStateOf<MsgRow?>(null) }
    var editChat by remember { mutableStateOf(false) }
    var inviteLink by remember { mutableStateOf<String?>(null) }
    var confirmClear by remember { mutableStateOf(false) }
    var contactForm by remember { mutableStateOf(false) }
    var photosOpen by remember { mutableStateOf(false) }
    var isContact by remember(chat.id) { mutableStateOf<Boolean?>(null) }
    var toast by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(toast) { if (toast != null) { kotlinx.coroutines.delay(2500); toast = null } }
    LaunchedEffect(chat.id) { if (!chat.isSecret) info = withContext(Dispatchers.IO) { RustCore.tgPeerInfo(chat.spec) } }
    val i = info
    LaunchedEffect(i) { if (i != null && i.has("contact")) isContact = i.optBoolean("contact") }
    val kind = i?.optString("kind").orEmpty().ifEmpty { chat.kind }
    val status = when (kind) {
        "user" -> if (chat.bot) "bot" else statusText(i?.optString("status").orEmpty())
        "group" -> "${i?.optInt("members") ?: 0} a’zo"
        "channel" -> "${compactCount(i?.optInt("members") ?: 0)} obunachi"
        "secret" -> "end-to-end shifrlangan maxfiy chat"
        else -> ""
    }
    Column(Modifier.fillMaxSize().background(c.grayBg).statusBarsPadding()) {
        Row(Modifier.fillMaxWidth().height(56.dp).background(c.bar), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(56.dp).clickable(onClick = onBack), contentAlignment = Alignment.Center) { BackArrowIcon(c.text) }
        }
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState())) {
            Column(Modifier.fillMaxWidth().background(c.bar).padding(bottom = 20.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Box(Modifier.clickable(enabled = !chat.isSecret) { photosOpen = true }) { ChatAvatar(chat, 96.dp) }
                BasicText(chat.name, Modifier.padding(top = 12.dp, start = 24.dp, end = 24.dp), style = TextStyle(color = c.text, fontSize = 22.sp, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center))
                if (status.isNotEmpty()) BasicText(status, Modifier.padding(top = 4.dp), style = TextStyle(color = if (status == "onlayn") Color(0xFF3390EC) else c.gray, fontSize = 15.sp))
            }
            Spacer(Modifier.height(8.dp))
            val rows = buildList {
                i?.optString("phone")?.takeIf { it.isNotEmpty() }?.let { add("Telefon" to "+$it") }
                i?.optString("username")?.takeIf { it.isNotEmpty() }?.let { add("Foydalanuvchi nomi" to "@$it") }
                i?.optString("about")?.takeIf { it.isNotEmpty() }?.let { add((if (kind == "user") "Bio" else "Tavsif") to it) }
            }
            if (rows.isNotEmpty()) {
                Column(Modifier.fillMaxWidth().background(c.bar).padding(vertical = 6.dp)) {
                    BasicText("Ma’lumot", Modifier.padding(start = 18.dp, top = 8.dp, bottom = 4.dp), style = TextStyle(color = c.value, fontSize = 15.sp, fontWeight = FontWeight.Medium))
                    rows.forEach { (k, v) ->
                        Column(Modifier.fillMaxWidth().clickable { copyText(ctx, v) }.padding(horizontal = 18.dp, vertical = 8.dp)) {
                            BasicText(v, style = TextStyle(color = c.text, fontSize = 16.sp))
                            BasicText(k, style = TextStyle(color = c.gray, fontSize = 13.sp))
                        }
                    }
                }
                Spacer(Modifier.height(8.dp))
            }
            if (chat.isSecret) Column(Modifier.fillMaxWidth().background(c.bar)) {
                InfoAction(R.drawable.msg_send, "Xabar yozish") { onWrite() }
                InfoAction(R.drawable.msg_delete, "Maxfiy chatni tugatish", danger = true) { confirmDelete = true }
            } else Column(Modifier.fillMaxWidth().background(c.bar)) {
                InfoAction(R.drawable.msg_send, "Xabar yozish") { onWrite() }
                if (kind == "user" && !chat.self && !chat.bot && isContact != null) InfoAction(R.drawable.msg_contacts, if (isContact == true) "Kontaktlardan o‘chirish" else "Kontaktlarga qo‘shish", danger = isContact == true) {
                    if (isContact == true) scope.launch {
                        val r = withContext(Dispatchers.IO) { RustCore.tgContactEdit("${chat.id}@${chat.access}", "", "", true) }
                        toast = r.optString("error").ifEmpty { isContact = false; "Kontaktlardan o‘chirildi" }
                    } else contactForm = true
                }
                if (kind == "user" && !chat.self && !chat.bot) InfoAction(R.drawable.msg_secret, "Maxfiy chat boshlash") {
                    scope.launch {
                        val r = withContext(Dispatchers.IO) { RustCore.tgSecretStart(chat.id, chat.access, chat.name) }
                        toast = r.optString("error").ifEmpty { "Maxfiy chat so‘rovi yuborildi — suhbatdosh qabul qilgach ro‘yxatda ochiladi" }
                    }
                }
                InfoAction(if (muted) R.drawable.msg_unmute else R.drawable.msg_mute, if (muted) "Bildirishnomani yoqish" else "Bildirishnomani o‘chirish") {
                    muted = !muted
                    scope.launch { withContext(Dispatchers.IO) { RustCore.tgMute(chat.spec, muted) } }
                }
                if (kind == "group" || kind == "channel") {
                    InfoAction(R.drawable.msg_edit, "Tahrirlash") { editChat = true }
                    InfoAction(R.drawable.msg_addbot, "A’zo qo‘shish") { onAddMembers() }
                    InfoAction(R.drawable.msg_link, "Taklif havolasi") {
                        val un = i?.optString("username").orEmpty()
                        if (un.isNotEmpty()) inviteLink = "https://t.me/$un"
                        else scope.launch { val r = withContext(Dispatchers.IO) { RustCore.tgInviteLink(chat.spec) }; val l = r.optString("link"); if (l.isNotEmpty()) inviteLink = l else toast = r.optString("error").ifEmpty { "Havola olinmadi" } }
                    }
                }
                InfoAction(R.drawable.msg_clear, "Tarixni tozalash") { confirmClear = true }
                if (kind == "user" && !chat.self) InfoAction(R.drawable.msg_block, if (blocked) "Blokdan chiqarish" else "Bloklash", danger = !blocked) {
                    blocked = !blocked
                    scope.launch { withContext(Dispatchers.IO) { RustCore.tgBlock(chat.spec, blocked) } }
                }
                InfoAction(R.drawable.msg_delete, "Suhbatni o‘chirish", danger = true) { confirmDelete = true }
            }
            if (kind == "group" || kind == "channel") {
                Spacer(Modifier.height(8.dp))
                MembersSection(chat, onOpenChat)
            }
            Spacer(Modifier.height(8.dp))
            if (!chat.isSecret) SharedMediaSection(chat) { viewer = it }
            Spacer(Modifier.height(24.dp))
        }
    }
    viewer?.let { MediaViewer(chat, it) { viewer = null } }
    if (photosOpen) ProfilePhotosViewer(chat, onClose = { photosOpen = false })
    if (contactForm) {
        TgForm("Kontaktlarga qo‘shish", listOf("Ism", "Familiya (ixtiyoriy)"), "Qo‘shish", initial = listOf(chat.name.substringBefore(' '), chat.name.substringAfter(' ', "")), onConfirm = { v ->
            contactForm = false
            scope.launch {
                val r = withContext(Dispatchers.IO) { RustCore.tgContactEdit("${chat.id}@${chat.access}", v[0], v[1], false) }
                toast = r.optString("error").ifEmpty { isContact = true; "Kontaktlarga qo‘shildi" }
            }
        }, onDismiss = { contactForm = false })
    }
    Bulletin(toast, Modifier.fillMaxWidth().padding(top = 80.dp))
    if (editChat) {
        TgForm("Tahrirlash", listOf("Nom", "Tavsif"), "Saqlash", initial = listOf(chat.name, i?.optString("about").orEmpty()), onConfirm = { v ->
            editChat = false
            scope.launch {
                val r = withContext(Dispatchers.IO) { RustCore.tgEditChat(chat.spec, v[0], v[1]) }
                toast = r.optString("error").ifEmpty { "Saqlandi" }
            }
        }, onDismiss = { editChat = false })
    }
    inviteLink?.let { l ->
        TgAlert("Taklif havolasi", l, "Nusxalash", onConfirm = { copyText(ctx, l); inviteLink = null; toast = "Nusxalandi" }, onDismiss = { inviteLink = null })
    }
    if (confirmClear) {
        TgAlert("Tarixni tozalash", "Suhbatdagi barcha xabarlar sizdan o‘chiriladi.", "Tozalash", danger = true, onConfirm = {
            confirmClear = false
            scope.launch { val r = withContext(Dispatchers.IO) { RustCore.tgClearHistory(chat.spec) }; toast = r.optString("error").ifEmpty { "Tarix tozalandi" } }
        }, onDismiss = { confirmClear = false })
    }
    if (confirmDelete) {
        TgAlert(
            title = "Suhbatni o‘chirish", text = "“${chat.name}” suhbati sizdan o‘chiriladi (guruh yoki kanal bo‘lsa — undan chiqasiz).",
            confirm = "O‘chirish", danger = true,
            onConfirm = { confirmDelete = false; scope.launch { withContext(Dispatchers.IO) { chat.deleteRemote() }; onDeleted() } },
            onDismiss = { confirmDelete = false },
        )
    }
}

@Composable
private fun InfoAction(icon: Int, label: String, danger: Boolean = false, onClick: () -> Unit) {
    val c = LocalTg.current
    Row(Modifier.fillMaxWidth().height(50.dp).clickable(onClick = onClick).padding(horizontal = 18.dp), verticalAlignment = Alignment.CenterVertically) {
        TgIcon(icon, if (danger) c.red else c.text.copy(alpha = 0.7f))
        BasicText(label, Modifier.padding(start = 18.dp), style = TextStyle(color = if (danger) c.red else c.text, fontSize = 16.sp))
    }
}

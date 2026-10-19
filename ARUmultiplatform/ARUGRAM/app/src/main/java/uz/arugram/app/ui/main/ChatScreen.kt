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
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uz.arugram.app.R
import uz.arugram.app.core.RustCore
import uz.arugram.app.ui.components.BackArrowIcon
import uz.arugram.app.ui.components.CloseIcon
import uz.arugram.app.ui.components.SearchIcon
import uz.arugram.app.ui.components.TgIcon
import uz.arugram.app.ui.theme.LocalTg

private val Blue = Color(0xFF3390EC)

/**
 * Suhbat oynasi (DrKLO `ChatActivity`): tepada orqaga / avatar / nom va holat ("yozmoqda...", a'zolar soni, onlayn), qidiruv va menyu; tanlash rejimida
 * javob / nusxa / uzatish / o'chirish. Mahkamlangan xabar paneli, kun ajratgichlari, pufakchalar, pastga tushirish tugmasi, xabar yozish paneli.
 */
@Composable
fun ChatScreen(chat: ChatRow, chats: List<ChatRow>, typing: Boolean, onBack: () -> Unit, onOpenInfo: () -> Unit) {
    val c = LocalTg.current
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val model = remember(chat.id) { ChatModel(chat, ctx.applicationContext) }
    DisposableEffect(chat.id) { onDispose { model.close(); VoicePlayer.stop() } }
    // Qoralama: suhbatdan chiqilganda saqlanadi (ro'yxatda ko'rinadi), yuborilganda o'chadi.
    var draftText by remember { mutableStateOf("") }
    DisposableEffect(chat.id) { onDispose { Drafts.save(ctx, chat.id, draftText) } }

    val selected = remember { mutableStateListOf<Int>() }
    var menuFor by remember { mutableStateOf<MsgRow?>(null) }
    var replyTo by remember { mutableStateOf<MsgRow?>(null) }
    var editing by remember { mutableStateOf<MsgRow?>(null) }
    val draftPrefs = remember { ctx.getSharedPreferences("drafts", android.content.Context.MODE_PRIVATE) }
    var draft by remember { mutableStateOf(TextFieldValue(draftPrefs.getString("d_${chat.id}", "").orEmpty())) }
    DisposableEffect(chat.id) { onDispose { } }
    var viewer by remember { mutableStateOf<MsgRow?>(null) }
    var forwardIds by remember { mutableStateOf<List<Int>?>(null) }
    var deleteIds by remember { mutableStateOf<List<Int>?>(null) }
    var chatMenu by remember { mutableStateOf(false) }
    var confirmDeleteChat by remember { mutableStateOf(false) }
    var searching by remember { mutableStateOf(false) }
    var query by remember { mutableStateOf("") }
    var subtitle by remember { mutableStateOf("") }
    var bulletin by remember { mutableStateOf<String?>(null) }
    var joined by remember { mutableStateOf(false) }
    val listState = rememberLazyListState()
    val canWrite = chat.kind != "channel" || chat.self
    val openLink = LocalOpenLink.current
    fun press(m: MsgRow, b: Button) {
        when (b.kind) {
            "url" -> if (!openLink(b.data)) openUrl(ctx, b.data)
            "cb" -> model.pressButton(m, b) { if (it.isNotEmpty()) bulletin = it }
            "text" -> model.send(b.data, 0)
        }
    }

    LaunchedEffect(chat.id) {
        model.markRead()
        model.loadPinned()
        val j = withContext(Dispatchers.IO) { RustCore.tgPeerInfo(chat.spec) }
        if (j.optString("error").isEmpty()) {
            subtitle = when (j.optString("kind")) {
                "user" -> if (chat.bot) "bot" else statusText(j.optString("status"))
                "group" -> "${j.optInt("members")} a’zo"
                "channel" -> "${compactCount(j.optInt("members"))} obunachi"
                else -> ""
            }
        }
    }
    // Yangi xabarlar: Rust yadrosining updates hisoblagichi o'zgarganda (yoki har 60 soniyada) suhbat yangilanadi.
    LaunchedEffect(chat.id) {
        var last = -1L
        var since = 0
        while (true) {
            val seq = withContext(Dispatchers.IO) { RustCore.tgUpdateSeq() }
            if (last == -1L || (seq != last && since >= 2) || since >= 40) {
                last = seq
                since = 0
                model.refresh()
            }
            since++
            delay(1_500)
        }
    }
    LaunchedEffect(model.error) {
        val e = model.error
        if (e != null) {
            bulletin = e
            delay(3500)
            if (bulletin == e) bulletin = null
            model.clearError()
        }
    }
    // Eski xabarlarni yuklash: ro'yxat oxiriga yaqinlashganda.
    val nearEnd by remember { derivedStateOf { listState.layoutInfo.visibleItemsInfo.lastOrNull()?.index?.let { it >= model.messages.size - 6 } ?: false } }
    LaunchedEffect(nearEnd, model.messages.size) { if (nearEnd) model.loadOlder() }
    LaunchedEffect(query) { delay(350); model.search(query) }

    BackHandler {
        when {
            menuFor != null -> menuFor = null
            selected.isNotEmpty() -> selected.clear()
            searching -> { searching = false; query = ""; model.clearSearch() }
            editing != null -> { editing = null; draft = TextFieldValue("") }
            replyTo != null -> replyTo = null
            else -> onBack()
        }
    }

    androidx.compose.runtime.SideEffect { draftText = if (editing == null) draft.text else "" }
    fun doSend() {
        val text = draft.text.trim()
        if (text.isEmpty()) return
        val e = editing
        val r = replyTo
        draft = TextFieldValue("")
        editing = null
        replyTo = null
        if (e != null) model.edit(e, text) else model.send(text, r?.id ?: 0) { draft = TextFieldValue(text) }
        scope.launch { listState.animateScrollToItem(0) }
    }

    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize().chatWallpaper()) {
            // ── Tepa panel ──
            Column(Modifier.fillMaxWidth().background(c.bar).statusBarsPadding()) {
                Row(Modifier.fillMaxWidth().height(56.dp), verticalAlignment = Alignment.CenterVertically) {
                    when {
                        selected.isNotEmpty() -> {
                            Box(Modifier.size(56.dp).clickable { selected.clear() }, contentAlignment = Alignment.Center) { CloseIcon(c.text) }
                            BasicText("${selected.size}", Modifier.weight(1f), style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold))
                            val sel = model.messages.filter { it.id in selected }
                            if (selected.size == 1 && canWrite) Box(Modifier.size(48.dp).clickable { replyTo = sel.firstOrNull(); selected.clear() }, contentAlignment = Alignment.Center) { TgIcon(R.drawable.input_reply, c.text) }
                            Box(Modifier.size(48.dp).clickable { copyText(ctx, sel.sortedBy { it.id }.joinToString("\n") { it.text }); selected.clear(); bulletin = "Nusxalandi" }, contentAlignment = Alignment.Center) { TgIcon(R.drawable.msg_copy, c.text) }
                            Box(Modifier.size(48.dp).clickable { forwardIds = selected.toList() }, contentAlignment = Alignment.Center) { TgIcon(R.drawable.input_forward, c.text) }
                            Box(Modifier.size(48.dp).clickable { deleteIds = selected.toList() }, contentAlignment = Alignment.Center) { TgIcon(R.drawable.msg_delete, c.text) }
                        }
                        searching -> {
                            val focus = remember { FocusRequester() }
                            LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
                            Box(Modifier.size(56.dp).clickable { searching = false; query = ""; model.clearSearch() }, contentAlignment = Alignment.Center) { BackArrowIcon(c.text) }
                            Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
                                if (query.isEmpty()) BasicText("Qidirish", style = TextStyle(color = c.hint, fontSize = 18.sp))
                                BasicTextField(query, { query = it }, singleLine = true, textStyle = TextStyle(color = c.text, fontSize = 18.sp), cursorBrush = SolidColor(c.inputActive), modifier = Modifier.fillMaxWidth().focusRequester(focus))
                            }
                            if (query.isNotEmpty()) Box(Modifier.size(56.dp).clickable { query = "" }, contentAlignment = Alignment.Center) { CloseIcon(c.text) }
                        }
                        else -> {
                            Box(Modifier.size(56.dp).clickable(onClick = onBack), contentAlignment = Alignment.Center) { BackArrowIcon(c.text) }
                            Row(Modifier.weight(1f).clickable(onClick = onOpenInfo), verticalAlignment = Alignment.CenterVertically) {
                                ChatAvatar(chat, 40.dp)
                                Column(Modifier.padding(start = 12.dp)) {
                                    BasicText(chat.name, style = TextStyle(color = c.text, fontSize = 17.sp, fontWeight = FontWeight.Bold), maxLines = 1, overflow = TextOverflow.Ellipsis)
                                    val sub = if (typing) "yozmoqda..." else subtitle
                                    if (sub.isNotEmpty()) BasicText(sub, style = TextStyle(color = if (typing || sub == "onlayn") Blue else c.gray, fontSize = 13.sp), maxLines = 1)
                                }
                            }
                            Box(Modifier.size(48.dp).clickable { searching = true }, contentAlignment = Alignment.Center) { SearchIcon(c.text) }
                            Box(Modifier.size(48.dp).clickable { chatMenu = true }, contentAlignment = Alignment.Center) { TgIcon(R.drawable.ic_ab_other, c.text) }
                        }
                    }
                }
                // Mahkamlangan xabar paneli
                val pin = model.pinned
                if (pin != null && !searching && selected.isEmpty()) {
                    Row(
                        Modifier.fillMaxWidth().height(40.dp).clickable {
                            val idx = model.messages.indexOfFirst { it.id == pin.id }
                            if (idx >= 0) scope.launch { listState.animateScrollToItem(idx) }
                        }.padding(horizontal = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Box(Modifier.size(2.dp, 28.dp).background(Blue))
                        Column(Modifier.padding(start = 8.dp)) {
                            BasicText("Mahkamlangan xabar", style = TextStyle(color = Blue, fontSize = 13.sp, fontWeight = FontWeight.Medium))
                            BasicText(pin.text.ifEmpty { pin.media }, style = TextStyle(color = c.text, fontSize = 14.sp), maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                    }
                }
            }

            // ── Xabarlar ──
            Box(Modifier.weight(1f).fillMaxWidth()) {
                val results = model.searchResults
                val list = results ?: model.messages
                when {
                    !model.loaded -> CircularProgressIndicator(Modifier.align(Alignment.Center).size(32.dp), color = c.inputActive, strokeWidth = 3.dp)
                    list.isEmpty() -> BasicText(
                        if (results != null) "Hech narsa topilmadi" else "Xabarlar yo‘q",
                        Modifier.align(Alignment.Center).clip(RoundedCornerShape(12.dp)).background(Color(0x66000000)).padding(horizontal = 12.dp, vertical = 6.dp),
                        style = TextStyle(color = Color.White, fontSize = 14.sp, textAlign = TextAlign.Center),
                    )
                    else -> LazyColumn(Modifier.fillMaxSize(), state = listState, reverseLayout = true) {
                        itemsIndexed(list, key = { _, m -> m.id }) { i, m ->
                            val older = list.getOrNull(i + 1)
                            val replied = if (m.reply > 0) list.firstOrNull { it.id == m.reply } else null
                            Column {
                                if (older == null || dayKey(older.date) != dayKey(m.date)) DateChip(m.date)
                                MessageBubble(
                                    m = m, chat = chat, replied = replied, selected = m.id in selected,
                                    onClick = {
                                        if (selected.isNotEmpty()) { if (m.id in selected) selected.remove(m.id) else selected.add(m.id) }
                                        else if (m.kind == "photo" || m.kind == "video") viewer = m
                                    },
                                    onLongClick = { if (selected.isEmpty()) menuFor = m else if (m.id !in selected) selected.add(m.id) },
                                    onOpenMedia = { if (selected.isEmpty()) viewer = m else if (m.id in selected) selected.remove(m.id) else selected.add(m.id) },
                                    onReact = { model.react(m, it) },
                                    onVote = { model.vote(m, it) },
                                    onButton = { b -> press(m, b) },
                                    onReplyClick = { id -> val idx = list.indexOfFirst { it.id == id }; if (idx >= 0) scope.launch { listState.animateScrollToItem(idx) } },
                                )
                            }
                        }
                    }
                }
                // Pastga tushirish tugmasi
                val showDown by remember { derivedStateOf { listState.firstVisibleItemIndex > 3 } }
                if (showDown && results == null) {
                    Box(
                        Modifier.align(Alignment.BottomEnd).padding(12.dp).size(42.dp).shadow(4.dp, CircleShape).clip(CircleShape).background(c.bar)
                            .clickable { scope.launch { listState.animateScrollToItem(0) } },
                        contentAlignment = Alignment.Center,
                    ) { uz.arugram.app.ui.components.ChevronDownIcon(c.gray) }
                }
                Bulletin(bulletin, Modifier.align(Alignment.BottomCenter).padding(bottom = 8.dp))
            }

            // ── Bot javob klaviaturasi ──
            val kbMsg = model.messages.firstOrNull { it.markup != null }
            val kb = kbMsg?.markup
            if (kbMsg != null && kb != null && !kb.inline && kbMsg.id == model.messages.firstOrNull()?.id) {
                MarkupRows(kb, { b -> press(kbMsg, b) }, Modifier.fillMaxWidth().background(c.bar).padding(horizontal = 6.dp, vertical = 4.dp))
            }
            // ── Yozish paneli ──
            if (chat.date == 0L && chat.kind != "user" && !joined) {
                Box(Modifier.fillMaxWidth().background(c.bar).navigationBarsPadding().height(48.dp).clickable {
                    scope.launch {
                        val r = withContext(Dispatchers.IO) { RustCore.tgJoin(chat.spec) }
                        if (r.optString("error").isEmpty()) joined = true else bulletin = r.optString("error")
                    }
                }, contentAlignment = Alignment.Center) { BasicText("QO‘SHILISH", style = TextStyle(color = Blue, fontSize = 15.sp, fontWeight = FontWeight.Bold)) }
            } else ChatInputBar(
                draft = draft, onDraft = { if (it.text.length > draft.text.length) model.typing(); draft = it }, replyTo = replyTo, editing = editing, canWrite = canWrite,
                onCancelContext = { editing = null; replyTo = null; draft = TextFieldValue("") },
                onSend = ::doSend,
                onSendFile = { f, kind, extra -> model.sendFile(f, kind, replyTo?.id ?: 0, "", extra); replyTo = null; scope.launch { listState.animateScrollToItem(0) } },
                onSendVoice = { f, secs -> model.sendFile(f, "voice", replyTo?.id ?: 0, "", "$secs:0:0"); replyTo = null },
                onSendSticker = { st -> model.sendSticker(st.ref, replyTo?.id ?: 0); replyTo = null; scope.launch { listState.animateScrollToItem(0) } },
            )
        }

        // ── Oynalar ──
        menuFor?.let { m ->
            val items = buildList {
                if (canWrite) add(MenuItem(R.drawable.input_reply, "Javob") { replyTo = m; editing = null })
                if (m.text.isNotEmpty()) add(MenuItem(R.drawable.msg_copy, "Nusxalash") { copyText(ctx, m.text); bulletin = "Nusxalandi" })
                if (m.out && m.text.isNotEmpty() && m.kind.isEmpty()) add(MenuItem(R.drawable.msg_edit, "Tahrirlash") { editing = m; replyTo = null; draft = TextFieldValue(m.text, TextRange(m.text.length)) })
                add(MenuItem(if (m.pinned) R.drawable.msg_unpin else R.drawable.msg_pin, if (m.pinned) "Mahkamni olib tashlash" else "Mahkamlash") { model.pin(m, !m.pinned) })
                add(MenuItem(R.drawable.input_forward, "Uzatish") { forwardIds = listOf(m.id) })
                if (m.text.isNotEmpty()) add(MenuItem(R.drawable.msg_share, "Ulashish") {
                    runCatching { ctx.startActivity(android.content.Intent.createChooser(android.content.Intent(android.content.Intent.ACTION_SEND).setType("text/plain").putExtra(android.content.Intent.EXTRA_TEXT, m.text), null).addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)) }
                })
                add(MenuItem(R.drawable.msg_select, "Tanlash") { selected.add(m.id) })
                add(MenuItem(R.drawable.msg_delete, "O‘chirish", danger = true) { deleteIds = listOf(m.id) })
            }
            MessageActions(items, onReact = { model.react(m, it) }, onDismiss = { menuFor = null })
        }
        if (chatMenu) {
            PopupMenu(
                listOf(
                    MenuItem(R.drawable.msg_info, "Ma’lumot") { onOpenInfo() },
                    MenuItem(if (chat.muted) R.drawable.msg_unmute else R.drawable.msg_mute, if (chat.muted) "Bildirishnomani yoqish" else "Bildirishnomani o‘chirish") {
                        scope.launch { withContext(Dispatchers.IO) { RustCore.tgMute(chat.spec, !chat.muted) } }
                    },
                    MenuItem(R.drawable.msg_clear, "Tarixni tozalash") {
                        scope.launch { withContext(Dispatchers.IO) { RustCore.tgClearHistory(chat.spec) }; model.refresh(); bulletin = "Tarix tozalandi" }
                    },
                    MenuItem(R.drawable.msg_delete, "Suhbatni o‘chirish", danger = true) { confirmDeleteChat = true },
                ),
                onDismiss = { chatMenu = false },
            )
        }
        forwardIds?.let { ids ->
            ChatPicker("Uzatish", chats, onPick = { t -> model.forwardTo(t, ids); forwardIds = null; selected.clear(); bulletin = "Xabar uzatildi" }, onDismiss = { forwardIds = null })
        }
        deleteIds?.let { ids ->
            TgAlert(
                title = if (ids.size == 1) "Xabarni o‘chirish" else "${ids.size} ta xabarni o‘chirish",
                text = "Xabar suhbatdagi hamma uchun o‘chiriladi (agar imkon bo‘lsa).",
                confirm = "O‘chirish", danger = true,
                onConfirm = { model.delete(ids); selected.clear(); deleteIds = null },
                onDismiss = { deleteIds = null },
            )
        }
        if (confirmDeleteChat) {
            TgAlert(
                title = "Suhbatni o‘chirish",
                text = "“${chat.name}” suhbati sizdan o‘chiriladi (guruh yoki kanal bo‘lsa — undan chiqasiz).",
                confirm = "O‘chirish", danger = true,
                onConfirm = {
                    confirmDeleteChat = false
                    scope.launch { withContext(Dispatchers.IO) { RustCore.tgDeleteDialog(chat.spec) }; onBack() }
                },
                onDismiss = { confirmDeleteChat = false },
            )
        }
        viewer?.let { MediaViewer(chat, it) { viewer = null } }
    }
}

@Composable
fun DateChip(unix: Long) {
    Box(Modifier.fillMaxWidth().padding(vertical = 6.dp), contentAlignment = Alignment.Center) {
        BasicText(
            dateLabel(unix),
            Modifier.clip(RoundedCornerShape(12.dp)).background(Color(0x66000000)).padding(horizontal = 10.dp, vertical = 3.dp),
            style = TextStyle(color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.Medium),
        )
    }
}

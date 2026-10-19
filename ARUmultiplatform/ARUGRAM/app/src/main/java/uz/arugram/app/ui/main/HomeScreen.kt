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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.material3.CircularProgressIndicator
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
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uz.arugram.app.R
import uz.arugram.app.auth.AuthModel
import uz.arugram.app.core.RustCore
import uz.arugram.app.notify.NotifyService
import uz.arugram.app.ui.components.SearchIcon
import uz.arugram.app.ui.components.TgIcon
import uz.arugram.app.ui.theme.LocalTg

/** Ekranlar stegi (orqaga tugmasi bilan qaytiladi). */
private sealed interface Dest {
    data class Chat(val row: ChatRow) : Dest
    data class Info(val row: ChatRow) : Dest
    data object Search : Dest
    data object NewChat : Dest
    data object Archive : Dest
    data object NewGroup : Dest
    data class AddMembers(val row: ChatRow) : Dest
    data class Settings(val page: String) : Dest
}

/**
 * Kirgandan keyingi bosh oyna (DrKLO `MainTabsActivity` + `DialogsActivity`): pastda suzuvchi tab paneli (Chatlar / Kontaktlar / Sozlamalar / Profil),
 * Chatlar tabida — jild yorliqlari, qidiruv pilyuli, suhbatlar ro'yxati (bosib turib: mahkamlash, ovozsiz, o'chirish) va yangi xabar tugmasi.
 * Suhbat, ma'lumot, qidiruv, yangi chat/guruh va sozlamalar ichki oynalari shu yerda `Dest` steki bilan ochiladi.
 */
@Composable
fun HomeScreen(auth: AuthModel) {
    val c = LocalTg.current
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val model = remember { ChatsModel(ctx.applicationContext) }
    val ui by model.ui.collectAsStateWithLifecycle()
    val stack = remember { mutableStateListOf<Dest>() }
    var tab by androidx.compose.runtime.saveable.rememberSaveable { androidx.compose.runtime.mutableIntStateOf(0) }
    var me by remember { mutableStateOf<ChatRow?>(null) }
    var meContact by remember { mutableStateOf("") }
    var typing by remember { mutableStateOf<Map<Long, Long>>(emptyMap()) }
    var folders by remember { mutableStateOf<List<Folder>>(emptyList()) }
    var folder by remember { androidx.compose.runtime.mutableIntStateOf(0) }
    var menuFor by remember { mutableStateOf<ChatRow?>(null) }
    var deleteFor by remember { mutableStateOf<ChatRow?>(null) }
    var addContact by remember { mutableStateOf(false) }
    var toast by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(toast) { if (toast != null) { delay(3000); toast = null } }
    var stories by remember { mutableStateOf<List<StoryPeer>>(emptyList()) }
    var storyStart by remember { mutableStateOf<Int?>(null) }
    var archived by remember { mutableStateOf<List<ChatRow>>(emptyList()) }
    suspend fun loadArchived() {
        val j = withContext(Dispatchers.IO) { RustCore.tgArchived() }
        if (j.optString("error").isEmpty()) archived = model.parse(j.optJSONArray("dialogs"))
    }
    val storyPicker = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) scope.launch {
            toast = "Hikoya joylanmoqda..."
            val err = postStory(ctx, uri, "", "all")
            toast = err.ifEmpty { "Hikoya joylandi" }
            stories = parseStories(withContext(Dispatchers.IO) { RustCore.tgStories() })
        }
    }

    LaunchedEffect(Unit) {
        Drafts.load(ctx)
        val j = withContext(Dispatchers.IO) { RustCore.tgMe() }
        if (j.optString("error").isEmpty() && j.has("id")) {
            me = ChatRow(j.optLong("id"), "user", j.optString("name"), false, 0, false, false, 0, "", false, false, false)
            val phone = j.optString("phone").let { if (it.isNotEmpty()) "+$it" else "" }
            val user = j.optString("username").let { if (it.isNotEmpty()) "@$it" else "" }
            meContact = listOf(phone, user).filter { it.isNotEmpty() }.joinToString("  ·  ")
        }
        folders = parseFolders(withContext(Dispatchers.IO) { RustCore.tgFolders() })
        stories = parseStories(withContext(Dispatchers.IO) { RustCore.tgStories() })
        loadArchived()
        if (settingsPrefs(ctx).getBoolean("notify", true)) NotifyService.start(ctx)
    }
    LaunchedEffect(ui.chats.map { it.id }, me?.id) { AvatarCache.ensure(ctx, (ui.chats.filter { !it.self }.map { if (it.peerId != 0L) it.peerId else it.id } + listOfNotNull(me?.id)).distinct()) }
    // Yangilanishlar: Rust yadrosi kelgan updates sonini hisoblaydi; o'zgarsa ro'yxat darhol yangilanadi (kamida 3 soniya oraliq, zaxira — 60 soniya).
    LaunchedEffect(Unit) {
        var last = -1L
        var sinceLoad = 0
        while (true) {
            val seq = withContext(Dispatchers.IO) { RustCore.tgUpdateSeq() }
            if (last == -1L || (seq != last && sinceLoad >= 2) || sinceLoad >= 40) {
                last = seq
                sinceLoad = 0
                model.load()
            }
            val t = withContext(Dispatchers.IO) { RustCore.tgTyping() }
            typing = buildMap { for (k in t.keys()) put(k.toLong(), t.optLong(k)) }
            sinceLoad++
            delay(1_500)
        }
    }

    BackHandler(enabled = stack.isNotEmpty()) { stack.removeAt(stack.lastIndex) }
    fun open(r: ChatRow) { stack.add(Dest.Chat(r)) }
    val openLink: (String) -> Boolean = link@{ url ->
        val invite = tmeInvite(url)
        if (invite != null) {
            scope.launch {
                val r = withContext(Dispatchers.IO) { RustCore.tgImportInvite(invite) }
                toast = r.optString("error").ifEmpty { "Guruhga qo‘shildingiz" }
                model.load()
            }
            return@link true
        }
        val name = tmeUsername(url) ?: return@link false
        scope.launch {
            val j = withContext(Dispatchers.IO) { RustCore.tgResolve(name) }
            if (j.optString("error").isEmpty()) {
                open(ChatRow(j.optLong("id"), j.optString("kind"), j.optString("name"), false, 0, false, false, 0, "@$name", false, false, false, access = j.optString("hash", "0")))
            }
        }
        true
    }
    fun live(r: ChatRow) = ui.chats.firstOrNull { it.id == r.id } ?: r

    androidx.compose.runtime.CompositionLocalProvider(LocalOpenLink provides openLink) {
    Box(Modifier.fillMaxSize().background(c.bg)) {
        // ── Asosiy tablar ──
        Box(Modifier.fillMaxSize()) {
            Column(Modifier.fillMaxSize()) {
                Box(Modifier.weight(1f).fillMaxWidth()) {
                    when (MainTab.entries[tab]) {
                        MainTab.Chats -> ChatsTab(
                            ui = ui, folders = folders, folder = folder, onFolder = { folder = it }, typing = typing,
                            onSearch = { stack.add(Dest.Search) }, onOpen = ::open, onLongClick = { menuFor = it }, onRetry = { model.load() },
                            archivedCount = archived.size, onArchive = { stack.add(Dest.Archive) }, stories = stories, meId = me?.id, onStory = { storyStart = it }, onAddStory = { storyPicker.launch(arrayOf("image/*", "video/*")) },
                        )
                        MainTab.Contacts -> ContactsTab(bottomPadding = 80.dp, onOpen = ::open, extra = listOf("Kontakt qo‘shish" to { addContact = true }))
                        MainTab.Settings -> SettingsTab(me, meContact, 80.dp, onOpenChat = { r -> open(r) }, onNavigate = { stack.add(Dest.Settings(it)) }, onLogout = { auth.logout() }, savedChat = ui.chats.firstOrNull { it.self })
                        MainTab.Profile -> ProfileTab(me, meContact, 80.dp, onPhoto = { uri ->
                            scope.launch {
                                val f = withContext(Dispatchers.IO) { copyUriToCache(ctx, uri) }
                                if (f != null) {
                                    val r = withContext(Dispatchers.IO) { RustCore.tgSetPhoto(f.absolutePath) }
                                    f.delete()
                                    toast = r.optString("error").ifEmpty { "Profil rasmi yangilandi" }
                                    me?.let { AvatarCache.invalidate(ctx, it.id); AvatarCache.ensure(ctx, listOf(it.id)) }
                                }
                            }
                        }, onEdit = { stack.add(Dest.Settings("account")) }, onSaved = { ui.chats.firstOrNull { it.self }?.let(::open) }, onLogout = { auth.logout() })
                    }
                    if (MainTab.entries[tab] == MainTab.Chats) {
                        // Yangi xabar tugmasi (DrKLO `floatingButton`): pastki o'ngda, tab panelidan tepada.
                        Box(
                            Modifier.align(Alignment.BottomEnd).padding(end = 20.dp, bottom = 80.dp).size(56.dp).shadow(6.dp, CircleShape).clip(CircleShape).background(c.fab)
                                .clickable { stack.add(Dest.NewChat) },
                            contentAlignment = Alignment.Center,
                        ) { TgIcon(R.drawable.ic_ab_new, Color.White, size = 24.dp) }
                    }
                }
            }
            MainTabsBar(MainTab.entries[tab], { tab = it.ordinal }, me, Modifier.align(Alignment.BottomCenter).navigationBarsPadding())
        }

        // ── Ichki oynalar (stekning oxirgisi ko'rinadi) ──
        when (val d = stack.lastOrNull()) {
            is Dest.Chat -> {
                val row = live(d.row)
                ChatScreen(
                    chat = row, chats = ui.chats, typing = typing[row.id] != null,
                    onBack = { stack.removeAt(stack.lastIndex) },
                    onOpenInfo = { stack.add(Dest.Info(row)) },
                )
            }
            is Dest.Info -> ChatInfo(
                chat = live(d.row), onBack = { stack.removeAt(stack.lastIndex) },
                onWrite = { stack.removeAt(stack.lastIndex) },
                onDeleted = { stack.clear(); model.load() },
                onOpenChat = { r -> open(r) },
                onAddMembers = { stack.add(Dest.AddMembers(d.row)) },
            )
            Dest.Archive -> Column(Modifier.fillMaxSize().background(c.bg).statusBarsPadding()) {
                Row(Modifier.fillMaxWidth().height(56.dp).background(c.bar), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(56.dp).clickable { stack.removeAt(stack.lastIndex) }, contentAlignment = Alignment.Center) { uz.arugram.app.ui.components.BackArrowIcon(c.text) }
                    BasicText("Arxivlangan chatlar", style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold))
                }
                if (archived.isEmpty()) Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { BasicText("Arxiv bo‘sh", style = TextStyle(color = c.gray, fontSize = 17.sp)) }
                else ChatList(archived, typing = typing, onLongClick = { menuFor = it }, onOpen = ::open)
            }
            Dest.Search -> SearchScreen(ui.chats, onOpen = { r -> stack.removeAt(stack.lastIndex); open(r) }, onBack = { stack.removeAt(stack.lastIndex) })
            Dest.NewChat -> Box(Modifier.fillMaxSize().background(c.bg)) {
                ContactsTab(
                    bottomPadding = 0.dp, title = "Yangi xabar", onBack = { stack.removeAt(stack.lastIndex) },
                    onOpen = { r -> stack.removeAt(stack.lastIndex); open(r) },
                    extra = listOf(
                        "Yangi guruh" to { stack.removeAt(stack.lastIndex); stack.add(Dest.NewGroup) },
                        "Yangi kanal" to { stack.removeAt(stack.lastIndex); stack.add(Dest.Settings("newchannel")) },
                    ),
                )
            }
            is Dest.AddMembers -> NewGroupScreen(onBack = { stack.removeAt(stack.lastIndex) }, onDone = { stack.removeAt(stack.lastIndex) }, target = d.row)
            Dest.NewGroup -> NewGroupScreen(onBack = { stack.removeAt(stack.lastIndex) }, onDone = { stack.clear(); model.load() })
            is Dest.Settings -> SettingsRouter(d.page, onBack = { stack.removeAt(stack.lastIndex) }, onNavigate = { stack.add(Dest.Settings(it)) }, folders = folders)
            null -> {}
        }

        storyStart?.let { st -> StoryViewer(stories, st, onClose = { storyStart = null; scope.launch { stories = parseStories(withContext(Dispatchers.IO) { RustCore.tgStories() }) } }) }

        if (addContact) {
            TgForm("Yangi kontakt", listOf("Telefon raqami (+998...)", "Ism", "Familiya (ixtiyoriy)"), "Qo‘shish", onConfirm = { v ->
                addContact = false
                scope.launch {
                    val r = withContext(Dispatchers.IO) { RustCore.tgAddContact(v[0], v[1], v[2]) }
                    toast = r.optString("error").ifEmpty { "Kontakt qo‘shildi" }
                }
            }, onDismiss = { addContact = false })
        }
        Bulletin(toast, Modifier.align(Alignment.BottomCenter).padding(bottom = 100.dp))

        // ── Suhbat ro'yxati menyusi ──
        menuFor?.let { r ->
            PopupMenu(
                listOf(
                    MenuItem(if (r.pinned) R.drawable.msg_unpin else R.drawable.msg_pin, if (r.pinned) "Mahkamni olib tashlash" else "Mahkamlash") { model.pin(r) },
                    MenuItem(if (r.muted) R.drawable.msg_unmute else R.drawable.msg_mute, if (r.muted) "Bildirishnomani yoqish" else "Bildirishnomani o‘chirish") { model.mute(r) },
                    if (archived.any { it.id == r.id }) MenuItem(R.drawable.msg_unarchive, "Arxivdan chiqarish") { scope.launch { withContext(Dispatchers.IO) { RustCore.tgArchive(r.spec, false) }; model.load(); loadArchived() } }
                    else MenuItem(R.drawable.msg_archive, "Arxivlash") { scope.launch { withContext(Dispatchers.IO) { RustCore.tgArchive(r.spec, true) }; model.load(); loadArchived() } },
                    MenuItem(R.drawable.msg_markread, "O‘qilgan deb belgilash") { scope.launch { withContext(Dispatchers.IO) { RustCore.tgMarkRead(r.spec) }; model.load() } },
                    MenuItem(R.drawable.msg_delete, "O‘chirish", danger = true) { deleteFor = r },
                ),
                onDismiss = { menuFor = null },
            )
        }
        deleteFor?.let { r ->
            TgAlert("Suhbatni o‘chirish", "“${r.name}” suhbati sizdan o‘chiriladi (guruh yoki kanal bo‘lsa — undan chiqasiz).", "O‘chirish", danger = true,
                onConfirm = { model.deleteDialog(r); deleteFor = null }, onDismiss = { deleteFor = null })
        }
    }
    }
}

/** Chatlar tabi: tepa panel, jild yorliqlari, qidiruv pilyuli, ro'yxat. */
@Composable
private fun ChatsTab(
    ui: ChatsUi, folders: List<Folder>, folder: Int, onFolder: (Int) -> Unit, typing: Map<Long, Long>,
    onSearch: () -> Unit, onOpen: (ChatRow) -> Unit, onLongClick: (ChatRow) -> Unit, onRetry: () -> Unit,
    archivedCount: Int = 0, onArchive: () -> Unit = {},
    stories: List<StoryPeer> = emptyList(), meId: Long? = null, onStory: (Int) -> Unit = {}, onAddStory: () -> Unit = {},
) {
    val c = LocalTg.current
    Column(Modifier.fillMaxSize().background(c.bg)) {
        Column(Modifier.fillMaxWidth().background(c.bar).statusBarsPadding()) {
            Row(Modifier.fillMaxWidth().height(56.dp), verticalAlignment = Alignment.CenterVertically) {
                // DrKLO (`hasMainTabs`): sarlavha "logotip" rangida (#168BDB).
                BasicText("ARUGRAM", Modifier.weight(1f).padding(start = 20.dp), style = TextStyle(color = Color(0xFF168BDB), fontSize = 20.sp, fontWeight = FontWeight.Bold), maxLines = 1)
                Box(Modifier.size(56.dp).clickable(onClick = onSearch), contentAlignment = Alignment.Center) { SearchIcon(c.text) }
            }
            if (folders.isNotEmpty()) {
                val unread = folders.associate { f -> f.id to ui.chats.filter { f.matches(it) }.sumOf { if (it.muted) 0 else it.unread } } + (0 to ui.chats.sumOf { if (it.muted) 0 else it.unread })
                FolderTabs(folders, folder, unread, onFolder)
            }
            StoriesRow(stories, meId, onOpen = onStory, onAdd = onAddStory)
            if (archivedCount > 0) {
                Row(Modifier.fillMaxWidth().height(48.dp).clickable(onClick = onArchive).padding(horizontal = 18.dp), verticalAlignment = Alignment.CenterVertically) {
                    TgIcon(R.drawable.msg_archive, c.gray, size = 24.dp)
                    BasicText("Arxivlangan chatlar", Modifier.padding(start = 18.dp).weight(1f), style = TextStyle(color = c.text, fontSize = 16.sp))
                    BasicText(archivedCount.toString(), style = TextStyle(color = c.gray, fontSize = 14.sp))
                }
            }
        }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            when {
                ui.loading -> CircularProgressIndicator(Modifier.align(Alignment.Center).size(32.dp), color = c.inputActive, strokeWidth = 3.dp)
                ui.error != null && ui.chats.isEmpty() -> Column(Modifier.align(Alignment.Center).padding(horizontal = 32.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    BasicText(ui.error.orEmpty(), style = TextStyle(color = c.gray, fontSize = 15.sp, textAlign = TextAlign.Center))
                    BasicText("Qayta urinish", Modifier.clickable(onClick = onRetry).padding(16.dp), style = TextStyle(color = c.value, fontSize = 15.sp))
                }
                else -> {
                    val sel = folders.firstOrNull { it.id == folder }
                    val shown = if (sel == null) ui.chats else ui.chats.filter { sel.matches(it) }
                    if (shown.isEmpty()) BasicText(if (ui.chats.isEmpty()) "Suhbatlar yo‘q" else "Bu jildda suhbat yo‘q", Modifier.align(Alignment.Center), style = TextStyle(color = c.gray, fontSize = 17.sp))
                    else ChatList(shown, bottomPadding = 150.dp, typing = typing, onLongClick = onLongClick, onOpen = onOpen)
                }
            }
        }
    }
}

/** Profil tabi: katta avatar, ism, telefon/@nom, "Tahrirlash" va "Saqlangan xabarlar". */
@Composable
private fun ProfileTab(me: ChatRow?, meContact: String, bottomPadding: androidx.compose.ui.unit.Dp, onPhoto: (android.net.Uri) -> Unit, onEdit: () -> Unit, onSaved: () -> Unit, onLogout: () -> Unit) {
    val c = LocalTg.current
    val picker = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.GetContent()) { uri -> if (uri != null) onPhoto(uri) }
    Column(Modifier.fillMaxSize().background(c.grayBg).statusBarsPadding(), horizontalAlignment = Alignment.CenterHorizontally) {
        Box(Modifier.fillMaxWidth().height(56.dp).background(c.bar), contentAlignment = Alignment.CenterStart) {
            BasicText("Profil", Modifier.padding(start = 20.dp), style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold))
        }
        if (me != null) {
            Column(Modifier.fillMaxWidth().background(c.bar).padding(vertical = 24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Box(Modifier.clickable { picker.launch("image/*") }) { ChatAvatar(me, 96.dp) }
                BasicText("Rasmni o‘zgartirish", Modifier.padding(top = 6.dp), style = TextStyle(color = c.value, fontSize = 13.sp))
                BasicText(me.name, Modifier.padding(top = 8.dp), style = TextStyle(color = c.text, fontSize = 22.sp, fontWeight = FontWeight.Bold))
                if (meContact.isNotEmpty()) BasicText(meContact, Modifier.padding(top = 4.dp), style = TextStyle(color = c.gray, fontSize = 15.sp))
            }
        }
        Column(Modifier.padding(top = 8.dp).fillMaxWidth().background(c.bar)) {
            Row(Modifier.fillMaxWidth().height(50.dp).clickable(onClick = onEdit).padding(horizontal = 18.dp), verticalAlignment = Alignment.CenterVertically) {
                TgIcon(R.drawable.msg_edit, c.text.copy(alpha = 0.7f))
                BasicText("Profilni tahrirlash", Modifier.padding(start = 18.dp), style = TextStyle(color = c.text, fontSize = 16.sp))
            }
            Row(Modifier.fillMaxWidth().height(50.dp).clickable(onClick = onSaved).padding(horizontal = 18.dp), verticalAlignment = Alignment.CenterVertically) {
                TgIcon(R.drawable.msg_saved, c.text.copy(alpha = 0.7f))
                BasicText("Saqlangan xabarlar", Modifier.padding(start = 18.dp), style = TextStyle(color = c.text, fontSize = 16.sp))
            }
        }
        Box(Modifier.padding(top = 8.dp).fillMaxWidth().background(c.bar).clickable(onClick = onLogout).padding(vertical = 16.dp), contentAlignment = Alignment.Center) {
            BasicText("Hisobdan chiqish", style = TextStyle(color = c.red, fontSize = 16.sp))
        }
    }
}

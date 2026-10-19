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
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.layout.layout
import kotlin.math.roundToInt
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
    var muteFor by remember { mutableStateOf<ChatRow?>(null) }
    var addContact by remember { mutableStateOf(false) }
    var topMenu by remember { mutableStateOf(false) }
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

    val setPhoto: (android.net.Uri) -> Unit = { uri ->
                            scope.launch {
                                val f = withContext(Dispatchers.IO) { copyUriToCache(ctx, uri) }
                                if (f != null) {
                                    val r = withContext(Dispatchers.IO) { RustCore.tgSetPhoto(f.absolutePath) }
                                    f.delete()
                                    toast = r.optString("error").ifEmpty { "Profil rasmi yangilandi" }
                                    me?.let { AvatarCache.invalidate(ctx, it.id); AvatarCache.ensure(ctx, listOf(it.id)) }
                                }
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
    Box(Modifier.fillMaxSize().background(c.bg).trackPress()) {
        // ── Asosiy tablar ──
        Box(Modifier.fillMaxSize()) {
            Column(Modifier.fillMaxSize()) {
                Box(Modifier.weight(1f).fillMaxWidth()) {
                    when (MainTab.entries[tab]) {
                        MainTab.Chats -> ChatsTab(
                            ui = ui, folders = folders, folder = folder, onFolder = { folder = it }, typing = typing,
                            onSearch = { stack.add(Dest.Search) }, onOpen = ::open, onLongClick = { menuFor = it }, onRetry = { model.load() },
                            archivedCount = archived.size, onArchive = { stack.add(Dest.Archive) }, stories = stories, meId = me?.id, onStory = { storyStart = it }, onAddStory = { storyPicker.launch(arrayOf("image/*", "video/*")) }, onMenu = { topMenu = true },
                            archived = archived,
                            onSwipe = { r -> if (r.pinned) model.pin(r) else scope.launch { withContext(Dispatchers.IO) { RustCore.tgArchive(r.spec, true) }; model.load(); loadArchived() } },
                        )
                        MainTab.Contacts -> ContactsTab(bottomPadding = 80.dp, onOpen = ::open, extra = listOf("Kontakt qo‘shish" to { addContact = true }))
                        MainTab.Settings -> SettingsTab(me, meContact, 80.dp, onOpenChat = { r -> open(r) }, onNavigate = { stack.add(Dest.Settings(it)) }, onLogout = { auth.logout() }, savedChat = ui.chats.firstOrNull { it.self }, onPhoto = setPhoto)
                        MainTab.Profile -> ProfileTab(me, meContact, 80.dp, onPhoto = setPhoto, onEdit = { stack.add(Dest.Settings("account")) }, onSaved = { ui.chats.firstOrNull { it.self }?.let(::open) }, onLogout = { auth.logout() })
                    }
                    if (MainTab.entries[tab] == MainTab.Chats) {
                        // Yangi xabar tugmasi (DrKLO `FragmentFloatingButton`: 48 dp) va ustida hikoya (kamera) tugmasi.
                        Box(
                            Modifier.align(Alignment.BottomEnd).padding(end = 24.dp, bottom = 136.dp).size(40.dp).shadow(4.dp, CircleShape).clip(CircleShape)
                                .background(if (c.dark) Color(0xFF1F1F1F) else Color.White).clickable { storyPicker.launch(arrayOf("image/*", "video/*")) },
                            contentAlignment = Alignment.Center,
                        ) { TgIcon(R.drawable.msg_camera, c.text, size = 22.dp) }
                        Box(
                            Modifier.align(Alignment.BottomEnd).padding(end = 20.dp, bottom = 80.dp).size(48.dp).shadow(6.dp, CircleShape).clip(CircleShape).background(c.fab)
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
                else ChatList(
                    archived, typing = typing, onLongClick = { menuFor = it }, onOpen = ::open,
                    onSwipe = { r -> scope.launch { withContext(Dispatchers.IO) { RustCore.tgArchive(r.spec, false) }; model.load(); loadArchived() } },
                    swipeLabel = { "Arxivdan chiqarish" },
                )
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
            is Dest.Settings -> SettingsRouter(d.page, onBack = { stack.removeAt(stack.lastIndex) }, onNavigate = { stack.add(Dest.Settings(it)) }, folders = folders, onFoldersChanged = { scope.launch { folders = parseFolders(withContext(Dispatchers.IO) { RustCore.tgFolders() }) } })
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
                    MenuItem(if (r.muted) R.drawable.msg_unmute else R.drawable.msg_mute, if (r.muted) "Bildirishnomani yoqish" else "Bildirishnomani o‘chirish") { if (r.muted) model.mute(r) else muteFor = r },
                    if (archived.any { it.id == r.id }) MenuItem(R.drawable.msg_unarchive, "Arxivdan chiqarish") { scope.launch { withContext(Dispatchers.IO) { RustCore.tgArchive(r.spec, false) }; model.load(); loadArchived() } }
                    else MenuItem(R.drawable.msg_archive, "Arxivlash") { scope.launch { withContext(Dispatchers.IO) { RustCore.tgArchive(r.spec, true) }; model.load(); loadArchived() } },
                    MenuItem(R.drawable.msg_markread, "O‘qilgan deb belgilash") { scope.launch { withContext(Dispatchers.IO) { RustCore.tgMarkRead(r.spec) }; model.load() } },
                    MenuItem(R.drawable.msg_delete, "O‘chirish", danger = true) { deleteFor = r },
                ),
                onDismiss = { menuFor = null },
            )
        }
        if (topMenu) AnchoredMenu(
            listOf(
                MenuRow(R.drawable.msg_groups, "Yangi guruh") { stack.add(Dest.NewGroup) },
                MenuRow(R.drawable.msg_archive, "Arxivlangan chatlar") { stack.add(Dest.Archive) },
                MenuRow(R.drawable.msg_markread, "Hammasini o‘qilgan deb belgilash") { scope.launch { ui.chats.filter { it.unread > 0 }.forEach { withContext(Dispatchers.IO) { RustCore.tgMarkRead(it.spec) } }; model.load() } },
            ),
            onDismiss = { topMenu = false },
        )
        muteFor?.let { r -> MuteMenu(onPick = { secs -> model.muteFor(r, secs); muteFor = null }, onDismiss = { muteFor = null }) }
        deleteFor?.let { r ->
            TgAlert("Suhbatni o‘chirish", "“${r.name}” suhbati sizdan o‘chiriladi (guruh yoki kanal bo‘lsa — undan chiqasiz).", "O‘chirish", danger = true,
                onConfirm = { model.deleteDialog(r); deleteFor = null }, onDismiss = { deleteFor = null })
        }
    }
    }
}

/**
 * Chatlar tabi (DrKLO `DialogsActivity`): tepada sarlavha (yig'ilgan holatda — hikoyalar avatarlari to'plami va qidiruv belgisi; pastga tortilsa —
 * hikoyalar qatori va qidiruv pilyuli ochiladi), jild yorliqlari, arxiv qatori va suhbatlar ro'yxati.
 */
@Composable
private fun ChatsTab(
    ui: ChatsUi, folders: List<Folder>, folder: Int, onFolder: (Int) -> Unit, typing: Map<Long, Long>,
    onSearch: () -> Unit, onOpen: (ChatRow) -> Unit, onLongClick: (ChatRow) -> Unit, onRetry: () -> Unit,
    archivedCount: Int = 0, onArchive: () -> Unit = {},
    stories: List<StoryPeer> = emptyList(), meId: Long? = null, onStory: (Int) -> Unit = {}, onAddStory: () -> Unit = {},
    onMenu: () -> Unit = {}, archived: List<ChatRow> = emptyList(), onSwipe: (ChatRow) -> Unit = {},
) {
    val c = LocalTg.current
    val density = androidx.compose.ui.platform.LocalDensity.current
    var expanded by androidx.compose.runtime.saveable.rememberSaveable { mutableStateOf(false) }
    val p by androidx.compose.animation.core.animateFloatAsState(if (expanded) 1f else 0f, androidx.compose.animation.core.tween(250), label = "dialogsHeader")
    val threshold = with(density) { 48.dp.toPx() }
    val conn = remember(threshold) {
        object : androidx.compose.ui.input.nestedscroll.NestedScrollConnection {
            var pull = 0f
            override fun onPreScroll(available: androidx.compose.ui.geometry.Offset, source: androidx.compose.ui.input.nestedscroll.NestedScrollSource): androidx.compose.ui.geometry.Offset {
                if (available.y < 0f) { pull = 0f; if (expanded) expanded = false }
                return androidx.compose.ui.geometry.Offset.Zero
            }
            override fun onPostScroll(consumed: androidx.compose.ui.geometry.Offset, available: androidx.compose.ui.geometry.Offset, source: androidx.compose.ui.input.nestedscroll.NestedScrollSource): androidx.compose.ui.geometry.Offset {
                if (source == androidx.compose.ui.input.nestedscroll.NestedScrollSource.UserInput && available.y > 0f) {
                    pull += available.y
                    if (pull > threshold && !expanded) { expanded = true; pull = 0f }
                }
                return androidx.compose.ui.geometry.Offset.Zero
            }
            override suspend fun onPostFling(consumed: androidx.compose.ui.unit.Velocity, available: androidx.compose.ui.unit.Velocity): androidx.compose.ui.unit.Velocity { pull = 0f; return androidx.compose.ui.unit.Velocity.Zero }
        }
    }
    val cluster = remember(stories) { stories.sortedByDescending { it.unseen }.take(3) }
    Column(Modifier.fillMaxSize().background(c.bg).nestedScroll(conn)) {
        Column(Modifier.fillMaxWidth().background(c.bg).statusBarsPadding()) {
            Box(Modifier.fillMaxWidth().height(56.dp)) {
                if (cluster.isNotEmpty() && p < 1f) {
                    Box(Modifier.align(Alignment.CenterStart).padding(start = 14.dp).alpha(1f - p).clickable { expanded = true }) { StoryCluster(cluster) }
                }
                Row(Modifier.fillMaxSize(), verticalAlignment = Alignment.CenterVertically) {
                    val startPad = 23.dp + (if (cluster.isNotEmpty()) 65.dp * (1f - p) else 0.dp)
                    BasicText("ARUGRAM", Modifier.weight(1f).padding(start = startPad), style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold), maxLines = 1)
                    if (p < 1f) Box(Modifier.size(48.dp).alpha(1f - p).clickable(onClick = onSearch), contentAlignment = Alignment.Center) { SearchIcon(c.text) }
                    Box(Modifier.size(48.dp).clickable(onClick = onMenu), contentAlignment = Alignment.Center) { TgIcon(R.drawable.ic_ab_other, c.text) }
                }
            }
            // Pastga tortilganda ochiladigan qism: hikoyalar qatori + qidiruv pilyuli.
            Column(
                Modifier.layout { m, cs ->
                    val pl = m.measure(cs.copy(maxHeight = androidx.compose.ui.unit.Constraints.Infinity))
                    layout(pl.width, (pl.height * p).roundToInt()) { pl.place(0, 0) }
                }.clipToBounds().alpha(p),
            ) {
                if (stories.isNotEmpty() || meId != null) StoriesRow(stories, meId, onOpen = onStory, onAdd = onAddStory)
                Row(
                    Modifier.padding(horizontal = 12.dp, vertical = 4.dp).fillMaxWidth().height(40.dp).clip(RoundedCornerShape(20.dp))
                        .background(if (c.dark) Color(0x14FFFFFF) else Color(0x0F000000)).clickable(onClick = onSearch).padding(start = 14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    SearchIcon(c.gray)
                    BasicText("Chatlarni qidirish", Modifier.padding(start = 14.dp), style = TextStyle(color = c.gray, fontSize = 16.sp), maxLines = 1)
                }
            }
            if (folders.isNotEmpty()) {
                val unread = folders.associate { f -> f.id to ui.chats.filter { f.matches(it) }.sumOf { if (it.muted) 0 else it.unread } } + (0 to ui.chats.sumOf { if (it.muted) 0 else it.unread })
                FolderTabs(folders, folder, unread, onFolder)
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
                    if (shown.isEmpty() && archivedCount == 0) BasicText(if (ui.chats.isEmpty()) "Suhbatlar yo‘q" else "Bu jildda suhbat yo‘q", Modifier.align(Alignment.Center), style = TextStyle(color = c.gray, fontSize = 17.sp))
                    else ChatList(
                        shown, bottomPadding = 150.dp, typing = typing, onLongClick = onLongClick, onOpen = onOpen, onSwipe = onSwipe,
                        header = if (archivedCount > 0 && sel == null) ({ ArchiveRow(archived, onArchive) }) else null,
                    )
                }
            }
        }
    }
}

/** Hikoyalari bor 3 ta akkauntning bir-birining ustiga chiqqan avatarlari (DrKLO `DialogStoriesCell` yig'ilgan holati). */
@Composable
private fun StoryCluster(peers: List<StoryPeer>) {
    val c = LocalTg.current
    Box(Modifier.width((40 + 12 * (peers.size - 1)).dp).height(40.dp)) {
        peers.asReversed().forEachIndexed { rev, p ->
            val i = peers.size - 1 - rev
            Box(Modifier.padding(start = (12 * i).dp).size(40.dp).clip(CircleShape).background(c.bg).padding(1.dp), contentAlignment = Alignment.Center) {
                androidx.compose.foundation.Canvas(Modifier.fillMaxSize()) {
                    val w = 2.dp.toPx()
                    drawCircle(
                        brush = if (p.unseen) androidx.compose.ui.graphics.Brush.sweepGradient(listOf(Color(0xFF34C76F), Color(0xFF3DA1FD), Color(0xFF34C76F))) else androidx.compose.ui.graphics.SolidColor(Color(0xFF8E9296)),
                        radius = size.minDimension / 2f - w / 2f, style = androidx.compose.ui.graphics.drawscope.Stroke(width = w),
                    )
                }
                ChatAvatar(p.row, 32.dp)
            }
        }
    }
}

/** "Arxivlangan chatlar" qatori (ro'yxat boshida): avatar o'rnida arxiv belgisi, ikkinchi qatorda arxivdagi suhbatlar nomlari. */
@Composable
private fun ArchiveRow(archived: List<ChatRow>, onClick: () -> Unit) {
    val c = LocalTg.current
    Box(Modifier.fillMaxWidth().height(70.dp).clickable(onClick = onClick)) {
        Box(
            Modifier.padding(start = 11.dp, top = 9.dp).size(52.dp).clip(CircleShape).background(if (c.dark) Color(0xFF2C2C2E) else Color(0xFFB4B9BE)),
            contentAlignment = Alignment.Center,
        ) { TgIcon(R.drawable.msg_archive, Color.White, size = 28.dp) }
        BasicText("Arxivlangan chatlar", Modifier.padding(start = 76.dp, top = 11.dp), style = TextStyle(color = c.gray, fontSize = 17.sp, fontWeight = FontWeight.Medium), maxLines = 1)
        BasicText(
            archived.take(6).joinToString(", ") { it.name }, Modifier.padding(start = 76.dp, end = 16.dp, top = 37.dp),
            style = TextStyle(color = c.gray, fontSize = 16.sp), maxLines = 1, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
        )
    }
}

/**
 * Profil tabi (DrKLO `ProfileActivity`, `my_profile`): tepa panelda tahrirlash (qalam) va ⋮ (chiqish); markazda avatar, ism, "onlayn";
 * "Hisob" bo'limida telefon, foydalanuvchi nomi va bio qatorlari (qiymat 16 sp, izoh 13 sp); alohida "Saqlangan xabarlar" qatori.
 */
@Composable
private fun ProfileTab(me: ChatRow?, meContact: String, bottomPadding: androidx.compose.ui.unit.Dp, onPhoto: (android.net.Uri) -> Unit, onEdit: () -> Unit, onSaved: () -> Unit, onLogout: () -> Unit) {
    val c = LocalTg.current
    val picker = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.GetContent()) { uri -> if (uri != null) onPhoto(uri) }
    var about by remember { mutableStateOf("") }
    var username by remember { mutableStateOf("") }
    var phone by remember { mutableStateOf("") }
    var menu by remember { mutableStateOf(false) }
    LaunchedEffect(me?.id) {
        val j = withContext(Dispatchers.IO) { RustCore.tgProfile() }
        if (j.optString("error").isEmpty()) {
            about = j.optString("about")
            username = j.optString("username").let { if (it.isNotEmpty()) "@$it" else "" }
            phone = j.optString("phone").let { if (it.isNotEmpty()) "+$it" else "" }
        }
    }
    val card = if (c.dark) c.bg else c.bar
    Box(Modifier.fillMaxSize().background(c.grayBg)) {
        Column(Modifier.fillMaxSize().verticalScroll(androidx.compose.foundation.rememberScrollState())) {
            Spacer(Modifier.statusBarsPadding().height(56.dp))
            if (me != null) {
                Column(Modifier.fillMaxWidth().padding(bottom = 16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Box(Modifier.clickable { picker.launch("image/*") }) { ChatAvatar(me, 100.dp) }
                    BasicText(me.name, Modifier.padding(top = 12.dp, start = 16.dp, end = 16.dp), style = TextStyle(color = c.text, fontSize = 22.sp, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center), maxLines = 1)
                    BasicText("onlayn", Modifier.padding(top = 2.dp), style = TextStyle(color = c.gray, fontSize = 14.sp))
                }
            }
            Column(Modifier.padding(horizontal = 12.dp)) {
                Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(card)) {
                    BasicText("Hisob", Modifier.padding(start = 18.dp, top = 16.dp, bottom = 4.dp), style = TextStyle(color = c.value, fontSize = 15.sp, fontWeight = FontWeight.Medium))
                    if (phone.isNotEmpty()) DetailRow(phone, "Telefon raqami")
                    if (username.isNotEmpty()) DetailRow(username, "Foydalanuvchi nomi")
                    if (about.isNotEmpty()) DetailRow(about, "Bio")
                    Spacer(Modifier.height(6.dp))
                }
                Spacer(Modifier.height(12.dp))
                Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(card).height(50.dp).clickable(onClick = onSaved).padding(horizontal = 18.dp), verticalAlignment = Alignment.CenterVertically) {
                    TgIcon(R.drawable.msg_saved, c.text.copy(alpha = 0.7f))
                    BasicText("Saqlangan xabarlar", Modifier.padding(start = 18.dp), style = TextStyle(color = c.text, fontSize = 16.sp))
                }
            }
            Spacer(Modifier.height(bottomPadding))
        }
        Row(Modifier.fillMaxWidth().statusBarsPadding().height(56.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.weight(1f))
            Box(Modifier.size(48.dp).clickable(onClick = onEdit), contentAlignment = Alignment.Center) { TgIcon(R.drawable.group_edit_profile, c.text) }
            Box(Modifier.size(48.dp).clickable { menu = true }, contentAlignment = Alignment.Center) { TgIcon(R.drawable.ic_ab_other, c.text) }
        }
        if (menu) AnchoredMenu(listOf(MenuRow(R.drawable.msg_leave, "Chiqish") { onLogout() }), onDismiss = { menu = false })
    }
}

@Composable
private fun DetailRow(value: String, label: String) {
    val c = LocalTg.current
    Column(Modifier.fillMaxWidth().padding(horizontal = 18.dp, vertical = 8.dp)) {
        BasicText(value, style = TextStyle(color = c.text, fontSize = 16.sp))
        BasicText(label, Modifier.padding(top = 2.dp), style = TextStyle(color = c.gray, fontSize = 13.sp))
    }
}

package uz.arumediatv.com

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material.icons.materialIcon
import androidx.compose.material.icons.materialPath
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.border
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.activity.SystemBarStyle
import uz.arumediatv.com.ui.common.FloatingNav
import uz.arumediatv.com.ui.common.FloatingNavHeight
import uz.arumediatv.com.ui.common.LocalBarPad
import uz.arumediatv.com.ui.common.NavItem
import androidx.activity.compose.BackHandler
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.ui.input.pointer.pointerInput
import uz.arumediatv.com.ui.billing.BillingScreen
import uz.arumediatv.com.ui.profile.PublicProfileScreen
import uz.arumediatv.com.ui.profile.StatDetailScreen
import uz.arumediatv.com.ui.profile.StatKind
import uz.arumediatv.com.ui.profile.StorageScreen
import uz.arumediatv.com.ui.support.SupportChatScreen
import uz.arumediatv.com.ui.packs.MyPacksScreen
import uz.arumediatv.com.ui.packs.PackDetailScreen
import uz.arumediatv.com.ui.admin.AdminHubScreen
import uz.arumediatv.com.ui.admin.AdminSection
import uz.arumediatv.com.ui.admin.AdminThreadsScreen
import uz.arumediatv.com.ui.admin.AdminUsersScreen
import uz.arumediatv.com.ui.admin.AdminReportsScreen
import uz.arumediatv.com.ui.admin.AdminChannelsScreen
import uz.arumediatv.com.ui.admin.AdminChannelAddScreen
import uz.arumediatv.com.ui.admin.AdminPacksScreen
import uz.arumediatv.com.ui.admin.AdminStatsScreen
import uz.arumediatv.com.ui.admin.AdminAppScreen
import uz.arumediatv.com.ui.admin.AdminEncodeScreen
import uz.arumediatv.com.ui.admin.AdminAnimeScreen
import uz.arumediatv.com.ui.admin.AdminAnimeEditScreen
import uz.arumediatv.com.ui.admin.AdminSeasonsScreen
import uz.arumediatv.com.ui.admin.AdminSeasonEditScreen
import uz.arumediatv.com.ui.admin.AdminEpisodesScreen
import uz.arumediatv.com.ui.admin.AdminEpisodeEditScreen
import uz.arumediatv.com.ui.season.SeasonScreen
import kotlinx.coroutines.launch
import uz.arumediatv.com.ui.catalog.CatalogScreen
import uz.arumediatv.com.ui.home.HomeScreen
import uz.arumediatv.com.ui.library.LibraryScreen
import uz.arumediatv.com.ui.login.LoginScreen
import uz.arumediatv.com.ui.login.BotLoginScreen
import uz.arumediatv.com.ui.profile.EditProfileScreen
import uz.arumediatv.com.ui.profile.ProfileScreen
import uz.arumediatv.com.ui.profile.SessionsScreen
import uz.arumediatv.com.ui.profile.SettingsScreen
import uz.arumediatv.com.ui.search.SearchScreen
import uz.arumediatv.com.ui.theme.AruColors
import uz.arumediatv.com.ui.theme.AruTheme

/** Papka belgisi (Flutter'dagi Kutubxona tugmasi). `icons-core` da yo'q, shuning uchun qo'lda chizilgan. */
private val FolderIcon: ImageVector = materialIcon(name = "Filled.Folder") {
    materialPath {
        moveTo(10.0f, 4.0f)
        horizontalLineTo(4.0f)
        curveToRelative(-1.1f, 0.0f, -1.99f, 0.9f, -1.99f, 2.0f)
        lineTo(2.0f, 18.0f)
        curveToRelative(0.0f, 1.1f, 0.9f, 2.0f, 2.0f, 2.0f)
        horizontalLineToRelative(16.0f)
        curveToRelative(1.1f, 0.0f, 2.0f, -0.9f, 2.0f, -2.0f)
        verticalLineTo(8.0f)
        curveToRelative(0.0f, -1.1f, -0.9f, -2.0f, -2.0f, -2.0f)
        horizontalLineToRelative(-8.0f)
        lineToRelative(-2.0f, -2.0f)
        close()
    }
}

private data class Tab(val title: String, val icon: ImageVector)

private val tabs = listOf(
    Tab("Bosh sahifa", Icons.Filled.Home),
    Tab("Qidiruv", Icons.Filled.Search),
    Tab("Katalog", uz.arumediatv.com.ui.common.AruIcons.GridView),
    Tab("Kutubxona", FolderIcon),
    Tab("Profil", Icons.Filled.Person),
)

/** Ekranlar stegi (tab ustiga ochiladi). Tablar ostida tirik turadi — qaytganda aylantirish joyi saqlanadi. */
private sealed interface Page {
    data object Login : Page
    data object BotLogin : Page
    data object Edit : Page
    data object Sessions : Page
    data object Settings : Page
    data object Billing : Page
    data object Storage : Page
    data object Support : Page
    data object Packs : Page
    data class PackDetail(val id: Long) : Page
    data object AdminHub : Page
    data object AdminThreads : Page
    data object AdminUsers : Page
    data object AdminReports : Page
    data object AdminChannels : Page
    data object AdminChannelAdd : Page
    data object AdminPacks : Page
    data object AdminStats : Page
    data object AdminApp : Page
    data object AdminAnime : Page
    data object AdminEncode : Page
    data class AdminAnimeEdit(val anime: org.json.JSONObject?) : Page
    data class AdminSeasons(val anime: org.json.JSONObject) : Page
    data class AdminSeasonEdit(val animeId: Long, val season: org.json.JSONObject?) : Page
    data class AdminEpisodes(val animeId: Long, val seasonId: Long, val title: String) : Page
    data class AdminEpisodeEdit(val animeId: Long, val seasonId: Long, val episode: org.json.JSONObject?) : Page
    data class AdminChat(val userId: Long, val name: String) : Page
    data class PublicUser(val id: Long) : Page
    data class Stat(val userId: Long, val kind: StatKind, val owner: String) : Page
    data class AnimeDownloads(val animeId: Long, val seasonId: Long, val title: String) : Page
    data class AnimeHistory(val item: uz.arumediatv.com.data.HistoryItem) : Page
    data class SeasonPage(val season: uz.arumediatv.com.data.Season, val episodeId: Long = 0, val quality: String? = null) : Page
}

class MainActivity : ComponentActivity() {
    override fun onStop() {
        super.onStop()
        // Ilova fonga ketdi: navbatdagi yozuvlar (sozlamalar, sevimlilar...) darhol bitta paket bo'lib yuboriladi.
        if (AppServices.ready) AppServices.sync.flushAsync(force = true)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT),
        )
        setContent { AruTheme { AruApp() } }
    }
}

@Composable
private fun AruApp() {
    var tab by rememberSaveable { mutableIntStateOf(0) }
    val stack = remember { mutableStateListOf<Page>() }
    val snack = remember { SnackbarHostState() }
    val ctx = androidx.compose.ui.platform.LocalContext.current
    fun push(p: Page) { stack.add(p) }
    fun pop() { if (stack.isNotEmpty()) stack.removeAt(stack.lastIndex) }
    val openSeason: (uz.arumediatv.com.data.Season) -> Unit = { push(Page.SeasonPage(it)) }
    val login = { push(Page.Login) }
    BackHandler(enabled = stack.isNotEmpty()) { pop() }
    // Admin xabari yozgan bo'lsa Profil tugmasida nuqta; har 30 soniyada arzon so'rov (faqat o'qilmaganlar soni).
    val unreadUser by AppServices.unread.count.collectAsStateWithLifecycle()
    val me by AppServices.auth.user.collectAsStateWithLifecycle()
    androidx.compose.runtime.LaunchedEffect(me?.id) {
        while (me != null) {
            AppServices.unread.refresh()
            kotlinx.coroutines.delay(30_000)
        }
    }
    val statusTop = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    val navBottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    // Menyu: balandligi 68dp + pastki chekka 12dp + navigatsiya paneli. Kontent shuning ostigacha cho'ziladi.
    val barPad = PaddingValues(top = statusTop, bottom = navBottom + BAR_HEIGHT + 12.dp)
    Box(Modifier.fillMaxSize().background(AruColors.Bg)) {
        CompositionLocalProvider(LocalBarPad provides barPad) {
            when (tab) {
                0 -> HomeScreen(onOpenSeason = openSeason, onLogin = login)
                1 -> SearchScreen(onOpenSeason = openSeason)
                2 -> CatalogScreen(onOpenSeason = openSeason)
                3 -> LibraryScreen(onOpenSeason = openSeason, onLogin = login, onOpenDownload = { s, e, q -> push(Page.SeasonPage(s, e, q)) }, onOpenAnimeHistory = { push(Page.AnimeHistory(it)) }, onOpenAnimeDownloads = { d -> push(Page.AnimeDownloads(d.animeId, d.seasonId, d.title)) })
                4 -> ProfileScreen(
                    onLogin = login,
                    onEdit = { push(Page.Edit) },
                    onSessions = { push(Page.Sessions) },
                    onSettings = { push(Page.Settings) },
                    onBilling = { push(Page.Billing) },
                    onStorage = { push(Page.Storage) },
                    onSupport = { push(Page.Support) },
                    onPacks = { push(Page.Packs) },
                    onAdmin = { push(Page.AdminHub) },
                    onOpenStat = { k -> AppServices.auth.user.value?.id?.let { push(Page.Stat(it, k, "")) } },
                )
                else -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text("${tabs[tab].title} — tez orada", color = AruColors.TextFaint)
                }
            }
        }
        // Suzuvchi menyu: chetlardan ajralgan yumaloq oyna (Flutter'dagidek), ostidan kontent ko'rinib turadi.
        FloatingNav(
            items = tabs.map { NavItem(it.title, it.icon) },
            selected = tab,
            onSelect = { tab = it },
            badges = if (unreadUser > 0) setOf(4) else emptySet(),
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .navigationBarsPadding()
                .padding(start = 16.dp, end = 16.dp, bottom = 12.dp),
        )
        SnackbarHost(
            snack,
            modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = navBottom + BAR_HEIGHT + 20.dp),
        )

        // ── Ekranlar steki: eng ustkisi ko'rinadi ──
        stack.lastOrNull()?.let { page ->
            Box(Modifier.fillMaxSize().background(AruColors.Bg).pointerInput(Unit) { detectTapGestures { } }.statusBarsPadding().navigationBarsPadding()) {
                when (page) {
                    Page.Login -> LoginScreen(onDone = ::pop, onClose = ::pop, onBot = { pop(); push(Page.BotLogin) })
                    Page.BotLogin -> BotLoginScreen(onDone = ::pop, onBack = ::pop)
                    Page.Edit -> EditProfileScreen(onBack = ::pop)
                    Page.Sessions -> SessionsScreen(onBack = ::pop)
                    Page.Settings -> SettingsScreen(onBack = ::pop)
                    Page.Billing -> BillingScreen(onBack = ::pop)
                    Page.Storage -> StorageScreen(onBack = ::pop)
                    Page.Support -> SupportChatScreen(userId = null, title = "Admin bilan bog'lanish", onBack = ::pop, onPacks = { push(Page.Packs) })
                    Page.Packs -> MyPacksScreen(onBack = ::pop, onOpenPack = { p -> push(Page.PackDetail(p.id)) })
                    is Page.PackDetail -> PackDetailScreen(page.id, onBack = ::pop)
                    Page.AdminHub -> AdminHubScreen(onBack = ::pop, onOpen = { s ->
                        when (s) {
                            AdminSection.CHATS -> push(Page.AdminThreads)
                            AdminSection.USERS -> push(Page.AdminUsers)
                            AdminSection.REPORTS -> push(Page.AdminReports)
                            AdminSection.CHANNELS -> push(Page.AdminChannels)
                            AdminSection.PACKS -> push(Page.AdminPacks)
                            AdminSection.STATS -> push(Page.AdminStats)
                            AdminSection.APP -> push(Page.AdminApp)
                            AdminSection.ANIME -> push(Page.AdminAnime)
                            AdminSection.ENCODE -> push(Page.AdminEncode)
                            else -> android.widget.Toast.makeText(ctx, "${s.title}: keyingi qadamda", android.widget.Toast.LENGTH_SHORT).show()
                        }
                    })
                    Page.AdminThreads -> AdminThreadsScreen(onBack = ::pop, onOpenThread = { t -> push(Page.AdminChat(t.userId, t.name)) })
                    Page.AdminUsers -> AdminUsersScreen(onBack = ::pop, onOpenProfile = { id -> push(Page.PublicUser(id)) }, onOpenChat = { id, n -> push(Page.AdminChat(id, n)) })
                    Page.AdminReports -> AdminReportsScreen(onBack = ::pop, onOpenProfile = { id -> if (id > 0) push(Page.PublicUser(id)) })
                    Page.AdminChannels -> AdminChannelsScreen(onBack = ::pop, onAdd = { push(Page.AdminChannelAdd) })
                    Page.AdminChannelAdd -> AdminChannelAddScreen(onBack = ::pop, onDone = ::pop)
                    Page.AdminPacks -> AdminPacksScreen(onBack = ::pop)
                    Page.AdminStats -> AdminStatsScreen(onBack = ::pop)
                    Page.AdminApp -> AdminAppScreen(onBack = ::pop)
                    Page.AdminEncode -> AdminEncodeScreen(onBack = ::pop)
                    Page.AdminAnime -> AdminAnimeScreen(onBack = ::pop, onOpenAnime = { a -> push(Page.AdminSeasons(a)) }, onEdit = { a -> push(Page.AdminAnimeEdit(a)) })
                    is Page.AdminAnimeEdit -> AdminAnimeEditScreen(page.anime, onBack = ::pop, onDone = ::pop)
                    is Page.AdminSeasons -> AdminSeasonsScreen(page.anime, onBack = ::pop, onOpenSeason = { s -> push(Page.AdminEpisodes(page.anime.optLong("id"), s.optLong("season_id"), s.optString("nomi").ifEmpty { "${s.optInt("bolim_id")}-bo'lim" })) }, onEdit = { s -> push(Page.AdminSeasonEdit(page.anime.optLong("id"), s)) })
                    is Page.AdminSeasonEdit -> AdminSeasonEditScreen(page.animeId, page.season, onBack = ::pop, onDone = ::pop)
                    is Page.AdminEpisodes -> AdminEpisodesScreen(page.animeId, page.seasonId, page.title, onBack = ::pop, onEdit = { e -> push(Page.AdminEpisodeEdit(page.animeId, page.seasonId, e)) })
                    is Page.AdminEpisodeEdit -> AdminEpisodeEditScreen(page.animeId, page.seasonId, page.episode, onBack = ::pop, onDone = ::pop)
                    is Page.AdminChat -> SupportChatScreen(userId = page.userId, title = page.name, onBack = ::pop)
                    is Page.PublicUser -> PublicProfileScreen(page.id, onBack = ::pop, onOpenStat = { u, k, o -> push(Page.Stat(u, k, o)) })
                    is Page.Stat -> StatDetailScreen(page.userId, page.kind, page.owner, onBack = ::pop, onOpenSeason = openSeason)
                    is Page.AnimeDownloads -> uz.arumediatv.com.ui.library.DownloadsAnimeScreen(
                        page.animeId, page.seasonId, page.title, onBack = ::pop,
                        onOpen = { s, e, q -> push(Page.SeasonPage(s, e, q)) },
                    )
                    is Page.AnimeHistory -> uz.arumediatv.com.ui.library.AnimeHistoryScreen(
                        page.item.animeId, page.item.seasonId, page.item.title, onBack = ::pop,
                        onOpenEpisode = { h ->
                            val s = uz.arumediatv.com.data.Season(h.animeId, h.seasonId, h.title, h.sectionNo, 0, h.photoUrl, 0.0, 0, 0, 0, emptyList(), "", null)
                            push(Page.SeasonPage(s, h.epizodId, h.quality.ifEmpty { null }))
                        },
                    )
                    is Page.SeasonPage -> SeasonScreen(
                        season = page.season,
                        onBack = ::pop,
                        onOpenSeason = { s -> pop(); push(Page.SeasonPage(s)) },
                        onOpenBilling = { push(Page.Billing) },
                        onOpenProfile = { id -> if (id > 0) push(Page.PublicUser(id)) },
                        onLogin = { push(Page.Login) },
                        startEpisodeId = page.episodeId,
                        startQuality = page.quality,
                        onPacks = { push(Page.Packs) },
                    )
                }
            }
        }
    }
}

private val BAR_HEIGHT = FloatingNavHeight

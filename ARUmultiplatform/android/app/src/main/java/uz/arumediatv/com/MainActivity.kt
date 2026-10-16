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
import uz.arumediatv.com.ui.season.SeasonScreen
import kotlinx.coroutines.launch
import uz.arumediatv.com.ui.catalog.CatalogScreen
import uz.arumediatv.com.ui.home.HomeScreen
import uz.arumediatv.com.ui.library.LibraryScreen
import uz.arumediatv.com.ui.login.LoginScreen
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
    Tab("Katalog", Icons.AutoMirrored.Filled.List),
    Tab("Kutubxona", FolderIcon),
    Tab("Profil", Icons.Filled.Person),
)

/** Ekranlar stegi (tab ustiga ochiladi). Tablar ostida tirik turadi — qaytganda aylantirish joyi saqlanadi. */
private sealed interface Page {
    data object Login : Page
    data object Edit : Page
    data object Sessions : Page
    data object Settings : Page
    data object Billing : Page
    data object Storage : Page
    data class PublicUser(val id: Long) : Page
    data class Stat(val userId: Long, val kind: StatKind, val owner: String) : Page
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
                3 -> LibraryScreen(onOpenSeason = openSeason, onLogin = login, onOpenDownload = { s, e, q -> push(Page.SeasonPage(s, e, q)) })
                4 -> ProfileScreen(
                    onLogin = login,
                    onEdit = { push(Page.Edit) },
                    onSessions = { push(Page.Sessions) },
                    onSettings = { push(Page.Settings) },
                    onBilling = { push(Page.Billing) },
                    onStorage = { push(Page.Storage) },
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
                    Page.Login -> LoginScreen(onDone = ::pop, onClose = ::pop)
                    Page.Edit -> EditProfileScreen(onBack = ::pop)
                    Page.Sessions -> SessionsScreen(onBack = ::pop)
                    Page.Settings -> SettingsScreen(onBack = ::pop)
                    Page.Billing -> BillingScreen(onBack = ::pop)
                    Page.Storage -> StorageScreen(onBack = ::pop)
                    is Page.PublicUser -> PublicProfileScreen(page.id, onBack = ::pop, onOpenStat = { u, k, o -> push(Page.Stat(u, k, o)) })
                    is Page.Stat -> StatDetailScreen(page.userId, page.kind, page.owner, onBack = ::pop, onOpenSeason = openSeason)
                    is Page.SeasonPage -> SeasonScreen(
                        season = page.season,
                        onBack = ::pop,
                        onOpenSeason = { s -> pop(); push(Page.SeasonPage(s)) },
                        onOpenBilling = { push(Page.Billing) },
                        onOpenProfile = { id -> if (id > 0) push(Page.PublicUser(id)) },
                        onLogin = { push(Page.Login) },
                        startEpisodeId = page.episodeId,
                        startQuality = page.quality,
                    )
                }
            }
        }
    }
}

private val BAR_HEIGHT = FloatingNavHeight

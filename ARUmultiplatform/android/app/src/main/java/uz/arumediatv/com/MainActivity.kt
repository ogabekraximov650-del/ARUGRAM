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
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Scaffold
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
import androidx.activity.compose.BackHandler
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.mutableStateOf
import kotlinx.coroutines.launch
import uz.arumediatv.com.ui.catalog.CatalogScreen
import uz.arumediatv.com.ui.home.HomeScreen
import uz.arumediatv.com.ui.login.LoginScreen
import uz.arumediatv.com.ui.profile.EditProfileScreen
import uz.arumediatv.com.ui.profile.ProfileScreen
import uz.arumediatv.com.ui.profile.SessionsScreen
import uz.arumediatv.com.ui.search.SearchScreen
import uz.arumediatv.com.ui.theme.AruColors
import uz.arumediatv.com.ui.theme.AruTheme

private data class Tab(val title: String, val icon: ImageVector)

private val tabs = listOf(
    Tab("Bosh sahifa", Icons.Filled.Home),
    Tab("Qidiruv", Icons.Filled.Search),
    Tab("Katalog", Icons.AutoMirrored.Filled.List),
    Tab("Kutubxona", Icons.Filled.Favorite),
    Tab("Profil", Icons.Filled.Person),
)

private enum class Overlay { NONE, LOGIN, EDIT, SESSIONS }

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent { AruTheme { AruApp() } }
    }
}

@Composable
private fun AruApp() {
    var tab by rememberSaveable { mutableIntStateOf(0) }
    var overlay by remember { mutableStateOf(Overlay.NONE) }
    val snack = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val openSeason: (uz.arumediatv.com.data.Season) -> Unit = {
        scope.launch { snack.showSnackbar("Anime sahifasi keyingi bosqichda qo'shiladi") }
    }
    val login = { overlay = Overlay.LOGIN }
    BackHandler(enabled = overlay != Overlay.NONE) { overlay = Overlay.NONE }
    if (overlay != Overlay.NONE) {
        Box(Modifier.fillMaxSize().background(AruColors.Bg).statusBarsPadding()) {
            when (overlay) {
                Overlay.LOGIN -> LoginScreen(onDone = { overlay = Overlay.NONE }, onClose = { overlay = Overlay.NONE })
                Overlay.EDIT -> EditProfileScreen(onBack = { overlay = Overlay.NONE })
                Overlay.SESSIONS -> SessionsScreen(onBack = { overlay = Overlay.NONE })
                Overlay.NONE -> {}
            }
        }
        return
    }
    Scaffold(
        containerColor = AruColors.Bg,
        snackbarHost = { SnackbarHost(snack) },
        bottomBar = {
            NavigationBar(containerColor = AruColors.Surface) {
                tabs.forEachIndexed { i, t ->
                    NavigationBarItem(
                        selected = tab == i,
                        onClick = { tab = i },
                        icon = { Icon(t.icon, contentDescription = t.title) },
                        label = { Text(t.title, maxLines = 1) },
                        colors = NavigationBarItemDefaults.colors(
                            selectedIconColor = AruColors.Accent2,
                            selectedTextColor = AruColors.Accent2,
                            indicatorColor = AruColors.AccentTint,
                            unselectedIconColor = AruColors.TextFaint,
                            unselectedTextColor = AruColors.TextFaint,
                        ),
                    )
                }
            }
        },
    ) { inner ->
        Box(Modifier.fillMaxSize().background(AruColors.Bg).padding(inner).statusBarsPadding()) {
            when (tab) {
                0 -> HomeScreen(onOpenSeason = openSeason, onLogin = login)
                1 -> SearchScreen(onOpenSeason = openSeason)
                2 -> CatalogScreen(onOpenSeason = openSeason)
                4 -> ProfileScreen(
                    onLogin = login,
                    onEdit = { overlay = Overlay.EDIT },
                    onSessions = { overlay = Overlay.SESSIONS },
                )
                else -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text("${tabs[tab].title} — tez orada", color = AruColors.TextFaint)
                }
            }
        }
    }
}

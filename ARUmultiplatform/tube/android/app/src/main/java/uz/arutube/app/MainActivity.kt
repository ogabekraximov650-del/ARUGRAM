package uz.arutube.app

import android.app.Activity
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

sealed interface Screen {
    data object Home : Screen
    data object Search : Screen
    data object Library : Screen
    data class Watch(val video: Video) : Screen
    data object Upload : Screen
    data class Shorts(val list: List<Video>, val start: Int) : Screen
}

val TubeRed = Color(0xFFFF0000)
val TubeGray = Color(0xFFAAAAAA)
val TubeChip = Color(0xFF272727)

private val TubeColors = darkColorScheme(
    primary = TubeRed,
    background = Color(0xFF0F0F0F),
    surface = Color(0xFF1F1F1F),
    onBackground = Color.White,
    onSurface = Color.White,
)

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        TubeCore.load()
        LocalStore.init(applicationContext)
        LinkCache.init(applicationContext)
        setContent {
            MaterialTheme(colorScheme = TubeColors) {
                // Surface: matn rangi (contentColor) oq bo'lishi uchun; usiz Text qora chiqadi.
                Surface(color = MaterialTheme.colorScheme.background, contentColor = Color.White) {
                    TubeApp()
                }
            }
        }
    }
}

@Composable
fun TubeApp() {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val holder = remember { PlayerHolder(ctx.applicationContext, scope) }
    var screen by remember { mutableStateOf<Screen>(Screen.Home) }
    var sheet by remember { mutableStateOf<Video?>(null) }
    // Yuklashdan qaytganda ro'yxat yangilansin.
    var refresh by remember { mutableIntStateOf(0) }

    // Ijro o'rni har 5 soniyada tarixga yoziladi ("davom ettirish" uchun).
    LaunchedEffect(holder.exo) {
        while (holder.exo != null) {
            delay(5_000)
            holder.save()
        }
    }
    DisposableEffect(Unit) { onDispose { holder.close() } }
    BackHandler(enabled = screen !is Screen.Home) { screen = Screen.Home }

    val open: (Video) -> Unit = { screen = Screen.Watch(it) }

    // Shorts tugmasi: barcha qisqa videolar yig'ilib, vertikal pleyer ochiladi.
    val openShorts: () -> Unit = {
        scope.launch {
            Repo.ensureLoaded(2)
            val list = Repo.all.filter { it.isShort }
            if (list.isNotEmpty()) {
                holder.close()
                screen = Screen.Shorts(list, 0)
            }
        }
    }

    CompositionLocalProvider(LocalQuickActions provides { sheet = it }) {
    sheet?.let { QuickActionsSheet(it) { sheet = null } }
    Column(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
        Box(Modifier.weight(1f)) {
            when (val s = screen) {
                is Screen.Home -> HomeScreen(
                    refreshKey = refresh,
                    onOpen = open,
                    onOpenShort = { list, i ->
                        holder.close()
                        screen = Screen.Shorts(list, i)
                    },
                    onSearch = { screen = Screen.Search },
                )
                is Screen.Search -> SearchScreen(onOpen = open)
                is Screen.Library -> LibraryScreen(onOpen = open)
                is Screen.Watch -> WatchScreen(video = s.video, holder = holder, onMinimize = { screen = Screen.Home }, onOpen = open)
                is Screen.Shorts -> ShortsScreen(list = s.list, start = s.start, onBack = { screen = Screen.Home })
                is Screen.Upload -> UploadScreen(onBack = { screen = Screen.Home }, onUploaded = { refresh++ })
            }
        }
        val full = screen is Screen.Watch || screen is Screen.Shorts
        if (!full) {
            MiniPlayer(holder, onOpen = open)
            NavigationBar(containerColor = Color(0xFF0F0F0F), modifier = Modifier.navigationBarsPadding()) {
                val colors = NavigationBarItemDefaults.colors(
                    selectedIconColor = Color.White,
                    selectedTextColor = Color.White,
                    unselectedIconColor = TubeGray,
                    unselectedTextColor = TubeGray,
                    indicatorColor = Color(0xFF272727),
                )
                NavigationBarItem(
                    selected = screen is Screen.Home,
                    onClick = { screen = Screen.Home },
                    icon = { Icon(Icons.Filled.Home, contentDescription = null) },
                    label = { Text("Bosh sahifa") },
                    colors = colors,
                )
                NavigationBarItem(
                    selected = false,
                    onClick = openShorts,
                    icon = { Icon(Icons.Filled.PlayArrow, contentDescription = null) },
                    label = { Text("Shorts") },
                    colors = colors,
                )
                NavigationBarItem(
                    selected = screen is Screen.Upload,
                    onClick = { screen = Screen.Upload },
                    icon = { Icon(Icons.Filled.Add, contentDescription = null) },
                    label = { Text("Yuklash") },
                    colors = colors,
                )
                NavigationBarItem(
                    selected = screen is Screen.Library,
                    onClick = { screen = Screen.Library },
                    icon = { Icon(Icons.Filled.Favorite, contentDescription = null) },
                    label = { Text("Kutubxona") },
                    colors = colors,
                )
            }
        }
    }
    }
}

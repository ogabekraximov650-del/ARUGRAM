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
import uz.arumediatv.com.ui.home.HomeScreen
import uz.arumediatv.com.ui.theme.AruColors
import uz.arumediatv.com.ui.theme.AruTheme

private data class Tab(val title: String, val icon: ImageVector)

private val tabs = listOf(
    Tab("Bosh sahifa", Icons.Filled.Home),
    Tab("Katalog", Icons.Filled.Search),
    Tab("Kutubxona", Icons.Filled.Favorite),
    Tab("Profil", Icons.Filled.Person),
)

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
    Scaffold(
        containerColor = AruColors.Bg,
        bottomBar = {
            NavigationBar(containerColor = AruColors.Surface) {
                tabs.forEachIndexed { i, t ->
                    NavigationBarItem(
                        selected = tab == i,
                        onClick = { tab = i },
                        icon = { Icon(t.icon, contentDescription = t.title) },
                        label = { Text(t.title) },
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
                0 -> HomeScreen(onOpenSeason = { /* Anime sahifasi keyingi bosqichda */ })
                else -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text("${tabs[tab].title} — tez orada", color = AruColors.TextFaint)
                }
            }
        }
    }
}

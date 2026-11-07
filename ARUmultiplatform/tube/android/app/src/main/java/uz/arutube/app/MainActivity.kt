package uz.arutube.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color

sealed interface Screen {
    data object Home : Screen
    data class Watch(val video: Video) : Screen
    data object Upload : Screen
}

private val TubeColors = darkColorScheme(
    primary = Color(0xFFFF3D3D),
    background = Color(0xFF0F0F0F),
    surface = Color(0xFF1B1B1B),
    onBackground = Color.White,
    onSurface = Color.White,
)

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        TubeCore.load()
        setContent {
            MaterialTheme(colorScheme = TubeColors) {
                TubeApp()
            }
        }
    }
}

@Composable
fun TubeApp() {
    var screen by remember { mutableStateOf<Screen>(Screen.Home) }
    // Yuklashdan qaytganda ro'yxat yangilansin.
    var refresh by remember { mutableIntStateOf(0) }
    BackHandler(enabled = screen !is Screen.Home) { screen = Screen.Home }

    androidx.compose.foundation.layout.Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
        when (val s = screen) {
            is Screen.Home -> HomeScreen(
                refreshKey = refresh,
                onOpen = { screen = Screen.Watch(it) },
                onUpload = { screen = Screen.Upload },
            )
            is Screen.Watch -> WatchScreen(video = s.video, onBack = { screen = Screen.Home }, onOpen = { screen = Screen.Watch(it) })
            is Screen.Upload -> UploadScreen(onBack = { screen = Screen.Home }, onUploaded = { refresh++ })
        }
    }
}

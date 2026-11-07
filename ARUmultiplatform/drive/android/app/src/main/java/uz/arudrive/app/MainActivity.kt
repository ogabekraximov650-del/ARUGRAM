package uz.arudrive.app

import android.content.Context
import android.content.pm.ActivityInfo
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil.ImageLoader
import uz.arudrive.app.data.Api
import uz.arudrive.app.data.DriveFile
import uz.arudrive.app.ui.AruColors
import uz.arudrive.app.ui.AruTheme
import uz.arudrive.app.ui.HomeScreen
import uz.arudrive.app.ui.PlayerScreen
import uz.arudrive.app.ui.UploadScreen

private var loader: ImageLoader? = null

/** Kichik rasmlar: imzolangan OkHttp mijozi (worker imzosiz so'rovni rad etadi). */
fun imageLoader(ctx: Context): ImageLoader =
    loader ?: ImageLoader.Builder(ctx.applicationContext).okHttpClient { Api.client }.build().also { loader = it }

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent { AruTheme { Root() } }
    }
}

@Composable
private fun Root() {
    var tab by remember { mutableIntStateOf(0) }
    var playing by remember { mutableStateOf<DriveFile?>(null) }
    var refresh by remember { mutableIntStateOf(0) }
    val ctx = androidx.compose.ui.platform.LocalContext.current

    val p = playing
    if (p != null) {
        BackHandler { playing = null }
        PlayerScreen(p) { playing = null }
        return
    }
    Column(Modifier.fillMaxSize().background(AruColors.Bg).statusBarsPadding()) {
        Box(Modifier.weight(1f)) {
            if (tab == 0) HomeScreen(refresh) { playing = it } else UploadScreen { refresh++ }
        }
        Row(Modifier.fillMaxWidth().background(AruColors.Surface).navigationBarsPadding()) {
            listOf("Bosh sahifa", "Yuklash").forEachIndexed { i, label ->
                Box(Modifier.weight(1f).clickable { tab = i; if (i == 0) refresh++ }.padding(vertical = 14.dp), contentAlignment = Alignment.Center) {
                    Text(label, color = if (tab == i) AruColors.Accent2 else AruColors.TextDim, fontWeight = if (tab == i) FontWeight.Bold else FontWeight.Normal, fontSize = 14.sp)
                }
            }
        }
    }
}

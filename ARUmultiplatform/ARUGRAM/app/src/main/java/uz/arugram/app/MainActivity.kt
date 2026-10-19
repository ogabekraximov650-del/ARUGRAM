package uz.arugram.app

import android.graphics.Color
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.ClipOp
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.layer.drawLayer
import androidx.compose.ui.graphics.layer.toImageBitmap
import androidx.compose.ui.graphics.rememberGraphicsLayer
import androidx.compose.ui.layout.onSizeChanged
import kotlinx.coroutines.launch
import kotlin.math.hypot
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import uz.arugram.app.auth.AuthModel
import uz.arugram.app.auth.Stage
import uz.arugram.app.ui.components.ArugramLogo
import uz.arugram.app.ui.intro.IntroScreen
import uz.arugram.app.ui.login.LoginFlow
import uz.arugram.app.ui.main.HomeScreen
import uz.arugram.app.ui.theme.ArugramTheme
import uz.arugram.app.ui.theme.LocalTg

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // Tizim panellari shaffof: oq fonda qora belgilar, tungi rejimda — aksincha (Telegram'dagidek).
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.auto(Color.TRANSPARENT, Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.auto(Color.TRANSPARENT, Color.TRANSPARENT),
        )
        super.onCreate(savedInstanceState)
        val auth = (application as ArugramApp).auth
        auth.start()
        val prefs = getSharedPreferences("ui", MODE_PRIVATE)
        setContent {
            val system = isSystemInDarkTheme()
            // Tanlov saqlanadi: 0 — tizim bo'yicha, 1 — kunduzgi, 2 — tungi.
            var mode by rememberSaveable { mutableIntStateOf(prefs.getInt("theme", 0)) }
            val dark = when (mode) { 1 -> false; 2 -> true; else -> system }
            DisposableEffect(dark) {
                val style = SystemBarStyle.auto(Color.TRANSPARENT, Color.TRANSPARENT) { dark }
                enableEdgeToEdge(statusBarStyle = style, navigationBarStyle = style)
                onDispose { }
            }
            ArugramTheme(dark) {
                Root(auth, dark) {
                    mode = if (dark) 1 else 2
                    prefs.edit().putInt("theme", mode).apply()
                }
            }
        }
    }
}

private enum class Screen { Loading, Fatal, Auth, Home }

@Composable
private fun Root(auth: AuthModel, dark: Boolean, onToggleTheme: () -> Unit) {
    var introDone by rememberSaveable { mutableStateOf(false) }
    var leaving by remember { mutableStateOf(false) }
    val ui by auth.ui.collectAsStateWithLifecycle()
    val c = LocalTg.current
    val scope = rememberCoroutineScope()
    val layer = rememberGraphicsLayer()
    var oldFrame by remember { mutableStateOf<ImageBitmap?>(null) }
    var reveal by remember { mutableStateOf<Pair<Offset, Float>?>(null) }
    val screen = when {
        !ui.ready -> Screen.Loading
        ui.fatal != null -> Screen.Fatal
        ui.stage == Stage.Done -> Screen.Home
        else -> Screen.Auth
    }
    val showIntro = !introDone && ui.stage == Stage.Phone

    // Mavzu almashishi: eski kadr olinadi, yangi mavzu bir zumda qo'yiladi va eski kadr tugma markazidan doira bo'lib "teshiladi".
    fun toggleWithReveal(center: Offset, size: androidx.compose.ui.unit.IntSize) {
        scope.launch {
            val bmp = runCatching { layer.toImageBitmap() }.getOrNull()
            if (bmp == null) { onToggleTheme(); return@launch }
            oldFrame = bmp
            onToggleTheme()
            val maxR = maxOf(
                hypot(center.x, center.y), hypot(size.width - center.x, center.y),
                hypot(center.x, size.height - center.y), hypot(size.width - center.x, size.height - center.y),
            )
            val a = Animatable(0f)
            a.animateTo(maxR, tween(400, easing = FastOutSlowInEasing)) { reveal = center to value }
            oldFrame = null; reveal = null
        }
    }

    var rootSize by remember { mutableStateOf(androidx.compose.ui.unit.IntSize.Zero) }
    Box(
        Modifier.fillMaxSize().background(c.bg).onSizeChanged { rootSize = it }
            .drawWithContent {
                layer.record { this@drawWithContent.drawContent() }
                drawLayer(layer)
                val bmp = oldFrame
                val rv = reveal
                if (bmp != null) {
                    val path = Path()
                    if (rv != null) path.addOval(Rect(rv.first, rv.second))
                    clipPath(path, ClipOp.Difference) { drawImage(bmp) }
                }
            },
    ) {
        Crossfade(targetState = screen, animationSpec = tween(250), label = "root") { s ->
            when (s) {
                Screen.Loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { ArugramLogo() }
                Screen.Fatal -> Box(Modifier.fillMaxSize().padding(32.dp), contentAlignment = Alignment.Center) {
                    BasicText(ui.fatal.orEmpty(), style = TextStyle(color = c.red, fontSize = 15.sp, textAlign = TextAlign.Center, lineHeight = 20.sp))
                }
                Screen.Auth -> Box(Modifier.fillMaxSize()) {
                    // Kirish oqimi tanishtiruv ostida tayyor turadi: tugma dumaloq tugmaga aylanayotganda sahifa ko'rinib keladi.
                    if (!showIntro || leaving) LoginFlow(auth, suppressFab = showIntro)
                    if (showIntro) IntroScreen(
                        dark = dark,
                        onToggleTheme = { center -> toggleWithReveal(center, rootSize) },
                        onStartMorph = { leaving = true },
                        onFinished = { introDone = true },
                    )
                }
                Screen.Home -> HomeScreen(auth)
            }
        }
    }
}

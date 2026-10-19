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
import androidx.compose.runtime.Composable
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
        setContent { ArugramTheme { Root(auth) } }
    }
}

private enum class Screen { Loading, Fatal, Login, Home }

@Composable
private fun Root(auth: AuthModel) {
    val ui by auth.ui.collectAsStateWithLifecycle()
    val c = LocalTg.current
    val screen = when {
        !ui.ready -> Screen.Loading
        ui.fatal != null -> Screen.Fatal
        ui.stage == Stage.Done -> Screen.Home
        else -> Screen.Login
    }
    Box(Modifier.fillMaxSize().background(c.bg)) {
        Crossfade(targetState = screen, animationSpec = tween(250), label = "root") { s ->
            when (s) {
                Screen.Loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { ArugramLogo() }
                Screen.Fatal -> Box(Modifier.fillMaxSize().padding(32.dp), contentAlignment = Alignment.Center) {
                    BasicText(ui.fatal.orEmpty(), style = TextStyle(color = c.red, fontSize = 15.sp, textAlign = TextAlign.Center, lineHeight = 20.sp))
                }
                Screen.Login -> LoginFlow(auth)
                Screen.Home -> HomeScreen(auth)
            }
        }
    }
}

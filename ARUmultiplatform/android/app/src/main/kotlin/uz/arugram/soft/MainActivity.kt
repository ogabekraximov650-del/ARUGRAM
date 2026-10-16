package uz.arugram.soft

import android.graphics.Color
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import uz.arugram.soft.ui.RootScreen
import uz.arugram.soft.ui.theme.AruTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // Flutter: `SystemUiMode.edgeToEdge` + shaffof status/navigatsiya paneli,
        // ikonkalar OQ (`SystemUiOverlayStyle`, `lib/main.dart`).
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
        )
        super.onCreate(savedInstanceState)
        val repo = (application as AruApp).seasons
        setContent { AruTheme { RootScreen(repo) } }
    }
}

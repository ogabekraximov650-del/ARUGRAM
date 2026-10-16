package uz.arumediatv.com.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

/** Flutter ilovasidagi `AppColors` bilan bir xil palitra (qora fon, bitta apelsin urg'u). */
object AruColors {
    val Bg = Color(0xFF0A0A0C)
    val Surface = Color(0xFF101012)
    val Card = Color(0xFF151517)
    val CardAlt = Color(0xFF1B1B1E)
    val Accent = Color(0xFFC2410C)
    val Accent2 = Color(0xFFE2620F)
    val Gold = Color(0xFFFFC93C)
    val AccentTint = Color(0xFF281914)
    val Border = Color(0x0FFFFFFF)
    val BorderBright = Color(0x1AFFFFFF)
    val Text = Color(0xFFF2F2F3)
    val TextDim = Color(0xFF9A9AA0)
    val TextFaint = Color(0xFF6B6B70)
}

private val Scheme = darkColorScheme(
    primary = AruColors.Accent,
    onPrimary = Color.White,
    secondary = AruColors.Accent2,
    background = AruColors.Bg,
    onBackground = AruColors.Text,
    surface = AruColors.Surface,
    onSurface = AruColors.Text,
    surfaceVariant = AruColors.Card,
    onSurfaceVariant = AruColors.TextDim,
    outline = AruColors.BorderBright,
)

@Composable
fun AruTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = Scheme, typography = Typography(), content = content)
}

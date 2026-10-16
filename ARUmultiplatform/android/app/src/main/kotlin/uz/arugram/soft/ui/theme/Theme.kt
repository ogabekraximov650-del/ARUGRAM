package uz.arugram.soft.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable

/** Flutter'dagi `ThemeData` (`lib/main.dart`) ning Compose ko'rinishi. */
private val AruScheme = darkColorScheme(
    primary = AruColors.accent,
    secondary = AruColors.accent2,
    background = AruColors.bg,
    surface = AruColors.card,
    error = AruColors.danger,
    onPrimary = AruColors.onAccent,
    onSecondary = AruColors.onAccent,
    onBackground = AruColors.text,
    onSurface = AruColors.text,
)

@Composable
fun AruTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = AruScheme, content = content)
}

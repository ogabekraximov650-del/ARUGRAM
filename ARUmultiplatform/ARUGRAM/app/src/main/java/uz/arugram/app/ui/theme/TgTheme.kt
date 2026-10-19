package uz.arugram.app.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.graphics.lerp as lerpColor

/**
 * Telegram Android'ning standart ranglari (DrKLO `ThemeColors.java`: kunduzgi "Blue" mavzu; tungi — `night.attheme`).
 * Nomlar Telegram'dagi kalitlarga mos: windowBackgroundWhite, ...BlackText, ...GrayText6, ...HintText, ...InputField(+Activated),
 * ...ValueText, chats_actionBackground, text_RedBold.
 */
@Immutable
class TgColors(
    val bg: Color,
    val text: Color,
    val gray: Color,
    val hint: Color,
    val input: Color,
    val inputActive: Color,
    val value: Color,
    val fab: Color,
    val fabIcon: Color,
    val red: Color,
    val divider: Color,
    val link: Color,
    val bar: Color,
    val grayBg: Color,
    val dark: Boolean,
)

val LightTg = TgColors(
    bg = Color(0xFFFFFFFF), text = Color(0xFF1A1D21), gray = Color(0xFF757575), hint = Color(0xFFA8A8A8),
    input = Color(0xFFDBDBDB), inputActive = Color(0xFF229AF0), value = Color(0xFF298ACF),
    fab = Color(0xFF65A9E0), fabIcon = Color.White, red = Color(0xFFCC4747), divider = Color(0xFFE6E6E6),
    link = Color(0xFF1C93E3), bar = Color(0xFFFFFFFF), grayBg = Color(0xFFF1F1F3), dark = false,
)

val DarkTg = TgColors(
    bg = Color(0xFF181819), text = Color.White, gray = Color(0xFF7D7D7D), hint = Color(0xFF7D7D7D),
    input = Color(0xFF505050), inputActive = Color(0xFF6EB2EF), value = Color(0xFF69B6FD),
    fab = Color(0xFF5EA0DB), fabIcon = Color.White, red = Color(0xFFEE686F), divider = Color(0xFF000000),
    link = Color(0xFF64B5EF), bar = Color(0xFF232326), grayBg = Color(0xFF000000), dark = true,
)

val LocalTg = staticCompositionLocalOf { LightTg }

fun blend(a: Color, b: Color, t: Float): Color = lerpColor(a, b, t.coerceIn(0f, 1f))

/** Telegram o'lchamlari dp bilan beriladi (shrift masshtabi hisobga olinmaydi) — shu sabab fontScale = 1. */
@Composable
fun ArugramTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val tg = if (dark) DarkTg else LightTg
    val d = LocalDensity.current
    val scheme = if (dark) darkColorScheme(primary = tg.inputActive, background = tg.bg, surface = tg.bg) else lightColorScheme(primary = tg.inputActive, background = tg.bg, surface = tg.bg)
    CompositionLocalProvider(LocalTg provides tg, LocalDensity provides Density(d.density, fontScale = 1f)) {
        MaterialTheme(colorScheme = scheme, content = content)
    }
}

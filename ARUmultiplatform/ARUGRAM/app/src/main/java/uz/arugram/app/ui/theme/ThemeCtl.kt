package uz.arugram.app.ui.theme

import androidx.compose.runtime.staticCompositionLocalOf

/** Mavzu tanlovi: 0 — tizim bo'yicha, 1 — kunduzgi, 2 — tungi. Sozlamalardan o'zgartiriladi, `MainActivity` saqlaydi. */
class ThemeCtl(val mode: Int, val set: (Int) -> Unit)

val LocalThemeCtl = staticCompositionLocalOf { ThemeCtl(0) {} }

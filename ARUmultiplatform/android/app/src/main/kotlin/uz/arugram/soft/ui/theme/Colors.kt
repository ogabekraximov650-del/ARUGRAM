package uz.arugram.soft.ui.theme

import androidx.compose.ui.graphics.Color

/**
 * Ranglar — Flutter ilovadagi `AppColors` (`lib/widgets/glass.dart`) bilan
 * BIR XIL. Qiymatlar o'sha fayldan ko'chirilgan.
 */
object AruColors {
    // Yuzalar
    val bg = Color(0xFF0A0A0C)
    val surface = Color(0xFF101012)
    val card = Color(0xFF151517)
    val cardAlt = Color(0xFF1B1B1E)

    // Urg'u
    val accent = Color(0xFFC2410C)
    val accent2 = Color(0xFFE2620F)
    val accent3 = Color(0xFFFFC93C)
    val accentTint = Color(0xFF281914)
    val onAccent = Color(0xFFFFFFFF)

    // Ramka (oq, 6% va 10% shaffoflik)
    val border = Color(0x0FFFFFFF)
    val borderBright = Color(0x1AFFFFFF)

    // Matn
    val text = Color(0xFFF2F2F3)
    val textDim = Color(0xFF9A9AA0)
    val textFaint = Color(0xFF6B6B70)

    // Holat
    val success = Color(0xFF4ADE80)
    val gold = Color(0xFFFFC93C)
    val danger = Color(0xFFE5484D)
}

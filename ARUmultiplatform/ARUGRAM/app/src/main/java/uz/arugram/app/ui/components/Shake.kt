package uz.arugram.app.ui.components

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer

/**
 * Xato bo'lganda ko'rinish gorizontal "silkinadi" (Telegram `AndroidUtilities.shakeView`: so'nuvchi tebranish).
 * [tick] har gal o'zgarganda qaytadan ishga tushadi (0 — o'zgarmaydi).
 */
@Composable
fun Modifier.shakeOnChange(tick: Int, amplitudeDp: Float = 6f): Modifier {
    val x = remember { Animatable(0f) }
    LaunchedEffect(tick) {
        if (tick == 0) return@LaunchedEffect
        val steps = floatArrayOf(-1f, 1f, -0.75f, 0.75f, -0.4f, 0.4f, 0f)
        for (s in steps) x.animateTo(s * amplitudeDp, tween(55))
    }
    return graphicsLayer { translationX = x.value * density }
}

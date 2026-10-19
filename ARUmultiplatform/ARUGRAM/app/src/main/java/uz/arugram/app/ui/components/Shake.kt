package uz.arugram.app.ui.components

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import kotlinx.coroutines.delay
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity

/**
 * Telegram'ning `AndroidUtilities.shakeViewSpring`: ko'rinishga chapga (-shift*100 px/s) turtki beriladi va u prujinada (qattiqlik 600,
 * so'nish 0,5 — `SpringForce` standarti) joyiga qaytib tebranadi. Maydon xatosida `shift` = 3,5 dp, kod katakchalarida — 10 dp.
 * [tick] har gal o'zgarganda qaytadan ishga tushadi (0 — o'zgarmaydi).
 */
@Composable
fun Modifier.shakeOnChange(tick: Int, shiftDp: Float = 3.5f, onEnd: (() -> Unit)? = null): Modifier {
    val x = remember { Animatable(0f) }
    val density = LocalDensity.current.density
    LaunchedEffect(tick) {
        if (tick == 0) return@LaunchedEffect
        x.snapTo(0f)
        x.animateTo(0f, spring(dampingRatio = 0.5f, stiffness = 600f, visibilityThreshold = 0.75f), initialVelocity = -shiftDp * density * 100f)
        onEnd?.invoke()
    }
    return graphicsLayer { translationX = x.value }
}

/**
 * `LoginActivity.onFieldError(view, allowErrorSelection = true)`: kontur 2 soniya qizil bo'ladi yoki matn o'zgarishi bilan darhol qaytadi.
 * [tick] oshganda boshlanadi (boshlang'ich qiymati hisobga olinmaydi); [textAfter] — xato paytidagi (tozalashdan keyingi) matn.
 */
@Composable
fun rememberFieldError(tick: Int, text: String, textAfter: String = text): Boolean {
    val base = remember { tick }
    var err by remember { mutableStateOf(false) }
    var textAtError by remember { mutableStateOf(text) }
    LaunchedEffect(tick) {
        if (tick == base) return@LaunchedEffect
        textAtError = textAfter
        err = true
        delay(2000)
        err = false
    }
    LaunchedEffect(text) { if (err && text != textAtError) err = false }
    return err
}

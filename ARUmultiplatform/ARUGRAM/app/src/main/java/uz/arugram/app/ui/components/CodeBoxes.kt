package uz.arugram.app.ui.components

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uz.arugram.app.ui.theme.LocalTg
import uz.arugram.app.ui.theme.blend
import kotlin.math.cos
import kotlin.math.max

/** `OvershootInterpolator(1.5)` — raqam paydo bo'lganda biroz oshib ketib qaytadi (Telegram `CodeNumberField.startEnterAnimation`). */
private val Overshoot = Easing { t ->
    val x = t - 1f
    x * x * ((1.5f + 1f) * x + 1.5f) + 1f
}

/** `ValueAnimator` ning standart interpolatori (`AccelerateDecelerateInterpolator`) — almashish / o'chish animatsiyalarida. */
private val AccelDecel = Easing { t -> (cos((t + 1f) * Math.PI) / 2.0 + 0.5).toFloat() }

/** `Theme.key_checkbox` — muvaffaqiyatli kod ramkasi. */
private val SuccessGreen = Color(0xFF5EC245)

/** `SpringForce`: sukunat nuqtasi 1 dagi 400 stiffness, tebranishsiz (focus / error / success progress). */
private fun progressSpring() = spring<Float>(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = 400f)

/**
 * Kod maydonlari (DrKLO `CodeFieldContainer` + `CodeNumberField`): har biri 34×42 dp (qo'ng'iroq turida 28×34), oralig'i 7 dp (5),
 * ramka 1,5 dp, burchak 4 dp, raqam 20 sp qalin.
 *  - fokusdagi katak ko'k, xatoda qizil, muvaffaqiyatda yashil (400 stiffness prujina);
 *  - bo'sh katakka raqam kirsa pastdan yuqoriga 350 ms'da (Overshoot 1,5) chiqadi, katak ichida qirqilgan;
 *  - raqam almashsa eskisi 220 ms'da kichrayib so'nadi, yangisi 0,5→1 masshtab va alpha bilan 220 ms'da kiradi;
 *  - o'chirilganda raqam 220 ms'da kichrayib so'nadi;
 *  - [success] — katakchalar ketma-ket (75 ms) yashil bo'ladi va "pulse" qiladi (`animateSuccessProgress`);
 *  - [focusSuppressed] — tekshirilayotganda fokus ramkasi o'zgarmaydi (`isFocusSuppressed`).
 * Kiritish Telegramning o'z raqam klaviaturasi (`TgNumPad`) orqali; katakchalar faqat ko'rsatadi.
 */
@Composable
fun TgCodeBoxes(
    value: String,
    length: Int,
    error: Boolean,
    modifier: Modifier = Modifier,
    success: Boolean = false,
    focusSuppressed: Boolean = false,
    compact: Boolean = false,
) {
    val w = if (compact) 28 else 34
    val h = if (compact) 34 else 42
    val gap = if (compact) 5 else 7
    val focusIndex = value.length.coerceAtMost(length - 1)
    Box(modifier, contentAlignment = Alignment.Center) {
        Row(horizontalArrangement = Arrangement.spacedBy(gap.dp)) {
            for (i in 0 until length) {
                CodeBox(i, value.getOrNull(i), i == focusIndex, error, success, focusSuppressed, w, h)
            }
        }
    }
}

@Composable
private fun CodeBox(index: Int, ch: Char?, focused: Boolean, error: Boolean, success: Boolean, suppressed: Boolean, widthDp: Int, heightDp: Int) {
    val c = LocalTg.current
    val scope = rememberCoroutineScope()
    val heightPx = with(LocalDensity.current) { heightDp.dp.toPx() }

    val focusP = remember { Animatable(0f) }
    val errorP = remember { Animatable(0f) }
    val successP = remember { Animatable(0f) }
    val successScale = remember { Animatable(1f) }

    // Fokus: tekshirish boshlanganda (isFocusSuppressed = true) hammasi 0 ga qaytadi, so'ng to'xtatib turilgan payt o'zgarmaydi.
    LaunchedEffect(suppressed) { if (suppressed) focusP.animateTo(0f, progressSpring()) }
    LaunchedEffect(focused, suppressed) {
        if (!suppressed) focusP.animateTo(if (focused) 1f else 0f, progressSpring())
    }
    LaunchedEffect(error) { errorP.animateTo(if (error) 1f else 0f, progressSpring()) }
    LaunchedEffect(success) {
        if (success) {
            delay(index * 75L)
            scope.launch { successP.animateTo(1f, progressSpring()) }
            // SpringForce(1).setStiffness(500).setDampingRatio(LOW_BOUNCY=0.75), boshlang'ich tezlik 4000/100.
            successScale.snapTo(1f)
            successScale.animateTo(1f, spring(dampingRatio = 0.75f, stiffness = 500f), initialVelocity = 40f)
        } else {
            successScale.snapTo(1f)
            successP.animateTo(0f, progressSpring())
        }
    }

    // Raqam animatsiyalari (har biri mustaqil, keyingi o'zgarish oldingisini to'xtatmaydi).
    val enter = remember { Animatable(1f) }
    val exit = remember { Animatable(1f) }
    var replace by remember { mutableStateOf(false) }
    var exitChar by remember { mutableStateOf<Char?>(null) }
    var prev by remember { mutableStateOf(ch) }
    LaunchedEffect(ch) {
        val old = prev
        prev = ch
        if (old == ch) return@LaunchedEffect
        // Xato bo'lganda maydonlar darhol tozalanadi (setText("") — chiqish animatsiyasisiz).
        if (old != null && !error) {
            exitChar = old
            scope.launch {
                exit.snapTo(0f)
                exit.animateTo(1f, tween(220, easing = AccelDecel))
            }
        }
        replace = old != null
        scope.launch {
            enter.snapTo(0f)
            if (old == null) enter.animateTo(1f, tween(350, easing = Overshoot))
            else enter.animateTo(1f, tween(220, easing = AccelDecel))
        }
    }

    Box(Modifier.size(width = widthDp.dp, height = heightDp.dp)) {
        Canvas(Modifier.fillMaxSize()) {
            val stroke = 1.5.dp.toPx()
            val focusClr = blend(c.input, c.inputActive, focusP.value)
            val errorClr = blend(focusClr, c.red, errorP.value)
            val color = blend(errorClr, SuccessGreen, successP.value)
            // Muvaffaqiyatda ramka masshtab bilan biroz kengayadi (`-max(0, stroke * (scale - 1))`).
            val grow = if (successP.value != 0f) max(0f, stroke * (successScale.value - 1f)) else 0f
            val inset = stroke - grow
            drawRoundRect(
                color = color,
                topLeft = Offset(inset, inset),
                size = Size(size.width - inset * 2f, size.height - inset * 2f),
                cornerRadius = CornerRadius(4.dp.toPx()),
                style = Stroke(width = stroke),
            )
        }
        val textStyle = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Medium)
        if (ch != null) {
            // Raqam ramka ichida (inset = ramka qalinligi) qirqiladi.
            Box(Modifier.fillMaxSize().padding(1.5.dp).clipToBounds()) {
                Box(
                    Modifier.fillMaxSize().graphicsLayer {
                        val p = enter.value
                        if (replace) {
                            val s = p * 0.5f + 0.5f
                            scaleX = s; scaleY = s; alpha = p.coerceIn(0f, 1f)
                        } else {
                            translationY = heightPx * (1f - p)
                        }
                    },
                    contentAlignment = Alignment.Center,
                ) { BasicText(ch.toString(), style = textStyle) }
            }
        }
        val ex = exitChar
        if (ex != null && exit.value < 1f) {
            Box(
                Modifier.fillMaxSize().graphicsLayer {
                    val s = (1f - exit.value) * 0.5f + 0.5f
                    scaleX = s; scaleY = s; alpha = (1f - exit.value).coerceIn(0f, 1f)
                },
                contentAlignment = Alignment.Center,
            ) { BasicText(ex.toString(), style = textStyle) }
        }
    }
}

package uz.arugram.app.ui.components

import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uz.arugram.app.ui.theme.LocalTg
import uz.arugram.app.ui.theme.blend

/** `OvershootInterpolator(1.5)` — raqam paydo bo'lganda biroz oshib ketib qaytadi (Telegram `CodeNumberField`). */
private val Overshoot = Easing { t ->
    val x = t - 1f
    x * x * ((1.5f + 1f) * x + 1.5f) + 1f
}

/**
 * Kod maydonlari (DrKLO `CodeFieldContainer` + `CodeNumberField`): har biri 34×42 dp, oralig'i 7 dp, ramka 1,5 dp, burchak 4 dp,
 * raqam 20 sp. Fokusdagi katak ko'k, xatoda qizil (prujinali o'tish); raqam kirganda 350 ms'da "sakrab" chiqadi, o'chganda 220 ms'da so'nadi.
 * Kiritish Telegramning o'z raqam klaviaturasi (`TgNumPad`) orqali; katakchalar faqat ko'rsatadi.
 */
@Composable
fun TgCodeBoxes(
    value: String,
    length: Int,
    error: Boolean,
    modifier: Modifier = Modifier,
) {
    val c = LocalTg.current
    Box(modifier, contentAlignment = Alignment.Center) {
        Row(horizontalArrangement = Arrangement.spacedBy(7.dp)) {
            for (i in 0 until length) {
                val focused = i == value.length.coerceAtMost(length - 1)
                CodeBox(value.getOrNull(i), focused, error)
            }
        }
    }
}

@Composable
private fun CodeBox(ch: Char?, focused: Boolean, error: Boolean) {
    val c = LocalTg.current
    val spring = spring<Float>(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = 500f)
    val focus by animateFloatAsState(if (focused) 1f else 0f, spring, label = "cbFocus")
    val err by animateFloatAsState(if (error) 1f else 0f, spring, label = "cbErr")
    val digit by animateFloatAsState(
        if (ch != null) 1f else 0f,
        tween(if (ch != null) 350 else 220, easing = if (ch != null) Overshoot else LinearEasing),
        label = "cbDigit",
    )
    var remembered by remember { mutableStateOf<Char?>(null) }
    LaunchedEffect(ch) { if (ch != null) remembered = ch }
    Box(Modifier.size(width = 34.dp, height = 42.dp), contentAlignment = Alignment.Center) {
        Canvas(Modifier.size(width = 34.dp, height = 42.dp)) {
            val stroke = 1.5.dp.toPx()
            val color = blend(blend(c.input, c.inputActive, focus), c.red, err)
            drawRoundRect(
                color = color,
                topLeft = Offset(stroke, stroke),
                size = Size(size.width - stroke * 2f, size.height - stroke * 2f),
                cornerRadius = CornerRadius(4.dp.toPx()),
                style = Stroke(width = stroke),
            )
        }
        // So'nayotgan raqam hali ko'rinib turishi uchun oxirgi qiymat eslab qolinadi.
        val shown = ch ?: remembered
        if (shown != null && digit > 0.01f) {
            BasicText(
                text = shown.toString(),
                style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Medium),
                modifier = Modifier.graphicsLayer {
                    scaleX = digit.coerceAtLeast(0f)
                    scaleY = digit.coerceAtLeast(0f)
                    alpha = digit.coerceIn(0f, 1f)
                },
            )
        }
    }
}

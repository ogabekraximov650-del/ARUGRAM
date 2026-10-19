package uz.arugram.app.ui.components

import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import uz.arugram.app.ui.theme.LocalTg

/**
 * Parol oynasidagi 120 dp li qulf (Telegram'da Lottie): sekin "nafas oladi"; parol yozilganda ("open") sirtmoq ko'tarilib ochiladi.
 * Bu o'zimiz chizgan nusxa.
 */
@Composable
fun LockHint(open: Boolean, modifier: Modifier = Modifier, size: Dp = 120.dp) {
    val c = LocalTg.current
    val t = rememberInfiniteTransition(label = "lock")
    val breath by t.animateFloat(0f, 1f, infiniteRepeatable(tween(2200, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "breath")
    val lift by animateFloatAsState(if (open) 1f else 0f, tween(380, easing = FastOutSlowInEasing), label = "lift")
    Canvas(modifier.size(size)) {
        val s = this.size.width / 120f
        val cx = 60f * s
        val sc = 1f + 0.025f * breath
        val bodyW = 64f * s * sc
        val bodyH = 48f * s * sc
        val bodyX = cx - bodyW / 2f
        val bodyY = 58f * s
        val stroke = 5f * s
        // Sirtmoq (yarim aylana + ikki oyoq); ochilganda yuqoriga ko'tariladi va o'ng oyog'i bo'shaydi.
        val r = 18f * s
        val up = 10f * s * lift
        val legTop = bodyY - 14f * s - up
        rotate(degrees = -14f * lift, pivot = Offset(cx - r, bodyY)) {
            drawArc(c.value, 180f, 180f, false, Offset(cx - r, legTop - r), Size(r * 2f, r * 2f), style = Stroke(stroke, cap = StrokeCap.Round))
            drawLine(c.value, Offset(cx - r, legTop), Offset(cx - r, bodyY), strokeWidth = stroke, cap = StrokeCap.Round)
            drawLine(c.value, Offset(cx + r, legTop), Offset(cx + r, bodyY - 8f * s * (1f - lift) ), strokeWidth = stroke, cap = StrokeCap.Round)
        }
        // Korpus
        drawRoundRect(c.value, Offset(bodyX, bodyY), Size(bodyW, bodyH), CornerRadius(9f * s))
        // Kalit teshigi
        drawCircle(Color.White, 5f * s, Offset(cx, bodyY + bodyH * 0.42f))
        drawLine(Color.White, Offset(cx, bodyY + bodyH * 0.42f), Offset(cx, bodyY + bodyH * 0.72f), strokeWidth = 4f * s, cap = StrokeCap.Round)
    }
}

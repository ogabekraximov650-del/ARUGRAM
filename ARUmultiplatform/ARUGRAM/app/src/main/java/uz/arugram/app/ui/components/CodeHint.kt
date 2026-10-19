package uz.arugram.app.ui.components

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
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
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import uz.arugram.app.ui.theme.LocalTg

/**
 * Kod oynasidagi 128 dp li animatsiyali rasm (Telegram'da Lottie `sms_incoming_info` / `code_laptop`): telefon va unga kelayotgan
 * xabar pufagi. Bu o'zimiz chizgan sodda nusxa: pufak "sakrab" chiqadi, ichida nuqtalar ketma-ket yonadi, keyin so'nib takrorlanadi.
 */
@Composable
fun CodeHint(modifier: Modifier = Modifier, size: Dp = 128.dp) {
    val c = LocalTg.current
    val t = rememberInfiniteTransition(label = "codeHint")
    val p by t.animateFloat(0f, 1f, infiniteRepeatable(tween(3600, easing = LinearEasing), RepeatMode.Restart), label = "p")
    Canvas(modifier.size(size)) {
        val s = this.size.width / 128f
        val stroke = 3f * s
        // Telefon
        val px = 36f * s
        val py = 18f * s
        val pw = 48f * s
        val ph = 92f * s
        drawRoundRect(c.value, Offset(px, py), Size(pw, ph), CornerRadius(9f * s), style = Stroke(width = stroke))
        drawLine(c.value, Offset(px + 16f * s, py + 8f * s), Offset(px + pw - 16f * s, py + 8f * s), strokeWidth = stroke * 0.8f, cap = androidx.compose.ui.graphics.StrokeCap.Round)
        drawCircle(c.value, 2.4f * s, Offset(px + pw / 2f, py + ph - 8f * s))
        // Pufak: 0.08–0.28 oralig'ida overshoot bilan o'sadi, 0.88–1.0 da so'nadi.
        val appear = ((p - 0.08f) / 0.20f).coerceIn(0f, 1f)
        val over = run { val x = appear - 1f; x * x * (2.5f * x + 1.5f) + 1f }
        val fade = if (p > 0.88f) 1f - (p - 0.88f) / 0.12f else 1f
        if (appear > 0f) {
            val scale = over.coerceAtLeast(0f)
            val bw = 62f * s * scale
            val bh = 34f * s * scale
            val bx = px + pw - 12f * s
            val by = py + 14f * s
            val bubble = c.value.copy(alpha = fade)
            drawRoundRect(bubble, Offset(bx, by), Size(bw, bh), CornerRadius(10f * s * scale))
            // Pufak dumi
            drawCircle(bubble, 4f * s * scale, Offset(bx + 6f * s * scale, by + bh + 2f * s * scale))
            // Nuqtalar ketma-ket yonadi
            for (i in 0 until 3) {
                val on = ((p - (0.34f + i * 0.12f)) / 0.12f).coerceIn(0f, 1f)
                val a = (0.35f + 0.65f * on) * fade
                drawCircle(Color.White.copy(alpha = a), 3.6f * s * scale, Offset(bx + bw * (0.28f + 0.22f * i), by + bh / 2f))
            }
        }
    }
}

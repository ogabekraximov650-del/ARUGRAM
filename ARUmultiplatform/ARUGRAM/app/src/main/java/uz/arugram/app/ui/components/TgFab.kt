package uz.arugram.app.ui.components

import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.unit.dp
import uz.arugram.app.ui.theme.LocalTg
import kotlin.math.cos
import kotlin.math.sin

/**
 * Telegram'dagi pastki o'ngdagi dumaloq tugma (DrKLO `FragmentFloatingButton` + `TransformableLoginButtonView`): 56 dp, o'ngdan 20 dp,
 * pastdan 14 dp (joylashuvni chaqiruvchi beradi). Ichida 2 dp qalinlikdagi strelka (chetidan 21 dp, "qanot" 9 dp, 45°).
 * Ko'rinish — kichrayib/o'sib chiqish; band bo'lsa strelka o'rniga aylanuvchi doira.
 */
@Composable
fun TgFab(visible: Boolean, busy: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier, check: Float = 0f) {
    val c = LocalTg.current
    val show by animateFloatAsState(if (visible) 1f else 0f, tween(200, easing = FastOutSlowInEasing), label = "fabShow")
    val arrow by animateFloatAsState(if (busy) 0f else 1f, tween(180), label = "fabArrow")
    if (show <= 0.01f) return
    Box(
        modifier
            .size(56.dp)
            .scale(0.3f + 0.7f * show)
            .alpha(show)
            .shadow(4.dp, CircleShape)
            .clip(CircleShape)
            .background(c.fab)
            .clickable(enabled = visible && !busy, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Canvas(Modifier.size(56.dp).alpha(arrow * (1f - check))) {
            val stroke = 2.dp.toPx()
            val pad = 21.dp.toPx()
            val cy = size.height / 2f
            val endX = pad + (size.width - pad * 2f)
            drawLine(c.fabIcon, Offset(pad, cy), Offset(endX, cy), strokeWidth = stroke, cap = StrokeCap.Round)
            val back = 9.dp.toPx()
            val bx = (endX - cos(Math.PI / 4) * back).toFloat()
            val by = (sin(Math.PI / 4) * back).toFloat()
            drawLine(c.fabIcon, Offset(endX, cy), Offset(bx, cy - by), strokeWidth = stroke, cap = StrokeCap.Round)
            drawLine(c.fabIcon, Offset(endX, cy), Offset(bx, cy + by), strokeWidth = stroke, cap = StrokeCap.Round)
        }
        if (check > 0.01f) {
            // Strelka -> belgi (`TRANSFORM_ARROW_CHECK`): belgi chizilib chiqadi.
            Canvas(Modifier.size(56.dp).alpha(check)) {
                val st = 2.dp.toPx()
                val a = Offset(20.dp.toPx(), 29.dp.toPx())
                val b = Offset(25.5.dp.toPx(), 34.5.dp.toPx())
                val cc = Offset(36.dp.toPx(), 22.dp.toPx())
                drawLine(c.fabIcon, a, a + (b - a) * check, strokeWidth = st, cap = StrokeCap.Round)
                drawLine(c.fabIcon, b, b + (cc - b) * check, strokeWidth = st, cap = StrokeCap.Round)
            }
        }
        if (busy) CircularProgressIndicator(Modifier.size(24.dp), color = Color.White, strokeWidth = 2.5.dp)
    }
}

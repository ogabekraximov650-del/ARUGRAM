package uz.arugram.app.ui.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** Orqaga strelkasi (ActionBar `ic_ab_back`): chapga qaragan strelka, 2 dp. */
@Composable
fun BackArrowIcon(color: Color, modifier: Modifier = Modifier, size: Dp = 24.dp) {
    Canvas(modifier.size(size)) {
        val s = this.size.width / 24f
        val w = 2f * s
        drawLine(color, Offset(5f * s, 12f * s), Offset(19f * s, 12f * s), strokeWidth = w, cap = StrokeCap.Round)
        drawLine(color, Offset(5f * s, 12f * s), Offset(11f * s, 6f * s), strokeWidth = w, cap = StrokeCap.Round)
        drawLine(color, Offset(5f * s, 12f * s), Offset(11f * s, 18f * s), strokeWidth = w, cap = StrokeCap.Round)
    }
}

/** O'ngga qaragan kichik strelka (`msg_inputarrow`): pastga qaragan "v" shakli. */
@Composable
fun ChevronDownIcon(color: Color, modifier: Modifier = Modifier, size: Dp = 24.dp) {
    Canvas(modifier.size(size)) {
        val s = this.size.width / 24f
        val p = Path().apply {
            moveTo(7f * s, 10f * s)
            lineTo(12f * s, 15f * s)
            lineTo(17f * s, 10f * s)
        }
        drawPath(p, color, style = Stroke(width = 1.6f * s, cap = StrokeCap.Round, join = StrokeJoin.Round))
    }
}

/** Parolni ko'rsatish/yashirish ko'zi. */
@Composable
fun EyeIcon(color: Color, crossed: Boolean, modifier: Modifier = Modifier, size: Dp = 24.dp) {
    Canvas(modifier.size(size)) {
        val s = this.size.width / 24f
        val st = Stroke(width = 1.8f * s, cap = StrokeCap.Round, join = StrokeJoin.Round)
        val eye = Path().apply {
            moveTo(2.5f * s, 12f * s)
            cubicTo(5f * s, 7f * s, 9f * s, 5.5f * s, 12f * s, 5.5f * s)
            cubicTo(15f * s, 5.5f * s, 19f * s, 7f * s, 21.5f * s, 12f * s)
            cubicTo(19f * s, 17f * s, 15f * s, 18.5f * s, 12f * s, 18.5f * s)
            cubicTo(9f * s, 18.5f * s, 5f * s, 17f * s, 2.5f * s, 12f * s)
            close()
        }
        drawPath(eye, color, style = st)
        drawCircle(color, radius = 3f * s, center = Offset(12f * s, 12f * s), style = st)
        if (crossed) drawLine(color, Offset(4f * s, 20f * s), Offset(20f * s, 4f * s), strokeWidth = 1.8f * s, cap = StrokeCap.Round)
    }
}

/** Qidiruv (kattalashtirgich). */
@Composable
fun SearchIcon(color: Color, modifier: Modifier = Modifier, size: Dp = 24.dp) {
    Canvas(modifier.size(size)) {
        val s = this.size.width / 24f
        val st = Stroke(width = 2f * s, cap = StrokeCap.Round)
        drawCircle(color, radius = 6f * s, center = Offset(10.5f * s, 10.5f * s), style = st)
        drawLine(color, Offset(15f * s, 15f * s), Offset(20f * s, 20f * s), strokeWidth = 2f * s, cap = StrokeCap.Round)
    }
}

/** Yopish (×). */
@Composable
fun CloseIcon(color: Color, modifier: Modifier = Modifier, size: Dp = 24.dp) {
    Canvas(modifier.size(size)) {
        val s = this.size.width / 24f
        drawLine(color, Offset(6f * s, 6f * s), Offset(18f * s, 18f * s), strokeWidth = 2f * s, cap = StrokeCap.Round)
        drawLine(color, Offset(18f * s, 6f * s), Offset(6f * s, 18f * s), strokeWidth = 2f * s, cap = StrokeCap.Round)
    }
}

/** Menyu (uch chiziq). */
@Composable
fun MenuIcon(color: Color, modifier: Modifier = Modifier, size: Dp = 24.dp) {
    Canvas(modifier.size(size)) {
        val s = this.size.width / 24f
        for (y in floatArrayOf(6f, 12f, 18f)) drawLine(color, Offset(4f * s, y * s), Offset(20f * s, y * s), strokeWidth = 2f * s, cap = StrokeCap.Round)
    }
}


/** Yuborish tugmasi (`ic_send`): o'ngga qaragan qog'oz samolyot. */
@Composable
fun SendIcon(color: Color, modifier: Modifier = Modifier, size: Dp = 24.dp) {
    Canvas(modifier.size(size)) {
        val s = this.size.width / 24f
        val p = Path().apply {
            moveTo(2.5f * s, 3.5f * s); lineTo(21.5f * s, 12f * s); lineTo(2.5f * s, 20.5f * s)
            lineTo(5f * s, 13f * s); lineTo(14f * s, 12f * s); lineTo(5f * s, 11f * s); close()
        }
        drawPath(p, color)
    }
}

/** DrKLO'ning o'z rasmli ikonkalari (`res/drawable-xxhdpi`), rangi — ustidan tint beriladi (Telegram ham `PorterDuff` bilan bo'yaydi). */
@Composable
fun TgIcon(@androidx.annotation.DrawableRes id: Int, tint: Color, modifier: Modifier = Modifier, size: Dp = 24.dp) {
    androidx.compose.foundation.Image(
        androidx.compose.ui.res.painterResource(id),
        contentDescription = null,
        modifier = modifier.size(size),
        colorFilter = androidx.compose.ui.graphics.ColorFilter.tint(tint),
    )
}

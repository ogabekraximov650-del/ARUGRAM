package uz.arugram.app.ui.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import uz.arugram.app.ui.theme.LocalTg

/** ARUGRAM belgisi: ko'k doira ichida oq qog'oz samolyot (o'zimiz chizgan). */
@Composable
fun ArugramLogo(modifier: Modifier = Modifier, size: Dp = 96.dp) {
    val c = LocalTg.current
    Canvas(modifier.size(size)) {
        val s = this.size.width / 100f
        drawCircle(c.fab, radius = 50f * s, center = Offset(50f * s, 50f * s))
        val plane = Path().apply {
            moveTo(24f * s, 49f * s)
            lineTo(75f * s, 28f * s)
            lineTo(64f * s, 73f * s)
            lineTo(50f * s, 61f * s)
            lineTo(43f * s, 69f * s)
            lineTo(41f * s, 56f * s)
            close()
        }
        drawPath(plane, Color.White)
        val fold = Path().apply {
            moveTo(41f * s, 56f * s)
            lineTo(66f * s, 38f * s)
            lineTo(50f * s, 61f * s)
            close()
        }
        drawPath(fold, Color(0xFFC8DAEA))
    }
}

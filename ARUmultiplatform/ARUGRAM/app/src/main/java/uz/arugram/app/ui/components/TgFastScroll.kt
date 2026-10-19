package uz.arugram.app.ui.components

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

private val Inactive = Color(0xFFC9CDD1)
private val Active = Color(0xFF3390EC)

/**
 * Tez aylantirgich (DrKLO `RecyclerListView.FastScroll`, LETTER_TYPE): o'ngda 5×30 dp li ingichka "tutqich" (to'xtab turganda kulrang
 * #C9CDD1), ushlanganda ko'k (#3390EC) bo'ladi va chapda 88 dp li pufak bilan 45 sp li joriy harf chiqadi. Pufakning pastki o'ng burchagi
 * kichik (4 dp) — tutqichga qaratilgan. Tutqich faqat o'ng 25 dp li chiziqda va o'z ustida bosilganda ushlanadi.
 */
@Composable
fun TgFastScroll(state: LazyListState, itemCount: Int, letterAt: (Int) -> String?, modifier: Modifier = Modifier) {
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()
    val measurer = rememberTextMeasurer()
    var dragging by remember { mutableStateOf(false) }
    var dragProgress by remember { mutableFloatStateOf(0f) }
    val bubble by animateFloatAsState(if (dragging) 1f else 0f, tween(150), label = "fsBubble")

    fun maxFirst(): Int = (itemCount - state.layoutInfo.visibleItemsInfo.size).coerceAtLeast(1)
    val scrollProgress = if (itemCount <= 1) 0f else (state.firstVisibleItemIndex.toFloat() / maxFirst()).coerceIn(0f, 1f)
    val progress = if (dragging) dragProgress else scrollProgress
    val letterIndex = if (dragging) (dragProgress * maxFirst()).toInt().coerceIn(0, (itemCount - 1).coerceAtLeast(0)) else state.firstVisibleItemIndex
    val letter = letterAt(letterIndex)

    Box(modifier.fillMaxSize()) {
        Canvas(Modifier.fillMaxHeight().width(132.dp).align(androidx.compose.ui.Alignment.CenterEnd)) {
            val d = density.density
            val y = kotlin.math.ceil((size.height - 54f * d) * progress)
            val left = (132f - 15f + 6f) * d
            val color = androidx.compose.ui.graphics.lerp(Inactive, Active, bubble)
            drawRoundRect(color, Offset(left, 12f * d + y), Size(5f * d, 30f * d), CornerRadius(2f * d))
            if (bubble > 0.001f && letter != null) {
                var top = y - 46f * d
                var diff = 0f
                if (top <= 12f * d) { diff = 12f * d - top; top = 12f * d }
                val rTop: Float
                val rBottom: Float
                if (diff <= 29f * d) {
                    rTop = 44f * d
                    rBottom = 4f * d + diff / (29f * d) * 40f * d
                } else {
                    val dd = diff - 29f * d
                    rBottom = 44f * d
                    rTop = 4f * d + (1f - dd / (29f * d)) * 40f * d
                }
                val path = Path().apply {
                    addRoundRect(
                        RoundRect(
                            10f * d, top, 98f * d, top + 88f * d,
                            topLeftCornerRadius = CornerRadius(44f * d), topRightCornerRadius = CornerRadius(rTop),
                            bottomRightCornerRadius = CornerRadius(rBottom), bottomLeftCornerRadius = CornerRadius(44f * d),
                        ),
                    )
                }
                val cx = left + 2.5f * d
                val cy = top + 30f * d + 12f * d - 12f * d
                withTransform({ scale(bubble, bubble, Offset(cx, y + 30f * d)) }) {
                    drawPath(path, color.copy(alpha = bubble))
                    val layout = measurer.measure(letter, TextStyle(color = Color.White, fontSize = 45.sp))
                    drawText(layout, topLeft = Offset(10f * d + (88f * d - layout.size.width) / 2f, top + (88f * d - layout.size.height) / 2f))
                }
            }
        }
        Box(
            Modifier.fillMaxHeight().width(25.dp).align(androidx.compose.ui.Alignment.CenterEnd).pointerInput(itemCount) {
                val d = density.density
                awaitEachGesture {
                    val down = awaitFirstDown(requireUnconsumed = false)
                    val h = size.height.toFloat()
                    val thumbTop = kotlin.math.ceil((h - 54f * d) * scrollProgress) + 12f * d
                    if (down.position.y < thumbTop || down.position.y > thumbTop + 30f * d) return@awaitEachGesture
                    down.consume()
                    var lastY = down.position.y
                    val startDy = lastY - thumbTop
                    var moved = false
                    dragProgress = scrollProgress
                    while (true) {
                        val event = awaitPointerEvent()
                        val ch = event.changes.firstOrNull() ?: break
                        if (!ch.pressed) break
                        if (!moved && kotlin.math.abs(ch.position.y - down.position.y) > viewConfiguration.touchSlop) { moved = true; dragging = true }
                        if (moved) {
                            val minY = 12f * d + startDy
                            val maxY = h - 42f * d + startDy
                            val newY = ch.position.y.coerceIn(minY, maxY)
                            dragProgress = (dragProgress + (newY - lastY) / (h - 54f * d)).coerceIn(0f, 1f)
                            lastY = newY
                            val target = (dragProgress * maxFirst()).toInt().coerceIn(0, (itemCount - 1).coerceAtLeast(0))
                            scope.launch { state.scrollToItem(target) }
                            ch.consume()
                        }
                    }
                    dragging = false
                }
            },
        )
    }
}

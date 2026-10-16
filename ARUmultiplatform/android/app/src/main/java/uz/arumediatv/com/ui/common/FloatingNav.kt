package uz.arumediatv.com.ui.common

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.border
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.abs
import kotlin.math.roundToInt
import uz.arumediatv.com.ui.theme.AruColors

class NavItem(val title: String, val icon: ImageVector)

/** Suzuvchi menyuning to'liq balandligi (10 + 52 + 10 + chegara). */
val FloatingNavHeight = 74.dp

private val EaseInOutSine = CubicBezierEasing(0.37f, 0f, 0.63f, 1f)

/**
 * Flutter `_BottomNav` ga mos: pushti "tabletka" bosilgan tugmaga SEKIN suzib boradi (oraliq tugmalarni birin-ketin
 * kattalashtirib o'tadi); tugma belgisi 1.22x kattalashib, ostidan yozuv ochiladi, tabletka ketgach — kichrayib yopiladi.
 * Bitta tugma oralig'iga 400 ms (kichik ekranda sekinroq), jami 480..1900 ms, egri chiziq — easeInOutSine.
 */
@Composable
fun FloatingNav(items: List<NavItem>, selected: Int, onSelect: (Int) -> Unit, modifier: Modifier = Modifier) {
    val pos = remember { Animatable(selected.toFloat()) }
    val widthDp = LocalConfiguration.current.screenWidthDp
    LaunchedEffect(selected) {
        val steps = abs(selected - pos.value)
        if (steps < 0.001f) return@LaunchedEffect
        val factor = if (widthDp <= 0) 1f else (600f / widthDp).coerceIn(0.9f, 1.2f)
        val ms = (steps * 400f * factor).coerceIn(480f, 1900f).roundToInt()
        pos.animateTo(selected.toFloat(), tween(ms, easing = EaseInOutSine))
    }

    val shape = RoundedCornerShape(28.dp)
    BoxWithConstraints(
        modifier
            .fillMaxWidth()
            .shadow(16.dp, shape, ambientColor = Color.Black, spotColor = Color.Black)
            .background(AruColors.Surface, shape)
            .border(1.dp, AruColors.Border, shape)
            .padding(horizontal = 6.dp, vertical = 10.dp),
    ) {
        // Animatsiya qiymati (`pos`) FAQAT chizish/joylashtirish bosqichida o'qiladi (lambdalar ichida) —
        // shuning uchun suzish paytida kompozitsiya qayta ishlamaydi va qotmaydi.
        val itemWidthPx = constraints.maxWidth / items.size.toFloat()
        val itemWidth = maxWidth / items.size
        val pillShape = RoundedCornerShape(20.dp)

        // Suzuvchi pushti tabletka
        Box(
            Modifier
                .width(itemWidth - 8.dp)
                .height(52.dp)
                .graphicsLayer { translationX = pos.value * itemWidthPx + 4.dp.toPx() }
                .shadow(8.dp, pillShape, ambientColor = AruColors.Accent, spotColor = AruColors.Accent)
                .background(Brush.linearGradient(listOf(AruColors.Accent, AruColors.Accent2)), pillShape),
        )

        Row(Modifier.fillMaxWidth()) {
            items.forEachIndexed { i, item ->
                fun t() = 1f - abs(pos.value - i).coerceIn(0f, 1f)
                Column(
                    Modifier
                        .weight(1f)
                        .height(52.dp)
                        .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) { onSelect(i) },
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement_Center,
                ) {
                    Icon(
                        item.icon,
                        contentDescription = item.title,
                        tint = Color.White,
                        modifier = Modifier
                            .height(22.dp)
                            .width(22.dp)
                            .graphicsLayer {
                                val k = t()
                                scaleX = 1f + 0.22f * k
                                scaleY = 1f + 0.22f * k
                                alpha = 0.40f + 0.60f * k
                            },
                    )
                    Box(
                        Modifier
                            .layout { m, c ->
                                val p = m.measure(c)
                                layout(p.width, (p.height * t()).roundToInt()) { p.placeRelative(0, 0) }
                            }
                            .clipToBounds()
                            .graphicsLayer { alpha = t() },
                    ) {
                        Text(
                            item.title,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            fontSize = 9.sp,
                            fontWeight = FontWeight.SemiBold,
                            color = Color.White,
                            modifier = Modifier.padding(top = 3.dp),
                        )
                    }
                }
            }
        }
    }
}

private val Arrangement_Center = androidx.compose.foundation.layout.Arrangement.Center

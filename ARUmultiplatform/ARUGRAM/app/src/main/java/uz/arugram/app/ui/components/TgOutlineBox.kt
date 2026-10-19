package uz.arugram.app.ui.components

import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.ClipOp
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import uz.arugram.app.ui.theme.LocalTg
import uz.arugram.app.ui.theme.blend
import androidx.compose.runtime.remember

/**
 * Telegram'dagi `OutlineTextContainerView` (DrKLO `ui/Components/OutlineTextContainerView.java`): chetida sarlavha o'yilgan
 * yumaloq (8 dp) ramka. Qiymatlar o'sha yerdan: yuqori bo'shliq 6 dp, ramka 0,5 dp -> tanlanganda 1,6667 dp, sarlavha 16 dp ->
 * 0,75 masshtab, sarlavha chetidan 14 dp, o'yiq 4 dp; tanlash/sarlavha/xato animatsiyalari — bukilmaydigan prujina (qattiqlik 500).
 *
 * @param selected tanlangan (fokus) — ramka qalinlashadi va rangi o'zgaradi
 * @param floated sarlavha tepada (kichik); `false` bo'lsa va [centerWhenEmpty] bo'lsa — markazda katta
 * @param freezeStyle Telegram'ning `forceUseCenter`i (davlat maydoni): rang/qalinlik o'zgarmaydi, sarlavha holati [floated] bilan boshqariladi
 * @param centerWhenEmpty maydon bo'sh va fokussiz bo'lganda sarlavha markazda turadi (parol maydoni)
 */
@Composable
fun TgOutlineBox(
    label: String,
    modifier: Modifier = Modifier,
    selected: Boolean = false,
    floated: Boolean = true,
    freezeStyle: Boolean = false,
    centerWhenEmpty: Boolean = false,
    error: Boolean = false,
    content: @Composable BoxScope.() -> Unit,
) {
    val c = LocalTg.current
    val spec = spring<Float>(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = 500f)
    val sel by animateFloatAsState(if (selected) 1f else 0f, spec, label = "sel")
    val title by animateFloatAsState(if (floated) 1f else 0f, spec, label = "title")
    val err by animateFloatAsState(if (error) 1f else 0f, spec, label = "err")
    val paint = remember { android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG) }
    val density = LocalDensity.current
    Box(
        modifier
            .fillMaxWidth()
            .height(58.dp)
            .drawBehind {
                val useCenter = freezeStyle || centerWhenEmpty
                val strokeRegular = maxOf(2f, 0.5.dp.toPx())
                val strokeSelected = 1.6667.dp.toPx()
                val stroke = if (freezeStyle) strokeRegular else strokeRegular + (strokeSelected - strokeRegular) * sel
                val textBase = blend(c.hint, c.value, if (freezeStyle) 0f else title)
                val textColor = blend(textBase, c.red, err)
                val lineBase = blend(c.input, c.inputActive, if (freezeStyle) 0f else sel)
                val lineColor = blend(lineBase, c.red, err)
                val paddingTop = 6.dp.toPx()
                val textSize = 16.dp.toPx()
                paint.textSize = textSize
                paint.color = textColor.toArgb()
                val textOffset = textSize / 2f - 1.75.dp.toPx()
                val topY = paddingTop + textOffset
                val centerY = size.height / 2f + textSize / 2f
                val textY = if (useCenter) topY + (centerY - topY) * (1f - title) else topY
                val scaleX = if (useCenter) 0.75f + 0.25f * (1f - title) else 0.75f
                val textWidth = paint.measureText(label) * scaleX
                val padL = 14.dp.toPx()
                val padT = 4.dp.toPx()

                // 1) Ramka: yuqori chizig'ining o'yiladigan qismi kesib tashlanadi (qolganini pastda chiziqlar bilan chizamiz).
                clipRect(left = padL - padT, top = paddingTop, right = size.width - (padL + padT), bottom = paddingTop + stroke * 2f, clipOp = ClipOp.Difference) {
                    drawRoundRect(
                        color = lineColor,
                        topLeft = Offset(stroke, paddingTop + stroke),
                        size = Size(size.width - stroke * 2f, size.height - paddingTop - stroke * 2f),
                        cornerRadius = CornerRadius(8.dp.toPx()),
                        style = Stroke(width = stroke, cap = StrokeCap.Round),
                    )
                }
                // 2) Yuqori chiziq: sarlavhaning ikki yonida (sarlavha markazdan ko'tarilganda o'yiq ochiladi).
                val left = padL - padT
                val lineY = paddingTop + stroke
                val right = size.width - stroke - 6.dp.toPx()
                val activeLeft = left + textWidth + (padL - padT)
                val fromLeft = left + textWidth / 2f
                val k = if (useCenter) title else 1f
                drawLine(lineColor, Offset(fromLeft + (activeLeft - fromLeft) * k, lineY), Offset(right, lineY), strokeWidth = stroke, cap = StrokeCap.Round)
                val fromRight = left + textWidth / 2f + padT
                drawLine(lineColor, Offset(left, lineY), Offset(fromRight + (left - fromRight) * k, lineY), strokeWidth = stroke, cap = StrokeCap.Round)
                // 3) Sarlavha matni.
                drawIntoCanvas { cv ->
                    val n = cv.nativeCanvas
                    n.save()
                    n.scale(scaleX, scaleX, padL + padT, textY)
                    n.drawText(label, padL, textY, paint)
                    n.restore()
                }
            }
            .padding(top = 6.dp),
        content = content,
    )
}

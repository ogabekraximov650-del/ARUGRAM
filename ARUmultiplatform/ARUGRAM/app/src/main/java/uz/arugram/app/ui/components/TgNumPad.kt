package uz.arugram.app.ui.components

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.waitForUpOrCancellation
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uz.arugram.app.R
import uz.arugram.app.ui.theme.LocalTg

const val NUMPAD_HEIGHT_DP = 230

private val Symbols = arrayOf("", "ABC", "DEF", "GHI", "JKL", "MNO", "PQRS", "TUV", "WXYZ", null, "+", null)

/**
 * Telegramning o'z raqam klaviaturasi (DrKLO `CustomPhoneKeyboardView`): balandligi 230 dp, chetlari 10 dp, tugmalar oralig'i 6 dp,
 * 3×4 to'r; raqam 24 sp (tugmaning 25 % joyida), harflar 14 sp (70 % joyida); tashqi burchaklar 24 dp, ichkilari 12 dp.
 * Tugma bosilganda 0,98 gacha kichrayadi. O'chirish tugmasi 200 ms ushlansa, har 50 ms'da takrorlaydi.
 */
@Composable
fun TgNumPad(onDigit: (Char) -> Unit, onBackspace: () -> Unit, modifier: Modifier = Modifier) {
    val digit = rememberUpdatedState(onDigit)
    val back = rememberUpdatedState(onBackspace)
    val sidePx = 10.dp
    val gap = 6.dp
    Layout(
        modifier = modifier,
        content = {
            for (i in 0 until 12) {
                if (i == 9) { Box(Modifier); continue }
                val num = if (i == 11) "" else (if (i == 10) "0" else (i + 1).toString())
                NumKey(i, num, Symbols[i].orEmpty(), isBack = i == 11, onDigit = { digit.value(it) }, onBack = { back.value() })
            }
        },
    ) { measurables, constraints ->
        val w = constraints.maxWidth
        val h = constraints.maxHeight
        val bw = (w - (sidePx * 2 + gap * 2).roundToPx()) / 3
        val bh = (h - (sidePx * 3 + gap * 2).roundToPx()) / 4
        val cc = Constraints.fixed(bw, bh)
        val placeables = measurables.map { it.measure(cc) }
        layout(w, h) {
            placeables.forEachIndexed { i, p ->
                val x = (i % 3) * (bw + gap.roundToPx()) + sidePx.roundToPx()
                val y = (i / 3) * (bh + gap.roundToPx()) + sidePx.roundToPx()
                p.place(x, y)
            }
        }
    }
}

@Composable
private fun NumKey(index: Int, num: String, symbols: String, isBack: Boolean, onDigit: (Char) -> Unit, onBack: () -> Unit) {
    val c = LocalTg.current
    val haptic = LocalHapticFeedback.current
    val scope = rememberCoroutineScope()
    val scale = remember { Animatable(1f) }
    var pressed by remember { mutableStateOf(false) }
    val top = index < 3
    val left = index % 3 == 0
    val right = index % 3 == 2
    val bottom = index > 8
    val shape = RoundedCornerShape(
        topStart = if (left && top) 24.dp else 12.dp,
        topEnd = if (right && top) 24.dp else 12.dp,
        bottomEnd = if (right && bottom) 24.dp else 12.dp,
        bottomStart = if (left && bottom) 24.dp else 12.dp,
    )
    val base = if (c.dark) Color(0x0FFFFFFF) else Color(0x0F000000)
    val press = if (c.dark) Color(0x1EFFFFFF) else Color(0x1E000000)
    Box(
        Modifier
            .graphicsLayer { scaleX = scale.value; scaleY = scale.value }
            .clip(shape)
            .background(if (pressed) press else base)
            .pointerInput(isBack) {
                awaitEachGesture {
                    awaitFirstDown()
                    pressed = true
                    scope.launch { scale.animateTo(0.98f, tween(120)) }
                    var repeater: kotlinx.coroutines.Job? = null
                    if (isBack) {
                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onBack()
                        repeater = scope.launch {
                            delay(200)
                            while (true) { haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove); onBack(); delay(50) }
                        }
                    }
                    val up = waitForUpOrCancellation()
                    repeater?.cancel()
                    pressed = false
                    scope.launch { scale.animateTo(1f, tween(180)) }
                    if (!isBack && up != null) {
                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onDigit(num[0])
                    }
                }
            },
    ) {
        if (isBack) {
            Image(
                painterResource(R.drawable.msg_clear_input),
                null,
                Modifier.fillMaxSize().padding(11.dp),
                colorFilter = ColorFilter.tint(c.text),
            )
        } else {
            // Raqam tugmaning 25 %, harflar 70 % kengligida markazlanadi (DrKLO `NumberButtonView.onDraw`).
            Layout(
                content = {
                    BasicText(num, style = TextStyle(color = c.text, fontSize = 24.sp))
                    BasicText(symbols, style = TextStyle(color = c.hint, fontSize = 14.sp))
                },
                modifier = Modifier.fillMaxSize(),
            ) { ms, cs ->
                val ps = ms.map { it.measure(Constraints()) }
                layout(cs.maxWidth, cs.maxHeight) {
                    ps[0].place((cs.maxWidth * 0.25f - ps[0].width / 2f).toInt(), (cs.maxHeight - ps[0].height) / 2)
                    ps[1].place((cs.maxWidth * 0.7f - ps[1].width / 2f).toInt(), (cs.maxHeight - ps[1].height) / 2)
                }
            }
        }
    }
}

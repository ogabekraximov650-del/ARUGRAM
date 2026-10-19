package uz.arugram.app.ui.intro

import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import uz.arugram.app.ui.components.ArugramLogo
import uz.arugram.app.ui.theme.LocalTg
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

/**
 * Kirishdan oldingi tanishtiruv oynasi (DrKLO `IntroActivity`): 6 ta sahifa (har birida o'zining harakatlanuvchi rasmi), pastda nuqtalar,
 * "Suhbatni boshlash" tugmasi (ustidan yorug'lik o'tib turadi), "Continue in English" havolasi, tepa o'ngda quyosh/oy (mavzu almashtirish).
 * Rasmlar Lottie emas — Canvas'da o'zimiz chizamiz.
 */
@Composable
fun IntroScreen(dark: Boolean, onToggleTheme: () -> Unit, onStart: () -> Unit) {
    val c = LocalTg.current
    var english by rememberSaveable { mutableStateOf(false) }
    val texts = if (english) IntroTexts.En else IntroTexts.Uz
    val pager = rememberPagerState(pageCount = { 6 })

    // Avtomatik varaq: foydalanuvchi tegmasa, har 4 soniyada keyingi sahifa.
    LaunchedEffect(pager.currentPage, pager.isScrollInProgress) {
        if (!pager.isScrollInProgress) {
            delay(4000)
            pager.animateScrollToPage((pager.currentPage + 1) % 6, animationSpec = tween(450, easing = FastOutSlowInEasing))
        }
    }

    Box(Modifier.fillMaxSize().background(c.bg).statusBarsPadding().navigationBarsPadding()) {
        Column(Modifier.fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally) {
            HorizontalPager(state = pager, modifier = Modifier.weight(1f).fillMaxWidth()) { page ->
                val active = pager.currentPage == page
                Column(
                    Modifier.fillMaxSize().padding(horizontal = 32.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                        IntroArt(page, active, Modifier.size(200.dp))
                    }
                    BasicText(
                        texts.titles[page],
                        style = TextStyle(color = c.text, fontSize = 24.sp, fontWeight = FontWeight.Medium, textAlign = TextAlign.Center),
                        modifier = Modifier.padding(top = 8.dp),
                    )
                    BasicText(
                        texts.bodies[page],
                        style = TextStyle(color = c.gray, fontSize = 15.sp, lineHeight = 21.sp, textAlign = TextAlign.Center),
                        modifier = Modifier.padding(top = 12.dp, bottom = 24.dp).height(66.dp),
                    )
                }
            }
            Dots(pager.currentPage, pager.currentPageOffsetFraction)
            Box(Modifier.padding(top = 28.dp, start = 24.dp, end = 24.dp).fillMaxWidth().height(50.dp)) { StartButton(texts.start, onStart) }
            BasicText(
                texts.switchLang,
                style = TextStyle(color = c.value, fontSize = 15.sp, textAlign = TextAlign.Center),
                modifier = Modifier.padding(top = 8.dp, bottom = 12.dp).clip(RoundedCornerShape(8.dp)).clickable { english = !english }.padding(horizontal = 16.dp, vertical = 10.dp),
            )
        }
        ThemeToggle(dark, onToggleTheme, Modifier.align(Alignment.TopEnd).padding(top = 8.dp, end = 12.dp))
    }
}

@Composable
private fun Dots(current: Int, fraction: Float) {
    val c = LocalTg.current
    Box(Modifier.height(10.dp), contentAlignment = Alignment.Center) {
        Canvas(Modifier.size(width = 6 * 16.dp, height = 10.dp)) {
            val r = 3.dp.toPx()
            val step = 16.dp.toPx()
            val startX = (size.width - step * 5) / 2f
            for (i in 0 until 6) drawCircle(c.hint.copy(alpha = 0.5f), r, Offset(startX + i * step, size.height / 2f))
            // Faol nuqta sahifa surilishi bilan birga yumshoq ko'chadi.
            val pos = (current + fraction).coerceIn(0f, 5f)
            drawCircle(c.fab, r * 1.15f, Offset(startX + pos * step, size.height / 2f))
        }
    }
}

/** Gradiyentli "Suhbatni boshlash" tugmasi, ustidan yorug' chiziq o'tib turadi (Telegram'dagi shimmer). */
@Composable
private fun StartButton(label: String, onClick: () -> Unit) {
    val c = LocalTg.current
    val tr = rememberInfiniteTransition(label = "shimmer")
    val x by tr.animateFloat(-0.4f, 1.4f, infiniteRepeatable(tween(2200, easing = LinearEasing), RepeatMode.Restart), label = "shimmerX")
    Box(
        Modifier.fillMaxSize().clip(RoundedCornerShape(12.dp)).background(Brush.horizontalGradient(listOf(Color(0xFF2CA5E0), Color(0xFF6BB7F0), c.fab))).clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Canvas(Modifier.fillMaxSize()) {
            val w = size.width
            val cx = w * x
            drawRect(
                Brush.horizontalGradient(listOf(Color.Transparent, Color.White.copy(alpha = 0.28f), Color.Transparent), startX = cx - w * 0.2f, endX = cx + w * 0.2f),
                size = size,
            )
        }
        BasicText(label, style = TextStyle(color = Color.White, fontSize = 17.sp, fontWeight = FontWeight.Medium))
    }
}

/** Quyosh <-> oy: bosilganda belgi aylanib, shakli o'zgaradi. */
@Composable
private fun ThemeToggle(dark: Boolean, onClick: () -> Unit, modifier: Modifier) {
    val c = LocalTg.current
    val t by animateFloatAsState(if (dark) 1f else 0f, tween(450, easing = FastOutSlowInEasing), label = "sunMoon")
    Box(modifier.size(44.dp).clip(CircleShape).clickable(onClick = onClick), contentAlignment = Alignment.Center) {
        Canvas(Modifier.size(28.dp).rotate(t * 180f)) {
            val ctr = Offset(size.width / 2f, size.height / 2f)
            val r = size.minDimension * (0.26f + 0.02f * t)
            val col = c.text
            drawCircle(col, r, ctr)
            // Oy: asosiy doiradan "tishlab" olingan ko'rinish — fon rangidagi doira siljib kirib keladi.
            if (t > 0f) drawCircle(c.bg, r * 0.85f, Offset(ctr.x + r * 0.7f * t, ctr.y - r * 0.55f * t))
            // Quyosh nurlari (oyga o'tganda yo'qoladi).
            val rays = 1f - t
            if (rays > 0.01f) {
                for (i in 0 until 8) {
                    val a = i * PI / 4
                    val r1 = r * 1.35f
                    val r2 = r * (1.35f + 0.5f * rays)
                    drawLine(col.copy(alpha = rays), Offset(ctr.x + (cos(a) * r1).toFloat(), ctr.y + (sin(a) * r1).toFloat()), Offset(ctr.x + (cos(a) * r2).toFloat(), ctr.y + (sin(a) * r2).toFloat()), strokeWidth = 2.dp.toPx(), cap = StrokeCap.Round)
                }
            }
        }
    }
}

// ---------------------------------------------------------------- sahifa rasmlari

@Composable
private fun IntroArt(page: Int, active: Boolean, modifier: Modifier) {
    val c = LocalTg.current
    val tr = rememberInfiniteTransition(label = "art")
    val p by tr.animateFloat(0f, 1f, infiniteRepeatable(tween(3600, easing = LinearEasing), RepeatMode.Restart), label = "artP")
    val ph = if (active) p else 0f
    val blue = c.fab
    Canvas(modifier) {
        val s = size.minDimension
        val ctr = Offset(size.width / 2f, size.height / 2f)
        // Orqa fon: yumshoq nafas oluvchi doira.
        val breathe = 1f + 0.04f * sin(ph * 2 * PI).toFloat()
        drawCircle(blue.copy(alpha = 0.12f), s * 0.5f * breathe, ctr)
        drawCircle(blue.copy(alpha = 0.10f), s * 0.36f * breathe, ctr)
        when (page) {
            0 -> plane(ctr, s, ph, blue)
            1 -> bubbles(ctr, s, ph, blue, c.bg)
            2 -> bolt(ctr, s, ph, blue)
            3 -> lock(ctr, s, ph, blue, c.bg)
            4 -> cloud(ctr, s, ph, blue, c.bg)
            else -> stars(ctr, s, ph, blue)
        }
    }
}

private fun DrawScope.plane(ctr: Offset, s: Float, ph: Float, col: Color) {
    val bob = sin(ph * 2 * PI).toFloat() * s * 0.03f
    // Iz: orqada kichrayib boruvchi doiralar
    for (i in 1..4) drawCircle(col.copy(alpha = 0.35f - i * 0.07f), s * (0.035f - i * 0.005f), Offset(ctr.x - s * (0.22f + i * 0.07f), ctr.y + s * 0.1f + bob + i * s * 0.02f))
    rotate(-18f + sin(ph * 2 * PI).toFloat() * 4f, Offset(ctr.x, ctr.y + bob)) {
        val x = ctr.x; val y = ctr.y + bob
        val body = Path().apply {
            moveTo(x - s * 0.26f, y + s * 0.02f); lineTo(x + s * 0.28f, y - s * 0.14f); lineTo(x + s * 0.05f, y + s * 0.2f)
            lineTo(x - s * 0.02f, y + s * 0.06f); close()
        }
        drawPath(body, col)
        val fold = Path().apply { moveTo(x - s * 0.02f, y + s * 0.06f); lineTo(x + s * 0.28f, y - s * 0.14f); lineTo(x - s * 0.06f, y + s * 0.14f); close() }
        drawPath(fold, Color.White.copy(alpha = 0.35f))
    }
}

private fun DrawScope.bubbles(ctr: Offset, s: Float, ph: Float, col: Color, bg: Color) {
    fun bubble(cx: Float, cy: Float, w: Float, h: Float, color: Color, left: Boolean) {
        drawRoundRect(color, Offset(cx - w / 2, cy - h / 2), Size(w, h), androidx.compose.ui.geometry.CornerRadius(h * 0.4f))
        val tail = Path().apply {
            val tx = if (left) cx - w * 0.3f else cx + w * 0.3f
            moveTo(tx, cy + h / 2 - 2f); lineTo(tx + (if (left) -h * 0.3f else h * 0.3f), cy + h / 2 + h * 0.3f); lineTo(tx + (if (left) h * 0.35f else -h * 0.35f), cy + h / 2 - 2f); close()
        }
        drawPath(tail, color)
    }
    val a = (sin(ph * 2 * PI).toFloat() + 1f) / 2f
    bubble(ctr.x - s * 0.08f, ctr.y - s * 0.12f + (a - .5f) * s * 0.03f, s * 0.5f, s * 0.24f, col, true)
    bubble(ctr.x + s * 0.08f, ctr.y + s * 0.14f - (a - .5f) * s * 0.03f, s * 0.46f, s * 0.22f, col.copy(alpha = 0.55f), false)
    // Yozilayotgan nuqtalar
    for (i in 0 until 3) {
        val k = ((sin((ph * 2 - i * 0.18f) * 2 * PI).toFloat() + 1f) / 2f)
        drawCircle(bg, s * 0.016f + k * s * 0.008f, Offset(ctr.x - s * 0.16f + i * s * 0.08f, ctr.y - s * 0.12f))
    }
}

private fun DrawScope.bolt(ctr: Offset, s: Float, ph: Float, col: Color) {
    val k = 1f + 0.06f * sin(ph * 4 * PI).toFloat()
    val x = ctr.x; val y = ctr.y
    val path = Path().apply {
        moveTo(x + s * 0.06f * k, y - s * 0.3f * k); lineTo(x - s * 0.16f * k, y + s * 0.04f * k); lineTo(x - s * 0.01f * k, y + s * 0.04f * k)
        lineTo(x - s * 0.07f * k, y + s * 0.3f * k); lineTo(x + s * 0.17f * k, y - s * 0.06f * k); lineTo(x + s * 0.02f * k, y - s * 0.06f * k); close()
    }
    drawPath(path, col)
    // Tezlik chiziqlari
    for (i in 0 until 3) {
        val m = ((ph + i / 3f) % 1f)
        drawLine(col.copy(alpha = (1f - m) * 0.6f), Offset(ctr.x - s * (0.3f + m * 0.15f), ctr.y - s * 0.12f + i * s * 0.12f), Offset(ctr.x - s * (0.4f + m * 0.15f), ctr.y - s * 0.12f + i * s * 0.12f), strokeWidth = 3.dp.toPx(), cap = StrokeCap.Round)
    }
}

private fun DrawScope.lock(ctr: Offset, s: Float, ph: Float, col: Color, bg: Color) {
    val open = (sin(ph * 2 * PI).toFloat() + 1f) / 2f * s * 0.025f
    val bx = ctr.x - s * 0.17f; val by = ctr.y - s * 0.02f
    drawRoundRect(col, Offset(bx, by), Size(s * 0.34f, s * 0.28f), androidx.compose.ui.geometry.CornerRadius(s * 0.04f))
    drawArc(col, 180f, 180f, false, Offset(ctr.x - s * 0.11f, by - s * 0.2f - open), Size(s * 0.22f, s * 0.4f), style = Stroke(s * 0.045f, cap = StrokeCap.Round))
    drawCircle(bg, s * 0.032f, Offset(ctr.x, by + s * 0.1f))
    drawLine(bg, Offset(ctr.x, by + s * 0.1f), Offset(ctr.x, by + s * 0.19f), strokeWidth = s * 0.03f, cap = StrokeCap.Round)
}

private fun DrawScope.cloud(ctr: Offset, s: Float, ph: Float, col: Color, bg: Color) {
    val dy = sin(ph * 2 * PI).toFloat() * s * 0.02f
    val y = ctr.y + dy
    drawCircle(col, s * 0.12f, Offset(ctr.x - s * 0.12f, y + s * 0.03f))
    drawCircle(col, s * 0.16f, Offset(ctr.x + s * 0.02f, y - s * 0.04f))
    drawCircle(col, s * 0.11f, Offset(ctr.x + s * 0.17f, y + s * 0.05f))
    drawRoundRect(col, Offset(ctr.x - s * 0.24f, y + s * 0.02f), Size(s * 0.5f, s * 0.14f), androidx.compose.ui.geometry.CornerRadius(s * 0.07f))
    // Tushayotgan/ko'tarilayotgan strelka
    val m = ph
    val ay = y + s * 0.02f + (1f - m) * s * 0.05f
    val a = (if (m < 0.2f) m / 0.2f else if (m > 0.8f) (1f - m) / 0.2f else 1f)
    val ac = bg.copy(alpha = a)
    drawLine(ac, Offset(ctr.x, ay + s * 0.09f), Offset(ctr.x, ay - s * 0.01f), strokeWidth = s * 0.025f, cap = StrokeCap.Round)
    drawLine(ac, Offset(ctr.x, ay - s * 0.01f), Offset(ctr.x - s * 0.04f, ay + s * 0.03f), strokeWidth = s * 0.025f, cap = StrokeCap.Round)
    drawLine(ac, Offset(ctr.x, ay - s * 0.01f), Offset(ctr.x + s * 0.04f, ay + s * 0.03f), strokeWidth = s * 0.025f, cap = StrokeCap.Round, )
}

private fun DrawScope.stars(ctr: Offset, s: Float, ph: Float, col: Color) {
    fun star(cx: Float, cy: Float, r: Float, rot: Float, color: Color) {
        val path = Path()
        for (i in 0 until 10) {
            val rr = if (i % 2 == 0) r else r * 0.45f
            val a = rot + i * PI / 5 - PI / 2
            val px = cx + (cos(a) * rr).toFloat(); val py = cy + (sin(a) * rr).toFloat()
            if (i == 0) path.moveTo(px, py) else path.lineTo(px, py)
        }
        path.close()
        drawPath(path, color, style = androidx.compose.ui.graphics.drawscope.Fill)
        drawPath(path, color, style = Stroke(r * 0.18f, join = StrokeJoin.Round))
    }
    star(ctr.x, ctr.y, s * 0.2f, (ph * 2 * PI).toFloat() * 0.15f, col)
    for (i in 0 until 5) {
        val a = ph * 2 * PI + i * 2 * PI / 5
        val tw = 0.5f + 0.5f * sin((ph * 3 + i * 0.4f) * 2 * PI).toFloat()
        star(ctr.x + (cos(a) * s * 0.36f).toFloat(), ctr.y + (sin(a) * s * 0.36f).toFloat(), s * (0.03f + 0.02f * tw), 0f, col.copy(alpha = 0.4f + 0.5f * tw))
    }
}

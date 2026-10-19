package uz.arugram.app.ui.intro

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.Easing
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
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Fill
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import uz.arugram.app.ui.theme.LocalTg
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.sin

private val TgBlue = Color(0xFF229AF0)          // ThemeColors.TELEGRAM_COLOR, featuredStickers_addButton
private val TgBlue2 = Color(0xFF56BAF0)         // featuredStickers_addButton2
private val AccelDecel = Easing { t -> (cos((t + 1f) * PI) / 2.0 + 0.5).toFloat() }

/** `OvershootInterpolator(tension)` — tugma qo'yib yuborilganda biroz oshib qaytadi (ScaleStateListAnimator). */
private fun overshoot(tension: Float) = Easing { t -> val x = t - 1f; x * x * ((tension + 1f) * x + tension) + 1f }

/**
 * Kirishdan oldingi tanishtiruv oynasi. O'lchamlar DrKLO `IntroActivity` dagi formulalar bo'yicha (dp):
 * H = oyna balandligi, to'rtdan biri q = H/4, rasm tepasi y0 = (3q - 275)/2; rasm 200x150; sarlavha y0+175 (26 sp, qalin, yon 18);
 * matn y0+175+26+18 (15 sp, yon 16, qator oralig'i +2.33); nuqtalar y0+289 (66x5, nuqta 5, qadam 11); tugma 48 balandlikda, kengligi 320
 * (yoki ekran-32), tepasi 3q+(q-48)/2; til havolasi (16 sp, balandligi 30) tugmadan 30 yuqorida; mavzu tugmasi 64x64, tepa-o'ngda (4 dp chekka).
 * Oyna "Suhbatni boshlash" bosilganda tugma pastki o'ngdagi dumaloq tugmaga aylanib o'tadi (300 ms), qolgani so'nadi.
 */
@Composable
fun IntroScreen(
    dark: Boolean,
    onToggleTheme: (Offset) -> Unit,
    onStartMorph: () -> Unit,
    onFinished: () -> Unit,
) {
    val c = LocalTg.current
    val scope = rememberCoroutineScope()
    var english by rememberSaveable { mutableStateOf(false) }
    val texts = if (english) IntroTexts.En else IntroTexts.Uz
    val pager = rememberPagerState(pageCount = { 6 })
    val morph = remember { Animatable(0f) }
    var started by remember { mutableStateOf(false) }
    val tr = rememberInfiniteTransition(label = "intro")
    val ph by tr.animateFloat(0f, 1f, infiniteRepeatable(tween(3600, easing = LinearEasing), RepeatMode.Restart), label = "introPhase")
    val d = LocalDensity.current
    val imeB = with(d) { WindowInsets.ime.getBottom(d).toDp() }
    val navB = with(d) { WindowInsets.navigationBars.getBottom(d).toDp() }
    val statusH = with(d) { WindowInsets.statusBars.getTop(d).toDp() }
    val m = morph.value

    BoxWithConstraints(Modifier.fillMaxSize().navigationBarsPadding()) {
        val w = maxWidth
        // Telegram'da konteyner butun ekran (status panel ostida ham), pastda navigatsiya paneli hisobga olinmaydi.
        val h = maxHeight
        val q = h / 4
        val y0 = (q * 3 - 275.dp) / 2
        val pos = pager.currentPage + pager.currentPageOffsetFraction

        Box(Modifier.fillMaxSize().alpha(1f - m).background(c.bg)) {
            // ---- Rasm (200x150), sahifalar almashganda yumshoq o'tadi
            Box(Modifier.offset(x = (w - 200.dp) / 2, y = y0).size(200.dp, 150.dp)) {
                val lo = kotlin.math.floor(pos.toDouble()).toInt().coerceIn(0, 5)
                val hi = (lo + 1).coerceAtMost(5)
                val t = (pos - lo).coerceIn(0f, 1f)
                IntroArt(lo, ph, Modifier.fillMaxSize().graphicsLayer { alpha = 1f - t; translationX = -t * 60.dp.toPx(); scaleX = 1f - 0.15f * t; scaleY = 1f - 0.15f * t })
                if (hi != lo) IntroArt(hi, ph, Modifier.fillMaxSize().graphicsLayer { alpha = t; translationX = (1f - t) * 60.dp.toPx(); scaleX = 0.85f + 0.15f * t; scaleY = 0.85f + 0.15f * t })
            }

            // ---- Sahifalar: sarlavha va matn
            HorizontalPager(state = pager, modifier = Modifier.fillMaxSize()) { page ->
                Box(Modifier.fillMaxSize()) {
                    if (page == 0) {
                        BasicText(
                            texts.titles[0],
                            style = TextStyle(color = c.text.copy(alpha = 0.9f), fontSize = 26.sp, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center, letterSpacing = 0.5.sp),
                            modifier = Modifier.offset(x = 18.dp, y = y0 + 175.dp).width(w - 36.dp),
                        )
                    } else {
                        BasicText(
                            texts.titles[page],
                            style = TextStyle(color = c.text, fontSize = 26.sp, fontWeight = FontWeight.Medium, textAlign = TextAlign.Center),
                            modifier = Modifier.offset(x = 18.dp, y = y0 + 175.dp).width(w - 36.dp),
                        )
                    }
                    BasicText(
                        boldTags(texts.bodies[page]),
                        style = TextStyle(color = c.text, fontSize = 15.sp, lineHeight = 20.sp, textAlign = TextAlign.Center),
                        modifier = Modifier.offset(x = 16.dp, y = y0 + 175.dp + 26.dp + 18.dp).width(w - 32.dp),
                    )
                }
            }

            // ---- Nuqtalar: 66x5, y0+150+122+17
            Dots(pos, dark, Modifier.offset(x = (w - 66.dp) / 2, y = y0 + 289.dp).size(66.dp, 5.dp))

            // ---- Til havolasi: balandligi 30, tugma tepasidan 30 yuqorida
            val btnTop = q * 3 + (q - 48.dp) / 2
            BasicText(
                texts.switchLang,
                style = TextStyle(color = if (dark) Color(0xFF64B5EF) else Color(0xFF1C93E3), fontSize = 16.sp, textAlign = TextAlign.Center),
                modifier = Modifier.offset(x = (w - 240.dp) / 2, y = btnTop - 60.dp).size(240.dp, 30.dp).clickable(
                    interactionSource = remember { MutableInteractionSource() }, indication = null,
                ) { if (!started) english = !english }.padding(top = 4.dp),
            )

            // ---- Mavzu tugmasi: 64x64, tepa-o'ngda; belgi 28 dp
            var themeCenter by remember { mutableStateOf(Offset.Zero) }
            Box(
                Modifier.align(Alignment.TopEnd).padding(top = 4.dp + statusH, end = 4.dp).size(64.dp)
                    .onGloballyPositioned { themeCenter = it.positionInRoot() + Offset(it.size.width / 2f, it.size.height / 2f) }
                    .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) { if (!started) onToggleTheme(themeCenter) },
                contentAlignment = Alignment.Center,
            ) { SunMoon(dark, Modifier.size(28.dp)) }
        }

        // ---- "Suhbatni boshlash" tugmasi: bosilganda pastki o'ngdagi dumaloq tugmaga aylanadi
        val btnTopDp = q * 3 + (q - 48.dp) / 2
        val fullW = if (w - 32.dp > 260.dp) 320.dp else w - 32.dp
        val bw = fullW + (56.dp - fullW) * m
        val bh = 48.dp + 8.dp * m
        val bx = (w - fullW) / 2 + ((w - 20.dp - 56.dp) - (w - fullW) / 2) * m
        val fabTop = maxHeight - 14.dp - 56.dp - (if (imeB > navB) imeB - navB else 0.dp)
        val by = btnTopDp + (fabTop - btnTopDp) * m
        StartButton(texts.start, !started, m, Modifier.offset(x = bx, y = by).size(bw, bh)) {
            if (started) return@StartButton
            started = true
            onStartMorph()
            scope.launch {
                morph.animateTo(1f, tween(300, easing = AccelDecel))
                onFinished()
            }
        }
    }
}

/** `**qalin**` belgilarini qalin matnga aylantiradi (Telegram `replaceTags`). */
private fun boldTags(src: String): AnnotatedString = buildAnnotatedString {
    val parts = src.split("**")
    parts.forEachIndexed { i, part ->
        if (i % 2 == 1) withStyle(SpanStyle(fontWeight = FontWeight.Medium)) { append(part) } else append(part)
    }
}

/** `BottomPagesView`: nuqta 5 dp, qadam 11 dp; faol nuqta sahifa almashganda cho'ziladi. */
@Composable
private fun Dots(pos: Float, dark: Boolean, modifier: Modifier) {
    Canvas(modifier) {
        val d = 5.dp.toPx(); val step = 11.dp.toPx(); val r = CornerRadius(2.5.dp.toPx())
        val inactive = if (dark) Color(0xFF555555) else Color(0xFFBBBBBB)
        for (i in 0 until 6) drawRoundRect(inactive, Offset(i * step, 0f), Size(d, d), r)
        val p = pos.coerceIn(0f, 5f)
        val i = kotlin.math.floor(p.toDouble()).toInt()
        val t = p - i
        val left = step * (i + max(0f, 2f * t - 1f))
        val right = step * (i + minOf(1f, 2f * t)) + d
        drawRoundRect(TgBlue, Offset(left, 0f), Size(right - left, d), r)
    }
}

/**
 * Gradiyentli tugma (chapdan o'ngga 229AF0 -> 56BAF0), to'liq yumaloq; ustidan yorug' dog' o'tadi (CellFlickerDrawable: kengligi 160 dp,
 * eng yorug'i oq 25%, tezligi 1200 ms/tugma kengligi, takrorlanish 2.0 — orada tanaffus). Bosganda 2% kichrayadi (80 ms), qo'yib yuborganda
 * 350 ms da oshib qaytadi.
 */
@Composable
private fun StartButton(label: String, enabled: Boolean, morph: Float, modifier: Modifier, onClick: () -> Unit) {
    val src = remember { MutableInteractionSource() }
    val pressed by src.collectIsPressedAsState()
    val sc by animateFloatAsState(
        if (pressed) 0.98f else 1f,
        if (pressed) tween<Float>(80) else tween<Float>(350, easing = overshoot(1.2f)),
        label = "btnScale",
    )
    val tr = rememberInfiniteTransition(label = "flicker")
    val prog by tr.animateFloat(0f, 2f, infiniteRepeatable(tween(2400, easing = LinearEasing), RepeatMode.Restart), label = "flickerP")
    Box(
        modifier.scale(sc).clip(CircleShape).background(Brush.horizontalGradient(listOf(TgBlue, TgBlue2)))
            .clickable(interactionSource = src, indication = null, enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Canvas(Modifier.fillMaxSize()) {
            val band = 160.dp.toPx()
            val x = (size.width + band * 2) * prog - band
            drawRect(
                Brush.horizontalGradient(listOf(Color.Transparent, Color.White.copy(alpha = 64f / 255f), Color.Transparent), startX = x, endX = x + band),
                size = size,
            )
        }
        if (morph < 1f) BasicText(label, style = TextStyle(color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.Medium), modifier = Modifier.alpha((1f - morph * 3f).coerceIn(0f, 1f)))
    }
}

/** Quyosh <-> oy (Telegram'da Lottie `sun`); rangi — tugma ko'k rangi. */
@Composable
private fun SunMoon(dark: Boolean, modifier: Modifier) {
    val t by animateFloatAsState(if (dark) 1f else 0f, tween(450, easing = FastOutSlowInEasing), label = "sunMoon")
    val bg = LocalTg.current.bg
    Canvas(modifier.graphicsLayer { rotationZ = t * 180f }) {
        val ctr = Offset(size.width / 2f, size.height / 2f)
        val r = size.minDimension * (0.26f + 0.02f * t)
        drawCircle(TgBlue, r, ctr)
        if (t > 0f) drawCircle(bg, r * 0.85f, Offset(ctr.x + r * 0.7f * t, ctr.y - r * 0.55f * t))
        val rays = 1f - t
        if (rays > 0.01f) {
            for (i in 0 until 8) {
                val a = i * PI / 4
                val r1 = r * 1.35f
                val r2 = r * (1.35f + 0.5f * rays)
                drawLine(TgBlue.copy(alpha = rays), Offset(ctr.x + (cos(a) * r1).toFloat(), ctr.y + (sin(a) * r1).toFloat()), Offset(ctr.x + (cos(a) * r2).toFloat(), ctr.y + (sin(a) * r2).toFloat()), strokeWidth = 2.dp.toPx(), cap = StrokeCap.Round)
            }
        }
    }
}

// ---------------------------------------------------------------- sahifa rasmlari

@Composable
private fun IntroArt(page: Int, ph: Float, modifier: Modifier) {
    val c = LocalTg.current
    val blue = TgBlue
    Canvas(modifier) {
        val s = size.minDimension
        val ctr = Offset(size.width / 2f, size.height / 2f)
        // Orqa fon: yumshoq nafas oluvchi doira.
        val breathe = 1f + 0.04f * sin(ph * 2 * PI).toFloat()
        drawCircle(blue.copy(alpha = 0.12f), s * 0.5f * breathe, ctr)
        drawCircle(blue.copy(alpha = 0.10f), s * 0.36f * breathe, ctr)
        when (page) {
            0 -> plane(ctr, s, ph, blue)
            1 -> bolt(ctr, s, ph, blue)
            2 -> bubbles(ctr, s, ph, blue, c.bg)
            3 -> stars(ctr, s, ph, blue)
            4 -> lock(ctr, s, ph, blue, c.bg)
            else -> cloud(ctr, s, ph, blue, c.bg)
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

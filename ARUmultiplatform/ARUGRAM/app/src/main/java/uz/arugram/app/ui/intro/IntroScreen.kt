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
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.clipPath
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
                IntroArt(lo, ph, 1f - t, Modifier.fillMaxSize().graphicsLayer { cameraDistance = 12f * density; rotationY = 70f * t; alpha = 1f - t; translationX = -t * 70.dp.toPx(); scaleX = 1f - 0.2f * t; scaleY = 1f - 0.2f * t })
                if (hi != lo) IntroArt(hi, ph, t, Modifier.fillMaxSize().graphicsLayer { cameraDistance = 12f * density; rotationY = -70f * (1f - t); alpha = t; translationX = (1f - t) * 70.dp.toPx(); scaleX = 0.8f + 0.2f * t; scaleY = 0.8f + 0.2f * t })
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

/** Quyosh/oy (Telegram'da Lottie `sun`): tungi mavzuda quyosh, kunduzgida oy ko'rinadi (bosilsa ikkinchisiga o'tadi). Rangi — tugma ko'k rangi. */
@Composable
private fun SunMoon(dark: Boolean, modifier: Modifier) {
    val t by animateFloatAsState(if (dark) 1f else 0f, tween(450, easing = FastOutSlowInEasing), label = "sunMoon")
    val bg = LocalTg.current.bg
    Canvas(modifier.graphicsLayer { rotationZ = (1f - t) * -90f }) {
        val ctr = Offset(size.width / 2f, size.height / 2f)
        val u = 1.dp.toPx()
        // Quyosh: to'ldirilgan doira (diametri 14 dp) va atrofida 8 ta qisqa nur.
        if (t > 0.01f) {
            drawCircle(TgBlue.copy(alpha = t), 7f * u * (0.6f + 0.4f * t), ctr)
            for (i in 0 until 8) {
                val a = i * PI / 4
                val r1 = 10.5f * u
                val r2 = 12.5f * u
                drawLine(TgBlue.copy(alpha = t), Offset(ctr.x + (cos(a) * r1).toFloat(), ctr.y + (sin(a) * r1).toFloat()), Offset(ctr.x + (cos(a) * r2).toFloat(), ctr.y + (sin(a) * r2).toFloat()), strokeWidth = 1.6f * u, cap = StrokeCap.Round)
            }
        }
        // Oy: yarim oy shakli (doiradan siljigan doira kesib olinadi).
        if (t < 0.99f) {
            val moon = Path().apply {
                addOval(androidx.compose.ui.geometry.Rect(ctr, 9f * u))
            }
            val cut = Path().apply { addOval(androidx.compose.ui.geometry.Rect(Offset(ctr.x + 5f * u, ctr.y - 4f * u), 8f * u)) }
            val crescent = Path.combine(androidx.compose.ui.graphics.PathOperation.Difference, moon, cut)
            drawPath(crescent, TgBlue.copy(alpha = 1f - t))
        }
    }
}


// ---------------------------------------------------------------- sahifa rasmlari
// Rasmlar Telegram kirish oynasining skrinshotidan o'lchab chizilgan (o'lchamlar 4x kattalashtirilgan kesimning pikselida, /4 = dp);
// markaz — 200x150 dp maydonning o'rtasi.

private val RedBox = Color(0xFFF64938)
private val RedDark = Color(0xFFD23929)
private val Navy = Color(0xFF1F3447)
private val NeedleRed = Color(0xFFE84C3D)
private val ArcRed = Color(0xFFF84735)
private val BubbleBlue = Color(0xFF2F5A84)
private val SafeBody = Color(0xFFC7CED4)
private val SafeDisc = Color(0xFFACB3BB)
private val SafeLight = Color(0xFFCACFD5)
private val KeyGray = Color(0xFF61666A)

@Composable
private fun IntroArt(page: Int, ph: Float, appear: Float, modifier: Modifier) {
    Canvas(modifier) {
        val u = 1.dp.toPx()
        val ctr = Offset(size.width / 2f, size.height / 2f)
        fun p(x: Float, y: Float) = Offset(ctr.x + (x - 360f) / 4f * u, ctr.y + (y - 346f) / 4f * u)
        fun l(v: Float) = v / 4f * u
        when (page) {
            0, 5 -> logo(::p, ::l, ph)
            1 -> speedometer(::p, ::l, ph, appear)
            2 -> gift(::p, ::l, ph)
            3 -> infinity(::p, ::l, ph)
            else -> safe(::p, ::l, ph)
        }
    }
}

private fun DrawScope.logo(p: (Float, Float) -> Offset, l: (Float) -> Float, ph: Float) {
    drawCircle(Color(0xFF229AF0), l(308f), p(360f, 346f))
    val plane = Path().apply {
        moveTo(p(178f, 366).x, p(178f, 366).y)
        lineTo(p(484f, 245).x, p(484f, 245).y)
        lineTo(p(446f, 490).x, p(446f, 490).y)
        lineTo(p(414f, 486).x, p(414f, 486).y)
        lineTo(p(316f, 418).x, p(316f, 418).y)
        lineTo(p(408f, 312).x, p(408f, 312).y)
        lineTo(p(332f, 372).x, p(332f, 372).y)
        lineTo(p(262f, 393).x, p(262f, 393).y)
        close()
    }
    drawPath(plane, Color.White)
    drawPath(plane, Color.White, style = Stroke(l(20f), join = StrokeJoin.Round))
}

private fun DrawScope.speedometer(p: (Float, Float) -> Offset, l: (Float) -> Float, ph: Float, appear: Float) {
    val c = p(360f, 346f)
    drawCircle(Navy, l(304f), c)
    drawCircle(Color.White, l(276f), c)
    drawCircle(Color(0xFFE3E5E5), l(240f), c, style = Stroke(l(14f)))
    // Qizil yoy: tashqi radiusi 241, ichki chetining radiusi burchakka qarab 172..241 (chap yuqorida uchi o'tkir).
    val outer = 241f
    val inner = listOf(-136f to 241f, -112f to 226f, -90f to 211f, -45f to 201f, -2f to 188f, 22f to 172f)
    fun pol(deg: Float, r: Float): Offset {
        val a = Math.toRadians(deg.toDouble())
        return Offset(c.x + (cos(a) * l(r)).toFloat(), c.y + (sin(a) * l(r)).toFloat())
    }
    val arc = Path()
    var first = true
    var d = -136f
    while (d <= 22f) {
        val o = pol(d, outer)
        if (first) { arc.moveTo(o.x, o.y); first = false } else arc.lineTo(o.x, o.y)
        d += 2f
    }
    for (k in inner.indices.reversed()) {
        val o = pol(inner[k].first, inner[k].second)
        arc.lineTo(o.x, o.y)
    }
    arc.close()
    drawPath(arc, ArcRed)
    // Mil: sahifa ko'ringanda chapdan o'ngga yuguradi, so'ng sal tebranib turadi.
    val e = FastOutSlowInEasing.transform(appear.coerceIn(0f, 1f))
    val ang = 29f - (1f - e) * 120f + sin(ph * 2 * PI).toFloat() * 2f * e
    val a = Math.toRadians(ang.toDouble())
    val dx = cos(a).toFloat(); val dy = sin(a).toFloat()
    val tail = l(88f); val head = l(222f)
    drawLine(NeedleRed, Offset(c.x - dx * tail, c.y - dy * tail), Offset(c.x + dx * head, c.y + dy * head), strokeWidth = l(20f), cap = StrokeCap.Round)
    drawCircle(Navy, l(40f), c)
}

private fun DrawScope.gift(p: (Float, Float) -> Offset, l: (Float) -> Float, ph: Float) {
    val tl = p(72f, 58f)
    val sq = Size(l(576f), l(577f))
    val cr = CornerRadius(l(105f))
    val drift = sin(ph * 2 * PI).toFloat()
    drawRoundRect(RedBox, tl, sq, cr)
    clipPath(Path().apply { addRoundRect(androidx.compose.ui.geometry.RoundRect(tl.x, tl.y, tl.x + sq.width, tl.y + sq.height, cr)) }) {
        // Xira belgilar (qalam, tabassum, joy, xabar, kamera) — sekin suzib yuradi.
        val dy = drift * l(6f)
        rotate(40f, p(205f, 135f)) { drawRoundRect(RedDark, p(182f, 70f), Size(l(46f), l(130f)), CornerRadius(l(10f))) }
        drawCircle(RedDark, l(80f), p(505f, 100f), style = Stroke(l(16f)))
        drawCircle(RedDark, l(10f), p(480f, 95f)); drawCircle(RedDark, l(10f), p(535f, 90f))
        drawCircle(RedDark, l(55f), p(105f, 365f)); drawRoundRect(RedDark, p(70f, 380f), Size(l(90f), l(90f)), CornerRadius(l(10f)))
        drawRoundRect(RedDark, p(160f, 515f + dy), Size(l(145f), l(115f)), CornerRadius(l(22f)))
        drawCircle(RedBox, l(11f), p(230f, 566f + dy))
        drawRoundRect(RedDark, p(505f, 462f - dy), Size(l(145f), l(120f)), CornerRadius(l(18f)))
        drawCircle(RedBox, l(33f), p(577f, 520f - dy), style = Stroke(l(10f)))
    }
    // Oq lentalar: vertikal (kengligi 48) va gorizontal (kengligi 46, uchlari qiyshiq).
    drawRect(Color.White, p(336f, 38f), Size(l(48f), l(617f)))
    val hb = Path().apply {
        moveTo(p(65f, 287f).x, p(65f, 287f).y); lineTo(p(715f, 287f).x, p(715f, 287f).y)
        lineTo(p(688f, 333f).x, p(688f, 333f).y); lineTo(p(38f, 333f).x, p(38f, 333f).y); close()
    }
    drawPath(hb, Color.White)
    // Kapalak: to'rtta halqa, ichi qizil tomchi.
    val loops = listOf(Offset(272f, 228f), Offset(448f, 228f), Offset(272f, 392f), Offset(448f, 392f))
    for ((k, o) in loops.withIndex()) {
        val c = p(o.x, o.y)
        drawOval(Color.White, Offset(c.x - l(66f), c.y - l(72f)), Size(l(132f), l(144f)))
        val hx = if (k % 2 == 0) 20f else -20f
        val hy = if (k < 2) 10f else -10f
        val hc = p(o.x + hx, o.y + hy)
        drawOval(RedBox, Offset(hc.x - l(42f), hc.y - l(46f)), Size(l(84f), l(92f)))
    }
    drawRect(Color.White, p(336f, 38f), Size(l(48f), l(60f)))
}

private fun DrawScope.infinity(p: (Float, Float) -> Offset, l: (Float) -> Float, ph: Float) {
    // Pufak: o'ng chetida tekis kesilgan (Telegram'dagidek), pastki chapda dum.
    clipRect(right = p(685f, 0f).x) {
        drawRoundRect(BubbleBlue, p(38f, 88f), Size(l(760f), l(474f)), CornerRadius(l(85f)))
    }
    val tail = Path().apply {
        moveTo(p(218f, 555f).x, p(218f, 555f).y); lineTo(p(195f, 655f).x, p(195f, 655f).y)
        lineTo(p(236f, 655f).x, p(236f, 655f).y); lineTo(p(366f, 562f).x, p(366f, 562f).y); lineTo(p(366f, 555f).x, p(366f, 555f).y); close()
    }
    drawPath(tail, BubbleBlue)
    // Cheksizlik belgisi (qalinligi 46).
    val inf = Path().apply {
        moveTo(p(360f, 322f).x, p(360f, 322f).y)
        cubicTo(p(405f, 262f).x, p(405f, 262f).y, p(455f, 223f).x, p(455f, 223f).y, p(510f, 223f).x, p(510f, 223f).y)
        cubicTo(p(555f, 223f).x, p(555f, 223f).y, p(585f, 265f).x, p(585f, 265f).y, p(585f, 322f).x, p(585f, 322f).y)
        cubicTo(p(585f, 379f).x, p(585f, 379f).y, p(555f, 421f).x, p(555f, 421f).y, p(510f, 421f).x, p(510f, 421f).y)
        cubicTo(p(455f, 421f).x, p(455f, 421f).y, p(405f, 382f).x, p(405f, 382f).y, p(360f, 322f).x, p(360f, 322f).y)
        cubicTo(p(315f, 262f).x, p(315f, 262f).y, p(265f, 223f).x, p(265f, 223f).y, p(210f, 223f).x, p(210f, 223f).y)
        cubicTo(p(165f, 223f).x, p(165f, 223f).y, p(135f, 265f).x, p(135f, 265f).y, p(135f, 322f).x, p(135f, 322f).y)
        cubicTo(p(135f, 379f).x, p(135f, 379f).y, p(165f, 421f).x, p(165f, 421f).y, p(210f, 421f).x, p(210f, 421f).y)
        cubicTo(p(265f, 421f).x, p(265f, 421f).y, p(315f, 382f).x, p(315f, 382f).y, p(360f, 322f).x, p(360f, 322f).y)
        close()
    }
    drawPath(inf, Color.White, style = Stroke(l(46f), cap = StrokeCap.Round, join = StrokeJoin.Round))
    // Mayda yulduzchalar: miltillaydi.
    val dots = listOf(93f to 188f, 147f to 153f, 251f to 187f, 342f to 198f, 424f to 168f, 480f to 115f, 516f to 178f, 623f to 458f, 468f to 473f, 306f to 496f, 280f to 578f, 230f to 252f, 457f to 265f, 520f to 383f)
    for ((k, d) in dots.withIndex()) {
        val tw = 0.5f + 0.5f * sin((ph * 2 + k * 0.23f) * 2 * PI).toFloat()
        drawCircle(Color.White.copy(alpha = 0.35f + 0.65f * tw), l(3f + 2f * tw), p(d.first, d.second))
    }
}

private fun DrawScope.safe(p: (Float, Float) -> Offset, l: (Float) -> Float, ph: Float) {
    val tl = p(72f, 58f)
    drawRoundRect(SafeBody, tl, Size(l(576f), l(577f)), CornerRadius(l(105f)))
    for (o in listOf(141f to 128f, 578f to 128f, 141f to 565f, 578f to 565f)) drawCircle(SafeDisc, l(19f), p(o.first, o.second))
    val c = p(360f, 346f)
    drawCircle(Color.Black.copy(alpha = 0.08f), l(262f), Offset(c.x, c.y + l(6f)))
    drawCircle(Color.White, l(260f), c)
    drawCircle(SafeDisc, l(222f), c)
    drawCircle(SafeLight, l(116f), c)
    drawCircle(KeyGray, l(34f), p(360f, 328f))
    drawRoundRect(KeyGray, p(345f, 335f), Size(l(31f), l(73f)), CornerRadius(l(15f)))
}

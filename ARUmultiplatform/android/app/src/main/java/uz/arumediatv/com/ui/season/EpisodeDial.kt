package uz.arumediatv.com.ui.season

import android.content.Context
import android.graphics.Paint
import android.graphics.Typeface
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.exponentialDecay
import androidx.compose.animation.core.spring
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.Episode
import uz.arumediatv.com.ui.common.AruIcons
import uz.arumediatv.com.ui.theme.AruColors
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.asin
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.floor
import kotlin.math.hypot
import kotlin.math.min
import kotlin.math.roundToInt
import kotlin.math.sin
import kotlin.math.sqrt

/** Qismlar boshqaruvi turi (Sozlamalardan): 0 — oddiy (pastdagi tugmalar), 1 — yarim aylana o'ngda, 2 — yarim aylana chapda. */
object DialPrefs {
    private const val KEY = "ep_control"
    private fun prefs() = AppServices.appContext.getSharedPreferences("aru_ui", Context.MODE_PRIVATE)

    var mode by mutableIntStateOf(read())
        private set

    private fun read(): Int = try { prefs().getInt(KEY, 0).coerceIn(0, 2) } catch (_: Throwable) { 0 }

    fun set(v: Int) {
        mode = v.coerceIn(0, 2)
        try { prefs().edit().putInt(KEY, mode).apply() } catch (_: Throwable) { }
    }
}

private const val CARD_H = 56
private const val PITCH = 64

private fun dialClock(ms: Long): String {
    val t = (ms / 1000).coerceAtLeast(0)
    val h = t / 3600
    val m = (t % 3600) / 60
    val s = t % 60
    return if (h > 0) String.format(java.util.Locale.US, "%d:%02d:%02d", h, m, s) else String.format(java.util.Locale.US, "%d:%02d", m, s)
}

/**
 * Qismlar: ekran chetidagi yarim aylana boshqaruv.
 *  - tashqi halqa: qism raqamlari — aylantirilsa kartalar aylana yoyiga yopishib siljiydi;
 *  - o'rta halqa: vaqt (har chiziq 5 soniya) — aylantirib qo'yib yuborilganda video shu joyga o'tadi;
 *  - ichkarida: play/pauza, oldingi va keyingi qism.
 */
@Composable
fun EpisodeDialTab(
    episodes: List<Episode>,
    currentId: Long?,
    player: AruPlayer,
    left: Boolean,
    hasPrev: Boolean,
    hasNext: Boolean,
    onPlay: (Episode) -> Unit,
    onPrev: () -> Unit,
    onNext: () -> Unit,
) {
    val asc = remember(episodes) { episodes.sortedBy { it.number } }
    val n = asc.size
    if (n == 0) return
    val curIdx = asc.indexOfFirst { it.id == currentId }.coerceAtLeast(0)
    var pos by remember { mutableFloatStateOf(curIdx.toFloat()) }
    var scrub by remember { mutableStateOf<Long?>(null) }
    val scope = rememberCoroutineScope()
    var job by remember { mutableStateOf<Job?>(null) }
    val haptic = LocalHapticFeedback.current
    val base by remember { derivedStateOf { pos.roundToInt().coerceIn(0, n - 1) } }

    // Qism almashganda (tugma yoki avto) g'ildirak shu qismga suriladi.
    LaunchedEffect(curIdx) {
        val from = pos
        if (from != curIdx.toFloat()) {
            val a = Animatable(from)
            a.animateTo(curIdx.toFloat(), spring()) { pos = value }
        }
    }
    LaunchedEffect(Unit) {
        var last = pos.roundToInt()
        snapshotFlow { pos.roundToInt() }.collect {
            if (it != last) { last = it; haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove) }
        }
    }

    val library by AppServices.library.state.collectAsStateWithLifecycle()
    val accent = AruColors.Accent
    val accent2 = AruColors.Accent2

    val textPaint = remember { Paint().apply { isAntiAlias = true; textAlign = Paint.Align.CENTER; typeface = Typeface.DEFAULT_BOLD } }

    BoxWithConstraints(Modifier.fillMaxSize().clipToBounds()) {
        val density = LocalDensity.current
        val wPx = with(density) { maxWidth.toPx() }
        val hPx = with(density) { maxHeight.toPx() }
        val dp1 = with(density) { 1.dp.toPx() }
        val r = min(172 * dp1, hPx / 2f - 2 * dp1)
        val r2 = r * 0.80f
        val r3 = r * 0.60f
        val pad = 14 * dp1
        val cy = hPx / 2f
        val pitch = PITCH * dp1
        val cardH = (CARD_H * dp1).roundToInt()
        val margin = 12 * dp1
        val gap = 10 * dp1
        val minW = 140 * dp1
        val maxIdx = (n - 1).toFloat()

        // ── Kartalar (aylana yoyiga moslashadi) ──
        for (i in (base - 4)..(base + 4)) {
            if (i < 0 || i >= n) continue
            val e = asc[i]
            key(e.id) {
                val isCur = e.id == currentId
                val selected = i == base
                val hist = remember(library.history, e.id) { library.history.firstOrNull { it.epizodId == e.id } }
                val hp = hist?.positionMs ?: 0L
                val hd = hist?.durationMs ?: 0L
                val finished = hd > 0 && hp > hd - 5000
                val status = when {
                    hd > 0 && hp >= 3000 && !finished -> "${dialClock(hp)} dan davom etasiz"
                    finished -> "Ko'rildi ✓"
                    isCur -> "Hozir ijro etilmoqda"
                    else -> "Ko'rilmagan"
                }
                Box(
                    Modifier
                        .offset {
                            val dy = (i - pos) * pitch
                            val bound = if (abs(dy) < r) sqrt(r * r - dy * dy) else 0f
                            val x = if (left) (if (abs(dy) < r) bound + gap else margin) else margin
                            IntOffset(x.roundToInt(), (cy + dy - cardH / 2f).roundToInt())
                        }
                        .layout { m, _ ->
                            val dy = (i - pos) * pitch
                            val bound = if (abs(dy) < r) sqrt(r * r - dy * dy) else 0f
                            val w = if (abs(dy) < r) (wPx - margin - bound - gap - margin).coerceAtLeast(minW) else wPx - 2 * margin
                            val p = m.measure(Constraints.fixed(w.roundToInt(), cardH))
                            layout(p.width, p.height) { p.place(0, 0) }
                        }
                        .graphicsLayer { alpha = (1f - ((abs(i - pos) - 3f).coerceIn(0f, 1f)) * 0.7f) }
                        .clip(RoundedCornerShape(16.dp))
                        .background(if (selected) AruColors.AccentTint else AruColors.Card)
                        .border(1.dp, if (selected) accent.copy(alpha = 0.7f) else AruColors.Border, RoundedCornerShape(16.dp))
                        .clickable { onPlay(e) },
                ) {
                    Row(Modifier.fillMaxSize().offset(x = 9.dp), verticalAlignment = Alignment.CenterVertically) {
                        Box(
                            Modifier.size(38.dp).clip(RoundedCornerShape(11.dp)).background(if (isCur) accent else AruColors.CardAlt),
                            contentAlignment = Alignment.Center,
                        ) {
                            if (isCur) Icon(Icons.Filled.PlayArrow, null, tint = Color.White, modifier = Modifier.size(22.dp))
                            else Text("${e.number}", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 14.sp)
                        }
                        Column(Modifier.offset(x = 10.dp)) {
                            Text("${e.number}-qism", color = if (isCur || selected) accent2 else Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1)
                            Text(status, color = AruColors.TextDim, fontSize = 10.5.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                    }
                }
            }
        }

        // ── Yarim aylana ──
        val boxW = r + pad
        val cxL = if (left) 0f else boxW          // markaz (lokal)
        val cyL = r
        val sgn = if (left) 1f else -1f           // ichkariga yo'nalish (x bo'yicha)
        Box(
            Modifier
                .align(if (left) Alignment.CenterStart else Alignment.CenterEnd)
                .size(with(density) { boxW.toDp() }, with(density) { (2 * r).toDp() })
                .pointerInput(left, n, r) {
                    var mode = 0
                    var lastAngle = 0f
                    var lastTick = 0L
                    val tracker = VelocityTracker()
                    fun angleOf(o: Offset): Float =
                        (atan2((o.y - cyL).toDouble(), ((o.x - cxL) * sgn).toDouble()) * 180.0 / PI).toFloat()
                    detectDragGestures(
                        onDragStart = { off ->
                            val d = hypot(off.x - cxL, off.y - cyL)
                            mode = if (d >= r2) 1 else if (d >= r3) 2 else 0
                            job?.cancel()
                            tracker.resetTracking()
                            if (mode == 2) {
                                scrub = player.positionMs
                                lastAngle = angleOf(off)
                                lastTick = (scrub ?: 0L) / 5000
                            }
                        },
                        onDrag = { change, drag ->
                            change.consume()
                            if (mode == 1) {
                                tracker.addPosition(change.uptimeMillis, change.position)
                                pos = (pos - drag.y / pitch).coerceIn(0f, maxIdx)
                            } else if (mode == 2) {
                                val a = angleOf(change.position)
                                var d = a - lastAngle
                                if (d > 180f) d -= 360f
                                if (d < -180f) d += 360f
                                lastAngle = a
                                val dur = player.durationMs.coerceAtLeast(0L)
                                val next = ((scrub ?: 0L) - (d / 6f * 5000f).toLong()).coerceIn(0L, if (dur > 0) dur else Long.MAX_VALUE)
                                scrub = next
                                val tick = next / 5000
                                if (tick != lastTick) { lastTick = tick; haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove) }
                            }
                        },
                        onDragEnd = {
                            if (mode == 1) {
                                val v = tracker.calculateVelocity().y
                                job = scope.launch {
                                    val a = Animatable(pos)
                                    a.updateBounds(0f, maxIdx)
                                    a.animateDecay(-v / pitch, exponentialDecay<Float>(frictionMultiplier = 1.8f)) { pos = value }
                                    a.animateTo(a.value.roundToInt().toFloat(), spring()) { pos = value }
                                }
                            } else if (mode == 2) {
                                scrub?.let { player.seekTo(it) }
                                scrub = null
                            }
                            mode = 0
                        },
                        onDragCancel = {
                            if (mode == 2) scrub = null
                            mode = 0
                        },
                    )
                },
        ) {
            Canvas(Modifier.fillMaxSize()) {
                val c = Offset(cxL, cyL)
                val sw = 1.2f * dp1
                drawCircle(Color(0xFF0F0F10), r, c)
                drawCircle(Color(0xFF3A3A3F), r, c, style = Stroke(sw))
                drawCircle(Color(0xFF151517), r2, c)
                drawCircle(Color(0xFF2F2F34), r2, c, style = Stroke(sw))
                drawCircle(Color(0xFF1C1C1F), r3, c)
                drawCircle(Color(0xFF3F3F44), r3, c, style = Stroke(1.5f * dp1))

                val nc = drawContext.canvas.nativeCanvas
                val rr = (r + r2) / 2f

                // Qism raqamlari va kichik chiziqlar (tashqi halqa).
                for (k in (base - 4)..(base + 4)) {
                    // raqam
                    if (k in 0 until n) {
                        val dy = (k - pos) * pitch
                        if (abs(dy) < rr - 14 * dp1) {
                            val sx = sqrt(rr * rr - dy * dy)
                            val x = cxL - sgn * sx
                            val y = cyL + dy
                            val alpha = Math.toDegrees(asin((dy / rr).toDouble())).toFloat()
                            val isSel = k == base
                            textPaint.color = if (isSel) accent2.toArgb() else 0xFFBBBBBB.toInt()
                            textPaint.textSize = (if (isSel) 20 else 15) * dp1
                            nc.save()
                            nc.rotate(if (left) -alpha else alpha, x, y)
                            nc.drawText("${asc[k].number}", x, y + textPaint.textSize * 0.35f, textPaint)
                            nc.restore()
                        }
                    }
                    // kichik chiziq (raqamlar orasida)
                    val dyh = (k + 0.5f - pos) * pitch
                    if (abs(dyh) < rr - 6 * dp1) {
                        val sa = (dyh / rr)
                        val ca = sqrt(1f - sa * sa)
                        val p1 = Offset(cxL - sgn * ca * (r - 3 * dp1), cyL + sa * (r - 3 * dp1))
                        val p2 = Offset(cxL - sgn * ca * (r - 10 * dp1), cyL + sa * (r - 10 * dp1))
                        drawLine(Color(0xFF55555A), p1, p2, 1.2f * dp1)
                    }
                }

                // Vaqt halqasi: har chiziq 5 soniya.
                val tNow = scrub ?: player.positionMs
                val dur = player.durationMs
                val baseT = tNow / 5000f
                val mid = floor(baseT).toInt()
                for (m in (mid - 14)..(mid + 14)) {
                    val tm = m * 5000L
                    if (tm < 0 || (dur > 0 && tm > dur)) continue
                    val ang = (m - baseT) * 6f
                    if (abs(ang) > 84f) continue
                    val rad = ang * PI.toFloat() / 180f
                    val long = m % 6 == 0
                    val ra = r2 - 3 * dp1
                    val rb = r2 - (if (long) 17 else 10) * dp1
                    val sa = sin(rad); val ca = cos(rad)
                    drawLine(
                        if (long) Color(0xFFAAAAAF) else Color(0xFF55555A),
                        Offset(cxL - sgn * ca * ra, cyL + sa * ra), Offset(cxL - sgn * ca * rb, cyL + sa * rb),
                        (if (long) 2f else 1.2f) * dp1,
                    )
                    if (long && abs(ang) > 9f) {
                        val rl = r2 - 28 * dp1
                        textPaint.color = 0xFF9A9AA0.toInt()
                        textPaint.textSize = 9.5f * dp1
                        nc.drawText(dialClock(tm), cxL - sgn * ca * rl, cyL + sa * rl + textPaint.textSize * 0.35f, textPaint)
                    }
                }
                // Hozirgi vaqt belgisi va yozuvi.
                drawLine(accent2, Offset(cxL - sgn * (r2 - 3 * dp1), cyL), Offset(cxL - sgn * (r2 - 20 * dp1), cyL), 3f * dp1)
                textPaint.color = accent2.toArgb()
                textPaint.textSize = 12f * dp1
                nc.drawText(dialClock(tNow), cxL - sgn * (r2 - 34 * dp1), cyL + textPaint.textSize * 0.35f, textPaint)

                // Tanlangan qism ko'rsatkichi (aylana chetida).
                val tipX = cxL - sgn * (r - 2 * dp1)
                val baseX = cxL - sgn * (r + 11 * dp1)
                val tri = androidx.compose.ui.graphics.Path().apply {
                    moveTo(tipX, cyL)
                    lineTo(baseX, cyL - 8 * dp1)
                    lineTo(baseX, cyL + 8 * dp1)
                    close()
                }
                drawPath(tri, accent2)
            }

            // Ichki tugmalar.
            val playing = player.playing && asc[base].id == currentId
            fun btn(dx: Float, dy: Float, sizeDp: Int, content: @Composable () -> Unit, enabled: Boolean, bg: Color, onClick: () -> Unit): @Composable () -> Unit = {
                val s = sizeDp * dp1
                Box(
                    Modifier
                        .offset { IntOffset((cxL + sgn * dx * dp1 - s / 2).roundToInt(), (cyL + dy * dp1 - s / 2).roundToInt()) }
                        .size(sizeDp.dp).clip(CircleShape).background(bg)
                        .clickable(enabled = enabled, onClick = onClick),
                    contentAlignment = Alignment.Center,
                ) { content() }
            }
            val white = Color.White
            btn(50f, 0f, 56, {
                Icon(if (playing) AruIcons.Pause else Icons.Filled.PlayArrow, null, tint = white, modifier = Modifier.size(30.dp))
            }, true, accent) {
                val sel = asc[base]
                if (sel.id != currentId) onPlay(sel) else player.toggle()
            }()
            btn(34f, -66f, 46, {
                Icon(AruIcons.SkipPrevious, null, tint = white.copy(alpha = if (hasPrev) 0.95f else 0.25f), modifier = Modifier.size(26.dp))
            }, hasPrev, Color.White.copy(alpha = 0.08f), onPrev)()
            btn(34f, 66f, 46, {
                Icon(AruIcons.SkipNext, null, tint = white.copy(alpha = if (hasNext) 0.95f else 0.25f), modifier = Modifier.size(26.dp))
            }, hasNext, Color.White.copy(alpha = 0.08f), onNext)()
        }
    }
}

private fun Color.toArgb(): Int = android.graphics.Color.argb((alpha * 255).roundToInt(), (red * 255).roundToInt(), (green * 255).roundToInt(), (blue * 255).roundToInt())

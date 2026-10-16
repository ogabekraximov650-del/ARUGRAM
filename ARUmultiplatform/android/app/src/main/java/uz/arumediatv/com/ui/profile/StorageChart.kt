package uz.arumediatv.com.ui.profile

// HALQA DIAGRAMMA (`CacheChart`) — Flutter `storage_screen.dart` dagi `_CacheChart` / `_ChartPainter` ning ko'chirmasi:
// bo'laklar bir-biriga ulangan holda siljiydi (3200 ms), bosilgan bo'lak 9 dp ga kattalashadi, ichida zarrachalar oqadi,
// foizlar halqa TASHQARISIDA (ustma-ust tushmaydi), o'rtada tanlangan hajm, hisoblanayotganda aylanuvchi yoy,
// tozalangach yashil halqa va belgi, tozalash paytida halqa `drain` bo'yicha bo'shaydi.

import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RadialGradient
import android.graphics.RectF
import android.graphics.Shader
import android.graphics.Typeface
import android.os.SystemClock
import androidx.compose.foundation.Canvas as ComposeCanvas
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import androidx.core.graphics.ColorUtils
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.ceil
import kotlin.math.cos
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin
import kotlin.math.sqrt

internal class ChartSlice(val label: String, val bytes: Long)

private class Share(val frac: Double, val pct: Double, val textScale: Double)

private class Sector(val center: Double, val half: Double, val pct: Double, val textAlpha: Double, val textScale: Double)

private class LabelItem(val home: Double, val text: String, val color: Int, val size: Float, val w: Float, val h: Float) {
    var angle = home
}

private const val RADIUS = 86.0
private const val THICK = 38.0
private const val BOX = 270.0
private const val MOVE_MS = 3200.0
private const val PRESS_MS = 200.0

internal val kChartColors = mapOf(
    "Videolar" to 0xFF3E8CF0.toInt(),
    "Posterlar" to 0xFF56B6F5.toInt(),
    "Vaqtinchalik fayllar" to 0xFF9A6FE2.toInt(),
    "Emoji, GIF va stikerlar" to 0xFFE08A3C.toInt(),
    "Tarix kadrlari" to 0xFFC95A8A.toInt(),
)

internal fun chartColorOf(label: String): Int = kChartColors[label] ?: 0xFFE8B730.toInt()

private const val GRAY_TEXT = 0xFF8A8A8E.toInt()
private const val SELECTOR = 0x14FFFFFF
private const val GREEN1 = 0xFF6ED556.toInt()
private const val GREEN2 = 0xFF41BA71.toInt()

private fun easeIO(x: Double) = if (x < .5) 4 * x * x * x else 1 - Math.pow(-2 * x + 2, 3.0) / 2

/** Foizlarni (yuzdan bir aniqlikda) jami 100 bo'ladigan qilib yaxlitlaydi (eng katta qoldiq usuli). */
internal fun roundPercents(shares: List<Double>): List<Double> {
    val raw = shares.map { it * 10000 }
    val out = raw.map { floor(it).toInt() }.toMutableList()
    val sum = raw.sum()
    var left = Math.round(sum).toInt() - out.sum()
    val order = raw.indices.sortedWith { a, b -> (raw[b] - out[b]).compareTo(raw[a] - out[a]) }
    for (i in order) {
        if (left <= 0) break
        out[i]++
        left--
    }
    return out.map { it / 100.0 }
}

/** `000.00` ko'rinishidagi foiz matni. */
internal fun pctText(p: Double): String = if (p > 0 && p < .005) "<0.01%" else String.format(java.util.Locale.US, "%.2f%%", p)

/** Flutter `formatBytes` bilan bir xil (vergul, 2/1/0 kasr). */
internal fun fmtBytes(bytes: Long): String {
    val b = bytes.toDouble()
    if (b < 1024) return "${Math.round(b)} B"
    val units = arrayOf("KB", "MB", "GB", "TB", "PB")
    var v = b / 1024
    var i = 0
    while (v >= 1024 && i < units.size - 1) {
        v /= 1024
        i++
    }
    val text = if (v >= 100) String.format(java.util.Locale.US, "%.0f", v) else if (v >= 10) String.format(java.util.Locale.US, "%.1f", v) else String.format(java.util.Locale.US, "%.2f", v)
    return "${text.replace('.', ',')} ${units[i]}"
}

internal class ChartModel {
    private var order = listOf<String>()
    private var from = mapOf<String, Share>()
    private var to = mapOf<String, Share>()
    private var fromBytes = 0.0
    private var toBytes = 0.0
    private var moveStart = -1e12
    private var slices = listOf<ChartSlice>()
    private var hadDrain = false
    private var started = false

    var pressed: String? = null
    var target: String? = null
    var pressV = 0.0
    var pressDir = 0
    var onPress: (String?) -> Unit = {}
    var drain: () -> Float? = { null }

    private fun now() = SystemClock.uptimeMillis().toDouble()
    private fun moveValue() = ((now() - moveStart) / MOVE_MS).coerceIn(0.0, 1.0)
    private fun t() = easeIO(moveValue())
    private fun bytesNow() = fromBytes + (toBytes - fromBytes) * t()
    private fun left() = 1 - (drain() ?: 0f).toDouble().coerceIn(0.0, 1.0)

    fun update(newSlices: List<ChartSlice>, selectedBytes: Long, drainActive: Boolean) {
        slices = newSlices
        if (!started) {
            started = true
            to = shares(newSlices)
            from = to
            mergeOrder()
            fromBytes = selectedBytes.toDouble()
            toBytes = fromBytes
            hadDrain = drainActive
            return
        }
        if (drainActive) {
            if (!hadDrain) {
                val cur = currentShares()
                val b = bytesNow()
                moveStart = -1e12
                from = cur
                to = cur
                fromBytes = b
                toBytes = b
            }
            hadDrain = true
            return
        }
        val next = shares(newSlices)
        val bytes = selectedBytes.toDouble()
        if (hadDrain) {
            hadDrain = false
            from = emptyMap()
            fromBytes = 0.0
            to = next
            toBytes = bytes
            mergeOrder()
            moveStart = now()
            return
        }
        if (!sameShares(next, to) || bytes != toBytes) {
            fromBytes = bytesNow()
            toBytes = bytes
            from = currentShares()
            to = next
            mergeOrder()
            moveStart = now()
        }
    }

    private fun mergeOrder() {
        val keep = from.keys + to.keys
        val ord = order.filter { it in keep }.toMutableList()
        val fresh = slices.filter { it.label in keep && it.label !in ord }.sortedBy { it.bytes }
        ord.addAll(fresh.map { it.label })
        for (k in keep) if (k !in ord) ord.add(k)
        order = ord
    }

    private fun currentShares(): Map<String, Share> {
        val t = t()
        val out = LinkedHashMap<String, Share>()
        for (k in order) {
            val a = from[k]
            val b = to[k]
            if (a == null && b == null) continue
            val s = Share(
                (a?.frac ?: 0.0) + ((b?.frac ?: 0.0) - (a?.frac ?: 0.0)) * t,
                (a?.pct ?: 0.0) + ((b?.pct ?: 0.0) - (a?.pct ?: 0.0)) * t,
                (a?.textScale ?: b!!.textScale) + ((b?.textScale ?: a!!.textScale) - (a?.textScale ?: b!!.textScale)) * t,
            )
            if (s.frac > 0.00001) out[k] = s
        }
        return out
    }

    private fun sameShares(a: Map<String, Share>, b: Map<String, Share>): Boolean {
        if (a.size != b.size) return false
        for ((k, v) in a) {
            val o = b[k] ?: return false
            if (abs(o.frac - v.frac) > 0.00001 || abs(o.pct - v.pct) > 0.001) return false
        }
        return true
    }

    private fun shares(list0: List<ChartSlice>): Map<String, Share> {
        val list = list0.filter { it.bytes > 0 }
        val sum = list.sumOf { it.bytes }
        if (sum <= 0) return emptyMap()
        var under = 0
        var minus = 0.0
        for (s in list) {
            val p = s.bytes.toDouble() / sum
            if (p > 0 && p < .02) {
                under++
                minus += p
            }
        }
        val percents = roundPercents(list.map { it.bytes.toDouble() / sum })
        val out = LinkedHashMap<String, Share>()
        for (i in list.indices) {
            var p = list[i].bytes.toDouble() / sum
            val textScale = if (p < .08 || percents[i] >= 100) .85 else 1.0
            p = if (p < .02) .02 else p * (1 - (.02 * under - minus))
            out[list[i].label] = Share(p, percents[i], textScale)
        }
        return out
    }

    fun visible(): Map<String, Sector> {
        val f = if (hadDrain) left() else 1.0
        val shares = currentShares()
        val out = LinkedHashMap<String, Sector>()
        var at = 0.0
        for (k in order) {
            val s = shares[k] ?: continue
            val half = s.frac * f * 180
            val alpha = (s.frac / .02).coerceIn(0.0, 1.0)
            out[k] = Sector(at + half, half, s.pct * f, alpha, s.textScale)
            at += half * 2
        }
        return out
    }

    fun visibleBytes(): Double = if (hadDrain) bytesNow() * left() else bytesNow()

    fun hit(x: Double, y: Double, w: Double): String? {
        val dx = x - w / 2
        val dy = y - BOX / 2
        val r = sqrt(dx * dx + dy * dy)
        if (r <= RADIUS - THICK || r >= RADIUS + 14) return null
        var a = Math.toDegrees(atan2(dy, dx))
        if (a < 0) a += 360.0
        for ((k, v) in visible()) if (a >= v.center - v.half && a <= v.center + v.half) return k
        return null
    }

    fun setPressed(label: String?) {
        if (label == target) return
        target = label
        onPress(label)
        if (label == null) {
            pressDir = -1
            return
        }
        pressed = label
        pressV = 0.0
        pressDir = 1
    }

    fun stepPress(dtMs: Double) {
        if (pressDir == 0) return
        pressV = (pressV + pressDir * dtMs / PRESS_MS).coerceIn(0.0, 1.0)
        if (pressV <= 0.0 || pressV >= 1.0) pressDir = 0
    }

    // ── CHIZISH ───────────────────────────────────────────────────
    private val textPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply { typeface = Typeface.DEFAULT_BOLD }

    fun draw(canvas: Canvas, width: Float, loading: Boolean, complete: Boolean) {
        val cx = width / 2.0
        val cy = BOX / 2
        val time = ((SystemClock.uptimeMillis() % 10_000L) / 10_000.0)
        if (loading) {
            paintLoading(canvas, cx, cy, SystemClock.uptimeMillis() / 1000.0)
            return
        }
        if (complete) {
            paintComplete(canvas, cx, cy)
            return
        }
        val all = visible()
        val keys = all.keys.sortedBy { if (it == pressed) 1 else 0 }
        for (label in keys) {
            val s = all.getValue(label)
            if (s.half <= 0) continue
            val grow = if (label == pressed) 9 * pressV else 0.0
            paintSector(canvas, cx, cy, s, chartColorOf(label), RADIUS + grow, time)
        }
        paintLabels(canvas, cx, cy, all)
        paintCenter(canvas, cx, cy)
    }

    private fun labelPos(cx: Double, cy: Double, angle: Double, w: Float, h: Float): DoubleArray {
        val dx = cos(Math.toRadians(angle))
        val dy = sin(Math.toRadians(angle))
        val gap = 10 + abs(dx) * w / 2 + abs(dy) * h / 2
        return doubleArrayOf(cx + dx * (RADIUS + 9 + gap), cy + dy * (RADIUS + 9 + gap))
    }

    private fun paintLabels(canvas: Canvas, cx: Double, cy: Double, all: Map<String, Sector>) {
        val items = ArrayList<LabelItem>()
        for ((k, s) in all) {
            if (s.half <= 0 || s.textAlpha <= 0.01 || s.pct <= 0) continue
            val base = ColorUtils.compositeColors(0x33FFFFFF, chartColorOf(k))
            val color = (base and 0xFFFFFF) or ((s.textAlpha.coerceIn(0.0, 1.0) * 255).toInt() shl 24)
            val size = (14.5 * s.textScale).toFloat()
            textPaint.textSize = size
            val txt = pctText(s.pct)
            val fm = textPaint.fontMetrics
            items.add(LabelItem(((s.center % 360) + 360) % 360, txt, color, size, textPaint.measureText(txt), fm.descent - fm.ascent))
        }
        if (items.isEmpty()) return
        items.sortBy { it.home }
        fun rectAt(l: LabelItem, angle: Double): RectF {
            val p = labelPos(cx, cy, angle, l.w, l.h)
            return RectF((p[0] - l.w / 2 - 3).toFloat(), (p[1] - l.h / 2 - 3).toFloat(), (p[0] + l.w / 2 + 3).toFloat(), (p[1] + l.h / 2 + 3).toFloat())
        }
        val step = 1.5
        val maxShift = 70.0
        val n = items.size
        var iter = 0
        while (iter < 120 && n > 1) {
            var moved = false
            for (i in 0 until n) {
                val j = (i + 1) % n
                if (n == 2 && i == 1) break
                val a = items[i]
                val b = items[j]
                if (!RectF.intersects(rectAt(a, a.angle), rectAt(b, b.angle))) continue
                if (abs(a.angle - step - a.home) <= maxShift) a.angle -= step
                if (abs(b.angle + step - b.home) <= maxShift) b.angle += step
                moved = true
            }
            if (!moved) break
            iter++
        }
        for (l in items) {
            val pos = labelPos(cx, cy, l.angle, l.w, l.h)
            if (abs(l.angle - l.home) > 4) {
                val home = Math.toRadians(l.home)
                val fx = cx + cos(home) * (RADIUS + 3)
                val fy = cy + sin(home) * (RADIUS + 3)
                val tx = pos[0] - cx
                val ty = pos[1] - cy
                val dist = sqrt(tx * tx + ty * ty)
                val len = dist - (l.w / 2 * abs(tx / dist) + l.h / 2 * abs(ty / dist)) - 3
                val toX = cx + tx / dist * len
                val toY = cy + ty / dist * len
                val a = ((l.color ushr 24) * .55).toInt()
                val p = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                    style = Paint.Style.STROKE; strokeWidth = 1.2f; strokeCap = Paint.Cap.ROUND; color = (l.color and 0xFFFFFF) or (a shl 24)
                }
                canvas.drawLine(fx.toFloat(), fy.toFloat(), toX.toFloat(), toY.toFloat(), p)
            }
            textPaint.textSize = l.size
            textPaint.color = l.color
            val fm = textPaint.fontMetrics
            canvas.drawText(l.text, (pos[0] - l.w / 2).toFloat(), (pos[1] - l.h / 2 - fm.ascent).toFloat(), textPaint)
        }
    }

    private fun paintSector(canvas: Canvas, cx: Double, cy: Double, s: Sector, color: Int, outer: Double, time: Double) {
        val inner = RADIUS - THICK
        val full = s.half * 2 >= 359
        val path = Path()
        val ro = RectF((cx - outer).toFloat(), (cy - outer).toFloat(), (cx + outer).toFloat(), (cy + outer).toFloat())
        val ri = RectF((cx - inner).toFloat(), (cy - inner).toFloat(), (cx + inner).toFloat(), (cy + inner).toFloat())
        if (full) {
            path.fillType = Path.FillType.EVEN_ODD
            path.addOval(ro, Path.Direction.CW)
            path.addOval(ri, Path.Direction.CW)
        } else {
            val from = (s.center - s.half).toFloat()
            val sweep = (s.half * 2 + .5).toFloat()
            path.arcTo(ro, from, sweep, true)
            path.arcTo(ri, from + sweep, -sweep, false)
            path.close()
        }
        val light = ColorUtils.compositeColors(0x30FFFFFF, color)
        val dark = ColorUtils.compositeColors(0x03000000, color)
        val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            shader = RadialGradient(cx.toFloat(), cy.toFloat(), RADIUS.toFloat(), intArrayOf(light, dark), floatArrayOf(.3f, 1f), Shader.TileMode.CLAMP)
        }
        canvas.drawPath(path, paint)
        canvas.save()
        canvas.clipPath(path)
        paintParticles(canvas, cx, cy, s, inner, outer, time)
        canvas.restore()
    }

    private fun paintParticles(canvas: Canvas, cx: Double, cy: Double, s: Sector, inner: Double, outer: Double, time: Double) {
        val step = 7.0
        val sz = 5.0
        val sqrt2 = sqrt(2.0)
        val start = (((s.center - s.half) % 360) + 360) % 360
        val end = start + s.half * 2
        val paint = Paint(Paint.ANTI_ALIAS_FLAG)
        var i = floor(start / step).toInt()
        val last = ceil(end / step).toInt()
        while (i <= last) {
            val angle = i * step
            val t = ((time + 100) * (1 + (sin(angle * 2000) + 1) * .25)) % 1
            val r = (inner - sz * sqrt2) + ((outer + sz * sqrt2) - (inner - sz * sqrt2)) * t
            val rad = Math.toRadians(angle)
            val px = cx + cos(rad) * r
            val py = cy + sin(rad) * r
            val wave = .25 * (sin(t * PI) - 1) + 1
            val alpha = (.65 * (-1.75 * abs(t - .5) + 1) * wave).coerceIn(0.0, 1.0)
            if (alpha > 0) {
                val scale = .75 * wave * (.8 + (sin(angle) + 1) * .25)
                paint.color = ((alpha * 255).toInt() shl 24) or 0xFFFFFF
                canvas.drawCircle(px.toFloat(), py.toFloat(), (2.4 * scale).toFloat(), paint)
            }
            i++
        }
    }

    private fun paintCenter(canvas: Canvas, cx: Double, cy: Double) {
        val text = fmtBytes(Math.round(visibleBytes())).split(' ')
        val top = text.first()
        val bottom = if (text.size < 2) "" else text.drop(1).joinToString(" ")
        val p = Paint(Paint.ANTI_ALIAS_FLAG).apply { typeface = Typeface.DEFAULT_BOLD; textSize = 32f; color = 0xFFFFFFFF.toInt() }
        val w = p.measureText(top)
        canvas.drawText(top, (cx - w / 2).toFloat(), (cy - 16 - 5 + 32 * .82).toFloat(), p)
        val b = Paint(Paint.ANTI_ALIAS_FLAG).apply { textSize = 12f; color = GRAY_TEXT }
        val bw = b.measureText(bottom)
        canvas.drawText(bottom, (cx - bw / 2).toFloat(), (cy + 22 - 6 + 12 * .82).toFloat(), b)
    }

    private fun paintLoading(canvas: Canvas, cx: Double, cy: Double, sec: Double) {
        val r = RADIUS - THICK / 2
        val stroke = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.STROKE; strokeWidth = THICK.toFloat(); color = SELECTOR }
        canvas.drawCircle(cx.toFloat(), cy.toFloat(), r.toFloat(), stroke)
        val start = Math.toDegrees(((sec * 2 * PI) % (2 * PI))).toFloat()
        val sweep = Math.toDegrees(PI * (.35 + .25 * sin(sec * PI))).toFloat()
        val arc = Paint(stroke).apply { color = 0x1FFFFFFF; strokeCap = Paint.Cap.ROUND }
        canvas.drawArc(RectF((cx - r).toFloat(), (cy - r).toFloat(), (cx + r).toFloat(), (cy + r).toFloat()), start, sweep, false, arc)
    }

    private fun paintComplete(canvas: Canvas, cx: Double, cy: Double) {
        val w = 10.0
        val d = RADIUS * 2
        val left = cx - RADIUS
        val top = cy - RADIUS
        val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            shader = android.graphics.LinearGradient(
                0f, (cy - 100).toFloat(), 0f, (cy + 100).toFloat(),
                intArrayOf(0x006ED556, GREEN1, GREEN2, 0x0041BA71), floatArrayOf(0f, .07f, .93f, 1f), Shader.TileMode.CLAMP,
            )
            style = Paint.Style.STROKE
            strokeWidth = w.toFloat()
            strokeCap = Paint.Cap.ROUND
            strokeJoin = Paint.Join.ROUND
        }
        canvas.drawCircle(cx.toFloat(), cy.toFloat(), (RADIUS - w / 2).toFloat(), paint)
        val check = Path().apply {
            moveTo((left + d * .348).toFloat(), (top + d * .538).toFloat())
            lineTo((left + d * .447).toFloat(), (top + d * .636).toFloat())
            lineTo((left + d * .678).toFloat(), (top + d * .402).toFloat())
        }
        canvas.drawPath(check, paint)
    }
}

@Composable
internal fun StorageChart(
    slices: List<ChartSlice>,
    selectedBytes: Long,
    loading: Boolean,
    complete: Boolean,
    drainActive: Boolean,
    drain: () -> Float?,
    onPress: (String?) -> Unit,
) {
    val model = remember { ChartModel() }
    model.onPress = onPress
    model.drain = drain
    model.update(slices, selectedBytes, drainActive)
    var tick by remember { mutableLongStateOf(0L) }
    LaunchedEffect(Unit) {
        var last = 0L
        while (true) {
            androidx.compose.runtime.withFrameNanos { n ->
                if (last != 0L) model.stepPress((n - last) / 1_000_000.0)
                last = n
                tick = n
            }
        }
    }
    val density = LocalDensity.current.density
    val interactive = !loading && !complete && !drainActive
    ComposeCanvas(
        Modifier.fillMaxWidth().height(BOX.dp).pointerInput(interactive) {
            if (!interactive) return@pointerInput
            awaitEachGesture {
                val down = awaitFirstDown(requireUnconsumed = false)
                model.setPressed(model.hit(down.position.x / density.toDouble(), down.position.y / density.toDouble(), size.width / density.toDouble()))
                do {
                    val e = awaitPointerEvent()
                    val p = e.changes.firstOrNull() ?: break
                    model.setPressed(model.hit(p.position.x / density.toDouble(), p.position.y / density.toDouble(), size.width / density.toDouble()))
                } while (e.changes.any { it.pressed })
                model.setPressed(null)
            }
        },
    ) {
        tick.hashCode()
        drawIntoCanvas { c ->
            val n = c.nativeCanvas
            n.save()
            n.scale(density, density)
            model.draw(n, size.width / density, loading, complete)
            n.restore()
        }
    }
}

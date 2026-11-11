package uz.arumediatv.com.ui.profile

// "KESH TOZALANMOQDA" ANIMATSIYASI — Flutter `lib/widgets/trash_chick.dart` ning to'liq ko'chirmasi.
//
// Telegram jo'jasi (`utyan`) uyining eshigidan chiqib chiqindini axlat qutisiga tashlaydi; sahna chiqindi HAJMIGA qarab:
//   1. 500 MB gacha — eshikdan chiqmaydi, mo'ralab qopni shu yerdan qutiga otadi;
//   2. 500 MB – 2 GB — qopni qo'lida qutigacha olib borib tashlaydi;
//   3. 2 – 5 GB — qopni sudrab olib boradi, terni artadi;
//   4. 5 GB+ — qop eshikka tiqiladi, otilib chiqadi, jo'ja ag'dariladi va yig'laydi.
// Sahna 1360 x 680 birlikda yozilgan va berilgan o'lchamga sig'diriladi. Animatsiya BIR marta o'ynaydi.

import android.graphics.Canvas
import android.graphics.LinearGradient
import android.graphics.Matrix
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RadialGradient
import android.graphics.RectF
import android.graphics.Shader
import androidx.compose.foundation.Canvas as ComposeCanvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.unit.Dp
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.round
import kotlin.math.sin
import kotlin.math.sqrt

/** Shu hajm uchun animatsiya davomiyligi (ms). */
fun trashChickDurationMs(bytes: Long): Long = (durationOf(tierOf(bytes)) * 1000).roundToLong()

private fun Double.roundToLong(): Long = round(this).toLong()

@Composable
fun TrashChickAnimation(bytes: Long, width: Dp, height: Dp) {
    val total = remember(bytes) { trashChickDurationMs(bytes) }
    var startNs by remember { mutableLongStateOf(0L) }
    var nowNs by remember { mutableLongStateOf(0L) }
    LaunchedEffect(bytes) {
        var first = true
        while (true) {
            androidx.compose.runtime.withFrameNanos { t ->
                if (first) {
                    startNs = t
                    first = false
                }
                nowNs = t
            }
        }
    }
    val scene = remember(bytes) { ChickScene(bytes) }
    Box(Modifier.size(width, height)) {
        ComposeCanvas(Modifier.size(width, height)) {
            val v = if (total > 0) (((nowNs - startNs) / 1_000_000.0) / total).coerceIn(0.0, 1.0) else 1.0
            drawIntoCanvas { c -> scene.draw(c.nativeCanvas, size.width, size.height, v) }
        }
    }
}

// ── SAHNA O'LCHAMLARI ─────────────────────────────────────────────
private const val W = 1360.0
private const val H = 680.0
private const val FLOOR = 600.0
private const val WALL_R = 262.0
private const val DL = 78.0
private const val DR = 222.0
private const val DT = 318.0
private const val DC = (DL + DR) / 2
private const val BL = 1186.0
private const val BR = 1300.0
private const val BT = 462.0
private const val BC = (BL + BR) / 2
private const val K = 0.40
private const val HW = 306 * K / 2
private const val PX = 294.0
private const val PY = 392.0
private const val CX = 300.0
private const val HEAD_C = 302.0
private const val STOP_X = BL - HW + 6
private const val DRAG_X = BL - 18 + HW
private const val MB = 1024L * 1024L

// ── YUMSHATISH ───────────────────────────────────────────────────
private fun clampD(v: Double, a: Double, b: Double) = max(a, min(b, v))
private fun seg(t: Double, a: Double, b: Double) = clampD((t - a) / (b - a), 0.0, 1.0)
private fun lerp(a: Double, b: Double, t: Double) = a + (b - a) * t
private fun eio(x: Double) = if (x < .5) 4 * x * x * x else 1 - (-2 * x + 2).pow(3) / 2
private fun eout(x: Double) = 1 - (1 - x).pow(3)
private fun sio(x: Double) = -(cos(PI * x) - 1) / 2
private fun backOut(x: Double, s: Double = 1.7) = 1 + (s + 1) * (x - 1).pow(3) + s * (x - 1).pow(2)

private fun tierOf(bytes: Long) = if (bytes < 500 * MB) 1 else if (bytes < 2048 * MB) 2 else if (bytes < 5120 * MB) 3 else 4

private fun durationOf(tier: Int) = doubleArrayOf(0.0, 7.0, 13.5, 16.5, 22.5)[tier]

/** Qop balandligi (sahna birligida), eni = 0.82 * balandlik. */
private fun bagSize(bytes: Long): Double {
    val g = bytes.toDouble() / MB
    if (g < 500) return lerp(34.0, 52.0, clampD(g / 500, 0.0, 1.0))
    if (g < 2048) return lerp(58.0, 92.0, clampD((g - 500) / 1548, 0.0, 1.0))
    if (g < 5120) return lerp(158.0, 190.0, clampD((g - 2048) / 3072, 0.0, 1.0))
    return 290.0
}

private data class V(val x: Double, val y: Double) {
    operator fun plus(o: V) = V(x + o.x, y + o.y)
    operator fun minus(o: V) = V(x - o.x, y - o.y)
    operator fun times(k: Double) = V(x * k, y * k)
    val dist: Double get() = sqrt(x * x + y * y)
}

private class Rc(val l: Double, val t: Double, val r: Double, val b: Double) {
    val cx get() = (l + r) / 2
    val cy get() = (t + b) / 2
    val w get() = r - l
    val c get() = V(cx, cy)
}

private class Walk(val x: Double, val ph: Double)

/** [t0, t1] da x0 -> x1, qadam uzunligi `stride` (ph < 0 — yurmayapti). */
private fun walk(t: Double, t0: Double, t1: Double, x0: Double, x1: Double, stride: Double): Walk {
    val u = seg(t, t0, t1)
    val steps = max(1, (abs(x1 - x0) / stride).let { round(it).toInt() })
    return Walk(lerp(x0, x1, u), if (u > 0 && u < 1) u * steps else -1.0)
}

private enum class Eyes { CALM, HAPPY, STRAIN, CRY }
private enum class BagMode { HAND, FLY, BIN, DRAG }

/** Old tomoni ko'rinadigan qop (eshik o'yig'ida; bo'g'zi jo'janing qo'lida). */
private class End(val cx: Double, val cy: Double, val d: Double, val sc: Double, val ry: Double, val tip: Double, val inside: Boolean)

// ── VAQT JADVALI ─────────────────────────────────────────────────
private class S(bytes: Long, val sec: Double) {
    val tier = tierOf(bytes)
    val bh = bagSize(bytes)
    val bw = bagSize(bytes) * .82
    var door = 0.0
    var x = DC
    var yaw = 0.0
    var ph = -1.0
    var stride = 1.0
    var lean = 0.0
    var lift = 0.0
    var sx = 1.0
    var sy = 1.0
    var rot = 0.0
    var sweat = 0.0
    var shake = 0.0
    var tears = 0.0
    var wind = 0.0
    var fling = 0.0
    var grip = 0.0
    var hop = 0.0
    var flail = 0.0
    var wipeBrow = 0.0
    var puff = -1.0
    var spark = -1.0
    var binWob = 0.0
    var flyT0 = 0.0
    var flyT1 = 0.0
    var flyPeak = 0.0
    var pivotR = false
    var happy = false
    var dragging = false
    var stuck = false
    var holding = false
    var sob = false
    var rubEye = false
    var rubAlt = false
    var flyBack = 0.0
    var jerk = 0.0
    var dragU = 0.0
    var dropAt = 99.0

    /** Chuqurlik: 0 — devor tekisligi va tashqari, < 0 — uy ichida (uzoqroq). */
    var depth = 0.0
    var throwArm: Char? = null
    var eyes = Eyes.CALM
    var bag = BagMode.HAND

    var hasEnd = false
    var endStuck = false
    var endFrom = 0.0
    var endAt = 0.0
    var endOut = 0.0
    var endPop = 0.0
    var endS: End? = null

    var bagPx = Double.NaN
    var bagAng = PI / 2
    var bagSq = 1.0
    var side = false
    var knot: V? = null

    init {
        when (tier) {
            1 -> tier1(sec)
            2 -> tier2(sec)
            3 -> tier3(sec)
            else -> tier4(sec)
        }
    }

    private fun tier1(t: Double) {
        door = eio(seg(t, 0.0, .7)) - eio(seg(t, 6.1, 6.9))
        x = DC + 18
        throwArm = 'a'
        depth = -.3 - .4 * eio(seg(t, 5.8, 6.2))
        var y = lerp(0.0, 1.0, eio(seg(t, .85, 1.15)))
        y = lerp(y, -1.0, eio(seg(t, 1.95, 2.35)))
        y = lerp(y, 1.0, eio(seg(t, 2.85, 3.15)))
        y = lerp(y, 0.0, eio(seg(t, 5.55, 5.85)))
        yaw = y
        lean = .13 * sin(PI * seg(t, 1.1, 1.95)) - .05 * sin(PI * seg(t, 2.3, 2.85))
        grip = eio(seg(t, .75, 1.05)) - eio(seg(t, 4.75, 5.0))
        wind = sio(seg(t, 3.2, 3.6)) * (1 - seg(t, 3.6, 3.68))
        fling = eout(seg(t, 3.6, 3.72)) * (1 - sio(seg(t, 4.0, 4.4)))
        flyT0 = 3.68
        flyT1 = 4.85
        flyPeak = 300.0
        if (t >= flyT0) bag = if (t < flyT1) BagMode.FLY else BagMode.BIN
        binWob = if (t > flyT1) exp(-(t - flyT1) * 5) * sin((t - flyT1) * 28) else 0.0
        puff = seg(t, 4.85, 5.5)
        spark = seg(t, 4.9, 5.8)
        hop = if (t > 4.95 && t < 5.5) sin(seg(t, 4.95, 5.5) * PI) else 0.0
        happy = t > 4.9 && t < 5.7
        eyes = if (happy) Eyes.HAPPY else Eyes.CALM
    }

    private fun tier2(t: Double) {
        door = eio(seg(t, 0.0, .7)) - eio(seg(t, 12.7, 13.4))
        var y = lerp(0.0, 1.0, eio(seg(t, .75, .95)))
        y = lerp(y, 0.0, eio(seg(t, 7.0, 7.25)))
        y = lerp(y, -1.0, eio(seg(t, 7.9, 8.2)))
        y = lerp(y, -2.0, eio(seg(t, 12.35, 12.6)))
        yaw = y
        depth = -.6 * (1 - eio(seg(t, .35, .85))) - .6 * eio(seg(t, 12.5, 12.95))
        if (t > .35 && t < .85) ph = seg(t, .35, .85) * 2
        var w = walk(t, .85, 5.6, DC, STOP_X - 60, 58.0)
        x = w.x
        if (t > .85) ph = w.ph
        wind = sio(seg(t, 5.75, 6.15)) * (1 - seg(t, 6.15, 6.22))
        fling = eout(seg(t, 6.15, 6.27)) * (1 - sio(seg(t, 6.6, 7.0)))
        flyT0 = 6.22
        flyT1 = 6.85
        flyPeak = 120.0
        if (t >= flyT0) bag = if (t < flyT1) BagMode.FLY else BagMode.BIN
        binWob = if (t > flyT1) exp(-(t - flyT1) * 5) * sin((t - flyT1) * 28) else 0.0
        puff = seg(t, 6.85, 7.5)
        spark = seg(t, 6.9, 7.8)
        hop = if (t > 7.1 && t < 7.75) sin(seg(t, 7.1, 7.75) * PI) else 0.0
        happy = t > 6.9 && t < 7.9
        eyes = if (happy) Eyes.HAPPY else Eyes.CALM
        if (t > 8.2) {
            w = walk(t, 8.2, 12.35, STOP_X - 60, DC, 58.0)
            x = w.x
            ph = w.ph
        }
    }

    private fun tier3(t: Double) {
        door = eio(seg(t, 0.0, .7)) - eio(seg(t, 15.8, 16.4))
        val xSide = DR + 80
        var y = -2.0
        y = lerp(y, -1.0, eio(seg(t, 1.3, 1.7)))
        y = lerp(y, 0.0, eio(seg(t, 9.0, 9.35)))
        y = lerp(y, -1.0, eio(seg(t, 10.6, 10.9)))
        y = lerp(y, -2.0, eio(seg(t, 15.2, 15.45)))
        yaw = y
        depth = -.7 * (1 - eio(seg(t, .5, 1.3))) - .7 * eio(seg(t, 15.35, 15.8))
        x = DC
        if (t > .5 && t < 1.3) ph = seg(t, .5, 1.3) * 2
        if (t >= 1.3) {
            val w = walk(t, 1.3, 1.9, DC, xSide, 40.0)
            x = w.x
            ph = w.ph
        }
        bag = BagMode.DRAG
        hasEnd = true
        endFrom = .5
        endAt = 1.9
        endOut = 2.3
        dragging = t >= 1.6 && t < 8.6
        holding = t >= .5 && t < 8.6
        if (t >= 2.3) {
            val w = walk(t, 2.3, 8.6, xSide, DRAG_X, 34.0)
            x = w.x
            ph = w.ph
            stride = .6
        }
        lean = if (dragging) .16 + .03 * sin(t * 9) else 0.0
        eyes = if (dragging) Eyes.STRAIN else Eyes.CALM
        sweat = if (dragging) 1.0 else max(0.0, 1 - (t - 8.7) * 1.5)
        dropAt = 8.6
        if (t > 8.7 && t < 10.4) {
            val u = seg(t, 8.7, 10.4)
            sy = 1 - .06 * sin(u * PI)
            sx = 1 + .05 * sin(u * PI)
            wipeBrow = sin(u * PI)
        }
        if (t > 10.9) {
            val w = walk(t, 10.9, 15.2, DRAG_X, DC, 58.0)
            x = w.x
            ph = w.ph
            stride = 1.0
        }
    }

    private fun tier4(t: Double) {
        door = eio(seg(t, 0.0, .8)) - eio(seg(t, 21.7, 22.4))
        bag = BagMode.DRAG
        pivotR = true
        val xSide = DR + 90
        var y = -2.0
        y = lerp(y, -1.0, eio(seg(t, 1.5, 1.9)))
        y = lerp(y, 0.0, eio(seg(t, 6.5, 6.9)))
        y = lerp(y, -1.0, eio(seg(t, 8.0, 8.3)))
        y = lerp(y, 0.0, eio(seg(t, 16.2, 16.5)))
        y = lerp(y, -1.0, eio(seg(t, 17.4, 17.7)))
        y = lerp(y, -2.0, eio(seg(t, 21.0, 21.3)))
        yaw = y
        depth = -.7 * (1 - eio(seg(t, .6, 1.5))) - .7 * eio(seg(t, 21.2, 21.7))
        x = DC
        if (t > .6 && t < 1.5) ph = seg(t, .6, 1.5) * 2
        hasEnd = true
        endStuck = true
        endFrom = .6
        endPop = 4.45
        if (t < 4.45) {
            if (t >= 1.5) {
                val w = walk(t, 1.5, 2.1, DC, xSide, 40.0)
                x = w.x
                ph = w.ph
            }
            jerk = max(0.0, sin(seg(t, 2.1, 4.45) * PI * 6)) * seg(t, 2.1, 4.45)
            if (t >= 2.1) x = xSide + jerk * 10
            lean = lerp(.08, .3, eio(seg(t, 2.1, 2.5))) + .08 * jerk
            shake = if (t > 2.1) 1.0 else 0.0
            eyes = if (t > 2.1) Eyes.STRAIN else Eyes.CALM
            sweat = if (t > 2.3) 1.0 else 0.0
            stuck = t >= 2.1
            holding = true
            dragging = t >= 1.5
            return
        }
        val tFall = 4.45
        val tHit = 4.8
        val lie = 1.32
        val r0: Double
        if (t < tHit) {
            val u = seg(t, tFall, tHit)
            r0 = .3 + (lie - .3) * u * u
            flyBack = u * u
        } else {
            val u = t - tHit
            r0 = lie - .3 * exp(-u * 5) * abs(sin(u * 11))
            flyBack = 1.0
        }
        var r = lerp(r0, .28, eio(seg(t, 6.0, 6.6)))
        r = lerp(r, 0.0, eio(seg(t, 7.4, 7.9)))
        rot = r
        if (t > tHit && t < tHit + .22) {
            val kq = sin(seg(t, tHit, tHit + .22) * PI)
            sy *= 1 - .14 * kq
            sx *= 1 + .1 * kq
        }
        eyes = if (t < tHit + .1) Eyes.CALM else Eyes.CRY
        sob = t > 5.1 && t < 7.4
        flail = if (t < 5.0) sin(seg(t, tFall, 5.0) * PI) else 0.0
        rubEye = (t > 6.9 && t < 7.5) || (t > 16.0 && t < 21.0)
        rubAlt = t < 7.5
        tears = if (t > 5.0) 1.0 else 0.0
        x = xSide
        if (t >= 8.4) {
            dragU = seg(t, 8.4, 16.0)
            ph = walk(t, 8.4, 16.0, 0.0, 700.0, 28.0).ph
            stride = .5
            dragging = t < 16.0
            lean = if (dragging) .22 + .04 * sin(t * 8) else 0.0
            shake = if (dragging) .4 else 0.0
        }
        holding = t >= 8.0 && t < 16.0
        if (t > 16.0 && t < 17.4) {
            val v = seg(t, 16.0, 17.4)
            sy = 1 - .07 * sin(v * PI)
            sx = 1 + .05 * sin(v * PI)
            sob = true
        }
        dropAt = 16.0
        if (t > 17.7) {
            val w3 = walk(t, 17.7, 21.0, DRAG_X, DC, 40.0)
            x = w3.x
            ph = w3.ph
            stride = .75
        }
    }
}

// ── 2D AFFIN ─────────────────────────────────────────────────────
private class Aff {
    var a = 1.0
    var b = 0.0
    var c = 0.0
    var d = 1.0
    var e = 0.0
    var f = 0.0

    fun translate(x: Double, y: Double): Aff {
        e += a * x + c * y
        f += b * x + d * y
        return this
    }

    fun scale(sx: Double, sy: Double): Aff {
        a *= sx
        b *= sx
        c *= sy
        d *= sy
        return this
    }

    fun rotate(r: Double): Aff {
        val co = cos(r)
        val si = sin(r)
        val na = a * co + c * si
        val nb = b * co + d * si
        val nc = -a * si + c * co
        val nd = -b * si + d * co
        a = na; b = nb; c = nc; d = nd
        return this
    }

    fun map(p: V) = V(a * p.x + c * p.y + e, b * p.x + d * p.y + f)

    fun unmap(p: V): V {
        val det = a * d - b * c
        val x = p.x - e
        val y = p.y - f
        return V((d * x - c * y) / det, (-b * x + a * y) / det)
    }

    fun matrix(): Matrix = Matrix().apply {
        setValues(floatArrayOf(a.toFloat(), c.toFloat(), e.toFloat(), b.toFloat(), d.toFloat(), f.toFloat(), 0f, 0f, 1f))
    }
}

// ── JO'JA SHAKLLARI ──────────────────────────────────────────────
private fun argc(op: Double) = if (op == 1.0) 6 else if (op == 3.0) 0 else 2

private class PartD(val p: UtyanPart) {
    val a = DoubleArray(p.a.size) { p.a[it].toDouble() }
    val b = DoubleArray(p.b.size) { p.b[it].toDouble() }
}

private val parts: Map<String, PartD> = kUtyanParts.associate { it.name to PartD(it) }

private fun mix(a: DoubleArray, b: DoubleArray, t: Double): DoubleArray {
    val o = a.copyOf()
    var i = 0
    while (i < a.size) {
        val n = argc(a[i++])
        var k = 0
        while (k < n) {
            o[i] = a[i] + (b[i] - a[i]) * t
            k++
            i++
        }
    }
    return o
}

private fun bbox(a: DoubleArray): Rc {
    var x0 = 1e9
    var y0 = 1e9
    var x1 = -1e9
    var y1 = -1e9
    var i = 0
    while (i < a.size) {
        val n = argc(a[i++])
        var k = 0
        while (k < n) {
            x0 = min(x0, a[i + k])
            x1 = max(x1, a[i + k])
            y0 = min(y0, a[i + k + 1])
            y1 = max(y1, a[i + k + 1])
            k += 2
        }
        i += n
    }
    return Rc(x0, y0, x1, y1)
}

private fun xform(a: DoubleArray, cx: Double, sx: Double, dx: Double, dy: Double): DoubleArray {
    val o = a.copyOf()
    var i = 0
    while (i < a.size) {
        val n = argc(a[i++])
        var k = 0
        while (k < n) {
            o[i + k] = cx + (a[i + k] - cx) * sx + dx
            o[i + k + 1] = a[i + k + 1] + dy
            k += 2
        }
        i += n
    }
    return o
}

/** Ko'zni tik ag'daradi: yumuq "U" -> xursand "^". */
private fun flipY(a: DoubleArray): DoubleArray {
    val cy = bbox(a).cy
    val o = a.copyOf()
    var i = 0
    while (i < a.size) {
        val n = argc(a[i++])
        var k = 0
        while (k < n) {
            o[i + k + 1] = 2 * cy - a[i + k + 1] + 6
            k += 2
        }
        i += n
    }
    return o
}

private fun pathOf(a: DoubleArray): Path {
    val p = Path()
    var i = 0
    while (i < a.size) {
        val op = a[i++]
        if (op == 0.0) {
            p.moveTo(a[i].toFloat(), a[i + 1].toFloat()); i += 2
        } else if (op == 1.0) {
            p.cubicTo(a[i].toFloat(), a[i + 1].toFloat(), a[i + 2].toFloat(), a[i + 3].toFloat(), a[i + 4].toFloat(), a[i + 5].toFloat()); i += 6
        } else if (op == 2.0) {
            p.lineTo(a[i].toFloat(), a[i + 1].toFloat()); i += 2
        } else {
            p.close()
        }
    }
    return p
}

/** Chapga burilganda yuz qismlari o'rin almashadi (uzoqdagi ko'z — chetda). */
private val faceSwap = mapOf("eye" to "eye_2", "eye_2" to "eye", "beak" to "beak", "beak_bl" to "beak_bl", "mouth" to "mouth")

private fun pose(name: String, yaw: Double): DoubleArray {
    val part = parts.getValue(name)
    if (yaw < -1) {
        val base = pose(name, -1.0)
        if (!faceSwap.containsKey(name)) return base
        return xform(base, 0.0, 1.0, -(-1 - yaw) * 170, 0.0)
    }
    if (yaw >= 0) return mix(part.a, part.b, yaw)
    val u = -yaw
    val ra = bbox(part.a)
    val swap = faceSwap[name]
    val rb: Rc
    val tx: Double
    if (swap != null) {
        rb = bbox(parts.getValue(swap).b)
        tx = 2 * HEAD_C - rb.cx
    } else {
        rb = bbox(part.b)
        tx = ra.cx - (rb.cx - ra.cx)
    }
    return xform(part.a, ra.cx, lerp(1.0, rb.w / ra.w, u), (tx - ra.cx) * u, (rb.cy - ra.cy) * u)
}

// ── QO'LLAR ──────────────────────────────────────────────────────
private const val SH_Y = 250.0
private const val SH_DX = 96.0
private const val ARM_L = 62.0

private class Arms(val shA: V, val shB: V, val a: V, val b: V, val face: Double)

private fun polar(p: V, deg: Double, l: Double) = V(p.x + cos(deg * PI / 180) * l, p.y + sin(deg * PI / 180) * l)
private fun lerp2(a: V, b: V, t: Double) = V(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
private fun capTo(sh: V, p: V, maxL: Double): V {
    val d = (p - sh).dist
    if (d <= maxL || d == 0.0) return p
    return sh + (p - sh) * (maxL / d)
}

// ── BO'YOQ YORDAMCHILARI ─────────────────────────────────────────
private fun col(v: Long) = v.toInt()
private fun withA(c: Int, a: Double) = (c and 0xFFFFFF) or ((clampD(a, 0.0, 1.0) * 255).toInt() shl 24)
private fun fillP(c: Int) = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.FILL; color = c }
private fun lineP(w: Double, c: Int, cap: Paint.Cap = Paint.Cap.BUTT, join: Paint.Join = Paint.Join.MITER) =
    Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.STROKE; strokeWidth = w.toFloat(); color = c; strokeCap = cap; strokeJoin = join }
private fun lerpColor(a: Int, b: Int, t: Double): Int {
    fun ch(s: Int) = (((a shr s) and 255) + (((b shr s) and 255) - ((a shr s) and 255)) * t).toInt().coerceIn(0, 255)
    return (ch(24) shl 24) or (ch(16) shl 16) or (ch(8) shl 8) or ch(0)
}
private fun linear(x0: Double, y0: Double, x1: Double, y1: Double, c0: Int, c1: Int) =
    Paint(Paint.ANTI_ALIAS_FLAG).apply { shader = LinearGradient(x0.toFloat(), y0.toFloat(), x1.toFloat(), y1.toFloat(), c0, c1, Shader.TileMode.CLAMP) }
private fun rectC(cx: Double, cy: Double, w: Double, h: Double) = RectF((cx - w / 2).toFloat(), (cy - h / 2).toFloat(), (cx + w / 2).toFloat(), (cy + h / 2).toFloat())
private fun rectL(l: Double, t: Double, w: Double, h: Double) = RectF(l.toFloat(), t.toFloat(), (l + w).toFloat(), (t + h).toFloat())
private fun rectLTRB(l: Double, t: Double, r: Double, b: Double) = RectF(l.toFloat(), t.toFloat(), r.toFloat(), b.toFloat())
private fun Canvas.rot(r: Double) = rotate(Math.toDegrees(r).toFloat())
private fun Canvas.tr(x: Double, y: Double) = translate(x.toFloat(), y.toFloat())
private fun Path.mv(x: Double, y: Double) = moveTo(x.toFloat(), y.toFloat())
private fun Path.ln(x: Double, y: Double) = lineTo(x.toFloat(), y.toFloat())
private fun Path.qd(x1: Double, y1: Double, x2: Double, y2: Double) = quadTo(x1.toFloat(), y1.toFloat(), x2.toFloat(), y2.toFloat())
private fun Path.cb(x1: Double, y1: Double, x2: Double, y2: Double, x3: Double, y3: Double) =
    cubicTo(x1.toFloat(), y1.toFloat(), x2.toFloat(), y2.toFloat(), x3.toFloat(), y3.toFloat())

private const val HOFF = HW * .55

// ── RASSOM ───────────────────────────────────────────────────────
private class ChickScene(private val bytes: Long) {
    private val dur = durationOf(tierOf(bytes))
    private val mouthRect = rectC(BC, BT, BR - BL + 16, 26.0)

    fun draw(canvas: Canvas, width: Float, height: Float, anim: Double) {
        val k = min(width / W, height / H)
        canvas.save()
        canvas.translate(((width - W * k) / 2).toFloat(), ((height - H * k) / 2).toFloat())
        canvas.scale(k.toFloat(), k.toFloat())
        canvas.clipRect(0f, 0f, W.toFloat(), H.toFloat())
        val s = resolve(S(bytes, anim * dur))
        var release: V? = null
        if (s.bag == BagMode.FLY || s.bag == BagMode.BIN) release = handWorld(S(bytes, s.flyT0 - 0.0001))
        val chickIn = s.depth < -.02
        val e = s.endS
        val overBin = s.x > BL - HW

        ground(canvas)
        fence(canvas)
        houseBack(canvas, s)
        binBack(canvas, s)
        if (e != null && e.inside) bagEnd(canvas, s, e)
        if (chickIn) {
            neck(canvas, s)
            chick(canvas, s)
        }
        houseFront(canvas)
        doorPanel(canvas, s)
        if (e != null && !e.inside) bagEnd(canvas, s, e)
        dragBag(canvas, s)
        if (!chickIn) neck(canvas, s)
        if (!chickIn && !overBin) chick(canvas, s)
        tearDrops(canvas, s)
        if (s.grip > .5) {
            val m = matrix(s)
            val ar = arms(s, m)
            canvas.save()
            canvas.clipRect(0f, 0f, DR.toFloat(), H.toFloat())
            canvas.concat(m.matrix())
            arm(canvas, ar.shB, ar.b)
            canvas.restore()
        }
        if (release != null) flying(canvas, s, release)
        binFront(canvas, s)
        if (overBin) chick(canvas, s)
        puff(canvas, s)
        sparks(canvas, s)
        canvas.restore()
    }

    // ── QOP HOLATI ─────────────────────────────────────────────────
    private fun bagLen(s: S) = .9 * s.bh * (1 + (1 - s.bagSq) * .35)
    private fun bagWW(s: S) = s.bw * s.bagSq
    private fun knotY(s: S) = FLOOR - bagWW(s) / 2 * .85 + 2

    private fun setKnot(s: S, knotX: Double) {
        s.bagAng = PI / 2
        s.bagPx = knotX - bagLen(s)
        s.knot = V(knotX, knotY(s))
    }

    private fun endState(s: S): End? {
        if (!s.hasEnd) return null
        val t = s.sec
        val d = s.bw
        var sc: Double
        var inside = true
        if (s.endStuck) {
            if (t < s.endFrom) return null
            sc = lerp(.78, 1.0, eio(seg(t, s.endFrom, 2.1))) + .03 * s.jerk
            if (t >= s.endPop) {
                inside = false
                sc = lerp(1.0, 1.05, eout(seg(t, s.endPop, s.endPop + .1)))
            }
        } else {
            if (t < s.endFrom || t >= s.endOut) return null
            sc = lerp(.75, 1.0, eio(seg(t, s.endFrom, s.endAt)))
            if (t >= s.endAt) {
                inside = false
                sc = 1.0
            }
        }
        val tip = if (s.endStuck) seg(t, s.endPop, s.endPop + .35) else (if (t >= s.endAt) seg(t, s.endAt, s.endOut) else 0.0)
        if (tip >= 1) return null
        val ry = d / 2 * .92 * sc
        return End(DC, FLOOR - ry + 2 - (if (inside) (1 - sc) * 40 else 0.0), d, sc, ry, tip, inside)
    }

    private fun resolve(s: S): S {
        if (s.bag != BagMode.DRAG) return s
        val t = s.sec
        val e = endState(s)
        s.endS = e
        s.side = false
        s.knot = null
        if (e != null && (s.holding || s.stuck)) s.knot = V(e.cx + 18, e.cy)
        if (s.tier == 3) {
            if (t >= s.endAt) {
                s.side = t >= s.endOut
                setKnot(s, (if (t < s.dropAt) s.x else DRAG_X) - HOFF)
                if (t >= s.dropAt) s.knot = null
            }
            return s
        }
        val pop = s.endPop
        if (t < pop) return s
        s.side = t >= pop + .35
        val restPx = 65.0
        s.bagSq = lerp(.86, 1.0, backOut(seg(t, pop, pop + .4), 2.5))
        s.bagAng = PI / 2
        s.bagPx = lerp(restPx - 70, restPx, eout(seg(t, pop, pop + .7)))
        val xSide = DR + 90
        val xLand = xSide + 80
        s.x = lerp(xSide, xLand, s.flyBack)
        if (t >= 8.0) {
            val xGrab = restPx + bagLen(s) + HOFF
            if (t < 8.4) {
                s.x = lerp(xLand, xGrab, eio(seg(t, 8.0, 8.4)))
                s.knot = V(restPx + bagLen(s), knotY(s))
            } else {
                s.x = lerp(xGrab, DRAG_X, s.dragU)
                setKnot(s, (if (t < s.dropAt) s.x else DRAG_X) - HOFF)
                if (t >= s.dropAt) s.knot = null
            }
        }
        if (t > 17.7) s.x = walk(t, 17.7, 21.0, DRAG_X, DC, 40.0).x
        return s
    }

    // ── JO'JA ──────────────────────────────────────────────────────
    private fun matrix(s: S): Aff {
        var rot = s.rot
        var lift = s.lift
        var sx = s.sx
        var sy = s.sy
        if (s.ph >= 0) {
            val ph = s.ph % 1
            val n = floor(s.ph).toInt()
            lift += sin(ph * PI) * 16 * s.stride
            rot += (if (n % 2 != 0) 1.0 else -1.0) * sin(ph * PI) * .06 * s.stride
            val land = if (ph < .15) 1 - ph / .15 else (if (ph > .85) (ph - .85) / .15 else 0.0)
            sy *= 1 - .06 * land
            sx *= 1 + .05 * land
        }
        val dir = if (s.yaw < 0) -1.0 else 1.0
        if (s.wind > 0 || s.fling > 0) {
            sy *= 1 - .08 * s.wind + .06 * s.fling
            sx *= 1 + .07 * s.wind - .04 * s.fling
            rot += (-.10 * s.wind + .12 * s.fling) * dir
        }
        if (s.hop > 0) {
            lift += s.hop * 46
            sy *= 1 + .06 * s.hop
            sx *= 1 - .04 * s.hop
        }
        rot += s.lean
        if (s.shake > 0) rot += sin(s.sec * 55) * .025 * s.shake
        if (s.sob) {
            val c = (s.sec * 1.7) % 1
            val v = if (c < .18) c / .18 else (1 - (c - .18) / .82).pow(2)
            sy *= 1 + .06 * v
            sx *= 1 - .03 * v
            if (s.pivotR) rot += .03 * v
        }
        sx *= 1 + .1 * s.depth
        sy *= 1 + .1 * s.depth
        lift -= 14 * s.depth
        val m = Aff()
        if (s.pivotR) m.translate(s.x + HW, FLOOR - lift).rotate(rot).translate(-HW, 0.0)
        else m.translate(s.x, FLOOR - lift).rotate(rot)
        m.scale(K * sx, K * sy).translate(-PX, -PY)
        return m
    }

    /** Qo'l nuqtalari (jo'janing 512 fazosida). */
    private fun arms(s: S, m: Aff): Arms {
        val y = s.yaw
        val ys = max(-1.0, y)
        val kk = 1 - .5 * abs(ys)
        val off = 34 * ys
        val shA = V(CX - SH_DX * kk + off, SH_Y)
        val shB = V(CX + SH_DX * kk + off, SH_Y)
        val face = if (y < -.05) -1.0 else 1.0
        fun mir(d: Double) = if (face > 0) d else 180 - d
        var aA = 115.0
        var aB = 65.0
        if (s.ph >= 0) {
            val ph = s.ph % 1
            val n = floor(s.ph).toInt()
            val sw = sin(ph * PI) * (if (n % 2 != 0) 1 else -1) * 24
            aA += sw
            aB -= sw
        }
        var a = polar(shA, aA, ARM_L)
        var b = polar(shB, aB, ARM_L)
        val nearIsB = face > 0
        val throwB = if (s.throwArm != null) s.throwArm == 'b' else nearIsB
        if (s.bag == BagMode.HAND || s.fling > 0) {
            val s0 = if (throwB) shB else shA
            val hy = min(320.0, PY - .9 * s.bh / K - 26)
            val hold = capTo(s0, V(s0.x + face * 58, hy), ARM_L * 1.25)
            val back = polar(s0, mir(160.0), ARM_L * .95)
            val up = polar(s0, mir(-55.0), ARM_L)
            var h = lerp2(hold, back, s.wind)
            h = lerp2(h, up, s.fling)
            if (throwB) b = h else a = h
        }
        if (s.grip > 0) {
            val p = m.unmap(V(DR + 8, FLOOR - 62))
            b = lerp2(b, capTo(shB, p, ARM_L * 2.2), s.grip)
        }
        if (s.happy) {
            val w = sin(s.sec * 22) * 14
            a = polar(shA, -125 + w, ARM_L)
            b = polar(shB, -55 - w, ARM_L)
        }
        val knot = s.knot
        if ((s.dragging || s.stuck || s.holding) && knot != null) {
            val tr = s.shake * sin(s.sec * 50) * 5
            val k = m.unmap(knot)
            val cap = if (s.endS != null) ARM_L * 1.35 else ARM_L * 1.9
            a = capTo(shA, V(k.x + 8, k.y + 12 + tr), cap)
            b = capTo(shB, V(k.x - 8, k.y - 12 - tr), cap)
        }
        if (s.flail > 0) {
            val w = sin(s.sec * 30) * 30
            a = lerp2(a, polar(shA, -140 + w, ARM_L), s.flail)
            b = lerp2(b, polar(shB, -40 - w, ARM_L), s.flail)
        }
        if (s.rubEye && !s.dragging) {
            val r = sin(s.sec * 16) * 10
            val e1 = bbox(pose("eye", y)).c
            val e2 = bbox(pose("eye_2", y)).c
            val ha = capTo(shA, V(e1.x + r, e1.y + 20), ARM_L * 1.7)
            val hb = capTo(shB, V(e2.x - r, e2.y + 20), ARM_L * 1.7)
            if (s.rubAlt) {
                if (floor(s.sec / .7).toInt() % 2 != 0) a = ha else b = hb
            } else if (nearIsB) b = hb else a = ha
        }
        if (s.wipeBrow > 0) {
            val h = V(HEAD_C + sin(s.sec * 10) * 30, 62.0)
            a = lerp2(a, capTo(shA, h, ARM_L * 1.8), s.wipeBrow)
        }
        return Arms(shA, shB, a, b, face)
    }

    private fun tip(sh: V, hand: V): V {
        val dv = hand - sh
        val d = if (dv.dist == 0.0) 1.0 else dv.dist
        return sh + dv * ((d + 20) / d)
    }

    private fun carry(s: S, ar: Arms): Char? =
        if (s.bag == BagMode.HAND || s.fling > 0) (s.throwArm ?: (if (ar.face > 0) 'b' else 'a')) else null

    private fun handWorld(s: S): V {
        val m = matrix(s)
        val ar = arms(s, m)
        val k = carry(s, ar) ?: (if (ar.face > 0) 'b' else 'a')
        val t = if (k == 'b') tip(ar.shB, ar.b) else tip(ar.shA, ar.a)
        return m.map(V(t.x, t.y + .4 * s.bh / K))
    }

    private fun pullWorld(s: S): V {
        val m = matrix(s)
        val ar = arms(s, m)
        return m.map(lerp2(ar.a, ar.b, .5))
    }

    /** Qanotsimon keng qo'l: ildizi yelkada, uchi — panja. */
    private fun arm(canvas: Canvas, sh: V, hand: V) {
        val dv = hand - sh
        val l = dv.dist + 24
        canvas.save()
        canvas.tr(sh.x, sh.y)
        canvas.rot(atan2(dv.y, dv.x))
        val p = Path().apply {
            mv(-8.0, -30.0)
            qd(l * .45, -36.0, l * .72, -26.0)
            cb(l + 8, -18.0, l + 8, 18.0, l * .72, 24.0)
            qd(l * .45, 32.0, -8.0, 30.0)
        }
        canvas.drawPath(p, linear(0.0, -30.0, 0.0, 30.0, col(0xFFFFE066), col(0xFFFFC81F)))
        canvas.drawPath(p, lineP(10.0, col(0xFFFA9016), Paint.Cap.ROUND, Paint.Join.ROUND))
        canvas.drawPath(
            Path().apply { mv(l * .22, -17.0); qd(l * .45, -22.0, l * .62, -16.0) },
            lineP(8.0, col(0xE6FFFFFF), Paint.Cap.ROUND, Paint.Join.ROUND),
        )
        canvas.restore()
    }

    private fun part(canvas: Canvas, name: String, flat: DoubleArray) {
        val p = parts.getValue(name).p
        val path = pathOf(flat)
        p.fill?.let { canvas.drawPath(path, fillP(it)) }
        p.stroke?.let {
            canvas.drawPath(path, lineP(p.width.toDouble(), it, if (p.butt) Paint.Cap.BUTT else Paint.Cap.ROUND, Paint.Join.ROUND))
        }
    }

    private fun chick(canvas: Canvas, s: S) {
        val m = matrix(s)
        val lift = s.lift + (if (s.ph >= 0) sin((s.ph % 1) * PI) * 16 else 0.0) + s.hop * 46
        val lie = if (s.pivotR) sin(s.rot) else 0.0
        canvas.drawOval(
            rectC(s.x + lie * HW, FLOOR + 3 + 14 * s.depth, 2 * (HW * 1.05 * (1 - lift / 250) + lie * HW * .6), 18.0),
            fillP(withA(0, clampD(.3 - lift / 300, 0.0, 1.0))),
        )
        canvas.save()
        canvas.concat(m.matrix())
        val y = s.yaw
        val ar = arms(s, m)
        val near = if (ar.face > 0) 'b' else 'a'
        val carry = carry(s, ar)
        val gripB = s.grip > .5
        val backV = y < -1.3
        if (backV) {
            arm(canvas, ar.shA, ar.a)
            arm(canvas, ar.shB, ar.b)
        } else if (abs(y) > .35) {
            val far = if (near == 'b') 'a' else 'b'
            if (!(far == 'b' && gripB) && far != carry) {
                if (far == 'a') arm(canvas, ar.shA, ar.a) else arm(canvas, ar.shB, ar.b)
            }
        }
        for (n in listOf("body", "head_bl3", "head", "head_bl1", "head_bl2")) part(canvas, n, pose(n, y))
        canvas.save()
        if (y < -1) {
            val clip = pathOf(pose("body", y))
            clip.addPath(pathOf(pose("head", y)))
            canvas.clipPath(clip)
        }
        for (n in listOf("beak", "beak_bl", "mouth")) part(canvas, n, pose(n, y))
        val e1 = pose("eye", y)
        val e2 = pose("eye_2", y)
        val c1 = bbox(e1).c
        val c2 = bbox(e2).c
        fun ink(w: Double, c: Int = col(0xFF000000)) = lineP(w, c, Paint.Cap.ROUND, Paint.Join.ROUND)
        when (s.eyes) {
            Eyes.HAPPY -> {
                part(canvas, "eye", flipY(e1))
                part(canvas, "eye_2", flipY(e2))
            }
            Eyes.STRAIN -> {
                for ((c, d) in listOf(c1 to 1.0, c2 to -1.0)) {
                    canvas.drawPath(
                        Path().apply { mv(c.x - 16 * d, c.y - 14); ln(c.x + 12 * d, c.y); ln(c.x - 16 * d, c.y + 14) },
                        ink(12.0),
                    )
                }
            }
            else -> {
                part(canvas, "eye", e1)
                part(canvas, "eye_2", e2)
            }
        }
        if (s.eyes == Eyes.CRY || s.eyes == Eyes.STRAIN) {
            val kq = if (s.eyes == Eyes.STRAIN) -1.0 else 1.0
            for ((c, d) in listOf(c1 to 1.0, c2 to -1.0)) {
                canvas.drawLine((c.x - 26 * d).toFloat(), (c.y - 34 + 10 * kq).toFloat(), (c.x + 18 * d).toFloat(), (c.y - 38 - 6 * kq).toFloat(), ink(10.0))
            }
        }
        if (s.tears > 0) {
            for ((c, d) in listOf(c1 to -1.0, c2 to 1.0)) {
                canvas.drawPath(
                    Path().apply { mv(c.x + 16 * d, c.y + 8); qd(c.x + 28 * d, c.y + 40, c.x + 24 * d, c.y + 70) },
                    ink(11.0, col(0xE678C8FF)),
                )
            }
        }
        if (s.sweat > 0) {
            val ph = (s.sec * 1.6) % 1
            val px = if (y < 0) 185.0 else 420.0
            val py = 50 + ph * 60
            val drop = Path().apply {
                mv(px, py - 28)
                qd(px + 18, py, px, py + 12)
                qd(px - 18, py, px, py - 28)
                close()
            }
            val alpha = s.sweat * (1 - ph * .6)
            canvas.drawPath(drop, fillP(withA(col(0xFF8FD3FF), alpha)))
            canvas.drawPath(drop, ink(6.0, withA(col(0xFF3D8FC4), alpha)))
        }
        canvas.restore()
        if (backV) {
            canvas.restore()
            return
        }
        if (s.bag == BagMode.HAND && carry != null) {
            val t = if (carry == 'b') tip(ar.shB, ar.b) else tip(ar.shA, ar.a)
            val sw = (if (s.ph >= 0) .16 * sin(s.ph * PI) else .05 * sin(s.sec * 3)) + s.wind * .5 * ar.face
            canvas.save()
            canvas.tr(t.x, t.y)
            canvas.rot(sw)
            bag(canvas, V(0.0, .4 * s.bh / K), s.bw / K, s.bh / K, 1.0, 0.0, local = true)
            canvas.restore()
        }
        val side = abs(y) > .35
        if (!side || near == 'a' || carry == 'a') arm(canvas, ar.shA, ar.a)
        if ((!side || near == 'b' || carry == 'b') && !gripB) arm(canvas, ar.shB, ar.b)
        canvas.restore()
    }

    // ── QOP ────────────────────────────────────────────────────────
    private fun bag(canvas: Canvas, c: V, w: Double, h: Double, sq: Double, rot: Double, local: Boolean = false) {
        canvas.save()
        canvas.tr(c.x, c.y)
        canvas.rot(rot)
        val ww = w * sq
        val hh = h * (1 + (1 - sq) * .35)
        val lw = if (local) 10.0 else clampD(h * .035, 3.2, 7.0)
        val top = -hh / 2
        val bot = hh / 2
        val neck = ww * .13
        val body = Path().apply {
            mv(-neck, top + hh * .16)
            cb(-ww * .62, top + hh * .26, -ww * .6, bot - hh * .02, -ww * .32, bot)
            cb(-ww * .12, bot + hh * .04, ww * .12, bot + hh * .04, ww * .32, bot)
            cb(ww * .6, bot - hh * .02, ww * .62, top + hh * .26, neck, top + hh * .16)
            close()
        }
        canvas.drawPath(body, linear(-ww / 2, 0.0, ww / 2, 0.0, col(0xFF4A4F58), col(0xFF2C2F35)))
        val edge = lineP(lw, col(0xFF121317), Paint.Cap.ROUND, Paint.Join.ROUND)
        canvas.drawPath(body, edge)
        val kt = top + hh * .16
        val knot = Path().apply {
            mv(-neck, kt)
            cb(-ww * .3, top + hh * .04, -ww * .3, top - hh * .06, -ww * .17, top - hh * .05)
            cb(-ww * .1, top - hh * .04, -ww * .04, top + hh * .06, 0.0, top + hh * .1)
            cb(ww * .04, top + hh * .06, ww * .1, top - hh * .04, ww * .17, top - hh * .05)
            cb(ww * .3, top - hh * .06, ww * .3, top + hh * .04, neck, kt)
            close()
        }
        canvas.drawPath(knot, fillP(col(0xFF3D4149)))
        canvas.drawPath(knot, edge)
        canvas.drawOval(rectC(0.0, kt - hh * .01, ww * .2, hh * .07), fillP(col(0xFF121317)))
        val fold = lineP(lw * .65, col(0xBF121317), Paint.Cap.ROUND)
        canvas.drawPath(Path().apply { mv(-ww * .05, top + hh * .28); qd(-ww * .14, top + hh * .55, -ww * .08, top + hh * .78) }, fold)
        canvas.drawPath(Path().apply { mv(ww * .2, top + hh * .3); qd(ww * .28, top + hh * .55, ww * .2, top + hh * .74) }, fold)
        canvas.drawPath(
            Path().apply { mv(-ww * .36, top + hh * .42); qd(-ww * .4, top + hh * .6, -ww * .3, top + hh * .78) },
            lineP(lw * .8, col(0x73FFFFFF), Paint.Cap.ROUND),
        )
        canvas.restore()
    }

    /** Yonboshlab yotgan, sudralayotgan qop: tubi `bagPx` da yerda. */
    private fun dragBag(canvas: Canvas, s: S) {
        if (s.bag != BagMode.DRAG || s.bagPx.isNaN() || !s.side) return
        val sq = s.bagSq
        val hh = s.bh * (1 + (1 - sq) * .35)
        val ww = s.bw * sq
        val py = FLOOR - (ww / 2) * abs(sin(s.bagAng)) * .85 + 2
        canvas.save()
        canvas.tr(s.bagPx, py)
        canvas.rot(s.bagAng)
        bag(canvas, V(0.0, -hh / 2), s.bw, s.bh, sq, 0.0)
        canvas.restore()
    }

    /** Eshik o'yig'idagi qop: old (yig'ilgan og'zi) tomoni bizga qarab turadi. */
    private fun bagEnd(canvas: Canvas, s: S, e: End) {
        var rx = e.d / 2 * e.sc
        var ry = e.ry
        var cx = e.cx
        var cy = e.cy
        if (e.tip > 0 && !s.bagPx.isNaN()) {
            val sq = s.bagSq
            val hh = s.bh * (1 + (1 - sq) * .35)
            val ww = s.bw * sq
            val u = eout(e.tip)
            val th = u * PI / 2
            val sn = sin(th)
            val cs = cos(th)
            val py = FLOOR - ww / 2 * .85 + 2
            val c = lerp(e.cx, s.bagPx + hh / 2, u)
            cy = lerp(cy, py, u)
            ry = lerp(ry, ww / 2 * .9, u)
            canvas.drawOval(rectC(c, FLOOR + 2, 2 * (hh / 2 * sn + rx * cs) * .95, 20.0), fillP(col(0x4D000000)))
            if (sn > .02) {
                canvas.save()
                canvas.tr(c, py)
                canvas.scale(sn.toFloat(), 1f)
                canvas.tr(-hh / 2, 0.0)
                canvas.rot(PI / 2)
                bag(canvas, V(0.0, -hh / 2), s.bw, s.bh, sq, 0.0)
                canvas.restore()
            }
            rx *= cs
            cx = c + hh / 2 * sn
            if (rx < 2) return
        } else {
            if (rx <= 0 || ry <= 0) return
            canvas.drawOval(rectC(cx, FLOOR + 2, 2 * rx * .95, 20.0), fillP(col(0x4D000000)))
        }
        canvas.save()
        canvas.tr(cx, cy)
        val oval = rectC(0.0, 0.0, 2 * rx, 2 * ry)
        // Fokusli radial gradient (API 24 da yo'q) — markazni (-0.3rx, -0.35ry) ga surib yaqinlashtiramiz.
        val g = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            shader = RadialGradient((-rx * .3).toFloat(), (-ry * .35).toFloat(), (rx * 1.2).toFloat(), col(0xFF555A64), col(0xFF25282D), Shader.TileMode.CLAMP)
        }
        canvas.drawOval(oval, g)
        canvas.drawOval(oval, lineP(6.0, col(0xFF121317)))
        val wr = lineP(4.0, col(0xBF121317), Paint.Cap.ROUND)
        val folds = arrayOf(
            doubleArrayOf(.3, .88, .3), doubleArrayOf(1.05, .62, -.25), doubleArrayOf(1.7, .8, .4), doubleArrayOf(2.55, .7, -.3),
            doubleArrayOf(3.3, .9, .2), doubleArrayOf(4.0, .55, -.35), doubleArrayOf(4.75, .78, .3), doubleArrayOf(5.5, .66, -.2),
        )
        for (f in folds) {
            val a = f[0]
            val l = f[1]
            val bend = f[2]
            canvas.drawPath(
                Path().apply {
                    mv(cos(a) * rx * l, sin(a) * ry * l)
                    qd(cos(a + bend) * rx * l * .55, sin(a + bend) * ry * l * .55, cos(a) * rx * .12, sin(a) * ry * .12)
                },
                wr,
            )
        }
        canvas.drawArc(rectC(0.0, 0.0, 2 * rx * .78, 2 * ry * .78), Math.toDegrees(PI * 1.08).toFloat(), Math.toDegrees(PI * .3).toFloat(), false, lineP(6.0, col(0x59FFFFFF)))
        canvas.drawCircle(0f, 0f, (ry * .13).toFloat(), fillP(col(0xFF121317)))
        canvas.restore()
    }

    /** Qop bo'g'zi: eshikdagi qopning yig'ilgan og'zi jo'janing qo'llarigacha cho'zilgan. */
    private fun neck(canvas: Canvas, s: S) {
        val e = s.endS
        if (e == null || !(s.holding || s.stuck)) return
        val h = pullWorld(s)
        val a = V(e.cx, e.cy)
        val c = V((a.x + h.x) / 2, (a.y + h.y) / 2 + (if (s.stuck) 4 else 14))
        fun at(u: Double) = a * ((1 - u) * (1 - u)) + c * (2 * u * (1 - u)) + h * (u * u)
        val w0 = min(54.0, e.ry * .42)
        val w1 = 14.0
        val left = ArrayList<V>()
        val right = ArrayList<V>()
        val n = 14
        for (i in 0..n) {
            val u = i.toDouble() / n
            val p = at(u)
            val dv = at(min(1.0, u + .01)) - at(max(0.0, u - .01))
            val d = if (dv.dist == 0.0) 1.0 else dv.dist
            val nv = V(-dv.y / d, dv.x / d)
            val w = w1 + (w0 - w1) * (1 - u).pow(2.2)
            left.add(p + nv * (w / 2))
            right.add(p - nv * (w / 2))
        }
        val body = Path()
        body.mv(left.first().x, left.first().y)
        for (p in left.drop(1)) body.ln(p.x, p.y)
        for (p in right.reversed()) body.ln(p.x, p.y)
        body.close()
        canvas.drawPath(body, fillP(col(0xFF34383F)))
        canvas.drawPath(body, lineP(5.0, col(0xFF121317), join = Paint.Join.ROUND))
        val fold = lineP(3.0, col(0xB3121317), Paint.Cap.ROUND)
        for (kf in doubleArrayOf(.3, .7)) {
            val f = Path()
            for (i in 0..10) {
                val q = lerp2(left[i], right[i], kf)
                if (i == 0) f.mv(q.x, q.y) else f.ln(q.x, q.y)
            }
            canvas.drawPath(f, fold)
        }
        canvas.drawCircle(h.x.toFloat(), h.y.toFloat(), 11f, fillP(col(0xFF3D4149)))
        canvas.drawCircle(h.x.toFloat(), h.y.toFloat(), 11f, lineP(4.0, col(0xFF121317)))
    }

    /** Ko'z yoshi tomchilari: erkin tushadi (g), yerga tegsa sachraydi. */
    private fun tearDrops(canvas: Canvas, s: S) {
        if (s.tears <= 0) return
        val m = matrix(s)
        val g = 2400.0
        for ((n, d, off) in listOf(Triple("eye", -1.0, 0.0), Triple("eye_2", 1.0, .31))) {
            val c = bbox(pose(n, s.yaw)).c
            val p = m.map(V(c.x + 24 * d, c.y + 72))
            for (i in 0 until 3) {
                val age = (s.sec + off + i * .23) % .69
                val yy = p.y + .5 * g * age * age
                val xx = p.x + d * 30 * age
                if (yy < FLOOR) {
                    canvas.drawOval(rectC(xx, yy, 7.0, 10.0), fillP(col(0xF296D7FF)))
                } else {
                    val tg = sqrt(max(0.0, 2 * (FLOOR - p.y) / g))
                    val a = age - tg
                    if (a < .15) {
                        val kq = a / .15
                        canvas.drawArc(
                            rectC(p.x + d * 30 * tg, FLOOR, 2 * (4 + 10 * kq), 2 * (2 + 2 * kq)),
                            180f, 180f, false,
                            lineP(2.5, withA(col(0xFF96D7FF), clampD(1 - kq, 0.0, 1.0))),
                        )
                    }
                }
            }
        }
    }

    private fun flying(canvas: Canvas, s: S, a: V) {
        if (s.bag != BagMode.FLY) return
        val u = seg(s.sec, s.flyT0, s.flyT1)
        val x = lerp(a.x, BC, u)
        val y = lerp(a.y, BT + 6, u) - 4 * s.flyPeak * u * (1 - u)
        canvas.save()
        if (u > .7) canvas.clipRect(0f, 0f, W.toFloat(), (BT + 2).toFloat())
        val st = 1 + .12 * sin(u * PI)
        bag(canvas, V(x, y), s.bw / st, s.bh * st, 1.0, u * .9 + sin(u * PI) * .35)
        canvas.restore()
    }

    // ── UY, PANJARA, QUTI ──────────────────────────────────────────
    private fun ground(canvas: Canvas) {
        canvas.drawRect(rectL(0.0, FLOOR, W, H - FLOOR), fillP(col(0xFF1F1A16)))
        canvas.drawRect(rectL(0.0, FLOOR, W, 8.0), fillP(col(0xFF2B241E)))
    }

    private fun fence(canvas: Canvas) {
        val top = FLOOR - 150
        val step = 74.0
        val ink = col(0xFF4A2A12)
        for (yy in doubleArrayOf(top + 40, top + 108)) {
            val r = rectL(WALL_R, yy, W - WALL_R, 18.0)
            canvas.drawRect(r, fillP(col(0xFF7A4A26)))
            canvas.drawRect(r, lineP(4.0, ink))
        }
        var x = WALL_R + 20
        while (x < W + step) {
            val p = Path().apply {
                mv(x, FLOOR); ln(x, top + 14); ln(x + 20, top); ln(x + 40, top + 14); ln(x + 40, FLOOR); close()
            }
            canvas.drawPath(p, linear(x, 0.0, x + 40, 0.0, col(0xFF9C6436), col(0xFF83512A)))
            canvas.drawPath(p, lineP(5.0, ink, join = Paint.Join.ROUND))
            canvas.drawLine((x + 9).toFloat(), (top + 24).toFloat(), (x + 9).toFloat(), (FLOOR - 14).toFloat(), lineP(4.0, col(0x38FFDCAA)))
            canvas.drawCircle((x + 20).toFloat(), (top + 49).toFloat(), 3.5f, fillP(ink))
            canvas.drawCircle((x + 20).toFloat(), (top + 117).toFloat(), 3.5f, fillP(ink))
            x += step
        }
        canvas.drawRect(rectL(WALL_R, top - 2, W - WALL_R, FLOOR - top + 2), fillP(col(0x47141414)))
    }

    private fun houseBack(canvas: Canvas, s: S) {
        val r = rectLTRB(DL, DT, DR, FLOOR)
        canvas.drawRect(r, linear(0.0, DT, 0.0, FLOOR, col(0xFF1D140E), col(0xFF3A271B)))
        canvas.drawRect(r, fillP(withA(col(0xFFFFC878), .25 * s.door)))
        val sill = rectL(DL - 14, FLOOR - 3, DR - DL + 28, 10.0)
        canvas.drawRect(sill, fillP(col(0xFF9A9792)))
        canvas.drawRect(rectL(DL - 14, FLOOR - 3, DR - DL + 28, 3.0), fillP(col(0xFFB5B2AC)))
        canvas.drawRect(sill, lineP(3.0, col(0xFF5A5855)))
    }

    private fun houseFront(canvas: Canvas) {
        val wall = Path().apply {
            fillType = Path.FillType.EVEN_ODD
            mv(-10.0, FLOOR); ln(-10.0, 170.0); ln(WALL_R, 262.0); ln(WALL_R, FLOOR); close()
            addRect(rectLTRB(DL, DT, DR, FLOOR), Path.Direction.CW)
        }
        canvas.drawPath(wall, linear(0.0, 0.0, WALL_R, 0.0, col(0xFFE9C79A), col(0xFFD4AB7A)))
        canvas.save()
        canvas.clipPath(wall)
        val plank = lineP(4.0, col(0x59966440))
        var yy = 200.0
        while (yy < FLOOR) {
            canvas.drawLine(-10f, yy.toFloat(), WALL_R.toFloat(), yy.toFloat(), plank)
            yy += 34
        }
        canvas.restore()
        canvas.drawLine(WALL_R.toFloat(), 262f, WALL_R.toFloat(), FLOOR.toFloat(), lineP(8.0, col(0xFF7A4A26)))
        for (base in listOf(rectLTRB(-10.0, FLOOR - 22, DL - 14, FLOOR), rectLTRB(DR + 14, FLOOR - 22, WALL_R, FLOOR))) {
            canvas.drawRect(base, fillP(col(0xFF8C8A86)))
            canvas.drawRect(base, lineP(5.0, col(0xFF5A5855)))
        }
        val frameFill = fillP(col(0xFFAA5A1F))
        val frameInk = lineP(7.0, col(0xFF6D3605))
        for (r in listOf(
            rectL(DL - 14, DT - 14, 14.0, FLOOR - DT + 14),
            rectL(DR, DT - 14, 14.0, FLOOR - DT + 14),
            rectL(DL - 14, DT - 14, DR - DL + 28, 14.0),
        )) {
            canvas.drawRect(r, frameFill)
            canvas.drawRect(r, frameInk)
        }
        val roofEdge = Path().apply { mv(-20.0, 110.0); ln(WALL_R + 46, 262.0); ln(WALL_R + 46, 286.0); ln(-20.0, 140.0); close() }
        canvas.drawPath(roofEdge, fillP(col(0xFFC4463A)))
        val roofInk = lineP(7.0, col(0xFF7A2620), join = Paint.Join.ROUND)
        canvas.drawPath(roofEdge, roofInk)
        val tiles = Path().apply { mv(-20.0, 30.0); ln(WALL_R + 46, 262.0); ln(-20.0, 262.0); close() }
        canvas.drawPath(tiles, fillP(col(0xFFD3554A)))
        canvas.save()
        canvas.clipPath(tiles)
        val row = lineP(5.0, col(0xB37A2620))
        for (i in 0 until 8) {
            val o = i * 34.0
            canvas.drawLine(-20f, (30 + o).toFloat(), (WALL_R + 46).toFloat(), (262 + o).toFloat(), row)
        }
        canvas.restore()
        canvas.drawLine(-20f, 30f, (WALL_R + 46).toFloat(), 262f, roofInk)
    }

    /** Eshik TASHQARIGA ochiladi: ilmog'i chap ustunda. */
    private fun doorPanel(canvas: Canvas, s: S) {
        val phi = s.door * 1.95
        val x0 = DL
        val y0 = DT
        val y1 = FLOOR
        val pw = DR - DL
        val xf = x0 + pw * cos(phi)
        val yc = (y0 + y1) / 2
        val hh = (y1 - y0) / 2 * (1 + sin(phi) * .12)
        val q = Path().apply { mv(x0, y0); ln(xf, yc - hh); ln(xf, yc + hh); ln(x0, y1); close() }
        val back = cos(phi) < 0
        val lit = abs(cos(phi))
        val c0 = if (back) lerpColor(col(0xFF7A3F14), col(0xFFA8612B), lit) else lerpColor(col(0xFFA8612B), col(0xFFD98A45), lit)
        val c1 = if (back) lerpColor(col(0xFF6A3510), col(0xFF93531F), lit) else lerpColor(col(0xFF93531F), col(0xFFC7773A), lit)
        canvas.drawPath(q, linear(x0, 0.0, if (xf == x0) x0 + 1 else xf, 0.0, c0, c1))
        fun at(u: Double, v: Double) = V(lerp(x0, xf, u), lerp(lerp(y0, yc - hh, u), lerp(y1, yc + hh, u), v))
        if (abs(xf - x0) > 10) {
            val panelInk = lineP(5.0, col(0xE66D3605), join = Paint.Join.ROUND)
            for (v in listOf(doubleArrayOf(.08, .42), doubleArrayOf(.52, .92))) {
                val p = Path().apply {
                    mv(at(.16, v[0]).x, at(.16, v[0]).y)
                    ln(at(.84, v[0]).x, at(.84, v[0]).y)
                    ln(at(.84, v[1]).x, at(.84, v[1]).y)
                    ln(at(.16, v[1]).x, at(.16, v[1]).y)
                    close()
                }
                canvas.drawPath(p, fillP(col(0x1A000000)))
                canvas.drawPath(p, panelInk)
            }
            val kr = at(.86, .55)
            val knob = rectC(kr.x, kr.y, 2 * (8 * lit + 2), 16.0)
            canvas.drawOval(knob, fillP(col(0xFFFFD527)))
            canvas.drawOval(knob, lineP(4.0, col(0xFF6D3605)))
        }
        canvas.drawPath(q, lineP(7.0, col(0xFF6D3605), join = Paint.Join.ROUND))
    }

    private fun binBack(canvas: Canvas, s: S) {
        canvas.save()
        canvas.tr(BC, BT)
        canvas.rot(s.binWob * .06)
        canvas.tr(-BC, -BT)
        canvas.drawOval(mouthRect, fillP(col(0xFF163F49)))
        canvas.restore()
    }

    private fun binFront(canvas: Canvas, s: S) {
        canvas.save()
        canvas.tr(BC, FLOOR)
        canvas.rot(s.binWob * .06)
        canvas.tr(-BC, -FLOOR)
        val body = Path().apply {
            mv(BL, BT); ln(BR, BT); ln(BR - 12, FLOOR - 14)
            qd(BR - 14, FLOOR, BR - 30, FLOOR)
            ln(BL + 30, FLOOR)
            qd(BL + 14, FLOOR, BL + 12, FLOOR - 14)
            close()
        }
        canvas.drawPath(
            body,
            Paint(Paint.ANTI_ALIAS_FLAG).apply {
                shader = LinearGradient(
                    BL.toFloat(), 0f, BR.toFloat(), 0f,
                    intArrayOf(col(0xFF58B3C9), col(0xFF3F97AD), col(0xFF2E7B8F)), floatArrayOf(0f, .55f, 1f), Shader.TileMode.CLAMP,
                )
            },
        )
        val ink = lineP(7.0, col(0xFF1B4D59), Paint.Cap.ROUND, Paint.Join.ROUND)
        canvas.drawPath(body, ink)
        val rib = lineP(5.0, col(0xBF1B4D59), Paint.Cap.ROUND)
        for (f in doubleArrayOf(.3, .5, .7)) {
            val x = lerp(BL + 18, BR - 18, f)
            canvas.drawLine(x.toFloat(), (BT + 26).toFloat(), lerp(x, BC, .08).toFloat(), (FLOOR - 22).toFloat(), rib)
        }
        canvas.drawLine((BL + 18).toFloat(), (BT + 24).toFloat(), (BL + 24).toFloat(), (BT + 84).toFloat(), lineP(6.0, col(0x8CFFFFFF), Paint.Cap.ROUND))
        canvas.drawOval(mouthRect, ink)
        canvas.restore()
    }

    // ── CHANG VA UCHQUNLAR ─────────────────────────────────────────
    private fun puff(canvas: Canvas, s: S) {
        if (!(s.puff > 0 && s.puff < 1)) return
        val a = 1 - eio(s.puff)
        val k = eout(s.puff)
        canvas.saveLayerAlpha(null, (255 * .9 * a).toInt().coerceIn(0, 255))
        val p = fillP(col(0xFFC7CCC1))
        for (d in listOf(
            doubleArrayOf(-48.0, -6.0, 13.0, 1.0), doubleArrayOf(-22.0, -26.0, 17.0, .8), doubleArrayOf(6.0, -30.0, 16.0, .9),
            doubleArrayOf(32.0, -20.0, 14.0, 1.0), doubleArrayOf(54.0, -4.0, 11.0, 1.1),
        )) {
            canvas.drawCircle(
                (BC + d[0] * (1 + k * .7 * d[3])).toFloat(), (BT - 10 + d[1] * (.6 + k * .9)).toFloat(),
                (d[2] * (.5 + k * .8) * (1 - .3 * s.puff)).toFloat(), p,
            )
        }
        canvas.restore()
    }

    private fun sparks(canvas: Canvas, s: S) {
        if (!(s.spark > 0 && s.spark < 1)) return
        val pts = listOf(doubleArrayOf(BL - 14, BT - 80, 1.0), doubleArrayOf(BR + 8, BT - 60, .8), doubleArrayOf(BC + 14, BT - 115, 1.1))
        val paint = fillP(col(0xFFFFD527))
        for (i in pts.indices) {
            val u = clampD(s.spark * 1.4 - i * .18, 0.0, 1.0)
            if (u <= 0 || u >= 1) continue
            val r = sin(u * PI) * 15 * pts[i][2]
            canvas.save()
            canvas.tr(pts[i][0], pts[i][1] - u * 22)
            canvas.rot(u * 1.5)
            val star = Path()
            for (j in 0 until 8) {
                val an = j * PI / 4
                val rr = if (j % 2 != 0) r * .35 else r
                if (j == 0) star.mv(cos(an) * rr, sin(an) * rr) else star.ln(cos(an) * rr, sin(an) * rr)
            }
            star.close()
            canvas.drawPath(star, paint)
            canvas.restore()
        }
    }
}

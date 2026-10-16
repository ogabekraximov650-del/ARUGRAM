package uz.arugram.soft.data

import kotlin.math.roundToLong

/** `1284512` -> `1.284.512` (Flutter: `formatCount`). */
fun formatCount(value: Number): String {
    val n = value.toDouble().roundToLong()
    val digits = kotlin.math.abs(n).toString()
    val sb = StringBuilder()
    for (i in digits.indices) {
        if (i > 0 && (digits.length - i) % 3 == 0) sb.append('.')
        sb.append(digits[i])
    }
    return if (n < 0) "-$sb" else sb.toString()
}

/**
 * Kartochka belgisi uchun QISQA raqam (Flutter: `formatCompact`):
 * `843`, `12,4 ming`, `1,3 mln`.
 */
fun formatCompact(value: Number): String {
    val n = value.toDouble().roundToLong()
    if (n < 1000) return "$n"
    fun cut(v: Double): String {
        val t = String.format(java.util.Locale.US, "%.1f", v).replace('.', ',')
        return if (t.endsWith(",0")) t.dropLast(2) else t
    }
    return if (n < 1_000_000) "${cut(n / 1000.0)} ming" else "${cut(n / 1_000_000.0)} mln"
}

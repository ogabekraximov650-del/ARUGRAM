package uz.arudrive.app.data

import android.content.Context

/**
 * Har video qayerga yetib ko'rilgani TELEFON XOTIRASIDA (SharedPreferences, ilova yopilsa ham, qayta o'rnatilmaguncha qoladi):
 * keyingi ochishda shu daqiqadan davom etadi (video qancha davomiylikda bo'lmasin). Video oxiriga yaqin (oxirgi 10 soniya yoki qisqa videoda oxirgi 10%) yoki tugagan bo'lsa — boshidan.
 */
object PlayPos {
    private const val MIN_MS = 3_000L        // 3 soniyadan kam ko'rilgan bo'lsa saqlanmaydi
    private const val END_GAP_MS = 10_000L   // oxirgi 10 soniya = tugagan hisoblanadi

    private fun prefs(ctx: Context) = ctx.applicationContext.getSharedPreferences("playpos", Context.MODE_PRIVATE)

    /** Davom ettirish joyi (ms); 0 — boshidan. */
    fun get(ctx: Context, id: String): Long = try { prefs(ctx).getLong("p_$id", 0L) } catch (_: Exception) { 0L }

    /** Ko'rilgan ulush 0..1 (kartadagi chiziq uchun); [durMs] — video davomiyligi. */
    fun fraction(ctx: Context, id: String, durMs: Long): Float {
        if (durMs <= 0) return 0f
        return (get(ctx, id).toFloat() / durMs).coerceIn(0f, 1f)
    }

    /** Joriy joyni saqlaydi (davomiylik noma'lum bo'lsa ham). Boshida yoki oxirida bo'lsa — o'chiradi. */
    fun save(ctx: Context, id: String, posMs: Long, durMs: Long) {
        try {
            val e = prefs(ctx).edit()
            val gap = if (durMs > 0) minOf(END_GAP_MS, durMs / 10) else END_GAP_MS
            if (posMs < MIN_MS || (durMs > 0 && posMs > durMs - gap)) e.remove("p_$id") else e.putLong("p_$id", posMs)
            e.apply()
        } catch (_: Exception) {
        }
    }

    fun clear(ctx: Context, id: String) {
        try { prefs(ctx).edit().remove("p_$id").apply() } catch (_: Exception) {}
    }
}

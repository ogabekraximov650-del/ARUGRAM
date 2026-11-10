package uz.arumediatv.com.vd

import android.content.Context
import androidx.media3.common.C
import androidx.media3.common.util.UnstableApi
import androidx.media3.extractor.SeekMap

/**
 * SIRPANUVCHI OYNA (eski tizimdagi `player_source.rs` kabi): ko'rish paytida telefonda doim "ijro joyi .. +1 daqiqa" oralig'i tayyor turadi.
 * Yetishmayotgan 1 MiB bo'laklar fonda 4 tadan to'plab (bitta so'rovda) olinadi ([VdBlocks] — pleyer bilan bir bo'lak ikki marta yuklanmaydi); oyna to'lsa
 * hech narsa yuklanmaydi. "1 daqiqa" aniq: vaqt -> bayt videoning o'z jadvalidan (`moov`, pleyer o'qigan [SeekMap]) olinadi, o'rtacha bitreytdan emas
 * (aks holda tinch sahnada 4-5 daqiqa yuklanib ketardi).
 */
@UnstableApi
object VdAhead {
    const val AHEAD_MS = 60_000L
    /** Bitta so'rovda olinadigan bo'laklar soni (worker so'rovlari 4 baravar kam; oldinda ba'zan 1 daqiqadan <= 3 MB ko'proq turadi). */
    private const val RUN = 4

    private class Target(val ctx: Context, val name: String, val total: Long, val key: ByteArray, val from: Long, val to: Long)

    private val lock = Object()
    private var target: Target? = null
    private var worker: Thread? = null

    @Volatile
    private var map: Pair<String, SeekMap>? = null

    /** Pleyer videoning jadvalini o'qidi (har ochilishda). */
    fun setSeekMap(name: String, m: SeekMap) {
        map = name to m
    }

    /** Ijro joyi o'zgardi: oyna shu joyga suriladi. Jadval hali o'qilmagan bo'lsa hech narsa qilinmaydi (pleyer o'zi so'rab oladi). */
    fun onPosition(ctx: Context, name: String, total: Long, key: ByteArray, posMs: Long) {
        val m = map?.takeIf { it.first == name }?.second ?: return
        if (!m.isSeekable || total <= 0) return
        val posUs = posMs.coerceAtLeast(0) * 1000
        val endUs = posUs + AHEAD_MS * 1000
        val (start, end) = try {
            val s = m.getSeekPoints(posUs).first.position
            val dur = m.durationUs
            val e = if (dur != C.TIME_UNSET && endUs >= dur) total - 1 else m.getSeekPoints(endUs).second.position
            s to maxOf(s, e)
        } catch (_: Exception) {
            return
        }
        val last = (total - 1) / BlockCache.BLOCK
        val from = (start / BlockCache.BLOCK).coerceIn(0, last)
        val to = (end / BlockCache.BLOCK).coerceIn(from, last)
        synchronized(lock) {
            val cur = target
            if (cur != null && cur.name == name && cur.from == from && cur.to == to) return
            target = Target(ctx.applicationContext, name, total, key, from, to)
            if (worker == null) worker = Thread({ loop() }, "vd-ahead").apply { isDaemon = true; start() }
            lock.notifyAll()
        }
    }

    /** Pleyer yopildi yoki boshqa fayl ochildi. */
    fun stop(name: String?) {
        synchronized(lock) {
            if (name == null || target?.name == name) target = null
        }
        if (name == null || map?.first == name) map = null
    }

    private fun loop() {
        var idle: Target? = null // shu oyna to'liq — yangi oyna kelguncha kutiladi
        var fails = 0
        while (true) {
            val t = synchronized(lock) {
                while (target == null || target === idle) lock.wait()
                target!!
            }
            val idx = (t.from..t.to).firstOrNull { !BlockCache.has(t.ctx, t.name, it) }
            if (idx == null) {
                idle = t
                fails = 0
                continue
            }
            try {
                VdBlocks.getRun(t.ctx, t.name, t.total, t.key, idx, RUN)
                fails = 0
            } catch (_: Exception) {
                // Tarmoq yo'q/xato: biroz kutib qayta urinadi (pleyerga ta'sir qilmaydi — u o'zi ham so'rab oladi).
                fails++
                synchronized(lock) { lock.wait(minOf(15_000L, 2_000L * fails)) }
            }
        }
    }
}

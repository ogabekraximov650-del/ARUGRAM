package uz.arumediatv.com.vd

import android.content.Context
import androidx.media3.common.util.UnstableApi
import androidx.media3.extractor.SeekMap

/**
 * UZ: Keyingi oynani oldindan olishning sof qoidasi (test qilinadi). Ijro joyidan boshlab keshda KETMA-KET turgan (hali ko'rilmagan) baytlar
 * [LOW_WATER] bo'lakdan (10 x 1 MiB = 10 MiB) kamaysa — birinchi yetishmayotgan bo'lakdan KANALDAGI 18 MiB oyna oxirigacha bitta so'rovda olinadi.
 * EN: pure prefetch rule: when fewer than 10 MiB of consecutive unplayed blocks are cached ahead of the playback position, fetch from the first missing block to the end of its 18 MiB window.
 */
object AheadPlan {
    /** Zaxira chegarasi: bo'lak (1 MiB) soni. Foydalanuvchi talabi: 10 MB. */
    const val LOW_WATER = 10

    /** Bir oynadagi 1 MiB bo'laklar soni (18 MiB). */
    const val PER_WIN = (VdCtr.CHUNK / BlockCache.BLOCK).toInt()

    /**
     * @param cur ijro joyidagi bo'lak raqami; @param last faylning oxirgi bo'lagi; @param has bo'lak telefonda bormi.
     * @return (birinchi yetishmayotgan bo'lak, olinadigan bo'laklar soni) yoki null — hozir hech narsa olinmaydi.
     */
    fun next(cur: Long, last: Long, has: (Long) -> Boolean): Pair<Long, Int>? {
        if (last < 0) return null
        val from = cur.coerceIn(0, last)
        var m = from
        while (m <= last && m - from < LOW_WATER && has(m)) m++
        if (m > last || m - from >= LOW_WATER) return null // fayl oxiri yoki zaxira yetarli
        val winEnd = minOf((m / PER_WIN + 1) * PER_WIN - 1, last)
        return m to (winEnd - m + 1).toInt()
    }
}

/**
 * SIRPANUVCHI OYNA: ko'rish paytida telefonda ijro joyidan oldinda kamida 10 MiB tayyor turadi ([AheadPlan]). Zaxira 10 MiB dan kamaysa keyingi
 * kanal oynasi (18 MiB, birinchi yetishmayotgan bo'lakdan oyna oxirigacha) BITTA so'rovda, oqim bilan olinadi ([VdBlocks.getRun] — pleyer bilan bir
 * bo'lak ikki marta yuklanmaydi). Surib yuborilsa u joyda keshda hech narsa yo'q — oyna darhol o'sha joydan olinadi (alohida bekor qilish kerak emas).
 * Ijro joyi (vaqt -> bayt) videoning o'z jadvalidan ([SeekMap]) olinadi.
 */
@UnstableApi
object VdAhead {
    private class Target(val ctx: Context, val name: String, val total: Long, val key: ByteArray, val cur: Long)

    private val lock = Object()
    private var target: Target? = null
    private var worker: Thread? = null

    @Volatile
    private var map: Pair<String, SeekMap>? = null

    /** Pleyer videoning jadvalini o'qidi (har ochilishda). */
    fun setSeekMap(name: String, m: SeekMap) {
        map = name to m
    }

    /** Ijro joyi o'zgardi (pleyer ~har soniyada). Jadval hali o'qilmagan bo'lsa hech narsa qilinmaydi (pleyer o'zi so'rab oladi). */
    fun onPosition(ctx: Context, name: String, total: Long, key: ByteArray, posMs: Long) {
        val m = map?.takeIf { it.first == name }?.second ?: return
        if (!m.isSeekable || total <= 0) return
        val start = try {
            m.getSeekPoints(posMs.coerceAtLeast(0) * 1000).first.position
        } catch (_: Exception) {
            return
        }
        val last = (total - 1) / BlockCache.BLOCK
        val cur = (start / BlockCache.BLOCK).coerceIn(0, last)
        synchronized(lock) {
            val t = target
            if (t != null && t.name == name && t.cur == cur) return
            target = Target(ctx.applicationContext, name, total, key, cur)
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
        var idle: Target? = null // shu joyda zaxira yetarli — ijro joyi o'zgarguncha kutiladi
        var fails = 0
        while (true) {
            val t = synchronized(lock) {
                while (target == null || target === idle) lock.wait()
                target!!
            }
            val last = (t.total - 1) / BlockCache.BLOCK
            val plan = AheadPlan.next(t.cur, last) { BlockCache.has(t.ctx, t.name, it) }
            if (plan == null) {
                idle = t
                fails = 0
                continue
            }
            try {
                VdBlocks.getRun(t.ctx, t.name, t.total, t.key, plan.first, plan.second)
                fails = 0
            } catch (_: Exception) {
                // Tarmoq yo'q/xato: biroz kutib qayta urinadi (pleyerga ta'sir qilmaydi — u o'zi ham so'rab oladi).
                fails++
                synchronized(lock) { lock.wait(minOf(15_000L, 2_000L * fails)) }
            }
        }
    }
}

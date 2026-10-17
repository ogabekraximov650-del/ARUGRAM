package uz.arumediatv.com.core

import android.app.ActivityManager
import android.content.Context

/**
 * Qurilma kuchsizmi: operativ xotira kam (<= 3 GB) yoki tizim o'zi "low RAM" deb belgilagan.
 * Kuchsiz qurilmada og'ir bezaklar o'chadi, rasmlar kichikroq va yengilroq formatda dekodlanadi, ko'rinmaydigan sahifalar oldindan chizilmaydi.
 */
object Perf {
    @Volatile
    var low: Boolean = false
        private set

    fun init(ctx: Context) {
        low = try {
            val am = ctx.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
            val mi = ActivityManager.MemoryInfo().also { am.getMemoryInfo(it) }
            am.isLowRamDevice || mi.totalMem in 1..3_300_000_000L
        } catch (_: Throwable) {
            false
        }
    }
}

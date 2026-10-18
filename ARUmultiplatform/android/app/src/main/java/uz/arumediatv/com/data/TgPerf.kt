package uz.arumediatv.com.data

import android.app.ActivityManager
import android.content.Context
import android.os.Build

/**
 * Telefon quvvatiga qarab animatsiya darajasi (Telegram ham kuchsiz telefonda panelni soddalashtiradi):
 * 0 — kuchsiz, 1 — o'rta, 2 — kuchli.
 */
object TgPerf {
    @Volatile var tier = 1
        private set

    fun init(ctx: Context) {
        val am = ctx.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
        val cores = Runtime.getRuntime().availableProcessors()
        tier = when {
            am.isLowRamDevice || am.memoryClass <= 128 || (cores <= 4 && Build.VERSION.SDK_INT < 29) -> 0
            am.memoryClass >= 256 && cores >= 8 -> 2
            else -> 1
        }
    }

    /** Bir vaqtda animatsiyali qatorlar soni (emoji va stikerlarda). */
    val rows: Int get() = if (tier == 0) 1 else 2
    /** Bir vaqtda video dekoderlar soni (faqat .webm uchun). */
    val videoSlots: Int get() = when (tier) { 0 -> 1; 1 -> 3; else -> 5 }
    /** Bir vaqtda aylantirish ishlari. */
    val convertJobs: Int get() = if (tier == 0) 1 else 2
    val tgsSize: Int get() = when (tier) { 0 -> 128; 1 -> 144; else -> 160 }
    val tgsFps: Int get() = when (tier) { 0 -> 12; 1 -> 15; else -> 20 }
    val tgsMaxFrames: Int get() = when (tier) { 0 -> 36; 1 -> 48; else -> 60 }
    val gifMaxSide: Int get() = when (tier) { 0 -> 200; 1 -> 240; else -> 288 }
    val gifMaxFrames: Int get() = when (tier) { 0 -> 36; 1 -> 48; else -> 60 }
}

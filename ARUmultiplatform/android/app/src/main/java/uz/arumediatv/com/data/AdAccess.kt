package uz.arumediatv.com.data

import android.content.Context
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * UZ: Reklama orqali ko'rish ruxsati. Mukofotli reklama oxirigacha ko'rilsa, shu hisob 60 daqiqa davomida istalgan animeni ko'ra oladi; vaqt tugagach pleyer pauzaga qo'yiladi.
 * Ruxsat telefonda hisob bo'yicha saqlanadi (ilova yopilsa ham 60 daqiqa davom etadi). Soat orqaga surilsa (`now < grantedAt`) ruxsat O'CHADI.
 * EN: ad-based access: finishing a rewarded ad unlocks 60 minutes of viewing for this account; when it runs out the player is paused. Stored per account; setting the clock back revokes it.
 * RU: dostup za prosmotr reklamy: 60 minut na akkaunt; po istechenii pleer ostanavlivaetsya.
 */
object AdAccess {
    const val WINDOW_MS = 60 * 60_000L
    private const val PREFS = "aru_ad_access"

    private val _grantedAt = MutableStateFlow(0L)
    /** Oxirgi reklama ko'rilgan vaqt (ms); 0 — yo'q. */
    val grantedAt: StateFlow<Long> = _grantedAt.asStateFlow()

    /** Sof mantiq (testlanadi): ruxsat shu paytda faolmi. */
    fun active(grantedAt: Long, now: Long): Boolean = grantedAt > 0 && now >= grantedAt && now < grantedAt + WINDOW_MS

    /** Qolgan vaqt (ms); faol bo'lmasa 0. */
    fun remainingMs(grantedAt: Long, now: Long): Long = if (active(grantedAt, now)) grantedAt + WINDOW_MS - now else 0L

    private fun key(userId: Long) = "g_$userId"

    /** Hisob o'zgarganda/ilova ochilganda: shu hisobning saqlangan ruxsati yuklanadi. */
    fun load(ctx: Context, userId: Long) {
        _grantedAt.value = if (userId <= 0) 0L else try {
            ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getLong(key(userId), 0L)
        } catch (_: Exception) {
            0L
        }
    }

    /** Reklama oxirigacha ko'rildi: 60 daqiqa shu zahoti boshlanadi. */
    fun grant(ctx: Context, userId: Long, now: Long = System.currentTimeMillis()) {
        if (userId <= 0) return
        _grantedAt.value = now
        try {
            ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putLong(key(userId), now).apply()
        } catch (_: Exception) {
        }
    }
}

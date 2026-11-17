package uz.arumediatv.com.data

import android.content.Context
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * UZ: Reklama orqali ko'rish vaqti. Mukofotli reklama oxirigacha ko'rilsa, shu hisobning ko'rish muddatiga 1 soat QO'SHILADI (har reklama oldingisining ustiga yig'iladi);
 * muddat tugagach pleyer pauzaga qo'yiladi. Muddat telefonda hisob bo'yicha saqlanadi. Soat orqaga surilsa (oxirgi yozuvdan oldingi vaqt) muddat O'CHADI.
 * EN: ad-based viewing time: each finished rewarded ad ADDS one hour to the account's viewing time (stacks); when it runs out the player is paused. Stored per account; setting the clock back revokes it.
 * RU: vremya prosmotra za reklamu: kazhdyy prosmotr dobavlyaet 1 chas (summiruetsya); po istechenii pleer ostanavlivaetsya.
 */
object AdAccess {
    const val WINDOW_MS = 60 * 60_000L
    private const val PREFS = "aru_ad_access"

    private val _until = MutableStateFlow(0L)
    /** Reklama muddati tugaydigan vaqt (ms); 0 — hech qachon ko'rilmagan. */
    val until: StateFlow<Long> = _until.asStateFlow()

    /** Sof mantiq (testlanadi): muddat shu paytda faolmi. */
    fun active(until: Long, now: Long): Boolean = until > now

    /** Qolgan vaqt (ms); faol bo'lmasa 0. */
    fun remainingMs(until: Long, now: Long): Long = if (active(until, now)) until - now else 0L

    /** Yangi muddat: faol bo'lsa mavjud muddatga, bo'lmasa hozirdan boshlab 1 soat qo'shiladi. */
    fun extended(until: Long, now: Long): Long = maxOf(until, now) + WINDOW_MS

    private fun key(userId: Long) = "u_$userId"
    private fun keyAt(userId: Long) = "s_$userId"

    /** Hisob o'zgarganda/ilova ochilganda: shu hisobning saqlangan muddati yuklanadi. Soat orqaga surilgan bo'lsa — o'chiriladi. */
    fun load(ctx: Context, userId: Long, now: Long = System.currentTimeMillis()) {
        _until.value = if (userId <= 0) 0L else try {
            val p = ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            val u = p.getLong(key(userId), 0L)
            if (now < p.getLong(keyAt(userId), 0L)) 0L else u
        } catch (_: Exception) {
            0L
        }
    }

    /** Reklama oxirigacha ko'rildi: ko'rish muddatiga 1 soat qo'shiladi. */
    fun grant(ctx: Context, userId: Long, now: Long = System.currentTimeMillis()) {
        if (userId <= 0) return
        val nu = extended(_until.value, now)
        _until.value = nu
        try {
            ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putLong(key(userId), nu).putLong(keyAt(userId), now).apply()
        } catch (_: Exception) {
        }
    }
}

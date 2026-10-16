package uz.arugram.soft.data

import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull

/**
 * Bitta bo'lim (`/api/seasons` javobidagi element).
 *
 * Flutter'da bu `Map<String, dynamic>`; maydonlar `int.tryParse('${...}')`
 * bilan o'qiladi, ya'ni son ham, matn ham kelishi mumkin. Bu yerda ham
 * xuddi shunday yumshoq o'qiladi.
 */
data class Season(
    val animeId: Int,
    val seasonId: Int,
    val nomi: String,
    val photoUrl: String?,
    val bolimId: Int,
    val epizodCount: Int,
    val ratingCount: Int,
    val ratingSum: Int,
    val viewsTotal: Long,
    val yosh: Int,
    /** `null` — server aytmagan (belgi chiqmaydi). */
    val free: Boolean?,
) {
    val rating: Double get() = if (ratingCount > 0) ratingSum.toDouble() / ratingCount else 0.0

    /**
     * Pullikmi (`seasonIsPaid`): faqat server `free: false` desa.
     * Ma'lumot yo'q bo'lsa belgi chiqmaydi — bepul bo'limga noto'g'ri
     * "Pullik" yozilib qolmasin.
     */
    val isPaid: Boolean get() = free == false

    val key: String get() = "$animeId:$seasonId"

    companion object {
        fun from(o: JsonObject): Season {
            fun text(k: String): String? = (o[k] as? JsonPrimitive)?.contentOrNull
            fun int(k: String): Int = text(k)?.trim()?.toIntOrNull() ?: 0
            fun long(k: String): Long = text(k)?.trim()?.toLongOrNull() ?: 0L
            return Season(
                animeId = int("anime_id"),
                seasonId = int("season_id"),
                nomi = text("nomi").orEmpty(),
                photoUrl = text("photo_url")?.takeIf { it.isNotEmpty() },
                bolimId = int("bolim_id"),
                epizodCount = int("epizod_count"),
                ratingCount = int("rating_count"),
                ratingSum = int("rating_sum"),
                viewsTotal = long("views_total"),
                yosh = int("yosh"),
                free = (o["free"] as? JsonPrimitive)?.booleanOrNull,
            )
        }
    }
}

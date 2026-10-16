package uz.arumediatv.com.data

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject

/** Anime bo'limi (`GET /api/seasons` qatori). Server ba'zi maydonlarni son, ba'zilarini matn qilib beradi — hammasi bardoshli o'qiladi. */
data class Season(
    val animeId: Long,
    val seasonId: Long,
    val name: String,
    val sectionNo: Int,
    val episodes: Int,
    val photoUrl: String,
    val ratingAvg: Double,
    val ratingCount: Int,
    val views: Long,
    val age: Int,
    val genres: List<String>,
    val year: String,
    /** `null` — server aytmagan. */
    val free: Boolean?,
) {
    val key: String get() = "$animeId:$seasonId"
}

private fun JsonObject.text(key: String): String {
    val v = this[key]
    return if (v is JsonPrimitive) v.contentOrNull ?: "" else ""
}

private fun JsonObject.long(key: String): Long {
    val t = text(key)
    return t.toLongOrNull() ?: t.toDoubleOrNull()?.toLong() ?: 0L
}

private fun JsonObject.flag(key: String): Boolean? {
    val v = this[key]
    return if (v is JsonPrimitive) v.booleanOrNull else null
}

fun JsonElement.toSeason(): Season? {
    val o = this as? JsonObject ?: return null
    val animeId = o.long("anime_id")
    val seasonId = o.long("season_id")
    if (animeId == 0L && seasonId == 0L) return null
    val ratingCount = o.long("rating_count").toInt()
    val ratingSum = o.long("rating_sum")
    return Season(
        animeId = animeId,
        seasonId = seasonId,
        name = o.text("nomi"),
        sectionNo = o.long("bolim_id").toInt(),
        episodes = o.long("epizod_count").toInt(),
        photoUrl = o.text("photo_url"),
        ratingAvg = if (ratingCount > 0) ratingSum.toDouble() / ratingCount else 0.0,
        ratingCount = ratingCount,
        views = o.long("views_total"),
        age = o.long("yosh").toInt(),
        genres = o.text("janri").split(',').map { it.trim() }.filter { it.isNotEmpty() },
        year = o.text("yili").trim(),
        free = o.flag("free"),
    )
}

object SeasonParser {
    private val json = Json { ignoreUnknownKeys = true; isLenient = true }

    /** Server javobi (JSON massiv) -> ro'yxat. Buzuq qatorlar o'tkazib yuboriladi. */
    fun parse(body: String): List<Season> {
        val arr: JsonArray = json.parseToJsonElement(body).jsonArray
        return arr.mapNotNull { it.toSeason() }
    }
}

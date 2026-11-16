package uz.arumediatv.com.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonArray
import org.json.JSONArray
import org.json.JSONObject
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.BuildConfig
import java.io.File

/** Tarjimon (`/api/translators`): logotip, nom va "haqida" matni. */
data class Translator(val id: Long, val name: String, val photoUrl: String, val bio: String, val animeCount: Int = 0)

/** Tarjimon sahifasidagi 4 ta raqam: animelar soni (anime id bo'yicha), ko'rinadigan qismlar, jami ko'rishlar, animelar o'rtacha bahosining o'rtachasi (0 — baholanmagan). */
data class TranslatorStats(val anime: Int = 0, val episodes: Int = 0, val views: Long = 0, val rating: Double = 0.0)

data class TranslatorPage(val translator: Translator, val stats: TranslatorStats, val seasons: List<Season>)

/** Tarjimonning oxirgi qo'shilgan (ro'yxatda eng yuqoridagi) `n` ta animesi: har anime bir marta, bo'limlardan birinchisi. */
fun latestAnime(seasons: List<Season>, trId: Long, n: Int = 2): List<Season> =
    seasons.filter { it.trId == trId }.distinctBy { it.animeId }.take(n)

object TranslatorRepo {
    private const val TTL = 5 * 60_000L
    @Volatile private var cache: List<Translator>? = null
    @Volatile private var at = 0L

    private fun parse(o: JSONObject) = Translator(
        id = o.optLong("id"),
        name = o.optString("name").trim(),
        photoUrl = o.optString("photo_url"),
        bio = o.optString("bio").trim(),
        animeCount = o.optInt("anime_count"),
    )

    private fun file() = File(File(AppServices.appContext.filesDir, "cache").apply { mkdirs() }, "translators.json")

    private fun parseList(text: String): List<Translator> = try {
        val a = JSONArray(text)
        (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map(::parse).filter { it.id > 0 && it.name.isNotEmpty() }
    } catch (_: Exception) {
        emptyList()
    }

    /** Tarjimonlar ro'yxati: 5 daqiqa xotirada; tarmoq yo'q bo'lsa telefondagi oxirgi nusxa. `null` — umuman yo'q. */
    suspend fun list(force: Boolean = false): List<Translator>? = withContext(Dispatchers.IO) {
        val c = cache
        if (!force && c != null && System.currentTimeMillis() - at < TTL) return@withContext c
        val (code, text) = Http.client.callText("${BuildConfig.API_BASE}/api/translators")
        if (code == 200) {
            val l = parseList(text)
            cache = l
            at = System.currentTimeMillis()
            runCatching { file().writeText(text) }
            return@withContext l
        }
        c ?: runCatching { file().takeIf { it.exists() }?.readText()?.let(::parseList) }.getOrNull()
    }

    suspend fun page(id: Long): TranslatorPage? = withContext(Dispatchers.IO) {
        val (code, text) = Http.client.callText("${BuildConfig.API_BASE}/api/translators/$id/page")
        if (code != 200) return@withContext null
        try {
            val j = JSONObject(text)
            val s = j.optJSONObject("stats")
            val seasons = Json { ignoreUnknownKeys = true; isLenient = true }
                .parseToJsonElement((j.optJSONArray("seasons") ?: JSONArray()).toString()).jsonArray.mapNotNull { it.toSeason() }
            TranslatorPage(
                translator = parse(j.getJSONObject("translator")),
                stats = TranslatorStats(s?.optInt("anime") ?: 0, s?.optInt("episodes") ?: 0, s?.optLong("views") ?: 0L, s?.optDouble("rating") ?: 0.0),
                seasons = seasons,
            )
        } catch (_: Exception) {
            null
        }
    }
}

/**
 * UZ: Kartalardagi tarjimon logotipi (`tr_id` -> rasm manzili). Ro'yxat bir marta (xotira/diskdagi nusxa bilan) olinadi.
 * EN: translator logos for list cards (`tr_id` -> photo url), loaded once.
 * RU: logotipy perevodchikov dlya kartochek, zagruzhayutsya odin raz.
 */
object TranslatorLogos {
    private val scope = kotlinx.coroutines.CoroutineScope(kotlinx.coroutines.SupervisorJob() + Dispatchers.Default)
    private val _map = kotlinx.coroutines.flow.MutableStateFlow<Map<Long, String>>(emptyMap())
    val map: kotlinx.coroutines.flow.StateFlow<Map<Long, String>> = _map
    @Volatile private var started = false

    fun ensure() {
        if (started) return
        started = true
        scope.launch {
            val l = TranslatorRepo.list()
            if (l == null) { started = false; return@launch }
            _map.value = l.filter { it.photoUrl.isNotEmpty() }.associate { it.id to it.photoUrl }
        }
    }
}

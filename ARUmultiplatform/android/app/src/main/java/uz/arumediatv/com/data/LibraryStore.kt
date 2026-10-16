package uz.arumediatv.com.data

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import okhttp3.OkHttpClient
import org.json.JSONObject
import uz.arumediatv.com.BuildConfig

/** Tomosha tarixi qatori (`GET /api/history`). */
data class HistoryItem(
    val animeId: Long,
    val seasonId: Long,
    val episodeNo: Int,
    val sectionNo: Int,
    val title: String,
    val photoUrl: String,
    val positionMs: Long,
    val durationMs: Long,
    val views: Int,
    val watchedMs: Long = 0,
    val quality: String = "",
    val epizodId: Long = 0,
    /** Oxirgi ko'rilgan vaqt, millisekund (epoch). */
    val updatedAt: Long,
) {
    val key: String get() = "$animeId:$seasonId:$episodeNo"

    /** 0..1 */
    val progress: Float get() = if (durationMs <= 0) 0f else (positionMs.toFloat() / durationMs).coerceIn(0f, 1f)
}

data class LibraryState(
    val favorites: List<Season> = emptyList(),
    val history: List<HistoryItem> = emptyList(),
    val loading: Boolean = false,
    val loaded: Boolean = false,
    val error: String? = null,
)

/**
 * Kutubxona: Sevimlilar (`GET /api/favorites`) va Tarix (`GET /api/history`). Faqat o'qish — yozuv (tarix, sevimli qo'shish)
 * pleyer va anime sahifasi bilan keladi va u `POST /api/sync` orqali bo'ladi (Turso yozuv qoidasi).
 * Ketma-ket ochishlarda serverga qayta bormaydi: 60 soniya "yangi" hisoblanadi.
 */
class LibraryStore(
    private val auth: AuthService,
    private val client: OkHttpClient = Http.client,
    private val base: String = BuildConfig.API_BASE,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val lock = Mutex()
    private val _state = MutableStateFlow(LibraryState())
    val state: StateFlow<LibraryState> = _state.asStateFlow()

    @Volatile
    private var loadedAt = 0L

    fun load(force: Boolean = false) {
        if (auth.sessionToken == null) return
        if (!force && System.currentTimeMillis() - loadedAt < FRESH_MS) return
        scope.launch {
            if (!lock.tryLock()) return@launch
            try {
                val token = auth.sessionToken ?: return@launch
                _state.update { it.copy(loading = true) }
                val (fav, hist) = coroutineScope {
                    val f = async { client.callJson("$base/api/favorites", bearer = token) }
                    val h = async { client.callJson("$base/api/history", bearer = token) }
                    f.await() to h.await()
                }
                val failed = !fav.ok || !hist.ok
                _state.update { s ->
                    s.copy(
                        favorites = if (fav.ok) parseFavorites(fav.json) else s.favorites,
                        history = if (hist.ok) parseHistory(hist.json) else s.history,
                        loading = false,
                        loaded = true,
                        error = if (failed) (fav.error ?: hist.error ?: "Server bilan bog'lanib bo'lmadi") else null,
                    )
                }
                if (!failed) loadedAt = System.currentTimeMillis()
            } finally {
                lock.unlock()
            }
        }
    }

    /** Pleyerda yurakcha bosildi: ro'yxat server javobini kutmasdan o'zgaradi (60 soniyalik "yangi" belgisi ham tushadi). */
    fun applyFavorite(season: Season, on: Boolean) {
        _state.update { s ->
            val rest = s.favorites.filterNot { it.animeId == season.animeId && it.seasonId == season.seasonId }
            s.copy(favorites = if (on) listOf(season) + rest else rest)
        }
        loadedAt = 0
    }

    /** Hisobdan chiqilganda. */
    fun clear() {
        loadedAt = 0
        _state.value = LibraryState()
    }

    companion object {
        private const val FRESH_MS = 60_000L

        fun parseFavorites(j: JSONObject?): List<Season> {
            if (j == null) return emptyList()
            return try {
                Json.parseToJsonElement(j.toString()).jsonObject["items"]?.jsonArray?.mapNotNull { it.toSeason() }.orEmpty()
            } catch (_: Exception) {
                emptyList()
            }
        }

        fun parseHistory(j: JSONObject?): List<HistoryItem> {
            val arr = j?.optJSONArray("items") ?: return emptyList()
            val out = ArrayList<HistoryItem>(arr.length())
            for (i in 0 until arr.length()) {
                val o = arr.optJSONObject(i) ?: continue
                val title = o.optString("season_name").ifBlank { o.optString("anime_name") }
                out += HistoryItem(
                    animeId = o.optLong("anime_id"),
                    seasonId = o.optLong("season_id"),
                    episodeNo = o.optInt("epizod_number"),
                    sectionNo = o.optInt("bolim_id"),
                    title = title,
                    photoUrl = o.optString("season_photo").ifBlank { o.optString("anime_photo") },
                    positionMs = o.optLong("position_ms"),
                    durationMs = o.optLong("duration_ms"),
                    views = o.optInt("view_count"),
                    watchedMs = o.optLong("watched_ms"),
                    quality = o.optString("last_quality"),
                    epizodId = o.optLong("epizod_id"),
                    updatedAt = o.optLong("updated_at"),
                )
            }
            return out
        }
    }
}

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
import android.content.Context
import okhttp3.OkHttpClient
import java.io.File
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
    /** Qismning ijro manzili va eng baland sifatli fayli (to'xtagan joydagi kadr uchun). */
    val videoUrl: String = "",
    val thumbUrl: String = "",
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
    context: Context,
    private val auth: AuthService,
    private val client: OkHttpClient = Http.client,
    private val base: String = BuildConfig.API_BASE,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val lock = Mutex()
    private val file = File(File(context.filesDir, "cache").apply { mkdirs() }, "library.json")
    private val restoreLock = Any()
    private var restored = false
    private val _state = MutableStateFlow(LibraryState())
    val state: StateFlow<LibraryState> = _state.asStateFlow()

    @Volatile
    private var loadedAt = 0L

    /**
     * Diskdagi nusxa (doimiy xotira): ilova qayta ochilganda Tarix/Sevimlilar serverni kutmasdan darhol chiqadi,
     * keyin server bilan yangilanadi. Boshqa hisob nusxasi o'qilmaydi.
     */
    fun restoreFromDisk() {
        synchronized(restoreLock) {
            if (restored) return
            val uid = auth.user.value?.id ?: return // hisob hali tiklanmagan — keyingi chaqiruvda
            restored = true
            try {
                if (!file.exists()) return
                val j = JSONObject(file.readText())
                if (j.optLong("uid") != uid) return
                val fav = parseFavorites(j.optJSONObject("fav"))
                val hist = parseHistory(j.optJSONObject("hist"))
                _state.update { s ->
                    if (s.loaded) s else s.copy(
                        favorites = if (s.favorites.isEmpty()) fav else s.favorites,
                        history = if (s.history.isEmpty()) hist else s.history,
                    )
                }
            } catch (_: Exception) {
            }
        }
    }

    private fun saveDisk(fav: JSONObject?, hist: JSONObject?) {
        try {
            val uid = auth.user.value?.id ?: return
            val tmp = File(file.parentFile, "library.json.tmp")
            tmp.writeText(JSONObject().put("uid", uid).put("fav", fav ?: JSONObject()).put("hist", hist ?: JSONObject()).toString())
            if (!tmp.renameTo(file)) { file.delete(); tmp.renameTo(file) }
        } catch (_: Exception) {
        }
    }

    fun load(force: Boolean = false) {
        if (auth.sessionToken == null) return
        scope.launch { restoreFromDisk() }
        if (!force && System.currentTimeMillis() - loadedAt < FRESH_MS) return
        scope.launch {
            if (!lock.tryLock()) return@launch
            try {
                val token = auth.sessionToken ?: return@launch
                restoreFromDisk()
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
                if (!failed) {
                    loadedAt = System.currentTimeMillis()
                    saveDisk(fav.json, hist.json)
                }
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
        scope.launch { saveCurrent() }
    }

    /** Tarixdan o'chirish: ro'yxatdan darhol olinadi, serverga SyncQueue orqali boradi. */
    fun removeHistory(h: HistoryItem) {
        _state.update { s -> s.copy(history = s.history.filterNot { it.animeId == h.animeId && it.seasonId == h.seasonId && it.epizodId == h.epizodId }) }
        uz.arumediatv.com.AppServices.sync.hideHistory(h.animeId, h.seasonId, h.epizodId)
        scope.launch { saveCurrent() }
    }

    /** Joriy holatni (tahrirlardan keyin) diskka yozadi. */
    private fun saveCurrent() {
        val s = _state.value
        saveDisk(favoritesJson(s.favorites), historyJson(s.history))
    }

    /** Hisobdan chiqilganda. */
    fun clear() {
        loadedAt = 0
        synchronized(restoreLock) { restored = false }
        try { file.delete() } catch (_: Exception) {}
        _state.value = LibraryState()
    }

    companion object {
        private const val FRESH_MS = 60_000L

        private fun favoritesJson(l: List<Season>): JSONObject {
            val arr = org.json.JSONArray()
            l.forEach { x ->
                arr.put(
                    JSONObject().put("anime_id", x.animeId).put("season_id", x.seasonId).put("nomi", x.name).put("bolim_id", x.sectionNo)
                        .put("epizod_count", x.episodes).put("photo_url", x.photoUrl).put("rating_count", x.ratingCount)
                        .put("rating_sum", Math.round(x.ratingAvg * x.ratingCount)).put("views_total", x.views).put("yosh", x.age)
                        .put("janri", x.genres.joinToString(",")).put("yili", x.year).put("holati", x.status).put("turi", x.kind)
                        .also { o -> x.free?.let { o.put("free", it) } },
                )
            }
            return JSONObject().put("items", arr)
        }

        private fun historyJson(l: List<HistoryItem>): JSONObject {
            val arr = org.json.JSONArray()
            l.forEach { h ->
                arr.put(
                    JSONObject().put("anime_id", h.animeId).put("season_id", h.seasonId).put("epizod_number", h.episodeNo).put("bolim_id", h.sectionNo)
                        .put("season_name", h.title).put("season_photo", h.photoUrl).put("position_ms", h.positionMs).put("duration_ms", h.durationMs)
                        .put("view_count", h.views).put("watched_ms", h.watchedMs).put("last_quality", h.quality).put("epizod_id", h.epizodId)
                        .put("updated_at", h.updatedAt).put("video_url", h.videoUrl).put("thumb_url", h.thumbUrl),
                )
            }
            return JSONObject().put("items", arr)
        }

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
                    videoUrl = o.optString("video_url"),
                    thumbUrl = o.optString("thumb_url"),
                )
            }
            return out
        }
    }
}

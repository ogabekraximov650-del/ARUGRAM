package uz.arugram.soft.data

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import okhttp3.OkHttpClient
import okhttp3.Request
import uz.arugram.soft.BuildConfig
import uz.arugram.soft.rust.RustCore
import java.io.File
import java.util.concurrent.TimeUnit

/** Ekranlar ko'radigan holat (Flutter: `SeasonsRepo` ning `ChangeNotifier`i). */
data class SeasonsState(
    val items: List<Season> = emptyList(),
    val loading: Boolean = false,
    val offline: Boolean = false,
)

/**
 * Bo'limlar ro'yxatining YAGONA manbasi (Flutter: `lib/services/seasons_repo.dart`).
 *
 * Tartib o'sha:
 *  1. diskdagi kesh darhol ko'rsatiladi (Rust keshi, shifrlangan);
 *  2. kesh 5 daqiqadan eski bo'lsa fonda yangilanadi;
 *  3. kesh bo'lmasa — yuklanish aylanasi bilan so'raladi;
 *  4. so'rov `ETag` bilan: o'zgarmagan bo'lsa server 304 qaytaradi.
 *
 * Yozuv yo'q — faqat o'qish (`GET /api/seasons`).
 */
class SeasonsRepo(context: Context) {
    private val dir: File = context.applicationContext.filesDir
    private val animePath = File(dir, "anime_cache.rustbin").path
    private val etagPath = File(dir, "etag_seasons.rustbin").path

    private val http = OkHttpClient.Builder()
        .callTimeout(10, TimeUnit.SECONDS)
        .build()

    private val _state = MutableStateFlow(SeasonsState())
    val state: StateFlow<SeasonsState> = _state.asStateFlow()

    private val seasonsUrl = "${BuildConfig.API_BASE}/api/seasons"

    suspend fun load() {
        val cached = withContext(Dispatchers.IO) { readCache(animePath) }
        if (!cached.isNullOrEmpty()) {
            _state.value = _state.value.copy(items = cached, loading = false)
            if (!withContext(Dispatchers.IO) { RustCore.cacheIsFresh(animePath) }) fetch()
            return
        }
        fetch(showLoading = true)
    }

    suspend fun fetch(showLoading: Boolean = false) {
        if (_state.value.loading) return
        // Server manzili berilmagan bo'lsa so'rov yuborilmaydi.
        if (BuildConfig.API_BASE.isBlank()) {
            _state.value = _state.value.copy(loading = false, offline = true)
            return
        }
        if (showLoading) _state.value = _state.value.copy(loading = true)
        try {
            val items = withContext(Dispatchers.IO) { request() }
            _state.value = _state.value.copy(
                items = items ?: _state.value.items,
                offline = false,
            )
        } catch (_: Exception) {
            _state.value = _state.value.copy(offline = true)
        }
        _state.value = _state.value.copy(loading = false)
    }

    /** Yuqoriga tortish: keshni tozalab, qayta so'raydi. */
    suspend fun refresh() {
        withContext(Dispatchers.IO) { RustCore.cacheClear(animePath) }
        fetch()
    }

    // ── Tarmoq ──────────────────────────────────────────────

    /** `null` — o'zgarmagan yoki javob 200 emas. */
    private fun request(): List<Season>? {
        val saved = readEtag()
        val req = Request.Builder().url(seasonsUrl).apply {
            if (saved != null) header("If-None-Match", saved.first)
        }.build()
        http.newCall(req).execute().use { r ->
            val body = when {
                r.code == 304 && saved != null -> saved.second
                r.code == 200 -> r.body?.string() ?: return null
                else -> return null
            }
            val arr = Json.parseToJsonElement(body).jsonArray
            val items = arr.map { Season.from(it.jsonObject) }
            if (r.code == 200) {
                RustCore.cacheSave(animePath, body)
                r.header("ETag")?.takeIf { it.isNotEmpty() }?.let { saveEtag(it, body) }
            }
            return items
        }
    }

    // ── Kesh (Rust orqali; .so yo'q bo'lsa — kesh yo'q) ─────

    private fun readCache(path: String): List<Season>? = try {
        RustCore.cacheGet(path)?.let { json ->
            Json.parseToJsonElement(json).jsonArray.map { Season.from(it.jsonObject) }
        }
    } catch (_: Exception) {
        null
    }

    /** `ETag` va javob matni: Flutter'dagi `EtagHttp` bilan bir xil shakl `[{"e":..,"b":..}]`. */
    private fun readEtag(): Pair<String, String>? = try {
        val first = RustCore.cacheGet(etagPath)
            ?.let { Json.parseToJsonElement(it).jsonArray.firstOrNull()?.jsonObject }
        val e = first?.get("e")?.jsonPrimitive?.content
        val b = first?.get("b")?.jsonPrimitive?.content
        if (!e.isNullOrEmpty() && b != null) e to b else null
    } catch (_: Exception) {
        null
    }

    private fun saveEtag(etag: String, body: String) {
        val arr: JsonArray = buildJsonArray {
            add(buildJsonObject {
                put("e", etag)
                put("b", body)
            })
        }
        RustCore.cacheSave(etagPath, arr.toString())
    }
}


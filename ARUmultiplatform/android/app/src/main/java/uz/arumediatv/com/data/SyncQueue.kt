package uz.arumediatv.com.data

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import okhttp3.OkHttpClient
import org.json.JSONArray
import org.json.JSONObject
import uz.arumediatv.com.BuildConfig
import uz.arumediatv.com.core.SecureStore
import java.io.File
import java.util.UUID

enum class SyncResult { DONE, OFFLINE, NO_ACCOUNT }

/**
 * YAGONA YOZUV NAVBATI (Flutter `SyncQueue` ga mos; CLAUDE.md: "Turso yozuvlari pul turadi").
 *
 * 1. Hamma yozuv avval telefonda (ekranda o'zgarish darhol ko'rinadi), serverga keyin bitta paket bo'lib ketadi (`POST /api/sync`).
 * 2. Navbat siqiladi: bir xil `key` — eskisi almashtiriladi (50 marta bosilgan tugma navbatda 1 ta qator).
 * 3. Yuborish: yangi yozuvdan 1 daqiqa keyin (shu orada kelganlar ham shu paketga tushadi), ilova fonga ketganda — darhol;
 *    ikki paket orasida kamida 2 daqiqa. Navbat diskda shifrlangan holda turadi — ilova yopilsa ham yo'qolmaydi.
 * 4. Har paketda bir martalik `batch_id`: javob yo'lda yo'qolib qayta yuborilsa, worker ikki marta sanamaydi.
 */
class SyncQueue(
    context: Context,
    private val auth: AuthService,
    private val client: OkHttpClient = Http.client,
    private val base: String = BuildConfig.API_BASE,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val file = File(File(context.filesDir, "cache").apply { mkdirs() }, "sync_queue.bin")
    private val sending = Mutex()
    private val rows = ArrayList<JSONObject>()
    private var loaded = false
    private var flushJob: Job? = null

    @Volatile
    private var lastSentAt = 0L

    /** `TrafficMeter` ulaydi: serverga yuboriladigan trafik (bayt) va yuborilgach ayirish. */
    var trafficPending: () -> Long = { 0L }
    var onTrafficReported: (Long) -> Unit = {}

    object Kind {
        const val HISTORY = "history"
        const val RATING = "rating"
        const val FAVORITE = "favorite"
        const val SETTINGS = "settings"
        const val PACK = "pack"
    }

    val hasPendingSettings: Boolean get() = synchronized(rows) { load(); rows.any { it.optString("kind") == Kind.SETTINGS } }

    val pendingCount: Int get() = synchronized(rows) { load(); rows.size }

    /** Hali yuborilmagan sevimli holatlari (`"anime:season"` -> yoqilgan/o'chirilgan). */
    fun pendingFavorites(): Map<String, Boolean> = synchronized(rows) {
        load()
        rows.filter { it.optString("kind") == Kind.FAVORITE }.associate {
            val d = it.optJSONObject("data")
            "${d?.optLong("anime_id")}:${d?.optLong("season_id")}" to (d?.optBoolean("on") == true)
        }
    }

    /** Hali yuborilmagan baholar. */
    fun pendingRatings(): Map<String, Int> = synchronized(rows) {
        load()
        rows.filter { it.optString("kind") == Kind.RATING }.associate {
            val d = it.optJSONObject("data")
            "${d?.optLong("anime_id")}:${d?.optLong("season_id")}" to (d?.optInt("stars") ?: 0)
        }
    }

    /** Emoji/GIF/stiker to'plami amali (`p:new:<id>`, `p:add:<fayl>`, `p:rm:<to'plam>:<element>`, `p:del:<id>`, `p:sub:<id>`, `p:clear:<id>`). */
    fun putPack(key: String, data: JSONObject) = put(Kind.PACK, key, data)

    /** Hali yuborilmagan to'plam amallari. */
    fun pendingPacks(): List<JSONObject> = synchronized(rows) {
        load()
        rows.filter { it.optString("kind") == Kind.PACK }.mapNotNull { it.optJSONObject("data") }
    }

    /** Navbatdan bitta yozuvni (kalit bo'yicha) olib tashlaydi. */
    fun removeKey(key: String) {
        synchronized(rows) {
            load()
            if (rows.removeAll { it.optString("key") == key }) save()
        }
    }

    fun putSettings(hidden: List<String>, consent: Boolean) =
        put(Kind.SETTINGS, "settings", JSONObject().put("hidden_stats", JSONArray(hidden)).put("chan_consent", consent))

    fun putFavorite(animeId: Long, seasonId: Long, on: Boolean) {
        if (animeId <= 0) return
        put(Kind.FAVORITE, "f:$animeId:$seasonId", JSONObject().put("anime_id", animeId).put("season_id", seasonId).put("on", on).put("updated_at", now()))
    }

    fun putRating(animeId: Long, seasonId: Long, stars: Int) {
        if (animeId <= 0 || stars !in 1..10) return
        put(Kind.RATING, "r:$animeId:$seasonId", JSONObject().put("anime_id", animeId).put("season_id", seasonId).put("stars", stars).put("updated_at", now()))
    }

    /** Tarixdan yashirish (serverda yozuv o'chirilmaydi, faqat belgilanadi). */
    fun hideHistory(animeId: Long, seasonId: Long, episodeId: Long) {
        if (animeId <= 0 || episodeId <= 0) return
        put(
            Kind.HISTORY, "h:$animeId:$seasonId:$episodeId",
            JSONObject().put("anime_id", animeId).put("season_id", seasonId).put("epizod_id", episodeId).put("deleted", true).put("updated_at", now()),
            op = "delete",
        )
    }

    /** Tomosha tarixi yozuvi (pleyer keladi). `newView` belgisi siqilganda yo'qolmaydi. */
    fun putHistory(animeId: Long, seasonId: Long, episodeId: Long, videoUrl: String, quality: String, positionMs: Long, durationMs: Long, watchedMs: Long, newView: Boolean) {
        if (animeId <= 0 || episodeId <= 0) return
        val key = "h:$animeId:$seasonId:$episodeId"
        val prevNew = synchronized(rows) {
            load()
            rows.firstOrNull { it.optString("key") == key }?.let { it.optString("op") == "set" && it.optJSONObject("data")?.optBoolean("new_view") == true } == true
        }
        put(
            Kind.HISTORY, key,
            JSONObject().put("anime_id", animeId).put("season_id", seasonId).put("epizod_id", episodeId)
                .put("video_url", videoUrl).put("last_quality", quality)
                .put("position_ms", positionMs).put("duration_ms", durationMs).put("watched_ms", watchedMs)
                .put("new_view", newView || prevNew).put("updated_at", now()),
        )
    }

    /** Hisobdan chiqilganda (navbat boshqa odamniki bo'lib qolmasin). */
    fun clear() {
        synchronized(rows) {
            rows.clear()
            loaded = true
            runCatching { file.delete() }
        }
        flushJob?.cancel()
    }

    /** Ilova fonga ketganda — darhol yuboradi. */
    fun flushAsync(force: Boolean = true) {
        scope.launch { flush(force) }
    }

    /** `force = false` — 2 daqiqalik oraliqni hurmat qiladi. */
    suspend fun flush(force: Boolean = true): SyncResult {
      return sending.withLock {
        val token = auth.sessionToken ?: return@withLock SyncResult.NO_ACCOUNT
        if (!force) {
            val wait = MIN_GAP_MS - (System.currentTimeMillis() - lastSentAt)
            if (wait > 0) {
                schedule(wait)
                return@withLock SyncResult.OFFLINE
            }
        }
        var guard = 0
        while ((pendingCount > 0 || trafficPending() > 0) && guard++ < 10) {
            if (!sendOnce(token)) return@withLock SyncResult.OFFLINE
        }
        SyncResult.DONE
      }
    }

    private fun sendOnce(token: String): Boolean {
        // Server paketga cheklov qo'yadi: tarix <= 100, baho/sevimli/to'plam amallari <= 50 (ortig'i butun paketni rad etadi).
        val batch: List<JSONObject> = synchronized(rows) {
            load()
            val cnt = HashMap<String, Int>()
            rows.filter { e ->
                val k = e.optString("kind")
                val n = (cnt[k] ?: 0) + 1
                cnt[k] = n
                n <= (if (k == Kind.HISTORY) 100 else if (k == Kind.SETTINGS) 1 else 50)
            }.take(MAX_ROWS_PER_BATCH)
        }
        if (batch.isEmpty() && trafficPending() <= 0) return true
        val history = JSONArray()
        val ratings = JSONArray()
        val favorites = JSONArray()
        val packOps = JSONArray()
        var settings: JSONObject? = null
        for (e in batch) {
            val d = e.optJSONObject("data") ?: continue
            when (e.optString("kind")) {
                Kind.HISTORY -> history.put(d)
                Kind.RATING -> ratings.put(d)
                Kind.FAVORITE -> favorites.put(d)
                Kind.PACK -> packOps.put(d)
                Kind.SETTINGS -> settings = d
            }
        }
        val traffic = trafficPending()
        val body = JSONObject()
            .put("batch_id", "${System.currentTimeMillis()}-${UUID.randomUUID().toString().take(8)}")
            .put("history", history).put("ratings", ratings).put("favorites", favorites).put("packs", packOps)
            .put("traffic_bytes", traffic)
        settings?.let { body.put("settings", it) }

        val r = client.callJson("$base/api/sync", "POST", body, token)
        lastSentAt = System.currentTimeMillis()
        return when {
            r.ok -> {
                dropSent(batch)
                if (traffic > 0) onTrafficReported(traffic)
                true
            }
            r.code == 401 -> false // sessiya tugagan
            r.code in 400..499 -> {
                // Server rad etdi: abadiy qayta yubormaymiz.
                dropSent(batch)
                false
            }
            else -> false
        }
    }

    /** Yuborilganlarni olib tashlaydi; yuborish paytida YANGILANGAN yozuv (`at` kattaroq) navbatda qoladi. */
    private fun dropSent(batch: List<JSONObject>) {
        val sent = batch.associate { it.optString("key") to it.optLong("at") }
        synchronized(rows) {
            rows.removeAll { e -> sent[e.optString("key")]?.let { e.optLong("at") <= it } == true }
            save()
        }
    }

    private fun put(kind: String, key: String, data: JSONObject, op: String = "set") {
        synchronized(rows) {
            load()
            rows.removeAll { it.optString("key") == key }
            rows += JSONObject().put("kind", kind).put("key", key).put("op", op).put("at", now()).put("data", data)
            while (rows.size > MAX_QUEUE_ROWS) rows.removeAt(0)
            save()
        }
        schedule(FLUSH_DELAY_MS)
    }

    private fun schedule(ms: Long) {
        flushJob?.cancel()
        flushJob = scope.launch {
            delay(ms)
            flush(force = false)
        }
    }

    private fun load() {
        if (loaded) return
        loaded = true
        try {
            if (!file.exists()) return
            val arr = JSONArray(String(SecureStore.open(file.readBytes()), Charsets.UTF_8))
            for (i in 0 until arr.length()) arr.optJSONObject(i)?.let { rows += it }
        } catch (_: Exception) {
            runCatching { file.delete() }
        }
    }

    private fun save() {
        try {
            val arr = JSONArray()
            rows.forEach { arr.put(it) }
            file.writeBytes(SecureStore.seal(arr.toString().toByteArray(Charsets.UTF_8)))
        } catch (_: Exception) {
        }
    }

    private fun now() = System.currentTimeMillis()

    private companion object {
        const val FLUSH_DELAY_MS = 60_000L
        const val MIN_GAP_MS = 120_000L
        const val MAX_ROWS_PER_BATCH = 150
        const val MAX_QUEUE_ROWS = 500
    }
}

package uz.arumediatv.com.data

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonArray
import okhttp3.OkHttpClient
import org.json.JSONArray
import org.json.JSONObject
import uz.arumediatv.com.BuildConfig
import java.io.File

val QUALITIES = listOf("1080p", "720p", "480p", "360p")

/** Bitta qism (`GET /api/epizods/:anime/:season` qatori). */
data class Episode(
    val id: Long,
    val number: Int,
    val name: String,
    val urls: Map<String, String>,
    val sizes: Map<String, Long>,
    /** Intro oraliqlari (millisekund). */
    val intro: List<LongRange>,
    val originVideo: String,
    val views: Long,
) {
    val qualities: List<String> get() = QUALITIES.filter { !urls[it].isNullOrEmpty() }

    fun urlFor(q: String?): String {
        if (q != null) urls[q]?.takeIf { it.isNotEmpty() }?.let { return it }
        return qualities.firstOrNull()?.let { urls[it] }.orEmpty()
    }

    fun qualityOf(url: String): String = QUALITIES.firstOrNull { urls[it] == url }.orEmpty()

    /** Qism kodlanayotgan paytda worker ASL videoni sifat o'rnida beradi — ilovada "Original". */
    fun isOrigin(q: String): Boolean {
        if (originVideo.isEmpty()) return false
        val u = urls[q].orEmpty()
        return u == originVideo || u.endsWith("/$originVideo")
    }

    fun qualityTitle(q: String) = if (isOrigin(q)) "Original" else q

    companion object {
        /** `1:23`, `83` (soniya), `1:02:03` -> millisekund; xato -> 0. */
        fun introMs(raw: String?): Long {
            val t = raw?.trim().orEmpty()
            if (t.isEmpty()) return 0
            if (!t.contains(':')) return (t.toLongOrNull() ?: 0).coerceAtLeast(0) * 1000
            val parts = t.split(':')
            if (parts.size > 3) return 0
            var total = 0L
            for (p in parts) {
                val v = p.trim().toLongOrNull() ?: return 0
                if (v < 0) return 0
                total = total * 60 + v
            }
            return total * 1000
        }

        fun from(o: JSONObject): Episode {
            val urls = QUALITIES.associateWith { o.optString("url_$it") }.filterValues { it.isNotEmpty() }
            val sizes = QUALITIES.associateWith { o.optLong("size_$it") }
            val intro = (1..9 step 2).mapNotNull { i ->
                val a = introMs(o.optString("intro_$i"))
                val b = introMs(o.optString("intro_${i + 1}"))
                if (a > 0 && b > a) a..b else null
            }
            return Episode(
                id = o.optLong("epizod_id"),
                number = o.optInt("epizod_number"),
                name = o.optString("epizod_name"),
                urls = urls, sizes = sizes, intro = intro,
                originVideo = o.optString("origin_video"),
                views = o.optLong("views_total"),
            )
        }
    }
}

/** Bo'lim ma'lumoti (`GET /api/season/:anime/:season`). */
data class SeasonInfo(
    val season: JSONObject,
    val rating: Double,
    val myStars: Int,
    val isFav: Boolean,
) {
    fun long(k: String): Long = season.optLong(k)
    fun str(k: String): String = season.optString(k).takeIf { it != "null" }.orEmpty()
    val views: Long get() = long("views_total")
    val watchMs: Long get() = long("watch_ms_total")
    val favCount: Long get() = long("fav_count")
    val ratingCount: Long get() = long("rating_count")

    fun toJson(): JSONObject = JSONObject().put("season", season).put("rating", rating).put("my_stars", myStars).put("is_fav", isFav)

    companion object {
        fun from(j: JSONObject) = SeasonInfo(
            season = j.optJSONObject("season") ?: JSONObject(),
            rating = j.optDouble("rating", 0.0).let { if (it.isNaN()) 0.0 else it },
            myStars = j.optInt("my_stars"),
            isFav = j.optBoolean("is_fav"),
        )
    }
}

/**
 * Pleyer sahifasi ma'lumotlari: qismlar, bo'lim ma'lumoti, boshqa bo'limlar — hammasi avval DISKDAGI nusxadan (oflayn),
 * keyin serverdan. Baho va sevimli — avval telefonda (`SyncQueue`), serverga bitta paket bo'lib ketadi.
 */
class SeasonRepository(
    context: Context,
    private val auth: AuthService,
    private val sync: () -> SyncQueue,
    private val library: () -> LibraryStore,
    private val client: OkHttpClient = Http.client,
    private val base: String = BuildConfig.API_BASE,
) {
    private val dir = File(context.filesDir, "cache").apply { mkdirs() }

    private fun read(name: String): String? = try {
        File(dir, name).takeIf { it.exists() }?.readText()
    } catch (_: Exception) {
        null
    }

    private fun write(name: String, text: String) {
        try {
            File(dir, name).writeText(text)
        } catch (_: Exception) {
        }
    }

    // ── Qismlar ──
    private fun parseEpisodes(text: String): List<Episode> = try {
        val a = JSONArray(text)
        (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map(Episode::from)
    } catch (_: Exception) {
        emptyList()
    }

    suspend fun cachedEpisodes(a: Long, s: Long): List<Episode>? = withContext(Dispatchers.IO) {
        read("eps_${a}_$s.json")?.let(::parseEpisodes)?.takeIf { it.isNotEmpty() }
    }

    /** `null` — tarmoq xatosi (kesh saqlanib qoladi). */
    suspend fun fetchEpisodes(a: Long, s: Long): List<Episode>? = withContext(Dispatchers.IO) {
        val (code, text) = client.callText("$base/api/epizods/$a/$s")
        if (code != 200) return@withContext null
        val list = parseEpisodes(text)
        write("eps_${a}_$s.json", text)
        list
    }

    // ── Bo'limlar (shu animening boshqa bo'limlari) ──
    private fun parseSeasons(text: String): List<Season> = try {
        Json.parseToJsonElement(text).jsonArray.mapNotNull { it.toSeason() }.sortedBy { it.sectionNo }
    } catch (_: Exception) {
        emptyList()
    }

    suspend fun cachedSeasons(a: Long): List<Season>? = withContext(Dispatchers.IO) {
        read("seasons_anime_$a.json")?.let(::parseSeasons)?.takeIf { it.isNotEmpty() }
    }

    suspend fun fetchSeasons(a: Long): List<Season>? = withContext(Dispatchers.IO) {
        val (code, text) = client.callText("$base/api/seasons/anime/$a")
        if (code != 200) return@withContext null
        write("seasons_anime_$a.json", text)
        parseSeasons(text)
    }

    // ── Bo'lim ma'lumoti, baho, sevimli ──
    private fun key(a: Long, s: Long) = "season_${a}_$s.json"

    fun infoFromDisk(a: Long, s: Long): SeasonInfo? = try {
        read(key(a, s))?.let { withPending(a, s, SeasonInfo.from(JSONObject(it))) }
    } catch (_: Exception) {
        null
    }

    suspend fun loadInfo(a: Long, s: Long): SeasonInfo? = withContext(Dispatchers.IO) {
        val (code, text) = client.callText("$base/api/season/$a/$s", auth.sessionToken)
        if (code != 200) return@withContext infoFromDisk(a, s)
        try {
            val j = JSONObject(text)
            write(key(a, s), text)
            withPending(a, s, SeasonInfo.from(j))
        } catch (_: Exception) {
            infoFromDisk(a, s)
        }
    }

    /** Serverdan kelgan javob ustiga hali yuborilmagan baho/sevimli qo'yiladi (yo'qolib qolmasin). */
    private fun withPending(a: Long, s: Long, info: SeasonInfo): SeasonInfo {
        val k = "$a:$s"
        var out = info
        sync().pendingFavorites()[k]?.let { fav ->
            if (fav != out.isFav) out = withFav(out, fav)
        }
        sync().pendingRatings()[k]?.let { stars ->
            if (stars != out.myStars) out = withStars(out, stars)
        }
        return out
    }

    private fun withFav(i: SeasonInfo, on: Boolean): SeasonInfo {
        val season = JSONObject(i.season.toString())
        val cnt = if (i.isFav == on) i.favCount else (i.favCount + if (on) 1 else -1).coerceAtLeast(0)
        season.put("fav_count", cnt)
        return SeasonInfo(season, i.rating, i.myStars, on)
    }

    /** Server qoidasi bilan bir xil: oddiy o'rtacha (eski baho ayiriladi, yangisi qo'shiladi). */
    private fun withStars(i: SeasonInfo, stars: Int): SeasonInfo {
        val oldCount = i.ratingCount
        val oldSum = if (i.season.has("rating_sum")) i.season.optLong("rating_sum") else Math.round(i.rating * oldCount)
        val count = oldCount + if (i.myStars > 0) 0 else 1
        val sum = oldSum + stars - i.myStars
        val rating = if (count <= 0) 0.0 else Math.round(sum.toDouble() / count * 100) / 100.0
        val season = JSONObject(i.season.toString())
        season.put("rating_count", count)
        season.put("rating_sum", sum)
        return SeasonInfo(season, rating, stars, i.isFav)
    }

    fun rate(a: Long, s: Long, stars: Int, current: SeasonInfo): SeasonInfo {
        val next = withStars(current, stars)
        write(key(a, s), next.toJson().toString())
        sync().putRating(a, s, stars)
        return next
    }

    fun setFavorite(season: Season, on: Boolean, current: SeasonInfo): SeasonInfo {
        val next = withFav(current, on)
        write(key(season.animeId, season.seasonId), next.toJson().toString())
        sync().putFavorite(season.animeId, season.seasonId, on)
        library().applyFavorite(season, on)
        return next
    }
}

/** Qayerda to'xtagani (qism fayli nomi bo'yicha) — keyingi safar shu joydan davom etadi. */
class WatchProgress(context: Context) {
    private val prefs = context.getSharedPreferences("aru_progress", Context.MODE_PRIVATE)

    fun save(url: String, positionMs: Long, durationMs: Long) {
        val n = Names.fileNameOf(url)
        if (n.isEmpty() || durationMs <= 0) return
        prefs.edit().putString(n, "$positionMs/$durationMs").apply()
    }

    /** Tugab qolgan (oxirgi 5 soniya) yoki 3 soniyadan kam bo'lsa 0 — boshidan. */
    fun positionOf(url: String): Long {
        val v = prefs.getString(Names.fileNameOf(url), null) ?: return 0
        val p = v.substringBefore('/').toLongOrNull() ?: return 0
        val d = v.substringAfter('/').toLongOrNull() ?: return 0
        return if (p < 3000 || p > d - 5000) 0 else p
    }

    /** 0..1 (qism ro'yxatidagi chiziq uchun). */
    fun fractionOf(url: String): Float {
        val v = prefs.getString(Names.fileNameOf(url), null) ?: return 0f
        val p = v.substringBefore('/').toLongOrNull() ?: return 0f
        val d = v.substringAfter('/').toLongOrNull() ?: return 0f
        return if (d <= 0) 0f else (p.toFloat() / d).coerceIn(0f, 1f)
    }
}

/** Pleyer manbasi: avval Telegram (mahalliy server), bo'lmasa worker `/api/play/...` (Cloudflare keshidan oqim). */
object PlaySource {
    data class Source(val uri: String, val viaTelegram: Boolean)

    suspend fun resolve(url: String, tg: TelegramService, preferWorker: Boolean = false): Source? {
        if (!preferWorker) tg.playUrl(url)?.let { return Source(it, true) }
        return worker(url)?.let { Source(it, false) }
    }

    fun worker(url: String): String? {
        val mark = "/api/image/"
        val i = url.indexOf(mark)
        if (i < 0) return url.takeIf { it.startsWith("http") }
        val name = url.substring(i + mark.length).substringBefore('?')
        val path = "/api/play/$name"
        val token = uz.arumediatv.com.core.RustCore.playToken(path)
        val host = url.substring(0, i).ifEmpty { BuildConfig.API_BASE }
        return if (token.isEmpty()) "$host$path" else "$host$path?t=$token"
    }
}

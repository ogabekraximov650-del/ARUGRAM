package uz.arumediatv.com.data

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import uz.arumediatv.com.core.RustCore
import java.io.File
import java.net.URLEncoder

/** Bitta video faylining kesh holati (Rust yadrosi hisoblaydi). */
data class VideoStat(
    val total: Long = 0,
    val downloaded: Long = 0,
    val downloading: Boolean = false,
    val queued: Boolean = false,
    val retrying: Boolean = false,
    val speed: Long = 0,
    val streams: Int = 0,
) {
    val ratio: Float get() = if (total > 0) (downloaded.toFloat() / total).coerceIn(0f, 1f) else 0f
    val percent: Int get() = (ratio * 100).toInt()
    val complete: Boolean get() = total > 0 && downloaded >= total
    val speedLabel: String
        get() = when {
            speed <= 0 -> ""
            speed >= 1 shl 20 -> String.format(java.util.Locale.US, "%.1f MB/s", speed / 1048576.0)
            else -> "${speed / 1024} KB/s"
        }

    companion object {
        fun from(o: JSONObject) = VideoStat(
            o.optLong("total"), o.optLong("downloaded"), o.optBoolean("downloading"), o.optBoolean("queued"),
            o.optBoolean("retrying"), o.optLong("speed"), o.optInt("streams"),
        )
    }
}

object Net {
    fun isOnline(ctx: Context): Boolean {
        val cm = ctx.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager ?: return true
        val caps = cm.getNetworkCapabilities(cm.activeNetwork ?: return false) ?: return false
        return caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
    }
}

/** Rust video-kesh serveri: yuklab olish va to'liq yuklangan faylni mahalliy manzil orqali ijro etish. */
class VideoCache(context: Context) {
    val dir = File(context.filesDir, "video_byte_cache").apply { mkdirs() }

    @Volatile
    private var port = 0

    @Synchronized
    fun ensureStarted(): Int {
        if (port > 0) return port
        port = RustCore.videoCacheStart(dir.absolutePath)
        return port
    }

    fun proxyUri(url: String): String? {
        val p = ensureStarted()
        return if (p > 0) "http://127.0.0.1:$p/v?u=${URLEncoder.encode(url, "UTF-8")}" else null
    }

    fun isComplete(url: String): Boolean = ensureStarted() > 0 && RustCore.videoComplete(url)

    /** Diskdagi jami hajm (baytlarda). */
    fun diskBytes(): Long = dir.walkTopDown().filter { it.isFile }.sumOf { it.length() }

    fun wipe() {
        ensureStarted()
        RustCore.videoWipe()
        runCatching { dir.listFiles()?.forEach { it.deleteRecursively() } }
    }
}

/**
 * Yuklab olish boshqaruvchisi (Flutter `DownloadManager`): holat Rust yadrosidan so'raladi (yuklanayotganda 500 ms,
 * aks holda 2-5 soniya, faqat kuzatilayotgan manzillar uchun). Internet qaytsa faol yuklashlar davom ettiriladi.
 */
class DownloadManager(
    private val context: Context,
    private val tg: TelegramService,
    private val cache: VideoCache,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val _stats = MutableStateFlow<Map<String, VideoStat>>(emptyMap())
    val stats: StateFlow<Map<String, VideoStat>> = _stats.asStateFlow()

    private val watchers = HashMap<Any, Set<String>>()
    private val active = HashSet<String>()
    private var loop: Job? = null
    private var netRegistered = false

    fun statOf(url: String): VideoStat = _stats.value[url] ?: VideoStat()

    fun watch(owner: Any, urls: Set<String>) {
        synchronized(watchers) { watchers[owner] = urls }
        ensureLoop()
    }

    fun unwatch(owner: Any) {
        synchronized(watchers) { watchers.remove(owner) }
    }

    private fun tracked(): Set<String> = synchronized(watchers) { watchers.values.flatten().toSet() + active }

    private fun ensureLoop() {
        if (loop?.isActive == true) return
        loop = scope.launch {
            while (isActive) {
                val urls = tracked().toList()
                if (urls.isEmpty()) {
                    delay(1000)
                    if (tracked().isEmpty()) break else continue
                }
                poll(urls)
                delay(if (_stats.value.values.any { it.downloading }) 500 else if (urls.size <= 16) 2000 else 5000)
            }
        }
    }

    private fun poll(urls: List<String>) {
        if (cache.ensureStarted() <= 0) return
        val raw = RustCore.videoStats(urls)
        val next = HashMap(_stats.value)
        for (u in urls) {
            val o = raw.optJSONObject(u) ?: continue
            val s = VideoStat.from(o)
            next[u] = s
            if (!s.downloading) active.remove(u)
            if (s.complete) tg.unhold("dl:$u")
        }
        if (next != _stats.value) _stats.value = next
    }

    fun download(url: String) {
        if (url.isEmpty()) return
        registerNet()
        active.add(url)
        val cur = statOf(url)
        _stats.value = _stats.value + (url to cur.copy(downloading = true))
        scope.launch {
            if (tg.active) {
                tg.playUrl(url)?.let { tg.hold("dl:$url") }
            }
            cache.ensureStarted()
            RustCore.videoDownload(url)
            poll(listOf(url))
        }
        ensureLoop()
    }

    fun pause(url: String) {
        if (url.isEmpty()) return
        RustCore.videoPause(url)
        active.remove(url)
        tg.unhold("dl:$url")
        _stats.value = _stats.value + (url to statOf(url).copy(downloading = false, queued = false))
    }

    fun delete(url: String) {
        if (url.isEmpty()) return
        RustCore.videoDelete(url)
        active.remove(url)
        tg.unhold("dl:$url")
        _stats.value = _stats.value + (url to statOf(url).copy(downloaded = 0, downloading = false, queued = false))
    }

    fun wipeAll() {
        active.clear()
        cache.wipe()
        _stats.value = emptyMap()
    }

    /** Internet qaytganda faol yuklashlar davom etadi. */
    private fun registerNet() {
        if (netRegistered) return
        netRegistered = true
        try {
            val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
            cm.registerDefaultNetworkCallback(object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) {
                    scope.launch {
                        for (u in active.toList()) {
                            if (tg.active) tg.playUrl(u)?.let { tg.hold("dl:$u") }
                            RustCore.videoDownload(u)
                        }
                    }
                }
            })
        } catch (_: Exception) {
        }
    }
}

/** Yuklanmalar ro'yxatidagi bitta (qism + sifat) qatori. */
data class DownloadItem(
    val animeId: Long,
    val seasonId: Long,
    val sectionNo: Int,
    val episodeId: Long,
    val episodeNo: Int,
    val title: String,
    val poster: String,
    val quality: String,
    val qualityTitle: String,
    val url: String,
    val stat: VideoStat,
) {
    val key: String get() = "$animeId:$seasonId:$episodeId:$quality"
}

/**
 * Diskdagi qismlar ro'yxatidan (`eps_*.json` — ochib ko'rilgan bo'limlar) qaysilari yuklab olingani yoki yuklanayotganini topadi.
 * Nom va poster: bo'limlar ro'yxati / bo'lim ma'lumoti nusxasidan.
 */
class DownloadsIndex(private val context: Context, private val dm: DownloadManager) {
    private val dir = File(context.filesDir, "cache")

    suspend fun collect(seasons: List<Season>): List<DownloadItem> = withContext(Dispatchers.IO) {
        val byKey = seasons.associateBy { it.key }
        val drafts = ArrayList<DownloadItem>()
        val urls = ArrayList<String>()
        val files = dir.listFiles { f -> f.name.startsWith("eps_") && f.name.endsWith(".json") } ?: emptyArray()
        for (f in files) {
            val parts = f.name.removePrefix("eps_").removeSuffix(".json").split('_')
            val a = parts.getOrNull(0)?.toLongOrNull() ?: continue
            val s = parts.getOrNull(1)?.toLongOrNull() ?: continue
            val eps = try {
                val arr = org.json.JSONArray(f.readText())
                (0 until arr.length()).mapNotNull { arr.optJSONObject(it) }.map(Episode::from)
            } catch (_: Exception) {
                continue
            }
            val season = byKey["$a:$s"]
            val infoName = try {
                File(dir, "season_${a}_$s.json").takeIf { it.exists() }?.let { JSONObject(it.readText()).optJSONObject("season") }
            } catch (_: Exception) {
                null
            }
            val title = season?.name ?: infoName?.optString("nomi").orEmpty()
            val poster = season?.photoUrl ?: infoName?.optString("photo_url").orEmpty()
            val section = season?.sectionNo ?: infoName?.optInt("bolim_id") ?: 0
            for (e in eps) for (q in e.qualities) {
                val u = e.urls[q].orEmpty()
                if (u.isEmpty()) continue
                urls += u
                drafts += DownloadItem(a, s, section, e.id, e.number, title, poster, q, e.qualityTitle(q), u, VideoStat())
            }
        }
        if (drafts.isEmpty() || cache().ensureStarted() <= 0) return@withContext emptyList()
        var raw = RustCore.videoStats(urls)
        // Ilova endi ochilgan bo'lsa birinchi javob bo'sh bo'lishi mumkin (disk fon oqimida skanerlanadi) — bir marta kutamiz.
        if (urls.none { (raw.optJSONObject(it)?.optLong("downloaded") ?: 0) > 0 }) {
            delay(1200)
            raw = RustCore.videoStats(urls)
        }
        drafts.mapNotNull { d ->
            val st = raw.optJSONObject(d.url)?.let(VideoStat::from) ?: return@mapNotNull null
            if (st.downloaded > 0 || st.downloading) d.copy(stat = st) else null
        }.sortedWith(compareByDescending<DownloadItem> { it.stat.downloading }.thenBy { it.title }.thenBy { it.episodeNo })
    }

    private fun cache() = uz.arumediatv.com.AppServices.videoCache
}

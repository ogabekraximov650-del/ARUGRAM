package uz.arumediatv.com.data

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import android.content.Context
import okhttp3.OkHttpClient
import uz.arumediatv.com.BuildConfig

/** Shaxsiy statistika (`GET /api/me/stats`, bitta so'rov). */
data class MyStats(
    val animes: Long = 0,
    val episodes: Long = 0,
    val watchMs: Long = 0,
    val traffic: Long = 0,
    val seasons: Long = 0,
    val favorites: Long = 0,
    val rated: Long = 0,
    val comments: Long = 0,
) {
    val watchHours: Double get() = watchMs / 3_600_000.0
}

class MyStatsStore(
    context: Context,
    private val auth: AuthService,
    private val client: OkHttpClient = Http.client,
    private val base: String = BuildConfig.API_BASE,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val _state = MutableStateFlow(MyStats())
    val state: StateFlow<MyStats> = _state.asStateFlow()

    private val prefs = context.getSharedPreferences("aru_mystats", Context.MODE_PRIVATE)

    @Volatile
    private var loadedAt = 0L

    /** Diskdagi oxirgi statistika (hisob bo'yicha) — ekran darhol to'ladi; "yangi" vaqti ham saqlanadi (qayta ochilganda so'rov ketmaydi). */
    private fun restore(uid: Long) {
        if (loadedAt != 0L || _state.value != MyStats()) return
        val raw = prefs.getString("s_$uid", null) ?: return
        try {
            val p = raw.split('|').map { it.toLong() }
            if (p.size < 8) return
            _state.value = MyStats(p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7])
            loadedAt = prefs.getLong("at_$uid", 0)
        } catch (_: Exception) {
        }
    }

    /** 2 daqiqa "yangi" hisoblanadi (ketma-ket ochishlarda serverga bormaydi). */
    fun load(force: Boolean = false) {
        val token = auth.sessionToken ?: return
        auth.user.value?.id?.let { restore(it) }
        if (!force && System.currentTimeMillis() - loadedAt < 120_000L) return
        scope.launch {
            val r = client.callJson("$base/api/me/stats", bearer = token)
            val j = r.json
            if (r.ok && j != null) {
                _state.value = MyStats(
                    animes = j.optLong("animes"), episodes = j.optLong("episodes"), watchMs = j.optLong("watch_ms"),
                    traffic = j.optLong("traffic"), seasons = j.optLong("seasons"), favorites = j.optLong("favorites"),
                    rated = j.optLong("rated"), comments = j.optLong("comments"),
                )
                loadedAt = System.currentTimeMillis()
                auth.user.value?.id?.let { uid ->
                    val m = _state.value
                    prefs.edit().putString("s_$uid", listOf(m.animes, m.episodes, m.watchMs, m.traffic, m.seasons, m.favorites, m.rated, m.comments).joinToString("|"))
                        .putLong("at_$uid", loadedAt).apply()
                }
            }
        }
    }

    fun clear() {
        loadedAt = 0
        _state.value = MyStats()
        prefs.edit().clear().apply()
    }
}

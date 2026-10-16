package uz.arumediatv.com.data

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
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
    private val auth: AuthService,
    private val client: OkHttpClient = Http.client,
    private val base: String = BuildConfig.API_BASE,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val _state = MutableStateFlow(MyStats())
    val state: StateFlow<MyStats> = _state.asStateFlow()

    @Volatile
    private var loadedAt = 0L

    /** 2 daqiqa "yangi" hisoblanadi (ketma-ket ochishlarda serverga bormaydi). */
    fun load(force: Boolean = false) {
        val token = auth.sessionToken ?: return
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
            }
        }
    }

    fun clear() {
        loadedAt = 0
        _state.value = MyStats()
    }
}

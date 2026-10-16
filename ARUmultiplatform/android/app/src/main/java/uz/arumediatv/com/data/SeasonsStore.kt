package uz.arumediatv.com.data

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

data class SeasonsState(
    val items: List<Season> = emptyList(),
    /** Birinchi yuklanish (kesh ham yo'q). */
    val loading: Boolean = true,
    val refreshing: Boolean = false,
    /** Oxirgi so'rov xato bilan tugagan: ro'yxat keshdan. */
    val offline: Boolean = false,
    val error: String? = null,
)

/** Bo'limlar ro'yxatining YAGONA manbasi: Bosh sahifa, Qidiruv va Katalog bir xil ro'yxatni ko'radi (Flutter'dagi `SeasonsRepo` kabi). */
class SeasonsStore(private val repo: SeasonsRepository) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val lock = Mutex()
    private val _state = MutableStateFlow(SeasonsState())
    val state: StateFlow<SeasonsState> = _state.asStateFlow()

    private var started = false

    /** Birinchi chaqiruvda: avval disk keshi (tarmoqsiz), keyin server (o'zgarmagan bo'lsa 304). */
    fun ensureLoaded() {
        if (started) return
        started = true
        scope.launch {
            repo.cached()?.let { c -> _state.update { it.copy(items = c, loading = false) } }
            fetch(force = false, refreshing = false)
        }
    }

    fun refresh() {
        if (_state.value.refreshing) return
        scope.launch {
            _state.update { it.copy(refreshing = true) }
            fetch(force = true, refreshing = true)
        }
    }

    private suspend fun fetch(force: Boolean, refreshing: Boolean) {
        lock.withLock {
            val r = repo.fetch(force)
            _state.update { s ->
                when (r) {
                    is FetchResult.Fresh -> s.copy(items = r.items, loading = false, refreshing = false, offline = false, error = null)
                    FetchResult.NotModified -> s.copy(loading = false, refreshing = false, offline = false, error = null)
                    is FetchResult.Failed -> s.copy(loading = false, refreshing = false, offline = true, error = r.message)
                }
            }
        }
    }
}

/** Katalog va qidiruv uchun sof (testlanadigan) mantiq. */
object SeasonQueries {
    /** Nom bo'yicha, katta-kichik harfga sezgirsiz. Bo'sh so'rov — bo'sh ro'yxat. */
    fun search(all: List<Season>, query: String): List<Season> {
        val q = query.trim().lowercase()
        if (q.isEmpty()) return emptyList()
        return all.filter { it.name.lowercase().contains(q) }
    }

    fun genres(all: List<Season>): List<String> = all.flatMap { it.genres }.toSortedSet().toList()

    fun years(all: List<Season>): List<String> =
        all.map { it.year }.filter { it.isNotEmpty() }.toSet().sortedByDescending { it.toIntOrNull() ?: 0 }

    fun filter(all: List<Season>, genres: Set<String>, years: Set<String>): List<Season> =
        all.filter { s ->
            (genres.isEmpty() || s.genres.any { it in genres }) && (years.isEmpty() || s.year in years)
        }
}

/** Katalog bo'limlari (Flutter `_Tab` bilan bir xil). */
enum class CatalogTab(val label: String) {
    ALL("Barchasi"),
    RATING("Reytingi baland"),
    VIEWS("Eng ko'p ko'rilgan"),
    ONGOING("Ongoing"),
    DONE("Tugallangan"),
    FILMS("Filmlar"),
    OVA("OVA qismlar");

    fun pick(rows: List<Season>): List<Season> =
        when (this) {
            ALL -> rows
            RATING -> rows.filter { it.ratingCount > 0 }.sortedByDescending { it.ratingAvg }
            VIEWS -> rows.filter { it.views > 0 }.sortedByDescending { it.views }
            ONGOING -> rows.filter { it.status == "Davom etmoqda" }
            DONE -> rows.filter { it.status == "Tugallangan" }
            FILMS -> rows.filter { it.kind == "FILM" }
            OVA -> rows.filter { it.kind == "OVA" }
        }
}

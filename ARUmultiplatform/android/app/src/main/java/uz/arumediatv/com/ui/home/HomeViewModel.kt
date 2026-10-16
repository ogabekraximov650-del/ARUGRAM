package uz.arumediatv.com.ui.home

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uz.arumediatv.com.data.FetchResult
import uz.arumediatv.com.data.Season
import uz.arumediatv.com.data.SeasonsRepository

data class HomeState(
    val items: List<Season> = emptyList(),
    /** Birinchi yuklanish (kesh ham yo'q). */
    val loading: Boolean = true,
    /** Pastga tortib yangilash. */
    val refreshing: Boolean = false,
    /** Oxirgi so'rov tarmoq/server xatosi bilan tugagan (ro'yxat keshdan ko'rsatilmoqda). */
    val offline: Boolean = false,
    val error: String? = null,
)

class HomeViewModel(app: Application) : AndroidViewModel(app) {
    private val repo = SeasonsRepository(app)
    private val _state = MutableStateFlow(HomeState())
    val state: StateFlow<HomeState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            // 1) Diskdagi kesh darhol (tarmoqsiz) — ekran bo'sh turmasin.
            repo.cached()?.let { cached -> _state.update { it.copy(items = cached, loading = false) } }
            // 2) Server: o'zgarmagan bo'lsa 304 (Turso'ga bormaydi).
            apply(repo.fetch())
        }
    }

    fun refresh() {
        if (_state.value.refreshing) return
        viewModelScope.launch {
            _state.update { it.copy(refreshing = true) }
            apply(repo.fetch(force = true))
        }
    }

    private fun apply(r: FetchResult) {
        _state.update { s ->
            when (r) {
                is FetchResult.Fresh -> s.copy(items = r.items, loading = false, refreshing = false, offline = false, error = null)
                FetchResult.NotModified -> s.copy(loading = false, refreshing = false, offline = false, error = null)
                is FetchResult.Failed -> s.copy(loading = false, refreshing = false, offline = true, error = r.message)
            }
        }
    }
}

package uz.arumediatv.com.data

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * OFLAYNDA FAQAT YUKLAB OLINGAN QISMLAR (Flutter `OfflineLibrary`): internet yo'q paytda ro'yxatlarda to'liq yuklab olingan qismi
 * bor bo'limlar (va qismlar) ko'rsatiladi, qolganlari yashiriladi — lekin o'chirilmaydi: internet qaytgach hammasi qaytadi.
 */
class OfflineLibrary(private val index: DownloadsIndex) {
    private val _seasons = MutableStateFlow<Set<String>>(emptySet())
    private val _episodes = MutableStateFlow<Set<String>>(emptySet())
    private val _ready = MutableStateFlow(false)

    val seasons: StateFlow<Set<String>> = _seasons.asStateFlow()
    val episodes: StateFlow<Set<String>> = _episodes.asStateFlow()

    /** Indeks yig'ib bo'lingunicha hech narsa yashirilmaydi (ro'yxat bir lahzaga bo'm-bo'sh ko'rinmasin). */
    val ready: StateFlow<Boolean> = _ready.asStateFlow()

    suspend fun refresh(all: List<Season>) {
        val items = try { index.collect(all) } catch (_: Exception) { return }
        val done = items.filter { it.stat.complete }
        _episodes.value = done.map { "${it.animeId}:${it.seasonId}:${it.episodeId}" }.toSet()
        _seasons.value = done.map { "${it.animeId}:${it.seasonId}" }.toSet()
        _ready.value = true
    }

    fun hasSeason(animeId: Long, seasonId: Long) = "$animeId:$seasonId" in _seasons.value
    fun hasEpisode(animeId: Long, seasonId: Long, episodeId: Long) = "$animeId:$seasonId:$episodeId" in _episodes.value
}

package uz.arumediatv.com.data

import org.junit.Assert.assertEquals
import org.junit.Test

class HistoryMergeTest {
    private val now = 10_000_000L
    private fun h(ep: Long, pos: Long, at: Long) = HistoryItem(
        animeId = 1, seasonId = 1, episodeNo = ep.toInt(), sectionNo = 1, title = "A", photoUrl = "", positionMs = pos, durationMs = 5_000_000,
        views = 1, epizodId = ep, updatedAt = at,
    )

    @Test
    fun newerLocalBeatsOlderServer() {
        val r = LibraryStore.mergeHistory(listOf(h(1, 31_000, now - 3_000_000)), listOf(h(1, 2_138_000, now - 1_000)), emptyList(), now)
        assertEquals(2_138_000L, r.single().positionMs)
    }

    @Test
    fun newerServerBeatsOlderLocal() {
        val r = LibraryStore.mergeHistory(listOf(h(1, 900_000, now - 1_000)), listOf(h(1, 31_000, now - 3_000_000)), emptyList(), now)
        assertEquals(900_000L, r.single().positionMs)
    }

    @Test
    fun freshLocalKeptWhenMissingOnServer_staleDropped() {
        val r = LibraryStore.mergeHistory(emptyList(), listOf(h(1, 1000, now - 60_000), h(2, 1000, now - 3_600_000)), emptyList(), now)
        assertEquals(listOf(1L), r.map { it.epizodId })
    }

    @Test
    fun pendingDeleteRemoves() {
        val r = LibraryStore.mergeHistory(listOf(h(1, 1000, now - 1_000)), emptyList(), listOf(SyncQueue.PendingHistory(1, 1, 1, true)), now)
        assertEquals(0, r.size)
    }
}

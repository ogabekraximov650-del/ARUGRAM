package uz.arumediatv.com.data

import org.junit.Assert.assertEquals
import org.junit.Test

class TranslatorsTest {
    private fun s(anime: Long, season: Long, tr: Long) = Season(
        animeId = anime, seasonId = season, name = "A$anime", sectionNo = 1, episodes = 1, photoUrl = "", ratingAvg = 0.0, ratingCount = 0,
        views = 0, age = 0, genres = emptyList(), year = "", free = true, trId = tr,
    )

    @Test
    fun latestTwoDistinctAnimeOfTranslator() {
        // Ro'yxat eng yangidan eskisiga: tarjimon 7 ning animelari 5 (2 ta bo'lim), 4 va 1.
        val all = listOf(s(5, 1, 7), s(5, 2, 7), s(9, 1, 3), s(4, 1, 7), s(1, 1, 7))
        val r = latestAnime(all, 7)
        assertEquals(listOf(5L, 4L), r.map { it.animeId })
        assertEquals(1L, r[0].seasonId) // anime 5 ning birinchi bo'limi
    }

    @Test
    fun fewerThanTwo() {
        assertEquals(1, latestAnime(listOf(s(2, 1, 8)), 8).size)
        assertEquals(0, latestAnime(listOf(s(2, 1, 8)), 9).size)
    }
}

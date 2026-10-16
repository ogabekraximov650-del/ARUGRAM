package uz.arumediatv.com.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SeasonParserTest {
    @Test
    fun serverMaydonlariSonYokiMatnBoLishiMumkin() {
        val body = """[
          {"anime_id": 1, "season_id": "2", "nomi": "Ruhlar olami", "bolim_id": "1", "epizod_count": 12,
           "photo_url": "https://x/y.jpg", "rating_count": "4", "rating_sum": 18, "views_total": "1500",
           "yosh": 12, "janri": "Fantastika, Drama ,", "yili": " 2001 ", "free": true}
        ]"""
        val list = SeasonParser.parse(body)
        assertEquals(1, list.size)
        val s = list[0]
        assertEquals(1L, s.animeId)
        assertEquals(2L, s.seasonId)
        assertEquals("1:2", s.key)
        assertEquals("Ruhlar olami", s.name)
        assertEquals(12, s.episodes)
        assertEquals(4.5, s.ratingAvg, 1e-9)
        assertEquals(1500L, s.views)
        assertEquals(listOf("Fantastika", "Drama"), s.genres)
        assertEquals("2001", s.year)
        assertEquals(true, s.free)
    }

    @Test
    fun buzuqQatorlarOtkazibYuboriladi() {
        val list = SeasonParser.parse("""[1, "x", {"nomi": "id yo'q"}, {"anime_id": 5, "season_id": 1, "nomi": "OK"}]""")
        assertEquals(listOf("OK"), list.map { it.name })
    }

    @Test
    fun reytingYoqBolsaNol() {
        val s = SeasonParser.parse("""[{"anime_id": 1, "season_id": 1}]""").first()
        assertEquals(0.0, s.ratingAvg, 0.0)
        assertNull(s.free)
        assertTrue(s.genres.isEmpty())
    }
}

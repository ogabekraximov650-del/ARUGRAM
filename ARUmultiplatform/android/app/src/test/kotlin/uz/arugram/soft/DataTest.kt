package uz.arugram.soft

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uz.arugram.soft.data.Season
import uz.arugram.soft.data.formatCompact
import uz.arugram.soft.data.formatCount

/** Flutter `lib/services/format.dart` va `seasons_repo.dart` bilan bir xil natija. */
class DataTest {
    @Test fun formatCountUsesDots() {
        assertEquals("843", formatCount(843))
        assertEquals("1.284.512", formatCount(1284512))
        assertEquals("-12.000", formatCount(-12000))
    }

    @Test fun formatCompactMatchesFlutter() {
        assertEquals("843", formatCompact(843))
        assertEquals("12,4 ming", formatCompact(12400))
        assertEquals("12 ming", formatCompact(12000)) // `12,0` emas
        assertEquals("1,3 mln", formatCompact(1284512))
    }

    private fun season(json: String) = Season.from(Json.parseToJsonElement(json).jsonObject)

    @Test fun seasonReadsNumbersAsTextOrInt() {
        val s = season(
            """{"anime_id":"7","season_id":2,"nomi":"Naruto","bolim_id":"3",
               "epizod_count":12,"rating_count":4,"rating_sum":"34","views_total":"1500","yosh":16,
               "photo_url":"https://x/y.jpg","free":false}""",
        )
        assertEquals("7:2", s.key)
        assertEquals(3, s.bolimId)
        assertEquals(8.5, s.rating, 1e-9)
        assertEquals(1500L, s.viewsTotal)
        assertTrue(s.isPaid)
    }

    @Test fun paidOnlyWhenServerSaysSo() {
        assertFalse(season("""{"anime_id":1,"season_id":1}""").isPaid)
        assertFalse(season("""{"anime_id":1,"season_id":1,"free":true}""").isPaid)
        assertTrue(season("""{"anime_id":1,"season_id":1,"free":false}""").isPaid)
    }

    @Test fun noRatingMeansZeroAndNoPhoto() {
        val s = season("""{"anime_id":1,"season_id":1,"photo_url":""}""")
        assertEquals(0.0, s.rating, 0.0)
        assertNull(s.photoUrl)
    }
}

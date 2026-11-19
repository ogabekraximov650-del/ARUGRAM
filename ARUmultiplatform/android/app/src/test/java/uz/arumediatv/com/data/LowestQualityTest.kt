package uz.arumediatv.com.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Qism birinchi marta ochilganda tanlanadigan eng past sifat. */
class LowestQualityTest {
    private fun ep(vararg q: String) = Episode(
        id = 1, number = 1, name = "1", urls = q.associateWith { "https://x/$it.mp4" }, sizes = emptyMap(),
        intro = emptyList(), originVideo = "", views = 0,
    )

    @Test
    fun enPastSifatTanlanadi() {
        assertEquals("360p", ep("1080p", "720p", "480p", "360p").lowestQuality)
        assertEquals("480p", ep("1080p", "480p").lowestQuality)
        assertEquals("1080p", ep("1080p").lowestQuality)
    }

    @Test
    fun videoYoqBolsaNull() {
        assertNull(ep().lowestQuality)
    }
}

package uz.arumediatv.com.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Qism birinchi marta ochilganda tanlanadigan eng yuqori sifat (1080p, bo'lmasa borlarining eng yuqorisi). */
class HighestQualityTest {
    private fun ep(vararg q: String) = Episode(
        id = 1, number = 1, name = "1", urls = q.associateWith { "https://x/$it.mp4" }, sizes = emptyMap(),
        intro = emptyList(), originVideo = "", views = 0,
    )

    @Test
    fun enYuqoriSifatTanlanadi() {
        assertEquals("1080p", ep("1080p", "720p", "480p", "360p").highestQuality)
        assertEquals("720p", ep("720p", "480p").highestQuality)
        assertEquals("360p", ep("360p").highestQuality)
    }

    @Test
    fun videoYoqBolsaNull() {
        assertNull(ep().highestQuality)
    }
}

package uz.arumediatv.com.data

import org.junit.Assert.assertEquals
import org.junit.Test

class LocalOverlayTest {
    private fun s(avg: Double, n: Int, v: Long) = Season(1, 1, "x", 1, 1, "", avg, n, v, 0, emptyList(), "", null)

    @Test fun firstRatingAddsCount() {
        val r = LocalOverlay.rated(s(8.0, 2, 0), 0, 5)
        assertEquals(3, r.ratingCount)
        assertEquals(7.0, r.ratingAvg, 0.001)
    }

    @Test fun changedRatingKeepsCount() {
        val r = LocalOverlay.rated(s(8.0, 2, 0), 6, 10) // 16 -> 20
        assertEquals(2, r.ratingCount)
        assertEquals(10.0, r.ratingAvg, 0.001)
    }

    @Test fun listOverlayApplies() {
        val h = listOf(SyncQueue.PendingHistory(1, 1, 7, false, newView = true))
        val out = LocalOverlay.list(listOf(s(0.0, 0, 2)), mapOf("1:1" to Pair(0, 9)), h)
        assertEquals(3L, out[0].views)
        assertEquals(9.0, out[0].ratingAvg, 0.001)
    }
}

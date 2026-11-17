package uz.arumediatv.com.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AdAccessTest {
    private val t0 = 1_000_000_000L
    private val hour = 60 * 60_000L

    @Test fun firstAdGivesOneHourFromNow() {
        val u = AdAccess.extended(0L, t0)
        assertEquals(t0 + hour, u)
        assertTrue(AdAccess.active(u, t0 + 59 * 60_000L))
        assertFalse(AdAccess.active(u, t0 + hour))
    }

    @Test fun secondAdStacksOnRemainingTime() {
        val first = AdAccess.extended(0L, t0)
        val second = AdAccess.extended(first, t0 + 20 * 60_000L)
        assertEquals(t0 + 2 * hour, second)
    }

    @Test fun adAfterExpiryStartsFromNow() {
        val first = AdAccess.extended(0L, t0)
        val later = t0 + 3 * hour
        assertEquals(later + hour, AdAccess.extended(first, later))
    }

    @Test fun neverGrantedIsInactive() {
        assertFalse(AdAccess.active(0L, t0))
    }

    @Test fun remainingCountsDown() {
        val u = t0 + hour
        assertEquals(hour, AdAccess.remainingMs(u, t0))
        assertEquals(30 * 60_000L, AdAccess.remainingMs(u, t0 + 30 * 60_000L))
        assertEquals(0L, AdAccess.remainingMs(u, t0 + 70 * 60_000L))
    }
}

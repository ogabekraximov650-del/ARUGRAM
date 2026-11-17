package uz.arumediatv.com.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AdAccessTest {
    private val t0 = 1_000_000_000L

    @Test fun activeForSixtyMinutes() {
        assertTrue(AdAccess.active(t0, t0))
        assertTrue(AdAccess.active(t0, t0 + 59 * 60_000L))
        assertFalse(AdAccess.active(t0, t0 + 60 * 60_000L))
        assertFalse(AdAccess.active(t0, t0 + 61 * 60_000L))
    }

    @Test fun neverGrantedIsInactive() {
        assertFalse(AdAccess.active(0L, t0))
    }

    @Test fun clockSetBackRevokes() {
        assertFalse(AdAccess.active(t0, t0 - 1))
    }

    @Test fun remainingCountsDown() {
        assertEquals(60 * 60_000L, AdAccess.remainingMs(t0, t0))
        assertEquals(30 * 60_000L, AdAccess.remainingMs(t0, t0 + 30 * 60_000L))
        assertEquals(0L, AdAccess.remainingMs(t0, t0 + 70 * 60_000L))
    }
}

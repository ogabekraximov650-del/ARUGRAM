package uz.arumediatv.com.data

import org.junit.Assert.assertEquals
import org.junit.Test

class AccountLimitTest {
    @Test
    fun limitDependsOnActiveSubscription() {
        val now = 1_000_000L
        assertEquals(3, AccountStore.limitFor(emptyList(), now))
        assertEquals(3, AccountStore.limitFor(listOf(0L, 0L, 0L), now))
        assertEquals(3, AccountStore.limitFor(listOf(500_000L), now)) // obuna tugagan
        assertEquals(5, AccountStore.limitFor(listOf(0L, now + 1, 0L), now))
        assertEquals(3, AccountStore.limitFor(listOf(now), now)) // aynan tugagan payt — faol emas
    }
}

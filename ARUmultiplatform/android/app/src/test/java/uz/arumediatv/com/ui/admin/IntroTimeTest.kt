package uz.arumediatv.com.ui.admin

import org.junit.Assert.assertEquals
import org.junit.Test

class IntroTimeTest {
    @Test
    fun vaqtMatni() {
        assertEquals("0:00", IntroTime.format(0))
        assertEquals("0:09", IntroTime.format(9_999))
        assertEquals("5:14", IntroTime.format(314_500))
        assertEquals("65:03", IntroTime.format(3_903_000))
        assertEquals("0:00", IntroTime.format(-5))
    }
}

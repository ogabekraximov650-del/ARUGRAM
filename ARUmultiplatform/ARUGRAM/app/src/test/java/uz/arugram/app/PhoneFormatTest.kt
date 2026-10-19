package uz.arugram.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uz.arugram.app.auth.CountryBook
import uz.arugram.app.auth.PhoneFormat

class PhoneFormatTest {
    @Test
    fun formatFollowsMask() {
        assertEquals("", PhoneFormat.format("", "XX XXX XX XX"))
        assertEquals("91", PhoneFormat.format("91", "XX XXX XX XX"))
        assertEquals("91 2", PhoneFormat.format("912", "XX XXX XX XX"))
        assertEquals("91 234 56 78", PhoneFormat.format("912345678", "XX XXX XX XX"))
        assertEquals("91 234 56 78 9", PhoneFormat.format("9123456789", "XX XXX XX XX"))
    }

    @Test
    fun hintShowsRemainingMask() {
        assertEquals("00 000 00 00", PhoneFormat.hintRest("", "XX XXX XX XX"))
        assertEquals(" 000 00 00", PhoneFormat.hintRest("91", "XX XXX XX XX"))
        assertEquals("", PhoneFormat.hintRest("912345678", "XX XXX XX XX"))
    }

    @Test
    fun offsetsMapBothWays() {
        val digits = "912345"
        val f = PhoneFormat.format(digits, "XX XXX XX XX") // "91 234 5"
        assertEquals("91 234 5", f)
        assertEquals(2, PhoneFormat.toFormatted(digits, "XX XXX XX XX", 2))
        assertEquals(4, PhoneFormat.toFormatted(digits, "XX XXX XX XX", 3))
        assertEquals(3, PhoneFormat.toDigits(f, 4))
    }

    @Test
    fun countriesParse() {
        val list = CountryBook.parse("998;UZ;Oʻzbekiston;XX XXX XX XX;1\n1;US;AQSH;XXX XXX XXXX;1\n1;CA;Kanada;XXX XXX XXXX;0\n")
        assertEquals(3, list.size)
        CountryBook.set(list)
        assertEquals("US", CountryBook.forCode("1")?.iso)
        assertEquals("UZ", CountryBook.forCode("998")?.iso)
        assertTrue(CountryBook.hasPrefix("99"))
        assertEquals("🇺🇿", list[0].flag)
    }
}

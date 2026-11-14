package uz.arumediatv.com.ui.admin

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CookieExportTest {
    @Test
    fun sarlavhaJuftlarga() {
        assertEquals(listOf("a" to "b", "c" to "d=e"), CookieExport.parse("a=b; c=d=e; ; =x"))
        assertTrue(CookieExport.parse(null).isEmpty())
    }

    @Test
    fun netscapeFormati() {
        val txt = CookieExport.netscape(
            listOf(Triple(".youtube.com", "SID", "1"), Triple(".youtube.com", "SID", "2"), Triple(".google.com", "__Host-X", "z")), 1000,
        )
        val lines = txt.lines().filter { it.isNotBlank() && !it.startsWith("#") }
        assertEquals(2, lines.size)
        assertEquals(".youtube.com\tTRUE\t/\tTRUE\t${1000 + 365L * 24 * 3600}\tSID\t1", lines[0])
        assertTrue(lines[1].startsWith("google.com\tFALSE\t/\tTRUE\t"))
        assertTrue(txt.startsWith("# Netscape HTTP Cookie File"))
    }

    @Test
    fun kirishBelgisi() {
        assertTrue(CookieExport.signedIn(listOf(Triple(".instagram.com", "sessionid", "x"))))
        assertFalse(CookieExport.signedIn(listOf(Triple(".youtube.com", "PREF", "x"))))
    }

    @Test
    fun chromeKabiKiritish() {
        assertEquals("https://www.instagram.com/accounts/login/", CookieExport.resolveInput("https://www.instagram.com/accounts/login/"))
        assertEquals("https://instagram.com", CookieExport.resolveInput("instagram.com"))
        assertEquals("https://www.google.com/search?q=instagram+kirish", CookieExport.resolveInput("instagram kirish"))
        assertEquals("https://www.google.com/search?q=youtube", CookieExport.resolveInput("youtube"))
        assertEquals("https://www.google.com", CookieExport.resolveInput("  "))
    }

    @Test
    fun domenlar() {
        assertEquals("instagram.com", CookieExport.baseDomain("www.instagram.com"))
        assertEquals("bbc.co.uk", CookieExport.baseDomain("m.bbc.co.uk"))
        assertEquals(listOf("youtube.com", "google.com"), CookieExport.relatedDomains("m.youtube.com"))
        assertEquals(listOf("instagram.com"), CookieExport.relatedDomains("www.instagram.com"))
    }
}

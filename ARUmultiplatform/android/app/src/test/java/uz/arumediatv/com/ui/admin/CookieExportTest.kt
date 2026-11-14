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
        assertTrue(lines[1].startsWith(".google.com\tFALSE\t/\tTRUE\t"))
        assertTrue(txt.startsWith("# Netscape HTTP Cookie File"))
    }

    @Test
    fun kirishBelgisi() {
        assertTrue(CookieExport.signedIn(listOf(Triple(".youtube.com", "LOGIN_INFO", "x"))))
        assertFalse(CookieExport.signedIn(listOf(Triple(".youtube.com", "PREF", "x"))))
    }
}

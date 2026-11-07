package uz.arudrive.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import uz.arudrive.app.data.Sign
import uz.arudrive.app.data.cardRatio
import uz.arudrive.app.data.humanSize
import org.junit.Test

class ModelsTest {
    @Test fun ratioClamped() {
        assertEquals(16f / 9f, cardRatio(0, 0), 0.001f)
        assertEquals(9f / 16f, cardRatio(1080, 1920), 0.001f)   // tik
        assertEquals(16f / 9f, cardRatio(3840, 1080), 0.001f)   // juda yotiq — kesiladi
        assertEquals(1f, cardRatio(500, 500), 0.001f)
    }

    @Test fun sizes() {
        assertEquals("1.5 MB", humanSize(1536 * 1024L))
        assertEquals("12 B", humanSize(12))
    }

    @Test fun signFormat() {
        val s = Sign.appSig("k", 1700000000, "get", "/api/bd/list")
        assertTrue(s.startsWith("v2.1700000000."))
        assertEquals(64, s.substringAfterLast('.').length)
        // HMAC-SHA256("k", "1700000000.GET./api/bd/list") — metod katta harfga o'tadi
        assertEquals(Sign.hmacHex("k", "1700000000.GET./api/bd/list"), s.substringAfterLast('.'))
        assertTrue(Sign.playToken("k", "/api/play/bd~x", 5).startsWith("5."))
    }
}

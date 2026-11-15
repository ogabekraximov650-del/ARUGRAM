package uz.arumediatv.com.data

import org.junit.Assert.assertEquals
import org.junit.Test

class ResumePointTest {
    private val min = 60_000L

    @Test
    fun siftNiAlmashtirganda25DaqiqadanDavomEtadi() {
        // A sifatda 12-daqiqada to'xtab, B ga o'tib 25-daqiqagacha ko'rilgan; qayta A bilan ochilganda faylga bog'liq joy 12 — 25 chiqishi kerak.
        assertEquals(25 * min, ResumePoint.choose(null, 25 * min, 1000, 12 * min, 500, 12 * min))
    }

    @Test
    fun aniqSoralganJoyBirinchi() {
        assertEquals(7 * min, ResumePoint.choose(7 * min, 25 * min, 1000, 12 * min, 500, 12 * min))
    }

    @Test
    fun serverTarixiYangiroqBoLsaOshaOlinadi() {
        assertEquals(40 * min, ResumePoint.choose(null, 25 * min, 1000, 40 * min, 9000, 0))
    }

    @Test
    fun mahalliyYoqBoLsaServerTarixi() {
        assertEquals(40 * min, ResumePoint.choose(null, 0, 0, 40 * min, 1, 5 * min))
    }

    @Test
    fun hechNarsaYoqBoLsaFaylJoyi() {
        assertEquals(5 * min, ResumePoint.choose(null, 0, 0, 0, 0, 5 * min))
    }
}

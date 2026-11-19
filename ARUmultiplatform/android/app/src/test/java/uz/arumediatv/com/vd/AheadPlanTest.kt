package uz.arumediatv.com.vd

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Oldindan olish qoidasi: oldinda 10 MiB dan kam qolsa — birinchi yetishmayotgan bo'lakdan 18 MiB oyna oxirigacha. */
class AheadPlanTest {
    private fun plan(cur: Long, last: Long, cached: Set<Long>) = AheadPlan.next(cur, last) { it in cached }

    @Test
    fun videoOchilganda_birinchiOynaButunOlinadi() {
        // 570 MiB fayl, keshda hech narsa yo'q: 0-bo'lakdan 18 ta (butun birinchi oyna).
        assertEquals(0L to 18, plan(0, 569, emptySet()))
    }

    @Test
    fun zaxira10MiBdanKamBolsa_keyingiOynaOlinadi() {
        // Birinchi oyna (0..17) olingan; ijro 10-bo'lakda: oldinda 8 ta (<10) -> 18-bo'lakdan butun ikkinchi oyna.
        val cached = (0L..17L).toSet()
        assertEquals(18L to 18, plan(10, 569, cached))
    }

    @Test
    fun zaxira10MiBdanKamBolmasa_hechNarsaOlinmaydi() {
        val cached = (0L..17L).toSet()
        assertNull("ijro 0-bo'lakda: oldinda 18 ta", plan(0, 569, cached))
        assertNull("ijro 8-bo'lakda: oldinda 10 ta (chegara)", plan(8, 569, cached))
    }

    @Test
    fun sirilganda_ushaJoydanOynaOlinadi() {
        // Keshda faqat boshi bor; 300-bo'lakka surildi: 300 yo'q -> 300 dan 17-oyna oxirigacha (oyna 16: 288..305) = 6 ta bo'lak.
        assertEquals(300L to 6, plan(300, 569, (0L..17L).toSet()))
    }

    @Test
    fun faylOxirida_oynaQisqa() {
        // 30 bo'lakli fayl (0..29): 18-bo'lakdan 29 gacha 12 ta.
        assertEquals(18L to 12, plan(12, 29, (0L..17L).toSet()))
    }

    @Test
    fun faylOxiriganChaKeshda_hechNarsaYoq() {
        assertNull(plan(25, 29, (0L..29L).toSet()))
        assertNull("oxirgi bo'lak keshda, oldinda 10 dan kam bo'lsa ham", plan(28, 29, (0L..29L).toSet()))
    }
}

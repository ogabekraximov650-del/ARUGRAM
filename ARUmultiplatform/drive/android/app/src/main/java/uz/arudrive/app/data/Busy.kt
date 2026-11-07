package uz.arudrive.app.data

import kotlinx.coroutines.delay
import java.util.concurrent.atomic.AtomicInteger

/**
 * ARUMEDIATV'dagi `gate.busy` kabi: pleyer ochiq paytda > 0. Fon so'rovlari (navbatni yangilash va h.k.) shu vaqt kutadi,
 * shunda video oqimi worker'ga ortiqcha so'rovlar bilan to'sqinlik qilmaydi va so'rovlar soni kamayadi.
 */
object Busy {
    val count = AtomicInteger(0)
    suspend fun awaitIdle() {
        while (count.get() > 0) delay(400)
    }
}

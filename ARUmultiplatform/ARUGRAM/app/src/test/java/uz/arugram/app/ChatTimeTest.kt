package uz.arugram.app

import org.junit.Assert.assertEquals
import org.junit.Test
import uz.arugram.app.ui.main.chatTime
import java.util.Calendar

class ChatTimeTest {
    @Test
    fun bugungiXabarSoatDaqiqaKorinadi() {
        val c = Calendar.getInstance().apply { set(Calendar.HOUR_OF_DAY, 0); set(Calendar.MINUTE, 5); set(Calendar.SECOND, 0) }
        assertEquals("00:05", chatTime(c.timeInMillis / 1000))
    }

    @Test
    fun bosVaqtBoshQator() {
        assertEquals("", chatTime(0))
    }

    @Test
    fun eskiXabarSanaKorinadi() {
        val c = Calendar.getInstance().apply { set(2020, Calendar.MARCH, 7, 12, 0, 0) }
        assertEquals("07.03.20", chatTime(c.timeInMillis / 1000))
    }
}

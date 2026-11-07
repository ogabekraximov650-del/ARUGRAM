package uz.arutube.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import kotlinx.serialization.decodeFromString

class FormatTest {
    @Test fun duration() {
        assertEquals("1:40:01", Format.duration(6001))
        assertEquals("1:15", Format.duration(75))
        assertEquals("0:00", Format.duration(0))
    }

    @Test fun count() {
        assertEquals("999", Format.count(999))
        assertEquals("1.2K", Format.count(1200))
        assertEquals("3.4M", Format.count(3_400_000))
    }

    @Test fun size() {
        assertEquals("1.28 GB", Format.size(1377003421))
        assertEquals("512 B", Format.size(512))
    }

    @Test fun date() {
        assertEquals("2026-11-06", Format.date("2026-11-06 07:54:34"))
    }

    @Test fun ago() {
        val f = java.text.SimpleDateFormat("yyyy-MM-dd HH:mm:ss", java.util.Locale.US).apply { timeZone = java.util.TimeZone.getTimeZone("UTC") }
        val now = f.parse("2026-11-08 07:54:34")!!.time
        assertEquals("2 kun oldin", Format.ago("2026-11-06 07:54:34", now))
        assertEquals("3 soat oldin", Format.ago("2026-11-08 04:54:34", now))
        assertEquals("hozirgina", Format.ago("2026-11-08 07:54:30", now))
        assertEquals("", Format.ago("xato", now))
    }

    @Test fun listJavobi() {
        val j = Api.json.decodeFromString<VideoPage>(
            """{"videos":[{"code":"a","title":"T","duration":6001,"views":3,"thumb":"u","embed":"e","extra":1}],"page":1,"pages":2,"total":9}""",
        )
        assertEquals(1, j.videos.size)
        assertEquals(6001L, j.videos[0].duration)
        assertEquals(2, j.pages)
        assertNull(j.error)
    }

    @Test fun shortsAniqlash() {
        assertEquals(true, Video(w = 720, h = 900).isShort)
        assertEquals(false, Video(w = 720, h = 409, duration = 10).isShort)
        assertEquals(true, Video(duration = 14).isShort)
        assertEquals(false, Video(duration = 6001).isShort)
    }

    @Test fun davomEttirish() {
        val v = Video(code = "a")
        assertEquals(true, Saved(v, posMs = 60_000, durMs = 6_000_000).inProgress)
        assertEquals(false, Saved(v, posMs = 2_000, durMs = 6_000_000).inProgress)
        assertEquals(false, Saved(v, posMs = 5_990_000, durMs = 6_000_000).inProgress)
        assertEquals(0.5f, Saved(v, posMs = 50, durMs = 100).fraction, 0.001f)
        assertEquals(0f, Saved(v).fraction, 0f)
    }

    @Test fun yuklashJavobi() {
        val r = Api.parseUpload("""{"msg":"OK","status":200,"result":[{"filecode":"x1","download_url":"https://d/d/x1"}]}""")
        assertEquals("x1", r.result[0].filecode)
    }
}

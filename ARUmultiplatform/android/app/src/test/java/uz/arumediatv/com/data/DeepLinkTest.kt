package uz.arumediatv.com.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class DeepLinkTest {
    @Test fun parsesEpisodeLink() {
        assertEquals(DeepLink.Target(12, 3, 45), DeepLink.parsePath("/s/12/3/45"))
    }

    @Test fun parsesSeasonLinkWithoutEpisode() {
        assertEquals(DeepLink.Target(12, 3, 0), DeepLink.parsePath("/s/12/3"))
    }

    @Test fun rejectsBadPaths() {
        assertNull(DeepLink.parsePath("/x/12/3/45"))
        assertNull(DeepLink.parsePath("/s/abc/3/45"))
        assertNull(DeepLink.parsePath("/s/12"))
        assertNull(DeepLink.parsePath("/s/0/3/45"))
        assertNull(DeepLink.parsePath(null))
    }

    @Test fun buildsUrl() {
        assertEquals("https://arumediatvuz.pages.dev/s/12/3/45", DeepLink.url(12, 3, 45))
        assertEquals("https://arumediatvuz.pages.dev/s/12/3", DeepLink.url(12, 3))
    }
}

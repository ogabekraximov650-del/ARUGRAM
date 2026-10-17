package uz.arumediatv.com.ui.packs

import android.graphics.Typeface
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.text.font.FontFamily
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import org.json.JSONObject
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.BuildConfig
import uz.arumediatv.com.data.Http
import uz.arumediatv.com.data.callJson
import java.io.File

/**
 * Emoji shrifti (Telegram uslubi). Shrift fayli yopiq kanalda (AES-128-CTR) turadi (`worker/src/emojifont.rs`):
 * ilova uni emoji oynasi ochilganda yuklab, qurilmada saqlaydi. Yuklab bo'lmasa — 5 urinishgacha, har bir muvaffaqiyatsiz
 * urinish workerga xabar qilinadi; 5 taga yetsa worker Actions'ni uyg'otib, faylni yangisiga almashtiradi.
 * APK ichida shrift yo'q: yuklanguncha tizim shrifti ishlatiladi.
 */
object EmojiFonts {
    // Shrift APK ichida YO'Q (u kanalda). Yuklanguncha tizim shrifti ishlatiladi.
    private val bundled: FontFamily = FontFamily.Default

    private var remote by mutableStateOf<FontFamily?>(null)

    val family: FontFamily get() = remote ?: bundled

    private val mutex = Mutex()
    private var diskLoaded = false
    private var checked = false

    private fun dir(): File = File(AppServices.appContext.filesDir, "emoji_font").apply { mkdirs() }

    private fun verOf(f: File): Long = f.nameWithoutExtension.toLongOrNull() ?: 0L

    private fun looksLikeFont(b: ByteArray): Boolean {
        if (b.size < 100_000) return false
        val m = String(b, 0, 4, Charsets.ISO_8859_1)
        return m == "\u0000\u0001\u0000\u0000" || m == "true" || m == "OTTO" || m == "ttcf"
    }

    private fun load(f: File): FontFamily? =
        try {
            val tf = Typeface.createFromFile(f)
            FontFamily(androidx.compose.ui.text.font.Typeface(tf))
        } catch (_: Throwable) {
            null
        }

    /** Fayldan yoki kanaldan. Bir vaqtda bitta chaqiruv; jarayon boshiga serverni bir marta tekshiradi. */
    suspend fun ensure() {
        mutex.withLock {
            withContext(Dispatchers.IO) {
                val d = dir()
                if (!diskLoaded) {
                    diskLoaded = true
                    d.listFiles()?.filter { it.name.endsWith(".ttf") }?.maxByOrNull(::verOf)?.let { f -> load(f)?.let { remote = it } }
                }
                if (checked || !AppServices.auth.isLoggedIn) return@withContext
                val token = AppServices.auth.sessionToken ?: return@withContext
                for (attempt in 1..5) {
                    val r = Http.client.callJson("${BuildConfig.API_BASE}/api/emojifont", bearer = token)
                    val j = r.json
                    if (!r.ok || j == null) return@withContext // tarmoq yo'q — urinish sanalmaydi
                    if (j.optBoolean("missing")) {
                        // Shrift kanalda hali yo'q: worker Actions'ni uyg'otdi — biroz kutib qayta so'raymiz.
                        delay(4_000L * attempt)
                        continue
                    }
                    val ver = j.optLong("ver")
                    val name = j.optString("name")
                    val have = d.listFiles()?.filter { it.name.endsWith(".ttf") }?.maxOfOrNull(::verOf) ?: 0L
                    if (ver <= 0 || name.isEmpty()) return@withContext
                    if (have == ver && remote != null) {
                        checked = true
                        return@withContext
                    }
                    val bytes = AppServices.telegram.fetchBytes(name)
                    val f = File(d, "$ver.ttf")
                    var good: FontFamily? = null
                    if (bytes != null && looksLikeFont(bytes)) {
                        f.writeBytes(bytes)
                        good = load(f)
                        if (good == null) f.delete()
                    }
                    if (good != null) {
                        remote = good
                        checked = true
                        d.listFiles()?.filter { it.name.endsWith(".ttf") && it != f }?.forEach { it.delete() }
                        return@withContext
                    }
                    // Yuklanmadi/ochilmadi: workerga xabar (5 ta bo'lsa Actions faylni yangilaydi).
                    Http.client.callJson("${BuildConfig.API_BASE}/api/emojifont/fail", "POST", JSONObject().put("ver", ver), token)
                    delay(2_000L * attempt)
                }
            }
        }
    }
}

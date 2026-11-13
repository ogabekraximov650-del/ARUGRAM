package uz.arumediatv.com.data

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.rememberUpdatedState
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import org.json.JSONObject
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.BuildConfig
import java.util.concurrent.TimeUnit

/**
 * UZ: JONLI YANGILANISH. Server `/api/live/wait` ~20 soniya ushlab turadi va tegishli jadvallar (anime/bo'lim/qism, admin uchun yana foydalanuvchilar, izohlar)
 * o'zgarganda `new=true` qaytaradi. Server BAZAGA BORMAYDI (faqat o'zgarish belgisi), ilova esa o'zgarishni sezgach ro'yxatni o'zi yangilaydi.
 * Faqat ekran ko'rinib turganda (STARTED) ishlaydi; internet yo'q bo'lsa kutadi.
 * EN: live updates via a long-poll on change marks (no DB access on the server); runs only while the screen is visible.
 * RU: живое обновление: long-poll по отметкам изменений; работает только пока экран виден.
 */
object Live {
    private val http by lazy { Http.client.newBuilder().readTimeout(40, TimeUnit.SECONDS).callTimeout(45, TimeUnit.SECONDS).build() }

    /** `kind`: "content" (hamma) yoki "users"/"reports" (faqat admin). Natija: (o'zgardimi, yangi belgi) yoki `null` (xato). Faqat IO oqimida. */
    private fun wait(kind: String, mk: Long): Pair<Boolean, Long>? {
        val (code, text) = http.callText("${BuildConfig.API_BASE}/api/live/wait?k=$kind&mk=$mk", AppServices.auth.sessionToken)
        if (code != 200) return null
        return try { val j = JSONObject(text); j.optBoolean("new") to j.optLong("mk", mk) } catch (_: Exception) { null }
    }

    /** O'zgarish bo'lganda [onChange] chaqiriladi (birinchi marta — faqat joriy belgi olinadi, chaqirilmaydi). Bekor qilinguncha davom etadi. */
    suspend fun loop(kind: String, onChange: suspend () -> Unit) {
        var mk = -1L
        var fails = 0
        while (currentCoroutineContext().isActive) {
            if (!Net.isOnline(AppServices.appContext)) { delay(5_000); continue }
            val r = withContext(Dispatchers.IO) { wait(kind, mk) }
            if (r == null) {
                fails++
                delay(minOf(30_000L, 3_000L * fails)) // xato/403/offline: sekinlashib qayta uriniladi
                continue
            }
            fails = 0
            val (changed, now) = r
            val first = mk < 0
            mk = now
            if (changed && !first) {
                try { onChange() } catch (e: kotlinx.coroutines.CancellationException) { throw e } catch (_: Exception) {}
            }
        }
    }
}

/** Ekran ko'rinib turganda `kind` o'zgarishini kuzatadi; o'zgarganda [onChange] (ro'yxatni yangilash). */
@Composable
fun LiveEffect(kind: String, onChange: suspend () -> Unit) {
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    val cb = rememberUpdatedState(onChange)
    LaunchedEffect(kind, lifecycle) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) { Live.loop(kind) { cb.value() } }
    }
}

package uz.arutube.app

import android.app.Activity
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.async
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * Reklamasiz havolalarni OLDINDAN tayyorlab qo'yadi: ro'yxat ochilganda birinchi videolar uchun yashirin WebView ketma-ket havolani topadi,
 * video bosilganda esa tayyor havola darhol ishlatiladi. Havolalar vaqtinchalik, shuning uchun 20 daqiqadan keyin eskiradi; pleyer xato
 * bersa, havola o'chirilib bir marta yangisi olinadi. Hamma chaqiruv asosiy (Main) oqimda — WebView shuni talab qiladi.
 */
object LinkCache {
    private const val TTL_MS = 20 * 60_000L
    private val cache = HashMap<String, Pair<Direct, Long>>()
    private val inflight = HashMap<String, Deferred<Direct?>>()
    private val prefetchLock = Mutex() // oldindan tayyorlash bitta-bitta: telefon qizib ketmasin

    fun get(code: String): Direct? {
        val e = cache[code] ?: return null
        if (System.currentTimeMillis() - e.second >= TTL_MS) {
            cache.remove(code)
            return null
        }
        return e.first
    }

    fun invalidate(code: String) {
        cache.remove(code)
    }

    /** Keshdan, ketayotgan so'rovdan yoki yangidan oladi (foydalanuvchi bosganda navbat kutmaydi). */
    suspend fun resolve(scope: CoroutineScope, act: Activity, v: Video): Direct? {
        get(v.code)?.let { return it }
        val job = inflight.getOrPut(v.code) {
            scope.async {
                try {
                    DirectLink.resolve(act, v.embed)?.also { cache[v.code] = it to System.currentTimeMillis() }
                } finally {
                    inflight.remove(v.code)
                }
            }
        }
        return job.await()
    }

    /** Ro'yxatning birinchi `n` ta videosi uchun havolani orqa fonda tayyorlaydi (allaqachon tayyori o'tkazib yuboriladi). */
    fun prefetch(scope: CoroutineScope, act: Activity, list: List<Video>, n: Int = 3) {
        val todo = list.filter { get(it.code) == null }.take(n)
        if (todo.isEmpty()) return
        scope.launch {
            for (v in todo) {
                if (get(v.code) == null) prefetchLock.withLock { resolve(scope, act, v) }
            }
        }
    }
}

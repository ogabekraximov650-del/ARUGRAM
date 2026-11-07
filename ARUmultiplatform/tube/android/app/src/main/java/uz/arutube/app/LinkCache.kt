package uz.arutube.app

import android.app.Activity
import android.content.Context
import android.content.SharedPreferences
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.async
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.MapSerializer
import kotlinx.serialization.builtins.serializer

/**
 * Reklamasiz havolalarni OLDINDAN tayyorlab qo'yadi: ro'yxat ochilganda birinchi videolar uchun yashirin WebView ketma-ket havolani topadi,
 * video bosilganda esa tayyor havola darhol ishlatiladi. DoodStream havolasi taxminan 8 soat ishlaydi, shuning uchun 7 soat saqlanadi va
 * ilova qayta ochilganda ham ishlatiladi (SharedPreferences); pleyer xato
 * bersa, havola o'chirilib bir marta yangisi olinadi. Hamma chaqiruv asosiy (Main) oqimda — WebView shuni talab qiladi.
 */
object LinkCache {
    private const val TTL_MS = 7 * 60 * 60_000L

    @Serializable
    private data class Entry(val d: Direct, val at: Long)

    private val cache = HashMap<String, Pair<Direct, Long>>()
    private var sp: SharedPreferences? = null
    private val ser = MapSerializer(String.serializer(), Entry.serializer())

    /** Ilova ochilganda: saqlangan havolalar tiklanadi (eskirganlari tashlanadi). */
    fun init(ctx: Context) {
        if (sp != null) return
        sp = ctx.getSharedPreferences("arutube_links", Context.MODE_PRIVATE)
        try {
            val now = System.currentTimeMillis()
            sp?.getString("links", null)?.let { raw ->
                Api.json.decodeFromString(ser, raw).forEach { (k, e) -> if (now - e.at < TTL_MS) cache[k] = e.d to e.at }
            }
        } catch (_: Exception) {
        }
    }

    private fun persist() {
        try {
            val m = cache.mapValues { Entry(it.value.first, it.value.second) }
            sp?.edit()?.putString("links", Api.json.encodeToString(ser, m))?.apply()
        } catch (_: Exception) {
        }
    }
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
        persist()
    }

    /** Keshdan, ketayotgan so'rovdan yoki yangidan oladi (foydalanuvchi bosganda navbat kutmaydi). */
    suspend fun resolve(scope: CoroutineScope, act: Activity, v: Video): Direct? {
        get(v.code)?.let { return it }
        val job = inflight.getOrPut(v.code) {
            scope.async {
                try {
                    DirectLink.resolve(act, v.embed)?.also {
                        cache[v.code] = it to System.currentTimeMillis()
                        persist()
                    }
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

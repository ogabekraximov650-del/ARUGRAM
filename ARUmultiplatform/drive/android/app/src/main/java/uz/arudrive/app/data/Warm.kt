package uz.arudrive.app.data

import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.datasource.ResolvingDataSource
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import java.io.IOException
import java.util.concurrent.ConcurrentHashMap

/**
 * Pleyer o'qiydigan har bir joy uchun avval kesh oynasi isitiladi: oyna keshda bo'lmaguncha ExoPlayer so'rov YUBORMAYDI
 * (video keshga tushmaguncha ochilmaydi). Isitilgan oynalar eslab qolinadi.
 */
@UnstableApi
object Warm {
    private val done = ConcurrentHashMap.newKeySet<String>()
    private val inflight = ConcurrentHashMap.newKeySet<String>()
    /** Fayl hajmi (isitish javobidan) — keyingi oynani oldindan isitish uchun. */
    val totals = ConcurrentHashMap<String, Long>()

    /** Keyingi bo'lak shu joyda keshga isitiladi: joriy bo'lakning 15-MB'i o'qilganda (foydalanuvchi talabi). */
    private const val PREFETCH_AT = 15L * 1024 * 1024

    /**
     * TANBAL OLDINDAN ISITISH (ARUMEDIATV "tanbal keshlash" kabi): pleyer joriy bo'lakning 15-MB'ini o'qiganda KEYINGI bo'lak
     * fonda keshga isitiladi — chegarada pleyer kutib qolmaydi, ko'rilmagan bo'lak esa isitilmaydi. [pos] — fayldagi MUTLAQ bayt joyi.
     */
    fun onRead(id: String, pos: Long) {
        val total = totals[id] ?: return
        val w = pos / Api.WARM_WINDOW
        if (pos - w * Api.WARM_WINDOW < PREFETCH_AT) return
        val next = w + 1
        if (next * Api.WARM_WINDOW >= total) return
        val key = "$id:$next"
        if (done.contains(key) || !inflight.add(key)) return
        Thread {
            try { if (Api.warmBlocking(id, next)) done.add(key) } finally { inflight.remove(key) }
        }.start()
    }

    /** ExoPlayer qaysi baytni o'qiyotganini kuzatadi (har so'rov uchun boshlanish joyi + o'qilgan bayt). */
    private class Tracker(val id: String) : androidx.media3.datasource.TransferListener {
        private val reads = java.util.Collections.synchronizedMap(java.util.IdentityHashMap<DataSpec, LongArray>())
        override fun onTransferInitializing(source: androidx.media3.datasource.DataSource, dataSpec: DataSpec, isNetwork: Boolean) {}
        override fun onTransferStart(source: androidx.media3.datasource.DataSource, dataSpec: DataSpec, isNetwork: Boolean) {
            reads[dataSpec] = longArrayOf(dataSpec.position, 0)
        }
        override fun onBytesTransferred(source: androidx.media3.datasource.DataSource, dataSpec: DataSpec, isNetwork: Boolean, bytesTransferred: Int) {
            val r = reads[dataSpec] ?: return
            r[1] += bytesTransferred
            onRead(id, r[0] + r[1])
        }
        override fun onTransferEnd(source: androidx.media3.datasource.DataSource, dataSpec: DataSpec, isNetwork: Boolean) { reads.remove(dataSpec) }
    }

    fun mediaSourceFactory(id: String, context: android.content.Context, size: Long): DefaultMediaSourceFactory {
        val resolver = ResolvingDataSource.Resolver { spec: DataSpec ->
            val w = spec.position / Api.WARM_WINDOW
            val key = "$id:$w"
            if (!done.contains(key)) {
                if (!Api.warmBlocking(id, w)) throw IOException("Kesh tayyor emas (oyna $w)")
                done.add(key)
            }
            spec
        }
        val network = ResolvingDataSource.Factory(DefaultHttpDataSource.Factory().setConnectTimeoutMs(15_000).setReadTimeoutMs(60_000).setTransferListener(Tracker(id)), resolver)
        // Telefondagi DOIMIY shifrlangan 1 MiB bo'laklar keshi: ko'rilgan joy tarmoqsiz ochiladi (ARUMEDIATV kabi).
        val app = context.applicationContext
        if (size <= 0) return DefaultMediaSourceFactory(context).setDataSourceFactory(network) // hajm noma'lum — keshsiz
        return DefaultMediaSourceFactory(context).setDataSourceFactory(
            androidx.media3.datasource.DataSource.Factory { EncCacheDataSource(app, id, size, network.createDataSource()) },
        )
    }

    private val prefetched = ConcurrentHashMap.newKeySet<String>()

    /**
     * Bosh sahifada ko'ringan videoning BIRINCHI bo'lagini keshga isitadi (belgi yangi bo'lsa yoki shu ishga tushishda urinilgan bo'lsa — hech narsa
     * qilmaydi). Natija muvaffaqiyatli bo'lsa "keshda" belgisi yoziladi (pleyer darhol ochiladi). Xatoda keyingi ishga tushishgacha qayta urinilmaydi.
     */
    suspend fun prefetchFirst(ctx: android.content.Context, id: String) {
        if (fresh(ctx, id) || !prefetched.add(id)) return
        // NonCancellable: ro'yxat aylantirilib so'rov bekor qilinsa ham boshlangan isitish tugaydi va belgi yoziladi.
        val ok = kotlinx.coroutines.withContext(kotlinx.coroutines.NonCancellable + kotlinx.coroutines.Dispatchers.IO) { Api.warmDetailed(id, 0).ok }
        if (ok) remember(ctx, id) else prefetched.remove(id)
    }

    fun markWarm(id: String, window: Long = 0) { done.add("$id:$window") }

    // ── "BU OYNA KESHDA KO'RILGAN" BELGISI (ARUMEDIATV `write_warm_marker` kabi) ──
    // Isitish muvaffaqiyatli tugagach doimiy xotiraga yoziladi. Belgi yangi bo'lsa pleyer darhol ochiladi (isitish so'rovi
    // kutilmaydi); kesh o'chirilgan bo'lsa pleyer 503 oladi va PlayerScreen oynani majburan qayta isitib davom etadi.
    private const val FRESH_MS = 7L * 24 * 3600 * 1000
    private fun prefs(ctx: android.content.Context) = ctx.applicationContext.getSharedPreferences("warm", android.content.Context.MODE_PRIVATE)

    fun fresh(ctx: android.content.Context, id: String): Boolean = try {
        System.currentTimeMillis() - prefs(ctx).getLong(id, 0L) < FRESH_MS
    } catch (_: Exception) { false }

    fun remember(ctx: android.content.Context, id: String) {
        markWarm(id, 0)
        try { prefs(ctx).edit().putLong(id, System.currentTimeMillis()).apply() } catch (_: Exception) {}
    }

    fun forget(ctx: android.content.Context, id: String) {
        done.removeAll { it.startsWith("$id:") }
        try { prefs(ctx).edit().remove(id).apply() } catch (_: Exception) {}
    }
}

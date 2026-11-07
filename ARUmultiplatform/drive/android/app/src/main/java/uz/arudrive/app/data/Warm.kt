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

    fun mediaSourceFactory(id: String, context: android.content.Context): DefaultMediaSourceFactory {
        val resolver = ResolvingDataSource.Resolver { spec: DataSpec ->
            val w = spec.position / Api.WARM_WINDOW
            val key = "$id:$w"
            if (!done.contains(key)) {
                if (!Api.warmBlocking(id, w)) throw IOException("Kesh tayyor emas (oyna $w)")
                done.add(key)
            }
            spec
        }
        return DefaultMediaSourceFactory(context).setDataSourceFactory(
            ResolvingDataSource.Factory(DefaultHttpDataSource.Factory().setConnectTimeoutMs(15_000).setReadTimeoutMs(60_000), resolver),
        )
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

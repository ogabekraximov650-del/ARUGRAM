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
}

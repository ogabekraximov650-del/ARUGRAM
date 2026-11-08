package uz.arumediatv.com.vd

import android.content.Context
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.datasource.ResolvingDataSource
import androidx.media3.datasource.TransferListener
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import java.io.IOException
import java.util.concurrent.ConcurrentHashMap

/**
 * Pleyer o'qiydigan har bir bo'lak (oyna) uchun avval kesh isitiladi, keyin baytlar shifri ochilib telefonning DOIMIY shifrlangan keshiga yoziladi
 * (ARUDRIVE `Warm`/`EncCacheDataSource` bilan bir xil). Keyingi bo'lak pleyer joriy bo'lakning 15-MB'ini o'qiganda fonda isitiladi.
 */
@UnstableApi
object VdWarm {
    private val done = ConcurrentHashMap.newKeySet<String>()
    private val inflight = ConcurrentHashMap.newKeySet<String>()
    val totals = ConcurrentHashMap<String, Long>()
    private const val PREFETCH_AT = 15L * 1024 * 1024

    fun onRead(name: String, pos: Long) {
        val total = totals[name] ?: return
        val w = pos / VdApi.WARM_WINDOW
        if (pos - w * VdApi.WARM_WINDOW < PREFETCH_AT) return
        val next = w + 1
        if (next * VdApi.WARM_WINDOW >= total) return
        val key = "$name:$next"
        if (done.contains(key) || !inflight.add(key)) return
        Thread {
            try { if (VdApi.warmBlocking(name, next)) done.add(key) } finally { inflight.remove(key) }
        }.start()
    }

    private class Tracker(val name: String) : TransferListener {
        private val reads = java.util.Collections.synchronizedMap(java.util.IdentityHashMap<DataSpec, LongArray>())
        override fun onTransferInitializing(source: DataSource, dataSpec: DataSpec, isNetwork: Boolean) {}
        override fun onTransferStart(source: DataSource, dataSpec: DataSpec, isNetwork: Boolean) { reads[dataSpec] = longArrayOf(dataSpec.position, 0) }
        override fun onBytesTransferred(source: DataSource, dataSpec: DataSpec, isNetwork: Boolean, bytesTransferred: Int) {
            val r = reads[dataSpec] ?: return
            r[1] += bytesTransferred
            onRead(name, r[0] + r[1])
        }
        override fun onTransferEnd(source: DataSource, dataSpec: DataSpec, isNetwork: Boolean) { reads.remove(dataSpec) }
    }

    fun forget(name: String) { done.removeAll { it.startsWith("$name:") } }

    fun markWarm(name: String, window: Long = 0) { done.add("$name:$window") }

    /** Pleyer manbasi: [BlockCache] (telefon) <- [CtrDataSource] (shifrni ochish) <- tarmoq (har oyna oldin isitiladi). */
    fun mediaSourceFactory(name: String, context: Context, size: Long, keyHex: String): DefaultMediaSourceFactory {
        val resolver = ResolvingDataSource.Resolver { spec: DataSpec ->
            val w = spec.position / VdApi.WARM_WINDOW
            val key = "$name:$w"
            if (!done.contains(key)) {
                if (!VdApi.warmBlocking(name, w)) throw IOException("Kesh tayyor emas (oyna $w)")
                done.add(key)
            }
            spec
        }
        val network = ResolvingDataSource.Factory(
            DefaultHttpDataSource.Factory().setConnectTimeoutMs(15_000).setReadTimeoutMs(60_000).setTransferListener(Tracker(name)), resolver)
        val app = context.applicationContext
        val key = VdCtr.hex(keyHex)
        return DefaultMediaSourceFactory(context).setDataSourceFactory(
            DataSource.Factory { EncCacheDataSource(app, name, size, CtrDataSource(network.createDataSource(), key)) },
        )
    }
}

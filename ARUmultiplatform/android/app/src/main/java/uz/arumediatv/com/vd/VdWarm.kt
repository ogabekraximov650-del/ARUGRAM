package uz.arumediatv.com.vd

import android.content.Context
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSource
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.extractor.DefaultExtractorsFactory
import androidx.media3.extractor.Extractor
import androidx.media3.extractor.ExtractorInput
import androidx.media3.extractor.ExtractorOutput
import androidx.media3.extractor.ExtractorsFactory
import androidx.media3.extractor.PositionHolder
import androidx.media3.extractor.SeekMap
import androidx.media3.extractor.TrackOutput
import androidx.media3.extractor.mp4.Mp4Extractor
import java.util.concurrent.ConcurrentHashMap

/**
 * Isitish holati (qaysi 18 MiB oyna Cloudflare keshida tayyor) va pleyer manbasi. Har bo'lak so'rovidan oldin oynasi isitiladi ([VdBlocks]);
 * keyingi oyna joriy oynaning 15-MB'i olinganda fonda isitiladi (serverda — telefon trafigi ketmaydi).
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

    fun forget(name: String) { done.removeAll { it.startsWith("$name:") } }

    fun markWarm(name: String, window: Long = 0) { done.add("$name:$window") }

    fun isWarm(name: String, window: Long): Boolean = done.contains("$name:$window")

    /** Videoning vaqt -> bayt jadvalini ([SeekMap]) 1 daqiqalik oyna ([VdAhead]) uchun ushlab qoladi; qolgan hammasi asl o'quvchiga uzatiladi. */
    private class TapExtractor(private val inner: Extractor, private val name: String) : Extractor {
        override fun sniff(input: ExtractorInput): Boolean = inner.sniff(input)
        override fun init(output: ExtractorOutput) = inner.init(object : ExtractorOutput {
            override fun track(id: Int, type: Int): TrackOutput = output.track(id, type)
            override fun endTracks() = output.endTracks()
            override fun seekMap(seekMap: SeekMap) {
                VdAhead.setSeekMap(name, seekMap)
                output.seekMap(seekMap)
            }
        })
        override fun read(input: ExtractorInput, seekPosition: PositionHolder): Int = inner.read(input, seekPosition)
        override fun seek(position: Long, timeUs: Long) = inner.seek(position, timeUs)
        override fun release() = inner.release()
        override fun getUnderlyingImplementation(): Extractor = inner.underlyingImplementation
    }

    /** Pleyer manbasi: telefon keshi ([BlockCache]) <- [VdBlocks] (aniq 1 MiB so'rovlar, shifrni ochish). */
    fun mediaSourceFactory(name: String, context: Context, size: Long, keyHex: String): DefaultMediaSourceFactory {
        val app = context.applicationContext
        val key = VdCtr.hex(keyHex)
        val extractors = ExtractorsFactory {
            DefaultExtractorsFactory().createExtractors().sortedBy { if (it is Mp4Extractor) 0 else 1 }
                .map<Extractor, Extractor> { TapExtractor(it, name) }.toTypedArray()
        }
        return DefaultMediaSourceFactory(DataSource.Factory { EncCacheDataSource(app, name, size, key) }, extractors)
    }
}

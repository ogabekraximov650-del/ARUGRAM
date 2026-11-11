package uz.arumediatv.com.data

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import okhttp3.Interceptor
import okhttp3.Response
import okhttp3.ResponseBody
import okio.Buffer
import okio.BufferedSource
import okio.ForwardingSource
import okio.Source
import okio.buffer
import uz.arumediatv.com.core.RustCore
import java.util.concurrent.atomic.AtomicLong

/**
 * Trafik hisobi (Flutter `TrafficService`): FAQAT tarmoqdan kelgan baytlar sanaladi — Rust yadrosi workerdan tortgan video
 * baytlari va ilovaning HTTP mijozi qabul qilgani (API). Mahalliy (127.0.0.1) uzatma sanalmaydi, shuning uchun oflayn ko'rish
 * trafik qo'shmaydi. Yig'indi telefonda (hisob bo'yicha) saqlanadi va `SyncQueue` paketi bilan serverga ketadi.
 */
class TrafficMeter(context: Context, private val auth: AuthService) {
    private val prefs = context.getSharedPreferences("aru_traffic", Context.MODE_PRIVATE)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val api = AtomicLong(0)
    private var lastVideo = -1L
    private var lastApi = 0L
    private var lastReportAt = prefs.getLong("reported_at", 0)
    private val _totals = MutableStateFlow<Map<String, Long>>(emptyMap())

    /** Toifalar bo'yicha umr bo'yi (shu telefonda, shu hisob uchun): `video`, `api`. */
    val totals: StateFlow<Map<String, Long>> = _totals.asStateFlow()

    /** Hali serverga yuborilmagan bayt. */
    val pendingBytes: Long get() = prefs.getLong("pend_${uid()}", 0)

    var onNeedFlush: (() -> Unit)? = null

    private fun uid(): Long = auth.user.value?.id ?: 0L

    fun addApi(n: Long) {
        if (n > 0) api.addAndGet(n)
    }

    fun start() {
        scope.launch {
            while (true) {
                sample()
                delay(30_000)
            }
        }
    }

    fun sample() {
        val u = uid()
        val v = RustCore.videoNetBytes()
        val a = api.get()
        val dv = if (lastVideo < 0) 0 else if (v >= lastVideo) v - lastVideo else v
        val da = if (a >= lastApi) a - lastApi else a
        lastVideo = v
        lastApi = a
        if (u == 0L) return
        val add = dv + da
        val e = prefs.edit()
        if (add > 0) e.putLong("pend_$u", prefs.getLong("pend_$u", 0) + add)
        if (dv > 0) e.putLong("tot_video_$u", prefs.getLong("tot_video_$u", 0) + dv)
        if (da > 0) e.putLong("tot_api_$u", prefs.getLong("tot_api_$u", 0) + da)
        e.apply()
        _totals.value = mapOf("video" to prefs.getLong("tot_video_$u", 0), "api" to prefs.getLong("tot_api_$u", 0))
        // Faqat trafik bo'lsa — 6 soatda bir marta (boshqa yozuvlar bilan birga ketsa undan oldin ham).
        if (pendingBytes > 0 && System.currentTimeMillis() - lastReportAt > 6 * 3600_000L) onNeedFlush?.invoke()
    }

    fun markReported(sent: Long) {
        val u = uid()
        if (u == 0L) return
        prefs.edit().putLong("pend_$u", (prefs.getLong("pend_$u", 0) - sent).coerceAtLeast(0)).putLong("reported_at", System.currentTimeMillis()).apply()
        lastReportAt = System.currentTimeMillis()
    }
}

/** HTTP mijozi qabul qilgan baytlarni sanaydi (mahalliy 127.0.0.1 so'rovlari kirmaydi). */
class CountingInterceptor(private val meter: () -> TrafficMeter?) : Interceptor {
    override fun intercept(chain: Interceptor.Chain): Response {
        val res = chain.proceed(chain.request())
        val host = chain.request().url.host
        if (host == "127.0.0.1" || host == "localhost") return res
        val body = res.body ?: return res
        val m = meter() ?: return res
        val counting = object : ResponseBody() {
            override fun contentType() = body.contentType()
            override fun contentLength() = body.contentLength()
            override fun source(): BufferedSource {
                val src: Source = object : ForwardingSource(body.source()) {
                    override fun read(sink: Buffer, byteCount: Long): Long {
                        val n = super.read(sink, byteCount)
                        if (n > 0) m.addApi(n)
                        return n
                    }
                }
                return src.buffer()
            }
        }
        return res.newBuilder().body(counting).build()
    }
}

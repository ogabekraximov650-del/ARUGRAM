package uz.arumediatv.com.vd

import android.content.Context
import okhttp3.Request
import uz.arumediatv.com.data.Http
import java.io.IOException
import java.io.InterruptedIOException
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.ExecutionException
import java.util.concurrent.Executors
import java.util.concurrent.FutureTask
import java.util.concurrent.TimeUnit

/**
 * Yangi tizim (`vd`) faylining 1 MiB bo'laklarini olishning YAGONA yo'li: pleyer ([EncCacheDataSource]), 1 daqiqalik oyna ([VdAhead]) va
 * kadr oluvchi ([VdFrameSource]) hammasi shu yerdan so'raydi. Trafik tejash qoidalari:
 *  - bo'lak telefonda bo'lsa ([BlockCache]) tarmoqqa borilmaydi;
 *  - bir bo'lakni bir vaqtda ikki joy so'rasa BITTA so'rov ketadi (ikkinchisi kutadi);
 *  - so'rov aniq shu bo'lak uchun (`Range: start-end`, "oxirigacha" emas) — ortiqcha bayt kelmaydi;
 *  - so'rov alohida oqimda: pleyer surilsa/yopilsa ham boshlangan bo'lak oxirigacha olinib keshga yoziladi (chala bo'lak tashlab yuborilmaydi).
 */
object VdBlocks {
    private val pool = Executors.newCachedThreadPool { r -> Thread(r, "vd-block").apply { isDaemon = true } }
    private val inflight = ConcurrentHashMap<String, FutureTask<ByteArray>>()
    private val http by lazy { Http.client.newBuilder().readTimeout(60, TimeUnit.SECONDS).build() }

    fun blockLen(total: Long, idx: Long): Int = minOf(BlockCache.BLOCK.toLong(), total - idx * BlockCache.BLOCK).toInt()

    /** Ochilgan bo'lak (telefondan yoki tarmoqdan); xatoda [IOException]. */
    fun get(ctx: Context, name: String, total: Long, key: ByteArray, idx: Long): ByteArray {
        val len = blockLen(total, idx)
        if (idx < 0 || len <= 0) throw IOException("bo'lak fayldan tashqarida")
        BlockCache.read(ctx, name, idx, len)?.let { return it }
        val app = ctx.applicationContext
        val k = "$name:$idx"
        val task = inflight.computeIfAbsent(k) {
            FutureTask(java.util.concurrent.Callable<ByteArray> {
                try {
                    BlockCache.read(app, name, idx, len) ?: fetch(name, total, key, idx).also { BlockCache.write(app, name, idx, it) }
                } finally {
                    inflight.remove(k)
                }
            }).also { pool.execute(it) }
        }
        try {
            return task.get()
        } catch (e: InterruptedException) {
            Thread.currentThread().interrupt()
            throw InterruptedIOException("to'xtatildi")
        } catch (e: ExecutionException) {
            throw (e.cause as? IOException) ?: IOException(e.cause?.message ?: "bo'lak olinmadi")
        }
    }

    private fun fetch(name: String, total: Long, key: ByteArray, idx: Long): ByteArray {
        val start = idx * BlockCache.BLOCK
        val end = minOf(start + BlockCache.BLOCK, total) - 1
        val w = start / VdCtr.CHUNK
        var err: IOException? = null
        var force = false
        for (attempt in 0 until 3) {
            if (force || !VdWarm.isWarm(name, w)) {
                if (!VdApi.warmBlocking(name, w, force)) throw IOException("Kesh tayyor emas (oyna $w)")
                VdWarm.markWarm(name, w)
                force = false
            }
            try {
                val req = Request.Builder().url(VdApi.playUrl(name)).header("Range", "bytes=$start-$end").build()
                http.newCall(req).execute().use { r ->
                    if (r.code == 206 || (r.code == 200 && start == 0L && end == total - 1)) {
                        val enc = r.body?.bytes()
                        if (enc != null && enc.size.toLong() == end - start + 1) {
                            VdWarm.onRead(name, end + 1)
                            return VdCtr.apply(key, start, enc)
                        }
                        err = IOException("bo'lak chala keldi")
                    } else {
                        err = IOException("HTTP ${r.code}")
                        if (r.code == 503) { // Cloudflare keshida oyna yo'q — majburan qayta isitiladi
                            VdWarm.forget(name)
                            force = true
                        }
                    }
                }
            } catch (e: IOException) {
                err = e
            }
            if (attempt < 2) try { Thread.sleep(500L * (attempt + 1)) } catch (_: InterruptedException) {}
        }
        throw err ?: IOException("bo'lak olinmadi")
    }
}

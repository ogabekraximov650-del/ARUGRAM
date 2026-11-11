package uz.arumediatv.com.vd

import android.content.Context
import okhttp3.Request
import uz.arumediatv.com.data.Http
import java.io.IOException
import java.io.InterruptedIOException
import java.util.concurrent.CompletableFuture
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.ExecutionException
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/**
 * Yangi tizim (`vd`) faylining 1 MiB bo'laklarini olishning YAGONA yo'li: pleyer ([EncCacheDataSource]), 1 daqiqalik oyna ([VdAhead]) va
 * kadr oluvchi ([VdFrameSource]) hammasi shu yerdan so'raydi. Trafik tejash qoidalari:
 *  - bo'lak telefonda bo'lsa ([BlockCache]) tarmoqqa borilmaydi;
 *  - bir bo'lakni bir vaqtda ikki joy so'rasa BITTA so'rov ketadi (ikkinchisi kutadi);
 *  - so'rov aniq kerakli bo'lak(lar) uchun (`Range: start-end`, "oxirigacha" emas) — ortiqcha bayt kelmaydi; oldindan olish ([getRun]) 4 tagacha
 *    bo'lakni bitta so'rovda oladi (worker so'rovlari kam bo'lsin);
 *  - so'rov alohida oqimda: pleyer surilsa/yopilsa ham boshlangan bo'lak oxirigacha olinib keshga yoziladi (chala bo'lak tashlab yuborilmaydi).
 */
object VdBlocks {
    private val pool = Executors.newCachedThreadPool { r -> Thread(r, "vd-block").apply { isDaemon = true } }
    private val inflight = ConcurrentHashMap<String, CompletableFuture<ByteArray>>()
    private val http by lazy { Http.client.newBuilder().readTimeout(60, TimeUnit.SECONDS).build() }

    fun blockLen(total: Long, idx: Long): Int = minOf(BlockCache.BLOCK.toLong(), total - idx * BlockCache.BLOCK).toInt()

    private fun await(f: CompletableFuture<ByteArray>): ByteArray {
        try {
            return f.get()
        } catch (e: InterruptedException) {
            Thread.currentThread().interrupt()
            throw InterruptedIOException("to'xtatildi")
        } catch (e: ExecutionException) {
            throw (e.cause as? IOException) ?: IOException(e.cause?.message ?: "bo'lak olinmadi")
        }
    }

    /** Ochilgan bo'lak (telefondan yoki tarmoqdan); xatoda [IOException]. */
    fun get(ctx: Context, name: String, total: Long, key: ByteArray, idx: Long): ByteArray {
        val len = blockLen(total, idx)
        if (idx < 0 || len <= 0) throw IOException("bo'lak fayldan tashqarida")
        BlockCache.read(ctx, name, idx, len)?.let { return it }
        val app = ctx.applicationContext
        val k = "$name:$idx"
        val mine = CompletableFuture<ByteArray>()
        val other = inflight.putIfAbsent(k, mine)
        if (other != null) return await(other)
        pool.execute {
            try {
                mine.complete(BlockCache.read(app, name, idx, len) ?: fetch(name, total, key, idx).also { BlockCache.write(app, name, idx, it) })
            } catch (e: Throwable) {
                mine.completeExceptionally(e)
            } finally {
                inflight.remove(k, mine)
            }
        }
        return await(mine)
    }

    /**
     * SO'ROV SONINI KAMAYTIRISH: [idx] dan boshlab ketma-ket YETISHMAYOTGAN bo'laklarni (ko'pi bilan [max] ta, bitta 18 MiB oyna ichida) BITTA
     * so'rovda oladi; har bo'lak kelishi bilan keshga yoziladi. Shu bo'laklarni pleyer so'rasa — kutadi (ikki marta yuklanmaydi). Hammasi olinguncha
     * kutadi; birinchi bo'lak olinmasa [IOException].
     */
    fun getRun(ctx: Context, name: String, total: Long, key: ByteArray, idx: Long, max: Int) {
        val app = ctx.applicationContext
        val lastBlock = (total - 1) / BlockCache.BLOCK
        val perWin = VdCtr.CHUNK / BlockCache.BLOCK
        val claimed = ArrayList<CompletableFuture<ByteArray>>()
        var i = idx
        while (claimed.size < max && i in 0..lastBlock && i / perWin == idx / perWin && !BlockCache.has(app, name, i)) {
            val f = CompletableFuture<ByteArray>()
            if (inflight.putIfAbsent("$name:$i", f) != null) break
            claimed += f
            i++
        }
        if (claimed.size <= 1) { // bitta bo'lak (yoki uni boshqa joy allaqachon olayapti) — oddiy yo'l
            claimed.firstOrNull()?.let { inflight.remove("$name:$idx", it) }
            if (idx in 0..lastBlock) get(app, name, total, key, idx)
            return
        }
        pool.execute {
            var done = 0
            try {
                done = fetchRun(app, name, total, key, idx, claimed)
            } catch (_: Throwable) {
            }
            // Oqim o'rtada uzilsa: qolgan bo'laklar bittadan (o'z qayta urinishlari bilan) olinadi.
            for (n in done until claimed.size) {
                val b = idx + n
                try {
                    claimed[n].complete(fetch(name, total, key, b).also { BlockCache.write(app, name, b, it) })
                } catch (e: Throwable) {
                    claimed[n].completeExceptionally(e)
                }
            }
            for (n in claimed.indices) inflight.remove("$name:${idx + n}", claimed[n])
        }
        val first = await(claimed[0])
        if (first.isEmpty()) throw IOException("bo'lak bo'sh")
        for (n in 1 until claimed.size) try { await(claimed[n]) } catch (e: InterruptedIOException) { throw e } catch (_: IOException) {}
    }

    /** Bitta so'rov bilan ketma-ket bo'laklar; nechta bo'lak to'liq olingani qaytadi. */
    private fun fetchRun(ctx: Context, name: String, total: Long, key: ByteArray, idx: Long, out: List<CompletableFuture<ByteArray>>): Int {
        val start = idx * BlockCache.BLOCK
        val end = minOf((idx + out.size) * BlockCache.BLOCK, total) - 1
        val w = start / VdCtr.CHUNK
        if (!VdWarm.isWarm(name, w)) {
            if (!VdApi.warmBlocking(name, w)) return 0
            VdWarm.markWarm(name, w)
        }
        var done = 0
        val req = Request.Builder().url(VdApi.playUrl(name)).header("Range", "bytes=$start-$end").build()
        http.newCall(req).execute().use { r ->
            if (r.code != 206 && !(r.code == 200 && start == 0L && end == total - 1)) return 0
            val ins = r.body?.byteStream() ?: return 0
            val cipher = VdCtr.cipherAt(key, start)
            for (n in out.indices) {
                val b = idx + n
                val len = blockLen(total, b)
                val buf = ByteArray(len)
                var got = 0
                while (got < len) {
                    val k = ins.read(buf, got, len - got)
                    if (k < 0) return done
                    got += k
                }
                val plain = cipher.update(buf) ?: return done
                if (plain.size != len) return done
                BlockCache.write(ctx, name, b, plain)
                out[n].complete(plain)
                done++
                VdWarm.onRead(name, b * BlockCache.BLOCK + len)
            }
        }
        return done
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

package uz.arumediatv.com.vd

import android.content.Context
import okhttp3.Call
import okhttp3.Request
import uz.arumediatv.com.data.Http
import uz.arumediatv.com.data.VideoStat
import java.io.IOException
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.Semaphore
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicLong

/**
 * Yangi tizim (`vd`) fayllarini telefonga TO'LIQ yuklab olish. Bo'laklar pleyer ishlatadigan o'sha shifrlangan keshga ([BlockCache]) yoziladi:
 * ko'rish paytida yig'ilgan joy qayta yuklanmaydi, to'liq yuklangan fayl esa internetsiz ochiladi ([EncCacheDataSource] tarmoqqa bormaydi).
 * Har 18 MiB oyna avval isitiladi (Telegram -> Cloudflare kesh), so'ng yetishmayotgan joyi Range bilan olinib shifri ochiladi.
 */
object VdDownloads {
    private const val FILES_AT_ONCE = 2
    private const val WINDOWS_AT_ONCE = 3
    private const val MAX_TRIES = 40

    private class Job(val name: String, val total: Long) {
        @Volatile var cancelled = false
        @Volatile var queued = true
        @Volatile var retrying = false
        val got = AtomicLong()
        val calls: MutableSet<Call> = ConcurrentHashMap.newKeySet()
        var lastGot = 0L
        var lastAt = 0L
        var speed = 0L
    }

    private val jobs = ConcurrentHashMap<String, Job>()
    private val slots = Semaphore(FILES_AT_ONCE)
    private val http by lazy { Http.client.newBuilder().readTimeout(2, TimeUnit.MINUTES).build() }

    /** Fayl holati; `null` — bu fayl yangi tizim orqali telefonda yo'q va yuklanmayapti (eski yo'l hisoblaydi). */
    fun stat(ctx: Context, name: String): VideoStat? {
        if (name.isEmpty()) return null
        val job = jobs[name]
        if (job == null && !BlockCache.exists(ctx, name)) return null
        val total = job?.total ?: VdApi.stored(ctx, name)?.size ?: 0L
        val have = BlockCache.storedBytes(ctx, name, total)
        if (job == null) return VideoStat(total = total, downloaded = have)
        val now = System.currentTimeMillis()
        synchronized(job) {
            val g = job.got.get()
            if (job.lastAt == 0L) {
                job.lastAt = now
                job.lastGot = g
            } else if (now - job.lastAt >= 900) {
                job.speed = (g - job.lastGot) * 1000 / (now - job.lastAt)
                job.lastAt = now
                job.lastGot = g
            }
        }
        return VideoStat(total, have, downloading = true, queued = job.queued, retrying = job.retrying, speed = job.speed, streams = if (job.queued) 0 else WINDOWS_AT_ONCE)
    }

    fun isComplete(ctx: Context, name: String): Boolean = stat(ctx, name)?.complete == true

    fun start(ctx: Context, info: VdApi.Info) {
        val app = ctx.applicationContext
        if (info.size <= 0) return
        val job = Job(info.name, info.size)
        if (jobs.putIfAbsent(info.name, job) != null) return
        BlockCache.pin(app, info.name)
        Thread({
            var held = false
            try {
                while (!job.cancelled && !held) held = slots.tryAcquire(1, TimeUnit.SECONDS)
                if (!job.cancelled) {
                    job.queued = false
                    fetchAll(app, job, VdCtr.hex(info.key))
                }
            } catch (_: Exception) {
            } finally {
                if (held) slots.release()
                jobs.remove(info.name, job)
            }
        }, "vd-dl").start()
    }

    fun pause(name: String) {
        val job = jobs.remove(name) ?: return
        job.cancelled = true
        job.calls.forEach { runCatching { it.cancel() } }
    }

    fun delete(ctx: Context, name: String) {
        if (name.isEmpty()) return
        pause(name)
        BlockCache.remove(ctx, name)
    }

    fun wipe(ctx: Context) {
        jobs.keys.toList().forEach(::pause)
        BlockCache.wipe(ctx)
    }

    private fun fetchAll(ctx: Context, job: Job, key: ByteArray) {
        val windows = (job.total + VdCtr.CHUNK - 1) / VdCtr.CHUNK
        val next = AtomicLong()
        val workers = (0 until minOf(WINDOWS_AT_ONCE.toLong(), windows).toInt()).map {
            Thread({
                while (!job.cancelled) {
                    val w = next.getAndIncrement()
                    if (w >= windows) break
                    var tries = 0
                    while (!job.cancelled) {
                        val ok = try { window(ctx, job, key, w, force = tries > 0) } catch (_: DiskError) { job.cancelled = true; false } catch (_: Exception) { false }
                        if (ok) { job.retrying = false; break }
                        if (job.cancelled) break
                        if (++tries >= MAX_TRIES) { job.cancelled = true; break }
                        job.retrying = true
                        try { Thread.sleep(minOf(15_000L, 2000L * tries)) } catch (_: InterruptedException) {}
                    }
                }
            }, "vd-dl-w")
        }
        workers.forEach { it.start() }
        workers.forEach { it.join() }
    }

    private class DiskError : IOException("xotiraga yozilmadi")

    /** Bitta 18 MiB oyna: yetishmayotgan birinchi 1 MiB bo'lakdan oyna oxirigacha oqim bilan olinadi; bor bo'laklar qayta yozilmaydi. */
    private fun window(ctx: Context, job: Job, key: ByteArray, w: Long, force: Boolean): Boolean {
        val name = job.name
        val perWin = VdCtr.CHUNK / BlockCache.BLOCK
        val lastBlock = (job.total - 1) / BlockCache.BLOCK
        val first = w * perWin
        val last = minOf(first + perWin - 1, lastBlock)
        val miss = (first..last).firstOrNull { !BlockCache.has(ctx, name, it) } ?: return true
        if (!VdApi.warmBlocking(name, w, force)) return false
        if (job.cancelled) return false
        val start = miss * BlockCache.BLOCK
        val end = minOf((last + 1) * BlockCache.BLOCK, job.total) - 1
        val call = http.newCall(Request.Builder().url(VdApi.playUrl(name)).header("Range", "bytes=$start-$end").build())
        job.calls.add(call)
        try {
            call.execute().use { r ->
                if (r.code != 206 && !(r.code == 200 && start == 0L && end == job.total - 1)) return false
                val ins = r.body?.byteStream() ?: return false
                val cipher = VdCtr.cipherAt(key, start)
                for (idx in miss..last) {
                    val len = minOf(BlockCache.BLOCK.toLong(), job.total - idx * BlockCache.BLOCK).toInt()
                    val buf = ByteArray(len)
                    var n = 0
                    while (n < len) {
                        if (job.cancelled) return false
                        val k = ins.read(buf, n, len - n)
                        if (k < 0) return false
                        n += k
                        job.got.addAndGet(k.toLong())
                    }
                    val plain = cipher.update(buf) ?: return false
                    if (plain.size != len) return false
                    if (!BlockCache.has(ctx, name, idx) && !BlockCache.write(ctx, name, idx, plain)) throw DiskError()
                }
            }
            return true
        } finally {
            job.calls.remove(call)
        }
    }
}

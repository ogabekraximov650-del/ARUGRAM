package uz.arumediatv.com.vd

import android.content.Context
import android.net.Uri
import androidx.core.content.FileProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.Request
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.Http
import java.io.File
import java.util.concurrent.TimeUnit

/**
 * Yozishma fayllari (ovozli xabar, hujjat) yangi tizimdan: bo'laklar Cloudflare keshidan olinib, shifri ochilib vaqtinchalik faylga yoziladi
 * (`cacheDir/vdchat/<nom>`); MediaPlayer va tashqi ilovalar shu fayl bilan ishlaydi. Yangi tizimda bo'lmasa `null` — eski yo'l.
 */
object VdChat {
    private fun dir(ctx: Context) = File(ctx.cacheDir, "vdchat").apply { mkdirs() }

    /** Ochilgan fayl (kerak bo'lsa yuklab olinadi) yoki `null` (yangi tizimda yo'q / tarmoq xatosi). */
    suspend fun file(ctx: Context, name: String): File? = withContext(Dispatchers.IO) {
        if (name.isEmpty()) return@withContext null
        val token = AppServices.auth.sessionToken ?: return@withContext null
        val info = VdApi.lookup(ctx, name, token)?.takeIf { it.ready } ?: return@withContext null
        val out = File(dir(ctx), name)
        if (out.isFile && out.length() == info.size) return@withContext out
        val key = VdCtr.hex(info.key)
        val tmp = File(dir(ctx), "$name.part")
        try {
            val http = Http.client.newBuilder().readTimeout(2, TimeUnit.MINUTES).build()
            tmp.outputStream().use { o ->
                var n = 0L
                while (n * VdCtr.CHUNK < info.size) {
                    if (!VdApi.warmBlocking(name, n)) return@withContext null
                    val start = n * VdCtr.CHUNK
                    val end = minOf(start + VdCtr.CHUNK, info.size) - 1
                    val req = Request.Builder().url(VdApi.playUrl(name)).header("Range", "bytes=$start-$end").build()
                    val enc = http.newCall(req).execute().use { r -> if (r.code != 206 && r.code != 200) null else r.body?.bytes() } ?: return@withContext null
                    if (enc.size.toLong() != end - start + 1) return@withContext null
                    o.write(VdCtr.apply(key, start, enc))
                    n++
                }
            }
            if (tmp.length() != info.size || !tmp.renameTo(out)) return@withContext null
            out
        } catch (_: Exception) {
            null
        } finally {
            if (tmp.exists()) tmp.delete()
        }
    }

    /** MediaPlayer uchun fayl yo'li. */
    suspend fun path(ctx: Context, name: String): String? = file(ctx, name)?.absolutePath

    /** Tashqi ilovaga berish uchun `content://` manzili (FileProvider). */
    suspend fun contentUri(ctx: Context, name: String): Uri? = file(ctx, name)?.let {
        try { FileProvider.getUriForFile(ctx, "${ctx.packageName}.files", it) } catch (_: Exception) { null }
    }

    /** Vaqtinchalik ochilgan fayllarni tozalash (ilova boshlanganda 1 kundan eskilari). */
    fun trim(ctx: Context) {
        val cutoff = System.currentTimeMillis() - 24 * 3600 * 1000L
        runCatching { dir(ctx).listFiles()?.forEach { if (it.lastModified() < cutoff) it.delete() } }
    }
}

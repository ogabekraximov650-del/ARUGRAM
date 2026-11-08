package uz.arumediatv.com.vd

import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody
import okio.BufferedSink
import org.json.JSONObject
import uz.arumediatv.com.BuildConfig
import uz.arumediatv.com.data.Http
import uz.arumediatv.com.data.callJson
import java.io.File
import java.io.FileInputStream
import java.security.SecureRandom
import java.util.concurrent.TimeUnit

/**
 * Admin faylini yuklash — ACTIONS orqali (ARUDRIVE kabi): fayl qurilmada YANGI tasodifiy kalit bilan (AES-128-CTR, oqim bilan, xotiraga
 * to'liq o'qilmaydi) shifrlanib R2 ga to'g'ridan-to'g'ri yuboriladi (worker faqat imzolangan havola beradi); keyin worker saqlash
 * workflow'ini (`vdstore.yml`) uyg'otadi, u 18 MiB bo'laklarni Telegram'ga saqlaydi. Telegram hisobi kerak emas.
 */
object VdUpload {
    class Result(val key: String?, val error: String?)

    private class EncryptedBody(val file: File, val key: ByteArray, val onProgress: (Long, Long) -> Unit) : RequestBody() {
        override fun contentType() = "application/octet-stream".toMediaType()
        override fun contentLength() = file.length()
        override fun isOneShot() = false
        override fun writeTo(sink: BufferedSink) {
            val c = VdCtr.cipherAt(key, 0)
            val total = file.length()
            var sent = 0L
            val buf = ByteArray(256 * 1024)
            FileInputStream(file).use { ins ->
                while (true) {
                    val n = ins.read(buf)
                    if (n < 0) break
                    sink.write(c.update(buf, 0, n))
                    sent += n
                    onProgress(sent, total)
                }
            }
        }
    }

    fun upload(path: String, name: String, bearer: String, onProgress: (Long, Long) -> Unit): Result {
        val f = File(path)
        if (!f.isFile || f.length() <= 0) return Result(null, "Fayl o'qilmadi")
        val key = ByteArray(16).also { SecureRandom().nextBytes(it) }
        val keyHex = key.joinToString("") { "%02x".format(it) }
        val base = BuildConfig.API_BASE
        val init = Http.client.callJson("$base/api/vd/up/init", "POST",
            JSONObject().put("name", name).put("size", f.length()).put("key", keyHex), bearer)
        val url = init.json?.optString("url").orEmpty()
        if (!init.ok || url.isEmpty()) return Result(null, init.error ?: "Yuklash boshlanmadi (HTTP ${init.code})")
        // R2 havolasi imzolangan — ilova imzosi kerak emas (oddiy mijoz), uzoq yuklash uchun cheklovsiz.
        val plain: OkHttpClient = OkHttpClient.Builder().connectTimeout(20, TimeUnit.SECONDS).writeTimeout(10, TimeUnit.MINUTES)
            .readTimeout(10, TimeUnit.MINUTES).callTimeout(0, TimeUnit.SECONDS).build()
        var last = "xato"
        repeat(4) { attempt ->
            try {
                plain.newCall(Request.Builder().url(url).put(EncryptedBody(f, key, onProgress)).build()).execute().use { r ->
                    if (r.isSuccessful) {
                        val done = Http.client.callJson("$base/api/vd/up/complete", "POST", JSONObject().put("name", name), bearer)
                        return if (done.ok) Result(keyHex, null) else Result(null, done.error ?: "Tasdiqlanmadi (HTTP ${done.code})")
                    }
                    last = "R2 HTTP ${r.code}"
                }
            } catch (e: Exception) {
                last = e.message ?: e.javaClass.simpleName
            }
            Thread.sleep(2000L * (attempt + 1))
        }
        return Result(null, "Yuklab bo'lmadi: $last")
    }
}

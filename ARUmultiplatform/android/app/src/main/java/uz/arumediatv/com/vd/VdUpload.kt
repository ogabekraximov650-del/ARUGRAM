package uz.arumediatv.com.vd

import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody
import okhttp3.RequestBody.Companion.toRequestBody
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
 * Fayl yuklash. KICHIK fayl (<= 18 MiB: rasm, profil rasmi, ovoz...) — worker o'zi shifrlab Telegram'ga saqlaydi (`/api/vd/small`, bir necha soniya).
 * KATTA fayl (video...) — ACTIONS orqali (ARUDRIVE kabi): qurilmada YANGI tasodifiy kalit bilan (AES-128-CTR, oqim bilan) shifrlanib R2 ga
 * to'g'ridan-to'g'ri yuboriladi (worker faqat imzolangan havola beradi); keyin worker saqlash workflow'ini (`vdstore.yml`) uyg'otadi, u 18 MiB
 * bo'laklarni Telegram'ga saqlaydi. Telegram hisobi kerak emas.
 */
object VdUpload {
    class Result(val key: String?, val error: String?)

    /** Shu ishga tushishda yuklangan fayllarning kalitlari (asl videoni kodlash navbatiga qo'yishda kerak). */
    private val keys = java.util.concurrent.ConcurrentHashMap<String, String>()
    fun keyOf(name: String): String = keys[name].orEmpty()

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

    /** Ochiq faylni oqim bilan yuboradi (kichik fayllar: worker o'zi shifrlaydi). */
    private class PlainBody(val file: File, val onProgress: (Long, Long) -> Unit) : RequestBody() {
        override fun contentType() = "application/octet-stream".toMediaType()
        override fun contentLength() = file.length()
        override fun isOneShot() = false
        override fun writeTo(sink: BufferedSink) {
            val total = file.length()
            var sent = 0L
            val buf = ByteArray(128 * 1024)
            FileInputStream(file).use { ins ->
                while (true) {
                    val n = ins.read(buf)
                    if (n < 0) break
                    sink.write(buf, 0, n)
                    sent += n
                    onProgress(sent, total)
                }
            }
        }
    }

    /**
     * KICHIK fayl (<= 18 MiB): worker o'zi shifrlab Telegram'ga saqlaydi (R2/Actions kerak emas, bir necha soniya). `null` — bu yo'l ishlamadi
     * (katta fayl, worker xatosi) va R2 + Actions yo'liga o'tiladi; ruxsat/kirish xatosi esa o'sha yo'lda ham rad etiladi, shu sabab darrov qaytariladi.
     */
    /** Kichik yo'l nega ishlamagani (R2 yo'li ham xato bersa foydalanuvchiga ko'rsatiladi). */
    @Volatile private var smallNote = ""

    private fun smallUpload(f: File, name: String, bearer: String, onProgress: (Long, Long) -> Unit): Result? {
        smallNote = ""
        val client = Http.client.newBuilder().writeTimeout(2, TimeUnit.MINUTES).readTimeout(2, TimeUnit.MINUTES).build()
        val req = Request.Builder()
            .url(BuildConfig.API_BASE + "/api/vd/small?name=" + java.net.URLEncoder.encode(name, "UTF-8"))
            .header("Authorization", "Bearer $bearer")
            .put(PlainBody(f, onProgress)).build()
        return try {
            client.newCall(req).execute().use { r ->
                val j = runCatching { JSONObject(r.body?.string().orEmpty()) }.getOrNull()
                when {
                    r.isSuccessful && j?.optBoolean("ok") == true && j.optString("key").length == 32 -> {
                        keys[name] = j.optString("key")
                        Result(j.optString("key"), null)
                    }
                    r.code == 401 || r.code == 403 -> Result(null, j?.optString("error")?.takeIf { it.isNotEmpty() } ?: "Ruxsat yo'q")
                    else -> { smallNote = "kichik yo'l HTTP ${r.code} ${j?.optString("error").orEmpty().take(120)}"; null }
                }
            }
        } catch (e: Exception) {
            smallNote = "kichik yo'l: ${e.javaClass.simpleName} ${e.message.orEmpty().take(80)}"
            null
        }
    }

    fun upload(path: String, name: String, bearer: String, onProgress: (Long, Long) -> Unit): Result {
        val f = File(path)
        if (!f.isFile || f.length() <= 0) return Result(null, "Fayl o'qilmadi")
        if (f.length() <= VdCtr.CHUNK) smallUpload(f, name, bearer, onProgress)?.let { return it }
        // Katta fayl: yangi tasodifiy kalit bilan har 18 MiB bo'lak shifrlanib KETMA-KET WORKER orqali yuboriladi (R2 va Actions kerak emas);
        // worker har bo'lakni Telegram'ga saqlaydi. Uzilsa shu joydan davom etadi.
        val key = ByteArray(16).also { SecureRandom().nextBytes(it) }
        val keyHex = key.joinToString("") { "%02x".format(it) }
        val base = BuildConfig.API_BASE
        val size = f.length()
        val init = Http.client.callJson("$base/api/vd/cup/init", "POST", JSONObject().put("name", name).put("size", size).put("key", keyHex), bearer)
        if (!init.ok) return Result(null, (init.error ?: "Yuklash boshlanmadi (HTTP ${init.code})") + if (smallNote.isNotEmpty()) " [$smallNote]" else "")
        if (init.json?.optBoolean("ready") == true) { keys[name] = keyHex; onProgress(size, size); return Result(keyHex, null) }
        val chunk = VdCtr.CHUNK
        val total = ((size + chunk - 1) / chunk).toInt()
        var done = init.json?.optInt("done", 0) ?: 0
        onProgress(minOf(size, done * chunk), size)
        val client = Http.client.newBuilder().writeTimeout(5, TimeUnit.MINUTES).readTimeout(5, TimeUnit.MINUTES).callTimeout(0, TimeUnit.SECONDS).build()
        java.io.RandomAccessFile(f, "r").use { raf ->
            while (done < total) {
                val off = done * chunk
                val buf = ByteArray(minOf(chunk, size - off).toInt())
                raf.seek(off)
                raf.readFully(buf)
                val enc = VdCtr.cipherAt(key, off).update(buf)
                var ok = false
                var last = "xato"
                for (attempt in 0 until 5) {
                    if (ok) break
                    try {
                        val req = Request.Builder()
                            .url("$base/api/vd/cup/chunk?name=" + java.net.URLEncoder.encode(name, "UTF-8") + "&i=$done")
                            .header("Authorization", "Bearer $bearer")
                            .put(enc.toRequestBody("application/octet-stream".toMediaType())).build()
                        client.newCall(req).execute().use { r ->
                            val j = runCatching { JSONObject(r.body?.string().orEmpty()) }.getOrNull()
                            when {
                                r.isSuccessful && j?.optBoolean("ok") == true -> { ok = true; done = maxOf(done + 1, j.optInt("done", 0)) }
                                r.code == 409 && j?.has("done") == true -> { done = j.optInt("done"); ok = true }
                                r.code == 401 || r.code == 403 -> return Result(null, j?.optString("error")?.takeIf { it.isNotEmpty() } ?: "Ruxsat yo'q")
                                else -> last = "HTTP ${r.code} ${j?.optString("error").orEmpty().take(120)}"
                            }
                        }
                    } catch (e: Exception) {
                        last = e.message ?: e.javaClass.simpleName
                    }
                    if (!ok) Thread.sleep(2000L * (attempt + 1))
                }
                if (!ok) return Result(null, "Yuklab bo'lmadi: $last")
                onProgress(minOf(size, done * chunk), size)
            }
        }
        val fin = Http.client.callJson("$base/api/vd/cup/complete", "POST", JSONObject().put("name", name), bearer)
        return if (fin.ok) { keys[name] = keyHex; Result(keyHex, null) } else Result(null, fin.error ?: "Tasdiqlanmadi (HTTP ${fin.code})")
    }
}

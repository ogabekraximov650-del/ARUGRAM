package uz.arudrive.app.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import okhttp3.Interceptor
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import uz.arudrive.app.BuildConfig
import java.util.concurrent.TimeUnit
import javax.crypto.Mac
import javax.crypto.spec.SecretKeySpec

/** So'rov imzosi (worker `verify_app_sig` bilan bir xil): `v2.<vaqt>.<hex(HMAC-SHA256("<vaqt>.<METOD>.<yo'l>"))>`. */
object Sign {
    fun hmacHex(secret: String, data: String): String {
        val m = Mac.getInstance("HmacSHA256")
        m.init(SecretKeySpec(secret.toByteArray(), "HmacSHA256"))
        return m.doFinal(data.toByteArray()).joinToString("") { "%02x".format(it) }
    }

    fun appSig(secret: String, ts: Long, method: String, path: String) =
        "v2.$ts.${hmacHex(secret, "$ts.${method.uppercase()}.$path")}"

    /** Pleyer tokeni (`?t=`): ExoPlayer sarlavha qo'sha olmaydi. Matn `"play.<yo'l>.<muddat>"`. */
    fun playToken(secret: String, path: String, exp: Long) = "$exp.${hmacHex(secret, "play.$path.$exp")}"
}

private class SignInterceptor : Interceptor {
    override fun intercept(chain: Interceptor.Chain): okhttp3.Response {
        val req = chain.request()
        val ts = System.currentTimeMillis() / 1000
        val sig = Sign.appSig(BuildConfig.APP_SECRET, ts, req.method, req.url.encodedPath)
        return chain.proceed(req.newBuilder().header("X-App-Sig", sig).build())
    }
}

object Api {
    const val PART_FALLBACK = 64L * 1024 * 1024
    /** Worker bilan bir xil: kesh oynasi = BITTA bo'lak (18 MiB). */
    const val WARM_WINDOW = 18L * 1024 * 1024
    private val json = Json { ignoreUnknownKeys = true }
    private val JSON = "application/json".toMediaType()

    val client: OkHttpClient by lazy {
        OkHttpClient.Builder()
            .addInterceptor(SignInterceptor())
            .connectTimeout(15, TimeUnit.SECONDS)
            .readTimeout(120, TimeUnit.SECONDS)
            .writeTimeout(120, TimeUnit.SECONDS)
            .build()
    }

    private fun url(path: String) = BuildConfig.API_BASE + path

    private fun call(req: Request): JsonObject {
        client.newCall(req).execute().use { r ->
            val text = r.body?.string().orEmpty()
            if (!r.isSuccessful) throw java.io.IOException("HTTP ${r.code} ${text.take(120)}")
            return json.parseToJsonElement(text).jsonObject
        }
    }

    suspend fun list(): List<DriveFile> = withContext(Dispatchers.IO) {
        client.newCall(Request.Builder().url(url("/api/bd/list")).build()).execute().use { r ->
            val text = r.body?.string().orEmpty()
            if (!r.isSuccessful) throw java.io.IOException("HTTP ${r.code}")
            json.decodeFromString(DriveList.serializer(), text).files
        }
    }

    fun thumbUrl(id: String) = url("/api/bd/thumb/$id.jpg")

    class Init(val id: String, val uploadId: String, val partSize: Long)

    /** Butun fayl R2 ga yuklanadi (ko'p qismli): yozuv + R2 yuklash identifikatori. */
    suspend fun init(name: String, size: Long, kind: String, mime: String, w: Int, h: Int, dur: Long): Init = withContext(Dispatchers.IO) {
        val body = buildJsonObject {
            put("name", JsonPrimitive(name)); put("size", JsonPrimitive(size)); put("kind", JsonPrimitive(kind))
            put("mime", JsonPrimitive(mime)); put("w", JsonPrimitive(w)); put("h", JsonPrimitive(h)); put("dur", JsonPrimitive(dur))
        }.toString().toRequestBody(JSON)
        val o = call(Request.Builder().url(url("/api/bd/init")).post(body).build())
        Init(o["id"]!!.jsonPrimitive.content, o["upload_id"]!!.jsonPrimitive.content, o["part_size"]?.jsonPrimitive?.contentOrNull?.toLongOrNull() ?: PART_FALLBACK)
    }

    /**
     * Fayldan BIR QISM (offset..offset+len) OQIM bilan yuboriladi: xotirada bufer YO'Q (64 MiB qismlar parallel ketsa ham xotira
     * o'smaydi). Qayta urinishda tana qayta ochiladi. [onBytes] — shu qismdan yuborilgan bayt.
     */
    class FileSliceBody(
        private val ctx: android.content.Context,
        private val uri: android.net.Uri,
        private val offset: Long,
        private val len: Long,
        private val onBytes: (Long) -> Unit,
    ) : okhttp3.RequestBody() {
        override fun contentType() = "application/octet-stream".toMediaType()
        override fun contentLength() = len
        override fun isOneShot() = false
        override fun writeTo(sink: okio.BufferedSink) {
            val pfd = ctx.contentResolver.openFileDescriptor(uri, "r")
            val fis = if (pfd != null) java.io.FileInputStream(pfd.fileDescriptor) else null
            val input: java.io.InputStream = try {
                if (fis != null) { fis.channel.position(offset); fis } else throw java.io.IOException("fd yo'q")
            } catch (_: Exception) {
                // Qidirib bo'lmaydigan manba: boshidan o'tkazib yuboriladi.
                val st = ctx.contentResolver.openInputStream(uri)!!
                var left = offset
                while (left > 0) { val k = st.skip(left); if (k <= 0) break; left -= k }
                st
            }
            try {
                val buf = ByteArray(64 * 1024)
                var done = 0L
                while (done < len) {
                    val r = input.read(buf, 0, minOf(buf.size.toLong(), len - done).toInt())
                    if (r < 0) throw java.io.IOException("fayl kutilganidan qisqa")
                    sink.write(buf, 0, r)
                    done += r
                    onBytes(done)
                }
            } finally {
                runCatching { input.close() }
                runCatching { pfd?.close() }
            }
        }
    }

    /** R2 ga `n`-qismni (1 dan) yuboradi; katta fayl soatlab yuklanadi, shu sabab 8 marta urinadi (kutish 15 s gacha); qaytadi: ETag. */
    suspend fun part(id: String, uploadId: String, n: Int, body: okhttp3.RequestBody, note: (String) -> Unit): String = withContext(Dispatchers.IO) {
        var last: Exception? = null
        repeat(8) { attempt ->
            try {
                note("Yuborilmoqda (${attempt + 1}/8)…")
                val u = url("/api/bd/part").toHttpUrl().newBuilder().addQueryParameter("id", id).addQueryParameter("uid", uploadId).addQueryParameter("n", n.toString()).build()
                val etag = call(Request.Builder().url(u).put(body).build())["etag"]!!.jsonPrimitive.content
                note("Qism qabul qilindi")
                return@withContext etag
            } catch (e: Exception) {
                last = e
                note("Urinish ${attempt + 1} xato: ${e.message ?: e.javaClass.simpleName}")
                kotlinx.coroutines.delay(minOf(2000L * (attempt + 1), 15_000L))
            }
        }
        throw last ?: java.io.IOException("qism yuborilmadi")
    }

    suspend fun thumb(id: String, jpeg: ByteArray) = withContext(Dispatchers.IO) {
        val u = url("/api/bd/thumb").toHttpUrl().newBuilder().addQueryParameter("id", id).build()
        call(Request.Builder().url(u).post(jpeg.toRequestBody("image/jpeg".toMediaType())).build())
    }

    suspend fun finish(id: String, uploadId: String, parts: List<Pair<Int, String>>) = withContext(Dispatchers.IO) {
        val body = buildJsonObject {
            put("id", JsonPrimitive(id)); put("upload_id", JsonPrimitive(uploadId))
            put("parts", kotlinx.serialization.json.JsonArray(parts.map { (n, e) -> buildJsonObject { put("n", JsonPrimitive(n)); put("etag", JsonPrimitive(e)) } }))
        }.toString().toRequestBody(JSON)
        call(Request.Builder().url(url("/api/bd/finish")).post(body).build())
    }

    class WarmResult(val ok: Boolean, val text: String, val total: Long = 0)

    /** Bitta isitish so'rovi: natija va tashxis matni (HTTP kodi, worker holati, vaqt, Cloudflare markazi). */
    fun warmDetailed(id: String, window: Long, force: Boolean = false): WarmResult {
        val req = Request.Builder().url(url("/api/warm/bd~$id?w=$window" + if (force) "&force=1" else "")).build()
        val t0 = System.currentTimeMillis()
        return try {
            client.newBuilder().readTimeout(15, TimeUnit.MINUTES).callTimeout(0, TimeUnit.SECONDS).build().newCall(req).execute().use { r ->
                val body = r.body?.string().orEmpty()
                val st = runCatching { json.parseToJsonElement(body).jsonObject["status"]?.jsonPrimitive?.contentOrNull }.getOrNull().orEmpty()
                val ray = r.header("cf-ray")?.substringAfterLast('-').orEmpty()
                val ms = System.currentTimeMillis() - t0
                val total = runCatching { json.parseToJsonElement(body).jsonObject["total"]?.jsonPrimitive?.longOrNull }.getOrNull() ?: 0L
                val ok = r.isSuccessful && (st == "cached" || st == "warmed")
                if (ok && total > 0) Warm.totals[id] = total
                WarmResult(ok, "HTTP ${r.code} ${st.ifEmpty { body.take(60) }} · ${ms / 1000.0}s · $ray", total)
            }
        } catch (e: Exception) {
            WarmResult(false, "Xato: ${e.javaClass.simpleName} ${e.message.orEmpty().take(80)} · ${(System.currentTimeMillis() - t0) / 1000.0}s")
        }
    }

    /**
     * `window` oynasini Telegramdan Cloudflare keshiga isitadi (worker `warm`). Qaytadi: true — kesh tayyor.
     *
     * ULANISH USHLAB TURILADI (ARUMEDIATV `video_cache.rs` dagi `warm_agent` kabi): worker isitishni shu so'rov ochiq turgan
     * paytda bajaradi. Cloudflare fon vazifasiga (`waitUntil`) javobdan keyin atigi 30 soniya beradi, mijoz ulanib turganda esa
     * davomiylikka chegara yo'q — shuning uchun o'qish chegarasi 15 daqiqa (katta oyna ~468 MiB 180 soniyada ko'chmasligi mumkin).
     */
    fun warmBlocking(id: String, window: Long): Boolean = warmDetailed(id, window).ok

    /** ARUMEDIATV kabi: birinchi urinish oddiy, muvaffaqiyatsiz bo'lsa MAJBURAN (`force=1`) qayta isitiladi. */
    suspend fun warm(id: String, window: Long = 0, note: (String) -> Unit = {}, forceFirst: Boolean = false): Boolean = withContext(Dispatchers.IO) {
        repeat(3) { n ->
            note("Worker'ga so'rov (${n + 1}/3)…")
            val r = warmDetailed(id, window, force = forceFirst || n > 0)
            note(r.text)
            if (r.ok) return@withContext true
            kotlinx.coroutines.delay(1500)
        }
        false
    }

    /** ExoPlayer manzili: yo'l imzolangan token bilan (1 soat). */
    fun playUrl(id: String): String {
        val path = "/api/play/bd~$id"
        val exp = System.currentTimeMillis() / 1000 + 3600
        return url(path) + "?t=" + Sign.playToken(BuildConfig.APP_SECRET, path, exp)
    }
}

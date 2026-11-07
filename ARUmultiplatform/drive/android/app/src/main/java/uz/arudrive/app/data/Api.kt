package uz.arudrive.app.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
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
    const val PART_FALLBACK = 18 * 1024 * 1024
    /** Worker bilan bir xil: kesh oynasi 26 ta 18 MiB bo'lak. */
    const val WARM_WINDOW = 26L * 18 * 1024 * 1024
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

    class Init(val id: String, val partSize: Int)

    /** Yangi fayl yozuvi (R2'siz): bo'laklar `chunk` bilan to'g'ridan-to'g'ri worker orqali Telegram'ga ketadi. */
    suspend fun newFile(name: String, size: Long, kind: String, mime: String, w: Int, h: Int, dur: Long): Init = withContext(Dispatchers.IO) {
        val body = buildJsonObject {
            put("name", JsonPrimitive(name)); put("size", JsonPrimitive(size)); put("kind", JsonPrimitive(kind))
            put("mime", JsonPrimitive(mime)); put("w", JsonPrimitive(w)); put("h", JsonPrimitive(h)); put("dur", JsonPrimitive(dur))
        }.toString().toRequestBody(JSON)
        val o = call(Request.Builder().url(url("/api/bd/new")).post(body).build())
        Init(o["id"]!!.jsonPrimitive.content, o["part_size"]?.jsonPrimitive?.contentOrNull?.toIntOrNull() ?: PART_FALLBACK)
    }

    /** `n`-bo'lak (0 dan) — aniq `part_size` bayt (oxirgisi qisqa). 3 marta urinadi. */
    suspend fun chunk(id: String, n: Int, bytes: ByteArray, len: Int) = withContext(Dispatchers.IO) {
        var last: Exception? = null
        repeat(3) { attempt ->
            try {
                val body = bytes.toRequestBody("application/octet-stream".toMediaType(), 0, len)
                val u = url("/api/bd/chunk").toHttpUrl().newBuilder().addQueryParameter("id", id).addQueryParameter("n", n.toString()).build()
                call(Request.Builder().url(u).put(body).build())
                return@withContext
            } catch (e: Exception) {
                last = e
                kotlinx.coroutines.delay(2000L * (attempt + 1))
            }
        }
        throw last ?: java.io.IOException("bo'lak yuborilmadi")
    }

    suspend fun seal(id: String) = withContext(Dispatchers.IO) {
        val body = buildJsonObject { put("id", JsonPrimitive(id)) }.toString().toRequestBody(JSON)
        call(Request.Builder().url(url("/api/bd/seal")).post(body).build())
    }

    suspend fun thumb(id: String, jpeg: ByteArray) = withContext(Dispatchers.IO) {
        val u = url("/api/bd/thumb").toHttpUrl().newBuilder().addQueryParameter("id", id).build()
        call(Request.Builder().url(u).post(jpeg.toRequestBody("image/jpeg".toMediaType())).build())
    }

    suspend fun finish(id: String, uploadId: String, parts: List<Pair<Int, String>>) = withContext(Dispatchers.IO) {
        val body = buildJsonObject {
            put("id", JsonPrimitive(id)); put("upload_id", JsonPrimitive(uploadId))
            put("parts", JsonArray(parts.map { (n, e) -> buildJsonObject { put("n", JsonPrimitive(n)); put("etag", JsonPrimitive(e)) } }))
        }.toString().toRequestBody(JSON)
        call(Request.Builder().url(url("/api/bd/finish")).post(body).build())
    }

    /**
     * `window` oynasini Telegramdan Cloudflare keshiga isitadi (worker `warm`). Qaytadi: true — kesh tayyor.
     * Bloklaydi (bir necha soniya), shu sabab IO oqimida chaqiriladi.
     */
    fun warmBlocking(id: String, window: Long): Boolean {
        val req = Request.Builder().url(url("/api/warm/bd~$id?w=$window")).build()
        return try {
            client.newBuilder().readTimeout(180, TimeUnit.SECONDS).build().newCall(req).execute().use { r ->
                val st = runCatching { json.parseToJsonElement(r.body?.string().orEmpty()).jsonObject["status"]?.jsonPrimitive?.contentOrNull }.getOrNull().orEmpty()
                r.isSuccessful && (st == "cached" || st == "warmed")
            }
        } catch (_: Exception) {
            false
        }
    }

    suspend fun warm(id: String, window: Long = 0): Boolean = withContext(Dispatchers.IO) {
        repeat(3) { if (warmBlocking(id, window)) return@withContext true else kotlinx.coroutines.delay(1500) }
        false
    }

    /** ExoPlayer manzili: yo'l imzolangan token bilan (1 soat). */
    fun playUrl(id: String): String {
        val path = "/api/play/bd~$id"
        val exp = System.currentTimeMillis() / 1000 + 3600
        return url(path) + "?t=" + Sign.playToken(BuildConfig.APP_SECRET, path, exp)
    }
}

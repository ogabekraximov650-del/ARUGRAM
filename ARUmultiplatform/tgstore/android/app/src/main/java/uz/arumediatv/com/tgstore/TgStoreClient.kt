package uz.arumediatv.com.tgstore

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.File
import java.net.URLEncoder
import java.util.concurrent.TimeUnit

/**
 * UZ: `tgstore-test` worker mijozi. Ilova qaysi yozuv qaysi bo'lakda ekanini ESLAB QOLADI (`where`) va bo'lakni
 *     diskda saqlaydi (bo'lak versiyasi o'zgarmas, shuning uchun keshni bekor qilish kerak emas).
 * EN: Client for the `tgstore-test` worker. The app REMEMBERS which chunk holds which record (`where`) and keeps
 *     chunks on disk (a chunk version is immutable, so no cache invalidation is needed).
 * RU: Klient worker'a `tgstore-test`. Prilozhenie ZAPOMINAET, v kakom chanke zapis (`where`), i khranit chanki na
 *     diske (versiya chanka neizmenna, inval'idatsiya keshа ne nuzhna).
 */
class TgStoreClient(
    private val base: String,
    private val key: String,
    private val cacheDir: File,
    private val http: OkHttpClient = OkHttpClient.Builder().callTimeout(60, TimeUnit.SECONDS).build(),
) {
    data class Where(val no: Int, val ver: Int)
    data class Timed<T>(val value: T, val ms: Long, val note: String = "")

    private val where = HashMap<String, Where>()   // "table/key" -> chunk
    private val json = Json { ignoreUnknownKeys = true }
    var diskHits = 0; private set
    var netHits = 0; private set

    private fun req(path: String) = Request.Builder().url(base.trimEnd('/') + path).header("Authorization", "Bearer $key")
    private fun enc(s: String) = URLEncoder.encode(s, "UTF-8").replace("+", "%20")

    private fun call(r: Request): Pair<Int, String> = http.newCall(r).execute().use { it.code to (it.body?.string() ?: "") }

    suspend fun health(): Timed<Boolean> = timed {
        val t = http.newCall(Request.Builder().url(base.trimEnd('/') + "/health").build()).execute().use { it.isSuccessful }
        t
    }

    /** UZ: bir paket yozish (put/del). Javobdan `where` yangilanadi. EN: one batch of puts/dels; `where` is refreshed from the reply. RU: odin paket put/del. */
    suspend fun write(table: String, puts: Map<String, JsonElement>, dels: List<String> = emptyList()): Timed<Int> = timed {
        val body = buildJsonObject {
            put("ops", buildJsonArray {
                for ((k, v) in puts) add(buildJsonObject { put("op", "put"); put("k", k); put("v", v) })
                for (k in dels) add(buildJsonObject { put("op", "del"); put("k", k) })
            })
        }.toString()
        val (code, text) = call(req("/w/$table").post(body.toRequestBody("application/json".toMediaType())).build())
        if (code != 200) error("write $code: $text")
        val results = json.parseToJsonElement(text).jsonObject["results"]!!.jsonObject
        synchronized(where) {
            for ((k, v) in results) {
                if (v is JsonObject) where["$table/$k"] = Where(v["no"]!!.jsonPrimitive.content.toInt(), v["ver"]!!.jsonPrimitive.content.toInt())
                else where.remove("$table/$k")
            }
        }
        results.size
    }

    /**
     * UZ: o'qish. Joyi ma'lum bo'lsa — diskdagi bo'lak (so'rovsiz); bo'lmasa bo'lak tarmoqdan (worker keshidan) olinadi.
     * EN: read. If the location is known — chunk from disk (no request); otherwise the chunk comes from the network (worker cache).
     * RU: chtenie. Esli mesto izvestno — chank s diska (bez zaprosa); inache s seti (kesh worker'a).
     */
    suspend fun read(table: String, k: String): Timed<JsonElement?> = timed {
        val w = synchronized(where) { where["$table/$k"] } ?: run {
            val (c, t) = call(req("/r/$table/${enc(k)}").build())
            if (c == 404) return@timed null
            if (c != 200) error("read $c: $t")
            val o = json.parseToJsonElement(t).jsonObject
            val ww = Where(o["no"]!!.jsonPrimitive.content.toInt(), o["ver"]!!.jsonPrimitive.content.toInt())
            synchronized(where) { where["$table/$k"] = ww }
            ww
        }
        ChunkCodec.decode(chunk(table, w)).get(k)
    }

    private fun chunk(table: String, w: Where): ByteArray {
        val f = File(cacheDir, "chunks/${table}_${w.no}_v${w.ver}")
        if (f.exists()) { diskHits++; return f.readBytes() }
        val r = http.newCall(req("/c/$table/${w.no}/${w.ver}").build()).execute()
        r.use {
            if (!it.isSuccessful) error("chunk ${it.code}")
            val b = it.body!!.bytes()
            f.parentFile?.mkdirs(); f.writeBytes(b)
            netHits++
            return b
        }
    }

    suspend fun stats(table: String): Timed<String> = timed { call(req("/stats/$table").build()).second }
    suspend fun gc(table: String): Timed<String> = timed { call(req("/gc/$table").post("".toRequestBody()).build()).second }
    suspend fun backup(table: String): Timed<String> = timed { call(req("/backup/$table").post("".toRequestBody()).build()).second }
    suspend fun restore(table: String): Timed<String> = timed { call(req("/restore/$table").post("".toRequestBody()).build()).second }

    fun forgetLocal() { synchronized(where) { where.clear() }; File(cacheDir, "chunks").deleteRecursively(); diskHits = 0; netHits = 0 }

    private suspend fun <T> timed(block: () -> T): Timed<T> = withContext(Dispatchers.IO) {
        val t = System.currentTimeMillis()
        val v = block()
        Timed(v, System.currentTimeMillis() - t)
    }
}

/** UZ: bo'lak formati: har qator `[kalit, qiymat]` JSON. EN: chunk format: one JSON `[key, value]` per line. RU: formaty chanka: stroka `[klyuch, znachenie]`. */
object ChunkCodec {
    fun decode(bytes: ByteArray): Map<String, JsonElement> {
        val out = LinkedHashMap<String, JsonElement>()
        for (line in String(bytes, Charsets.UTF_8).split('\n')) {
            if (line.isEmpty()) continue
            val a = Json.parseToJsonElement(line) as JsonArray
            out[a[0].jsonPrimitive.content] = a[1]
        }
        return out
    }

    fun encode(map: Map<String, JsonElement>): ByteArray =
        buildString { for ((k, v) in map) append(JsonArray(listOf(JsonPrimitive(k), v)).toString()).append('\n') }.toByteArray(Charsets.UTF_8)
}

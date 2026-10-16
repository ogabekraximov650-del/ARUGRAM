package uz.arumediatv.com.data

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import uz.arumediatv.com.BuildConfig
import java.io.File

sealed interface FetchResult {
    data class Fresh(val items: List<Season>) : FetchResult
    data object NotModified : FetchResult
    data class Failed(val message: String) : FetchResult
}

/**
 * Bo'limlar ro'yxati: diskdagi kesh (oflayn) + `If-None-Match` (o'zgarmagan bo'lsa server 304 qaytaradi — Turso'ga bormaydi).
 * Flutter ilovasidagi `SeasonsRepo` bilan bir xil mantiq.
 */
class SeasonsRepository(
    context: Context,
    private val client: OkHttpClient = Http.client,
    private val baseUrl: String = BuildConfig.API_BASE,
) {
    private val dir = File(context.filesDir, "cache").apply { mkdirs() }
    private val bodyFile = File(dir, "seasons.json")
    private val etagFile = File(dir, "seasons.etag")

    /** Diskdagi oxirgi ro'yxat (tarmoqsiz). Yo'q yoki buzuq bo'lsa `null`. */
    suspend fun cached(): List<Season>? = withContext(Dispatchers.IO) {
        try {
            if (!bodyFile.exists()) null else SeasonParser.parse(bodyFile.readText()).takeIf { it.isNotEmpty() }
        } catch (_: Exception) {
            null
        }
    }

    suspend fun fetch(force: Boolean = false): FetchResult = withContext(Dispatchers.IO) {
        try {
            val rb = Request.Builder().url("$baseUrl/api/seasons")
            val etag = if (!force && bodyFile.exists() && etagFile.exists()) etagFile.readText().trim() else ""
            if (etag.isNotEmpty()) rb.header("If-None-Match", etag)
            client.newCall(rb.build()).execute().use { r ->
                when {
                    r.code == 304 -> FetchResult.NotModified
                    r.isSuccessful -> {
                        val body = r.body?.string().orEmpty()
                        val items = SeasonParser.parse(body)
                        bodyFile.writeText(body)
                        r.header("ETag")?.let { etagFile.writeText(it) } ?: etagFile.delete()
                        FetchResult.Fresh(items)
                    }
                    r.code == 403 -> FetchResult.Failed("Server ilovani tanimadi (403)")
                    else -> FetchResult.Failed("Server xatosi (${r.code})")
                }
            }
        } catch (e: Exception) {
            FetchResult.Failed(e.message ?: "Tarmoq xatosi")
        }
    }
}

package uz.arutube.app

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/** Worker (`ARUmultiplatform/tube/worker`) bilan gaplashadi; so'rovlar Rust yadrosi orqali. */
object Api {
    val json = Json { ignoreUnknownKeys = true; coerceInputValues = true }
    private val base get() = BuildConfig.API_BASE
    private val token get() = BuildConfig.APP_TOKEN

    private inline fun <reified T> parse(text: String, fallback: (String) -> T): T =
        try {
            json.decodeFromString<T>(text)
        } catch (e: Exception) {
            fallback(if (text.isBlank()) "Javob yo'q" else "Javob tushunarsiz: ${text.take(120)}")
        }

    suspend fun videos(page: Int): VideoPage = withContext(Dispatchers.IO) {
        parse(TubeCore.call("GET", "$base/api/videos?page=$page&per_page=24", token)) { VideoPage(error = it) }
    }

    suspend fun ticket(): Ticket = withContext(Dispatchers.IO) {
        parse(TubeCore.call("GET", "$base/api/upload-ticket", token)) { Ticket(error = it) }
    }

    suspend fun remote(url: String, title: String): Remote = withContext(Dispatchers.IO) {
        val body = json.encodeToString(
            JsonObject.serializer(),
            JsonObject(mapOf("url" to JsonPrimitive(url), "title" to JsonPrimitive(title))),
        )
        parse(TubeCore.call("POST", "$base/api/remote", token, body)) { Remote(error = it) }
    }

    fun parseUpload(text: String): UploadResult = parse(text) { UploadResult(error = it) }
}

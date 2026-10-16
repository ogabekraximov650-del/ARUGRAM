package uz.arumediatv.com.data

import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject

/** Server javobi: HTTP kodi + (bo'lsa) JSON tanasi. */
data class ApiResult(val code: Int, val json: JSONObject?, val error: String?) {
    val ok: Boolean get() = code in 200..299
}

private val JSON_TYPE = "application/json".toMediaType()

/** Imzolangan so'rov (`Http.client`) + ixtiyoriy `Authorization: Bearer`. Tarmoq xatosi — `code = 0`. Faqat IO oqimida chaqiring. */
fun OkHttpClient.callJson(
    url: String,
    method: String = "GET",
    body: JSONObject? = null,
    bearer: String? = null,
): ApiResult =
    try {
        val rb = Request.Builder().url(url)
        if (bearer != null) rb.header("Authorization", "Bearer $bearer")
        when (method) {
            "GET" -> rb.get()
            "DELETE" -> rb.delete()
            else -> rb.method(method, (body?.toString() ?: "{}").toRequestBody(JSON_TYPE))
        }
        newCall(rb.build()).execute().use { r ->
            val text = r.body?.string().orEmpty()
            val j = try {
                if (text.isNotEmpty()) JSONObject(text) else null
            } catch (_: Exception) {
                null
            }
            ApiResult(r.code, j, j?.optString("error")?.takeIf { it.isNotEmpty() })
        }
    } catch (e: Exception) {
        ApiResult(0, null, e.message ?: "Tarmoq xatosi")
    }

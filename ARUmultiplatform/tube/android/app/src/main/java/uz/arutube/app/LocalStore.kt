package uz.arutube.app

import android.content.Context
import android.content.SharedPreferences
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.serializer

/** Ko'rish tarixi yozuvi: video va qayerda to'xtagani (millisekund). */
@Serializable
data class Saved(val video: Video, val posMs: Long = 0, val durMs: Long = 0, val at: Long = 0) {
    val fraction: Float get() = if (durMs > 0) (posMs.toFloat() / durMs).coerceIn(0f, 1f) else 0f

    /** "Davom ettirish" ro'yxati uchun: boshlangan, lekin tugamagan. */
    val inProgress: Boolean get() = durMs > 0 && posMs > 5_000 && posMs < durMs * 0.95
}

/** Qurilmadagi saqlash (bazasiz): tarix, "keyin ko'rish" va oxirgi qidiruvlar. SharedPreferences'da JSON ko'rinishida. */
object LocalStore {
    private var sp: SharedPreferences? = null

    var history by mutableStateOf<List<Saved>>(emptyList())
        private set
    var later by mutableStateOf<List<Video>>(emptyList())
        private set
    var queries by mutableStateOf<List<String>>(emptyList())
        private set
    var liked by mutableStateOf<List<String>>(emptyList())
        private set
    var disliked by mutableStateOf<List<String>>(emptyList())
        private set

    fun init(ctx: Context) {
        if (sp != null) return
        sp = ctx.getSharedPreferences("arutube", Context.MODE_PRIVATE)
        history = read("history", ListSerializer(Saved.serializer()))
        later = read("later", ListSerializer(Video.serializer()))
        queries = read("queries", ListSerializer(String.serializer()))
        liked = read("liked", ListSerializer(String.serializer()))
        disliked = read("disliked", ListSerializer(String.serializer()))
    }

    private fun <T> read(key: String, ser: kotlinx.serialization.KSerializer<List<T>>): List<T> =
        try {
            sp?.getString(key, null)?.let { Api.json.decodeFromString(ser, it) } ?: emptyList()
        } catch (e: Exception) {
            emptyList()
        }

    private fun <T> write(key: String, ser: kotlinx.serialization.KSerializer<List<T>>, v: List<T>) {
        try {
            sp?.edit()?.putString(key, Api.json.encodeToString(ser, v))?.apply()
        } catch (_: Exception) {
        }
    }

    fun progress(code: String): Saved? = history.firstOrNull { it.video.code == code }

    /** Tarixga yozadi (eng yangi tepada, 100 tagacha). */
    fun record(v: Video, posMs: Long, durMs: Long) {
        history = (listOf(Saved(v, posMs, durMs, System.currentTimeMillis())) + history.filter { it.video.code != v.code }).take(100)
        write("history", ListSerializer(Saved.serializer()), history)
    }

    fun clearHistory() {
        history = emptyList()
        write("history", ListSerializer(Saved.serializer()), history)
    }

    fun isLater(code: String): Boolean = later.any { it.code == code }

    fun toggleLater(v: Video) {
        later = if (isLater(v.code)) later.filter { it.code != v.code } else listOf(v) + later
        write("later", ListSerializer(Video.serializer()), later)
    }

    fun addQuery(q: String) {
        val t = q.trim()
        if (t.isEmpty()) return
        queries = (listOf(t) + queries.filter { !it.equals(t, ignoreCase = true) }).take(12)
        write("queries", ListSerializer(String.serializer()), queries)
    }

    fun isLiked(code: String): Boolean = code in liked

    /** Qurilmadagi "yoqdi" belgisi (server yo'q — faqat shu telefonda). `force` berilsa shu holatga o'tkaziladi. */
    fun toggleLike(code: String, force: Boolean? = null) {
        if (force == null) {
            liked = if (isLiked(code)) liked - code else listOf(code) + liked
            if (isLiked(code) && isDisliked(code)) {
                disliked = disliked - code
                write("disliked", ListSerializer(String.serializer()), disliked)
            }
        } else if (force != isLiked(code)) {
            liked = if (force) listOf(code) + liked else liked - code
        }
        write("liked", ListSerializer(String.serializer()), liked)
    }

    fun isDisliked(code: String): Boolean = code in disliked

    /** "Yoqmadi" — yoqdi bilan bir vaqtda bo'lmaydi. */
    fun toggleDislike(code: String) {
        if (isDisliked(code)) {
            disliked = disliked - code
        } else {
            disliked = listOf(code) + disliked
            toggleLike(code, false)
        }
        write("disliked", ListSerializer(String.serializer()), disliked)
    }

    fun removeFromHistory(code: String) {
        history = history.filter { it.video.code != code }
        write("history", ListSerializer(Saved.serializer()), history)
    }

    fun clearQueries() {
        queries = emptyList()
        write("queries", ListSerializer(String.serializer()), queries)
    }
}

/** Yuklangan videolar xotirasi: qidiruv uchun barcha sahifalar yig'iladi. */
object Repo {
    var all by mutableStateOf<List<Video>>(emptyList())
        private set
    var loading by mutableStateOf(false)
        private set
    private var done = false

    fun merge(list: List<Video>) {
        val known = all.map { it.code }.toSet()
        val fresh = list.filter { it.code !in known }
        if (fresh.isNotEmpty()) all = all + fresh
    }

    suspend fun ensureLoaded(maxPages: Int = 5) {
        if (loading || done) return
        loading = true
        var p = 1
        while (p <= maxPages) {
            val r = Api.videos(p)
            if (r.error != null) break
            merge(r.videos)
            if (p >= r.pages) {
                done = true
                break
            }
            p++
        }
        loading = false
    }
}

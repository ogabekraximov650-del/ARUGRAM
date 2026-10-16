package uz.arumediatv.com.data

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import org.json.JSONArray
import org.json.JSONObject
import uz.arumediatv.com.BuildConfig
import java.io.File

enum class CommentSort(val code: String, val label: String) {
    NEW("yangi", "Yangilar"),
    LIKES("layk", "Layklar"),
    REPLIES("javob", "Javoblar"),
}

data class Comment(
    val id: String,
    val parentId: String,
    val userId: Long,
    val firstName: String,
    val username: String,
    val photoUrl: String,
    val body: String,
    /** Stiker (`stk_...`) yoki GIF (kanaldagi fayl nomi). */
    val mediaFile: String,
    /** `sticker` / `gif` / bo'sh. */
    val mediaType: String,
    val createdAt: Long,
    val likes: Int,
    val replyCount: Int,
    val liked: Boolean,
) {
    val isReply: Boolean get() = parentId.isNotEmpty()

    val name: String
        get() = firstName.trim().ifEmpty { username.trim().takeIf { it.isNotEmpty() }?.let { "@$it" } ?: "Foydalanuvchi" }

    fun toJson(): JSONObject = JSONObject()
        .put("id", id).put("parent_id", parentId).put("user_id", userId).put("first_name", firstName).put("username", username)
        .put("photo_url", photoUrl).put("body", body).put("media_file", mediaFile).put("media_type", mediaType)
        .put("created_at", createdAt).put("likes", likes).put("reply_count", replyCount).put("liked", liked)

    companion object {
        fun from(o: JSONObject) = Comment(
            id = o.optString("id"), parentId = o.optString("parent_id").takeIf { it != "null" }.orEmpty(),
            userId = o.optLong("user_id"), firstName = o.optString("first_name"), username = o.optString("username"),
            photoUrl = o.optString("photo_url"), body = o.optString("body"),
            mediaFile = o.optString("media_file"), mediaType = o.optString("media_type"),
            createdAt = o.optLong("created_at"), likes = o.optInt("likes"), replyCount = o.optInt("reply_count"), liked = o.optBoolean("liked"),
        )

        fun list(arr: JSONArray?): List<Comment> {
            if (arr == null) return emptyList()
            return (0 until arr.length()).mapNotNull { arr.optJSONObject(it) }.filter { !it.optBoolean("deleted") }.map(::from)
        }
    }
}

data class CommentsState(
    val items: List<Comment> = emptyList(),
    /** Ochilgan javoblar: bosh izoh id -> javoblar. */
    val replies: Map<String, List<Comment>> = emptyMap(),
    val loadingReplies: Set<String> = emptySet(),
    val sort: CommentSort = CommentSort.NEW,
    val loading: Boolean = false,
    val loadingMore: Boolean = false,
    val hasMore: Boolean = false,
    val loaded: Boolean = false,
    val error: String? = null,
) {
    val total: Int get() = items.size + items.sumOf { it.replyCount }
}

/**
 * Bitta bo'limning izohlari (Flutter `CommentsController`). Diskdagi nusxa darhol ko'rinadi; `mk/at` belgisi bilan serverdan
 * "o'zgarmagan" javobi olinsa ro'yxat qayta yuklanmaydi (Turso o'qishi tejaladi); ochiq turganda `/wait` bilan jonli yangilanadi.
 */
class CommentsStore(
    context: Context,
    private val auth: AuthService,
    val animeId: Long,
    val seasonId: Long,
    private val client: OkHttpClient = Http.client,
    private val base: String = BuildConfig.API_BASE,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val api = "$base/api/comments"
    private val file = File(File(context.filesDir, "cache").apply { mkdirs() }, "comments_${animeId}_$seasonId.json")
    private val _state = MutableStateFlow(CommentsState())
    val state: StateFlow<CommentsState> = _state.asStateFlow()

    @Volatile private var mk = -1L
    @Volatile private var at = 0L
    private var page = 0
    private var watching: Job? = null
    private val likeBusy = HashSet<String>()

    private val token: String? get() = auth.sessionToken

    fun loadFromDisk() {
        if (_state.value.items.isNotEmpty() || _state.value.sort != CommentSort.NEW) return
        try {
            if (!file.exists()) return
            val j = JSONObject(file.readText())
            val rows = Comment.list(j.optJSONArray("items"))
            if (j.optLong("u") == (auth.user.value?.id ?: 0L)) {
                mk = j.optLong("mk", -1)
                at = j.optLong("at")
            }
            _state.update { it.copy(items = rows, loaded = true) }
        } catch (_: Exception) {
        }
    }

    private fun saveDisk(items: List<Comment>) {
        if (_state.value.sort != CommentSort.NEW) return
        try {
            val arr = JSONArray(); items.forEach { arr.put(it.toJson()) }
            file.writeText(JSONObject().put("mk", mk).put("at", at).put("u", auth.user.value?.id ?: 0L).put("items", arr).toString())
        } catch (_: Exception) {
        }
    }

    fun setSort(s: CommentSort) {
        if (_state.value.sort == s) return
        page = 0
        _state.update { it.copy(sort = s, items = emptyList(), replies = emptyMap(), hasMore = false, loaded = false) }
        scope.launch { load(force = true) }
    }

    suspend fun load(force: Boolean = false) = withContext(Dispatchers.IO) {
        val s = _state.value
        if (s.loading || (s.loaded && !force)) return@withContext
        _state.update { it.copy(loading = true, error = null) }
        val canSame = s.sort == CommentSort.NEW && s.items.isNotEmpty() && mk >= 0
        val r = client.callJson("$api/$animeId/$seasonId?page=0&sort=${s.sort.code}${if (canSame) "&mk=$mk&at=$at" else ""}", bearer = token)
        val j = r.json
        when {
            r.ok && canSame && j?.optBoolean("same") == true -> _state.update { it.copy(loading = false, loaded = true) }
            r.ok && j != null -> {
                mk = j.optLong("mk", -1); at = j.optLong("at")
                val rows = Comment.list(j.optJSONArray("items"))
                page = 0
                _state.update { it.copy(items = rows, replies = emptyMap(), hasMore = j.optBoolean("has_more"), loaded = true, loading = false, error = null) }
                saveDisk(rows)
            }
            else -> _state.update { it.copy(loading = false, error = if (r.code == 0) "Internet yo'q" else "Izohlar yuklanmadi (${r.code})") }
        }
    }

    fun loadMore() {
        val s = _state.value
        if (s.loadingMore || s.loading || !s.hasMore) return
        _state.update { it.copy(loadingMore = true) }
        scope.launch {
            val next = page + 1
            val r = client.callJson("$api/$animeId/$seasonId?page=$next&sort=${s.sort.code}", bearer = token)
            val j = r.json
            if (r.ok && j != null) {
                val rows = Comment.list(j.optJSONArray("items"))
                page = next
                _state.update { st ->
                    val have = st.items.map { it.id }.toSet()
                    st.copy(items = st.items + rows.filter { it.id !in have }, hasMore = j.optBoolean("has_more"), loadingMore = false)
                }
            } else _state.update { it.copy(loadingMore = false) }
        }
    }

    fun toggleReplies(id: String) {
        val s = _state.value
        if (s.replies.containsKey(id)) {
            _state.update { it.copy(replies = it.replies - id) }
            return
        }
        if (id in s.loadingReplies) return
        _state.update { it.copy(loadingReplies = it.loadingReplies + id) }
        scope.launch {
            val r = client.callJson("$api/$animeId/$seasonId/$id", bearer = token)
            val list = if (r.ok) Comment.list(r.json?.optJSONArray("items")).reversed() else null
            _state.update { st -> st.copy(loadingReplies = st.loadingReplies - id, replies = if (list != null) st.replies + (id to list) else st.replies) }
        }
    }

    /** `null` — yuborildi; aks holda xato matni. */
    suspend fun add(body: String, parentId: String = "", mediaFile: String = "", mediaType: String = ""): String? = withContext(Dispatchers.IO) {
        val text = body.trim()
        if (text.isEmpty() && mediaFile.isEmpty()) return@withContext "Izoh bo'sh"
        val b = JSONObject().put("anime_id", animeId).put("season_id", seasonId).put("parent_id", parentId).put("body", text)
        if (mediaFile.isNotEmpty()) b.put("media_file", mediaFile).put("media_type", mediaType)
        val r = client.callJson(api, "POST", b, token)
        val j = r.json
        if (!r.ok || j == null) return@withContext r.error ?: if (r.code == 0) "Internet yo'q — qaytadan urinib ko'ring" else "Izoh yuborilmadi"
        val c = Comment.from(j)
        _state.update { st ->
            if (c.isReply) {
                val items = st.items.map { if (it.id == c.parentId) it.copy(replyCount = it.replyCount + 1) else it }
                val rep = st.replies[c.parentId]
                st.copy(items = items, replies = if (rep != null) st.replies + (c.parentId to rep + c) else st.replies)
            } else st.copy(items = listOf(c) + st.items)
        }
        null
    }

    suspend fun remove(id: String): String? = withContext(Dispatchers.IO) {
        val snap = _state.value
        _state.update { st -> st.copy(items = st.items.filterNot { it.id == id }, replies = st.replies.mapValues { (_, v) -> v.filterNot { it.id == id } }) }
        val r = client.callJson("$api/$id", "DELETE", bearer = token)
        if (!r.ok) {
            _state.update { it.copy(items = snap.items, replies = snap.replies) }
            return@withContext r.error ?: if (r.code == 0) "Internet yo'q" else "O'chirilmadi"
        }
        val parent = r.json?.optString("parent_id").orEmpty().takeIf { it != "null" }.orEmpty()
        if (parent.isNotEmpty()) _state.update { st -> st.copy(items = st.items.map { if (it.id == parent && it.replyCount > 0) it.copy(replyCount = it.replyCount - 1) else it }) }
        saveDisk(_state.value.items)
        null
    }

    /** Layk shu zahoti o'zgaradi; server javobi bilan tasdiqlanadi (xato bo'lsa qaytariladi). */
    suspend fun toggleLike(id: String): String? = withContext(Dispatchers.IO) {
        if (token == null) return@withContext "Layk bosish uchun hisobingizga kiring"
        fun mapC(f: (Comment) -> Comment) = _state.update { st ->
            st.copy(items = st.items.map { if (it.id == id) f(it) else it }, replies = st.replies.mapValues { (_, v) -> v.map { if (it.id == id) f(it) else it } })
        }
        val cur = (_state.value.items + _state.value.replies.values.flatten()).firstOrNull { it.id == id } ?: return@withContext null
        val wasLiked = cur.liked
        val wasLikes = cur.likes
        mapC { it.copy(liked = !wasLiked, likes = if (!wasLiked) wasLikes + 1 else (wasLikes - 1).coerceAtLeast(0)) }
        if (!likeBusy.add(id)) return@withContext null
        var error: String? = null
        try {
            loop@ for (i in 0 until 5) {
                val r = client.callJson("$api/like", "POST", JSONObject().put("id", id), token)
                if (!r.ok) {
                    error = r.error ?: "Layk yuborilmadi"
                    break@loop
                }
                val liked = r.json?.optBoolean("liked") == true
                val now = (_state.value.items + _state.value.replies.values.flatten()).firstOrNull { it.id == id }
                if (now != null && liked == now.liked) {
                    mapC { it.copy(likes = r.json?.optInt("likes", it.likes) ?: it.likes) }
                    break@loop
                }
            }
        } finally {
            likeBusy.remove(id)
        }
        if (error != null) mapC { it.copy(liked = wasLiked, likes = wasLikes) }
        error
    }

    // ── Jonli yangilanish ──
    fun startWatching() {
        if (watching?.isActive == true) return
        watching = scope.launch {
            while (isActive) {
                if (mk < 0 || _state.value.loading) { delay(2000); continue }
                val t0 = System.currentTimeMillis()
                val (code, text) = try { client.callText("$api/wait?mk=$mk") } catch (_: Exception) { 0 to "" }
                if (!isActive) return@launch
                if (code == 200) {
                    val j = try { JSONObject(text) } catch (_: Exception) { null }
                    if (j?.optBoolean("new") == true) {
                        // Belgini AVVAL yangilaymiz — aks holda bir xil o'zgarish qayta-qayta yuklanadi.
                        val m = j.optLong("mk", -1)
                        if (m > mk) mk = m
                        load(force = true)
                    }
                } else delay(5000)
                // Server darhol qaytarsa (uzoq so'rov emas) — so'rovlar to'xtovsiz ketmasin.
                if (System.currentTimeMillis() - t0 < 1500) delay(4000)
            }
        }
    }

    fun stopWatching() {
        watching?.cancel()
        watching = null
    }

    fun close() {
        stopWatching()
        scope.cancel()
    }

    /** Shikoyat yuborish (`POST /api/reports`). `null` — qabul qilindi. */
    suspend fun report(commentId: String, reason: String): String? = withContext(Dispatchers.IO) {
        if (token == null) return@withContext "Shikoyat yuborish uchun hisobingizga kiring"
        if (reason.trim().length < 10) return@withContext "Iltimos, shikoyat sababini batafsilroq yozing"
        val r = client.callJson("$base/api/reports", "POST", JSONObject().put("kind", "comment").put("target_id", commentId).put("reason", reason.trim()), token)
        if (r.ok) null else r.error ?: if (r.code == 0) "Internet yo'q — qaytadan urinib ko'ring" else "Shikoyat yuborilmadi"
    }
}

// ══════════════ Ommaviy profil va statistika ro'yxatlari ══════════════

data class PublicProfile(val raw: JSONObject) {
    val id: Long get() = raw.optLong("id")
    val name: String get() = "${raw.optString("first_name")} ${raw.optString("last_name")}".trim().ifEmpty { raw.optString("username").takeIf { it.isNotEmpty() }?.let { "@$it" } ?: "Foydalanuvchi $id" }
    val username: String get() = raw.optString("username")
    val photoUrl: String get() = raw.optString("photo_url")
    val premium: Boolean get() = raw.optBoolean("premium")
    val adminView: Boolean get() = raw.optBoolean("admin_view")
    val hidden: List<String> get() = raw.optJSONArray("hidden")?.let { a -> (0 until a.length()).map { a.optString(it) } }.orEmpty()
    fun has(k: String) = raw.has(k)
    fun long(k: String) = raw.optLong(k)
}

class PublicRepository(
    context: Context,
    private val auth: AuthService,
    private val client: OkHttpClient = Http.client,
    private val base: String = BuildConfig.API_BASE,
) {
    private val dir = File(context.filesDir, "cache").apply { mkdirs() }

    fun cached(id: Long): PublicProfile? = try {
        File(dir, "user_$id.json").takeIf { it.exists() }?.let { PublicProfile(JSONObject(it.readText())) }
    } catch (_: Exception) {
        null
    }

    /** `first` — profil (yoki `null`), `second` — xato matni. */
    suspend fun load(id: Long): Pair<PublicProfile?, String?> = withContext(Dispatchers.IO) {
        val r = client.callJson("$base/api/user/$id", bearer = auth.sessionToken)
        val j = r.json
        if (r.ok && j != null) {
            try { File(dir, "user_$id.json").writeText(j.toString()) } catch (_: Exception) {}
            PublicProfile(j) to null
        } else null to (if (r.code == 0) "Internet yo'q" else "Foydalanuvchi topilmadi")
    }

    /** Statistika ro'yxati sahifasi (`kind`: episodes, seasons, favorites, rated, comments). */
    suspend fun stats(id: Long, kind: String, page: Int): Triple<List<JSONObject>, Boolean, String?> = withContext(Dispatchers.IO) {
        val r = client.callJson("$base/api/user/$id/stats/$kind?page=$page", bearer = auth.sessionToken)
        val j = r.json
        when {
            r.code == 403 -> Triple(emptyList(), false, "Bu statistika yashirilgan")
            r.ok && j != null -> {
                val a = j.optJSONArray("items")
                Triple((0 until (a?.length() ?: 0)).mapNotNull { a?.optJSONObject(it) }, j.optBoolean("has_more"), null)
            }
            else -> Triple(emptyList(), false, if (r.code == 0) "Internet yo'q" else "Ro'yxat kelmadi")
        }
    }
}

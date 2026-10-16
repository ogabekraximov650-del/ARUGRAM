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

data class ChatMessage(
    val id: String,
    val fromAdmin: Boolean,
    val body: String,
    val createdAt: Long,
    val mediaUrl: String = "",
    /** `image`, `video`, `voice`, `round`, `sticker`, `gif` yoki bo'sh. */
    val mediaType: String = "",
    val mediaMs: Long = 0,
    val pending: Boolean = false,
    val seen: Boolean = false,
) {
    val hasMedia: Boolean get() = mediaUrl.isNotEmpty() && mediaType.isNotEmpty()

    fun toJson(): JSONObject = JSONObject().put("id", id).put("from_admin", fromAdmin).put("body", body).put("created_at", createdAt)
        .put("media_url", mediaUrl).put("media_type", mediaType).put("media_ms", mediaMs).put("seen", seen)

    companion object {
        fun from(o: JSONObject) = ChatMessage(
            id = o.optString("id"), fromAdmin = o.optBoolean("from_admin"), body = o.optString("body").takeIf { it != "null" }.orEmpty(),
            createdAt = o.optLong("created_at"), mediaUrl = o.optString("media_url").takeIf { it != "null" }.orEmpty(),
            mediaType = o.optString("media_type").takeIf { it != "null" }.orEmpty(), mediaMs = o.optLong("media_ms"), seen = o.optBoolean("seen"),
        )
    }
}

data class ChatThread(
    val userId: Long, val firstName: String, val username: String, val photoUrl: String,
    val lastBody: String, val lastAt: Long, val lastFromAdmin: Boolean, val unread: Int,
) {
    val name: String get() = firstName.trim().ifEmpty { username.trim().takeIf { it.isNotEmpty() }?.let { "@$it" } ?: "Foydalanuvchi $userId" }

    companion object {
        fun from(o: JSONObject) = ChatThread(
            o.optLong("user_id"), o.optString("first_name"), o.optString("username"), o.optString("photo_url"),
            o.optString("last_body"), o.optLong("last_at"), o.optBoolean("last_from_admin"), o.optInt("unread"),
        )
    }
}

/** Profil tugmasidagi va "Admin bilan bog'lanish" yonidagi nuqta: faqat o'qilmaganlar SONI so'raladi (xabarlarning o'zi emas). */
class UnreadBadge(private val auth: AuthService, private val client: OkHttpClient = Http.client, private val base: String = BuildConfig.API_BASE) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val _count = MutableStateFlow(0)
    val count: StateFlow<Int> = _count.asStateFlow()
    private var last = 0L

    /** Admin uchun: barcha suhbatlardagi o'qilmaganlar yig'indisi. */
    private val _adminCount = MutableStateFlow(0)
    val adminCount: StateFlow<Int> = _adminCount.asStateFlow()

    private var srvCount = -1
    private var mk = -1L
    private var at = 0L

    fun refresh(force: Boolean = false) {
        val token = auth.sessionToken ?: run { _count.value = 0; _adminCount.value = 0; return }
        if (!force && System.currentTimeMillis() - last < 20_000) return
        last = System.currentTimeMillis()
        scope.launch {
            val r = client.callJson("$base/api/chat/unread?u=$srvCount&mk=$mk&at=$at", bearer = token)
            val j = r.json ?: return@launch
            if (r.ok) {
                val n = j.optInt("unread")
                srvCount = n
                mk = j.optLong("mk", -1)
                at = j.optLong("at")
                // Server har javobda "bu admin sanog'imi" deb aytadi — ilova taxmin qilmaydi.
                if (j.optBoolean("admin")) { _adminCount.value = n; _count.value = 0 } else { _count.value = n; _adminCount.value = 0 }
            }
        }
    }

    fun markRead() {
        _count.value = 0
    }

    fun clear() {
        _count.value = 0
        _adminCount.value = 0
        last = 0
        srvCount = -1; mk = -1; at = 0
    }
}

/**
 * Admin bilan yozishma (Flutter `ChatController`). `userId == null` — o'z suhbatim; admin boshqa odam suhbatini ochsa — shu odamniki.
 * Diskdagi nusxa darhol; yangi xabarlar `since` bilan; ochiq turganda `/wait` bilan jonli.
 */
class ChatStore(
    context: Context,
    private val auth: AuthService,
    val userId: Long? = null,
    private val client: OkHttpClient = Http.client,
    private val base: String = BuildConfig.API_BASE,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val file = File(File(context.filesDir, "cache").apply { mkdirs() }, "chat_${userId ?: 0}.json")
    private val url = if (userId == null) "$base/api/chat" else "$base/api/chat/thread/$userId"
    private val _items = MutableStateFlow<List<ChatMessage>>(emptyList())
    val items: StateFlow<List<ChatMessage>> = _items.asStateFlow()
    private val _error = MutableStateFlow<String?>(null)
    val error: StateFlow<String?> = _error.asStateFlow()
    private val _loading = MutableStateFlow(false)
    val loading: StateFlow<Boolean> = _loading.asStateFlow()
    private var loaded = false
    private var watching: Job? = null
    private val token: String? get() = auth.sessionToken
    private val adminView: Boolean get() = userId != null

    private val lastAt: Long get() = _items.value.filter { !it.pending }.maxOfOrNull { it.createdAt } ?: 0

    fun loadFromDisk() {
        if (_items.value.isNotEmpty()) return
        try {
            if (!file.exists()) return
            val arr = JSONArray(file.readText())
            _items.value = (0 until arr.length()).mapNotNull { arr.optJSONObject(it) }.map(ChatMessage::from)
            loaded = true
        } catch (_: Exception) {
        }
    }

    private fun saveDisk() {
        try {
            val arr = JSONArray(); _items.value.filter { !it.pending }.forEach { arr.put(it.toJson()) }
            file.writeText(arr.toString())
        } catch (_: Exception) {
        }
    }

    suspend fun load(force: Boolean = false) = withContext(Dispatchers.IO) {
        if (_loading.value) return@withContext
        if (loaded && !force) return@withContext
        val t = token ?: run { _error.value = "Avval hisobingizga kiring"; return@withContext }
        _loading.value = true
        val since = if (loaded) lastAt else 0L
        val r = client.callJson(if (since > 0) "$url?since=$since" else url, bearer = t)
        val j = r.json
        if (r.ok && j != null) {
            val arr = j.optJSONArray("items")
            val rows = (0 until (arr?.length() ?: 0)).mapNotNull { arr?.optJSONObject(it) }.map(ChatMessage::from)
            val seenIds = j.optJSONArray("seen_ids")?.let { a -> (0 until a.length()).map { a.optString(it) }.toSet() }.orEmpty()
            val allIds = j.optJSONArray("all_ids")?.let { a -> (0 until a.length()).map { a.optString(it) }.toSet() }
            _items.update { cur ->
                var list = cur.map { if (!it.seen && it.id in seenIds) it.copy(seen = true) else it }
                if (allIds != null) list = list.filter { it.pending || it.id in allIds }
                if (since > 0) {
                    val have = list.map { it.id }.toSet()
                    val fresh = rows.filter { it.id !in have }
                    if (fresh.isNotEmpty()) list = list.filter { !it.pending } + fresh
                    list
                } else rows
            }
            saveDisk()
            loaded = true
            _error.value = null
            if (!adminView) uz.arumediatv.com.AppServices.unread.markRead()
        } else if (!loaded) _error.value = if (r.code == 0) "Internet yo'q" else "Yuklanmadi (${r.code})"
        _loading.value = false
    }

    /** `null` — yuborildi (yoki bo'sh); aks holda xato matni. */
    suspend fun send(body: String, mediaFile: String = "", mediaType: String = "", mediaMs: Long = 0): String? = withContext(Dispatchers.IO) {
        val text = body.trim()
        if (text.isEmpty() && mediaFile.isEmpty()) return@withContext null
        val temp = "tmp${System.nanoTime()}"
        if (mediaFile.isEmpty()) {
            _items.update { it + ChatMessage(temp, adminView, text, System.currentTimeMillis(), pending = true) }
            loaded = true
        }
        val b = JSONObject().put("body", text)
        userId?.let { b.put("user_id", it) }
        if (mediaFile.isNotEmpty()) b.put("media_file", mediaFile)
        if (mediaType.isNotEmpty()) b.put("media_type", mediaType)
        if (mediaMs > 0) b.put("media_ms", mediaMs)
        val r = client.callJson("$base/api/chat", "POST", b, token)
        val j = r.json
        _items.update { l -> l.filterNot { it.id == temp } }
        if (!r.ok || j == null) return@withContext r.error ?: if (r.code == 0) "Internet yo'q — qaytadan urinib ko'ring" else "Yuborilmadi"
        val saved = ChatMessage.from(j)
        _items.update { l -> if (l.any { it.id == saved.id }) l else l + saved }
        loaded = true
        saveDisk()
        null
    }

    suspend fun remove(ids: Collection<String>): String? = withContext(Dispatchers.IO) {
        val gone = ids.toSet()
        if (gone.isEmpty()) return@withContext null
        val before = _items.value
        _items.update { l -> l.filterNot { it.id in gone } }
        val server = gone.filter { !it.startsWith("tmp") }
        if (server.isNotEmpty()) {
            val r = client.callJson("$base/api/chat/messages/delete", "POST", JSONObject().put("ids", JSONArray(server)), token)
            if (!r.ok) {
                _items.value = before
                return@withContext r.error ?: if (r.code == 0) "Internet yo'q" else "O'chirilmadi"
            }
        }
        saveDisk()
        null
    }

    private var ver = 0L
    private var mk = -1L
    private var at = 0L

    /** `/wait` — server ~19 soniya ushlab turadi va faqat suhbat o'zgarganda `new` qaytaradi (Turso'dan arzon `ver` bilan). */
    fun startWatching() {
        if (watching?.isActive == true) return
        watching = scope.launch {
            while (isActive) {
                val t = token
                if (t == null || !loaded) { delay(2000); continue }
                val q = "ver=$ver&mk=$mk&at=$at&since=$lastAt${userId?.let { "&user_id=$it" } ?: ""}"
                val (code, text) = client.callText("$base/api/chat/wait?$q", t)
                if (!isActive) return@launch
                if (code == 200) {
                    val j = try { JSONObject(text) } catch (_: Exception) { null } ?: continue
                    val newVer = if (j.has("ver")) j.optLong("ver") else null
                    mk = j.optLong("mk", -1); at = j.optLong("at")
                    if (j.optBoolean("new")) load(force = true)
                    if (newVer != null && newVer != ver) ver = newVer
                } else delay(3000)
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
}

/** Admin: barcha suhbatlar ro'yxati. */
class ChatThreadsStore(
    private val auth: AuthService,
    private val client: OkHttpClient = Http.client,
    private val base: String = BuildConfig.API_BASE,
) {
    private val _items = MutableStateFlow<List<ChatThread>>(emptyList())
    val items: StateFlow<List<ChatThread>> = _items.asStateFlow()
    private val _state = MutableStateFlow("")
    val error: StateFlow<String> = _state.asStateFlow()

    suspend fun load() = withContext(Dispatchers.IO) {
        val r = client.callJson("$base/api/chat/threads", bearer = auth.sessionToken)
        val arr = r.json?.optJSONArray("items")
        if (r.ok && arr != null) {
            _items.value = (0 until arr.length()).mapNotNull { arr.optJSONObject(it) }.map(ChatThread::from)
            _state.value = ""
        } else _state.value = if (r.code == 0) "Internet yo'q" else "Yuklanmadi (${r.code})"
    }

    suspend fun remove(userId: Long): String? = withContext(Dispatchers.IO) {
        val r = client.callJson("$base/api/chat/thread/$userId", "DELETE", bearer = auth.sessionToken)
        if (r.ok) { _items.value = _items.value.filterNot { it.userId == userId }; null } else r.error ?: "O'chirilmadi"
    }
}

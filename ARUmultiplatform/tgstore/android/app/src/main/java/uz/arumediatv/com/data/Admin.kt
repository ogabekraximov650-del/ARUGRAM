package uz.arumediatv.com.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import org.json.JSONArray
import org.json.JSONObject
import uz.arumediatv.com.BuildConfig

/**
 * Admin so'rovlari: Bearer bilan. Har bir amal SERVERDA tekshiriladi (`admin_only`) — ilovani o'zgartirib birovning balansini
 * o'zgartirib bo'lmaydi.
 */
class AdminApi(private val auth: AuthService, private val client: OkHttpClient = Http.client, private val base: String = BuildConfig.API_BASE) {
    private val token: String? get() = auth.sessionToken

    suspend fun get(path: String): ApiResult = withContext(Dispatchers.IO) { client.callJson("$base$path", bearer = token) }
    suspend fun post(path: String, body: JSONObject = JSONObject()): ApiResult = withContext(Dispatchers.IO) { client.callJson("$base$path", "POST", body, token) }
    suspend fun delete(path: String): ApiResult = withContext(Dispatchers.IO) { client.callJson("$base$path", "DELETE", bearer = token) }

    fun errorOf(r: ApiResult, fallback: String): String = r.error ?: if (r.code == 0) "Internet yo'q" else if (r.code == 403) "Bu bo'lim faqat admin uchun" else "$fallback (${r.code})"
}

// ══════════ Foydalanuvchilar ══════════

enum class UserSort(val code: String, val label: String) { NEW("new", "Yangi qo'shilgan"), ONLINE("online", "Oxirgi onlayn") }

data class AdminUser(
    val id: Long, val username: String, val firstName: String, val lastName: String, val photoUrl: String, val telegramId: Long,
    val balance: Long, val banned: Boolean, val banUntil: Long, val banReason: String, val createdAt: Long, val lastLoginAt: Long, val subUntil: Long,
) {
    val name: String get() = "$firstName $lastName".trim().ifEmpty { if (username.isNotEmpty()) "@$username" else "Foydalanuvchi $id" }
    val subDaysLeft: Int get() { val ms = subUntil - System.currentTimeMillis(); return if (ms <= 0) 0 else Math.ceil(ms / 86_400_000.0).toInt() }

    companion object {
        fun from(o: JSONObject) = AdminUser(
            o.optLong("id"), o.optString("username"), o.optString("first_name"), o.optString("last_name"), o.optString("photo_url"),
            o.optLong("telegram_id"), o.optLong("balance"), o.optBoolean("banned"), o.optLong("ban_until"), o.optString("ban_reason"),
            o.optLong("created_at"), o.optLong("last_login_at"), o.optLong("sub_until"),
        )
    }
}

data class AdminUsersState(
    val items: List<AdminUser> = emptyList(), val sort: UserSort = UserSort.NEW, val query: String = "", val total: Int = 0,
    val loading: Boolean = false, val loadingMore: Boolean = false, val hasMore: Boolean = false, val error: String? = null, val page: Int = 0,
)

class AdminUsersStore(private val api: AdminApi) {
    private val _state = MutableStateFlow(AdminUsersState())
    val state: StateFlow<AdminUsersState> = _state.asStateFlow()

    private suspend fun fetch(page: Int): Triple<List<AdminUser>, Boolean, String?> {
        val s = _state.value
        val r = api.get("/api/admin/users?sort=${s.sort.code}&page=$page&q=${java.net.URLEncoder.encode(s.query, "UTF-8")}")
        val j = r.json
        if (r.ok && j != null) {
            val a = j.optJSONArray("items")
            _state.update { it.copy(total = j.optInt("total")) }
            return Triple((0 until (a?.length() ?: 0)).mapNotNull { a?.optJSONObject(it) }.map(AdminUser::from), j.optBoolean("has_more"), null)
        }
        return Triple(emptyList(), false, api.errorOf(r, "Yuklanmadi"))
    }

    suspend fun reload(sort: UserSort? = null, query: String? = null) {
        _state.update { it.copy(sort = sort ?: it.sort, query = query ?: it.query, loading = true, error = null) }
        val (rows, more, err) = fetch(0)
        _state.update { it.copy(items = if (rows.isNotEmpty() || err == null) rows else it.items, hasMore = more, error = err, loading = false, page = 0) }
    }

    suspend fun loadMore() {
        val s = _state.value
        if (s.loadingMore || s.loading || !s.hasMore) return
        _state.update { it.copy(loadingMore = true) }
        val next = s.page + 1
        val (rows, more, _) = fetch(next)
        _state.update { st ->
            val have = st.items.map { it.id }.toSet()
            st.copy(items = st.items + rows.filter { it.id !in have }, hasMore = more, page = next, loadingMore = false)
        }
    }

    /** Amallar: `balance` (+/- amount), `sub` (+/- days), `sub_clear`, `ban` (days, reason), `unban`. `null` — bajarildi. */
    suspend fun act(userId: Long, action: String, amount: Long = 0, days: Int = 0, reason: String = ""): String? {
        val b = JSONObject().put("action", action).put("days", days)
        if (amount != 0L) b.put("amount", amount)
        if (reason.isNotEmpty()) b.put("reason", reason)
        val r = api.post("/api/admin/user/$userId", b)
        val j = r.json
        if (!r.ok || j == null) return api.errorOf(r, "Bajarilmadi")
        _state.update { st ->
            st.copy(items = st.items.map { u ->
                if (u.id != userId) u else u.copy(
                    balance = j.optLong("balance", u.balance),
                    banned = if (j.has("banned")) j.optBoolean("banned") else u.banned,
                    banUntil = if (j.has("banned")) j.optLong("ban_until") else u.banUntil,
                    banReason = if (j.has("banned")) j.optString("ban_reason") else u.banReason,
                    subUntil = if (j.has("subscription_until")) j.optLong("subscription_until") else u.subUntil,
                )
            })
        }
        return null
    }
}

// ══════════ Shikoyatlar ══════════

data class ReportPerson(val id: Long, val firstName: String, val username: String, val photoUrl: String) {
    val name: String get() = firstName.trim().ifEmpty { if (username.isNotEmpty()) "@$username" else "Foydalanuvchi" }

    companion object {
        fun from(o: JSONObject?) = ReportPerson(o?.optLong("id") ?: 0, o?.optString("first_name").orEmpty(), o?.optString("username").orEmpty(), o?.optString("photo_url").orEmpty())
    }
}

data class AdminReport(
    val id: String, val kind: String, val targetId: String, val targetBody: String, val targetAlive: Boolean,
    val animeId: Long, val seasonId: Long, val reason: String, val createdAt: Long, val author: ReportPerson, val reporter: ReportPerson,
) {
    val sourceLabel: String get() = when (kind) { "comment" -> "Izohdan"; "dm" -> "Shaxsiy yozishmadan"; else -> kind }

    companion object {
        fun from(o: JSONObject) = AdminReport(
            o.optString("id"), o.optString("kind", "comment"), o.optString("target_id"), o.optString("target_body"), o.optBoolean("target_alive"),
            o.optLong("anime_id"), o.optLong("season_id"), o.optString("reason"), o.optLong("created_at"),
            ReportPerson.from(o.optJSONObject("target_user")), ReportPerson.from(o.optJSONObject("reporter")),
        )
    }
}

data class AdminReportsState(val items: List<AdminReport> = emptyList(), val total: Int = 0, val loading: Boolean = false, val loadingMore: Boolean = false, val hasMore: Boolean = false, val page: Int = 0, val error: String? = null, val loaded: Boolean = false)

class AdminReportsStore(private val api: AdminApi) {
    private val _state = MutableStateFlow(AdminReportsState())
    val state: StateFlow<AdminReportsState> = _state.asStateFlow()

    private fun parse(a: JSONArray?) = (0 until (a?.length() ?: 0)).mapNotNull { a?.optJSONObject(it) }.map(AdminReport::from)

    suspend fun load() {
        _state.update { it.copy(loading = true, error = null) }
        val r = api.get("/api/admin/reports?page=0")
        val j = r.json
        if (r.ok && j != null) {
            val rows = parse(j.optJSONArray("items"))
            _state.update { it.copy(items = rows, total = j.optInt("total", rows.size), hasMore = j.optBoolean("has_more"), page = 0, loading = false, loaded = true) }
        } else _state.update { it.copy(loading = false, error = api.errorOf(r, "Yuklanmadi")) }
    }

    suspend fun loadMore() {
        val s = _state.value
        if (s.loadingMore || s.loading || !s.hasMore) return
        _state.update { it.copy(loadingMore = true) }
        val r = api.get("/api/admin/reports?page=${s.page + 1}")
        val j = r.json
        if (r.ok && j != null) {
            val rows = parse(j.optJSONArray("items"))
            _state.update { st -> val have = st.items.map { it.id }.toSet(); st.copy(items = st.items + rows.filter { it.id !in have }, hasMore = j.optBoolean("has_more"), page = st.page + 1, loadingMore = false) }
        } else _state.update { it.copy(loadingMore = false) }
    }

    /** Shikoyatni ro'yxatdan tozalaydi. */
    suspend fun remove(id: String): String? {
        val before = _state.value
        _state.update { st -> st.copy(items = st.items.filterNot { it.id == id }, total = (st.total - 1).coerceAtLeast(0)) }
        val r = api.delete("/api/admin/report/$id")
        if (r.ok) return null
        _state.value = before
        return api.errorOf(r, "Tozalanmadi")
    }

    /** Shikoyat qilingan izohni butunlay o'chiradi (admin ham o'chira oladi). */
    suspend fun deleteComment(commentId: String): String? {
        val r = api.delete("/api/comments/$commentId")
        return if (r.ok) null else api.errorOf(r, "O'chirilmadi")
    }
}

// ══════════ Umumiy statistika ══════════

data class StatBlock(val daily: Long = 0, val weekly: Long = 0, val monthly: Long = 0, val total: Long = 0) {
    companion object {
        fun from(o: JSONObject?) = StatBlock(o?.optLong("daily") ?: 0, o?.optLong("weekly") ?: 0, o?.optLong("monthly") ?: 0, o?.optLong("total") ?: 0)
    }
}

data class AppStats(
    val anime: Long = 0, val seasons: Long = 0, val episodes: Long = 0, val newEpisodes: StatBlock = StatBlock(),
    val users: StatBlock = StatBlock(), val views: StatBlock = StatBlock(), val animeViews: StatBlock = StatBlock(),
    val seasonViews: StatBlock = StatBlock(), val traffic: StatBlock = StatBlock(), val watch: StatBlock = StatBlock(),
) {
    companion object {
        fun from(j: JSONObject): AppStats {
            val c = j.optJSONObject("content")
            return AppStats(
                c?.optLong("anime") ?: 0, c?.optLong("seasons") ?: 0, c?.optLong("episodes") ?: 0,
                StatBlock(c?.optLong("episodes_daily") ?: 0, c?.optLong("episodes_weekly") ?: 0, c?.optLong("episodes_monthly") ?: 0, c?.optLong("episodes") ?: 0),
                StatBlock.from(j.optJSONObject("users")), StatBlock.from(j.optJSONObject("views")), StatBlock.from(j.optJSONObject("anime_views")),
                StatBlock.from(j.optJSONObject("season_views")), StatBlock.from(j.optJSONObject("traffic")), StatBlock.from(j.optJSONObject("watch")),
            )
        }
    }
}

data class SeriesPoint(val t: String, val v: Long)

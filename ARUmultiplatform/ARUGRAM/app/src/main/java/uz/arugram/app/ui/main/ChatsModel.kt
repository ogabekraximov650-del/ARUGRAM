package uz.arugram.app.ui.main

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uz.arugram.app.core.RustCore

/** Bitta suhbat (Rust `dialogs_json` qatori). */
data class ChatRow(
    val id: Long,
    val kind: String,
    val name: String,
    val pinned: Boolean,
    val unread: Int,
    val muted: Boolean,
    val out: Boolean,
    val date: Long,
    val text: String,
    val bot: Boolean,
    val self: Boolean,
    val verified: Boolean,
    val readOut: Int = 0,
    val access: String = "0",
    val mentions: Int = 0,
    val topId: Int = 0,
    /** Maxfiy chatda: suhbatdoshning foydalanuvchi id'si (avatar uchun). */
    val peerId: Long = 0,
) {
    val isSecret: Boolean get() = kind == "secret"

    /** Maxfiy chatning serverdagi raqami (`id = SECRET_BASE - eid`). */
    val secretId: Int get() = (SECRET_BASE - id).toInt()

    /** Rust tomonidagi suhbat manzili: `id` yoki `id@access_hash` (hash bilan dialoglarni aylanib chiqmasdan to'g'ridan-to'g'ri ochiladi). */
    val spec: String get() = if (access.isNotEmpty() && access != "0") "$id@$access" else "$id"
}

/** Maxfiy chatlar oddiy suhbatlar id'lari bilan to'qnashmasligi uchun alohida diapazonda. */
const val SECRET_BASE = -2_000_000_000_000L

/** Suhbatni tark etish / o'chirish: maxfiy chat serverga "tugatildi" deb bildiriladi. */
fun ChatRow.deleteRemote(): org.json.JSONObject = if (isSecret) RustCore.tgSecretDiscard(secretId) else RustCore.tgDeleteDialog(spec)

data class ChatsUi(val loading: Boolean = true, val chats: List<ChatRow> = emptyList(), val error: String? = null)

/** "Chatlar" ro'yxati holati: Rust yadrosidan suhbatlarni oladi (yangilash — tortib yangilash yoki qayta ochish). */
class ChatsModel(private val context: android.content.Context) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val prefs = context.getSharedPreferences("chats_cache", android.content.Context.MODE_PRIVATE)
    private val _ui = MutableStateFlow(ChatsUi())
    val ui: StateFlow<ChatsUi> = _ui

    init {
        // Oxirgi ro'yxat darhol ko'rinadi (Telegram ham keshdan boshlaydi), keyin tarmoqdan yangilanadi.
        prefs.getString("dialogs", null)?.let { raw -> runCatching { parse(org.json.JSONArray(raw)) }.getOrNull()?.let { list -> _ui.value = ChatsUi(loading = false, chats = sort(list)) } }
    }

    private fun sort(list: List<ChatRow>) = list.sortedWith(compareByDescending<ChatRow> { it.pinned }.thenByDescending { it.date })

    fun parse(arr: org.json.JSONArray?): List<ChatRow> = buildList {
        for (i in 0 until (arr?.length() ?: 0)) {
            val o = arr!!.getJSONObject(i)
            add(
                ChatRow(
                    id = o.optLong("id"), kind = o.optString("kind"), name = o.optString("name"), pinned = o.optBoolean("pinned"),
                    unread = o.optInt("unread"), muted = o.optBoolean("muted"), out = o.optBoolean("out"), date = o.optLong("date"),
                    text = o.optString("text"), bot = o.optBoolean("bot"), self = o.optBoolean("self"), verified = o.optBoolean("verified"),
                    readOut = o.optInt("read_out"), access = o.optString("access", "0"), mentions = o.optInt("mentions"), topId = o.optInt("top"),
                ),
            )
        }
    }

    /** Maxfiy chatlar (Rust `rust_tg_secret_list`) ro'yxat qatorlari. */
    private fun secretRows(j: org.json.JSONObject): List<ChatRow> = buildList {
        val a = j.optJSONArray("chats") ?: return@buildList
        for (i in 0 until a.length()) {
            val o = a.getJSONObject(i)
            val waiting = o.optString("state") == "waiting"
            add(
                ChatRow(
                    id = SECRET_BASE - o.optLong("id"), kind = "secret", name = o.optString("name").ifEmpty { "Maxfiy chat" }, pinned = false,
                    unread = o.optInt("unread"), muted = false, out = o.optBoolean("out"), date = o.optLong("date"),
                    text = if (waiting) "Suhbatdosh qabul qilishini kutmoqda..." else o.optString("text").ifEmpty { "Maxfiy chat boshlandi" },
                    bot = false, self = false, verified = false, access = o.optString("hash", "0"), topId = o.optInt("top"), peerId = o.optLong("user"),
                ),
            )
        }
    }

    /** Suhbatni ro'yxatda mahkamlash/yechish (darhol ko'rinadi, keyin serverdan tasdiqlanadi). */
    fun pin(row: ChatRow) {
        _ui.update { st -> st.copy(chats = sort(st.chats.map { if (it.id == row.id) it.copy(pinned = !row.pinned) else it })) }
        scope.launch { withContext(Dispatchers.IO) { RustCore.tgPinDialog(row.spec, !row.pinned) }; load() }
    }

    fun mute(row: ChatRow) = muteFor(row, if (row.muted) 0L else 1L)

    /** `secs`: 0 — yoqish, 1 — doimiy, boshqasi — shuncha soniyaga. */
    fun muteFor(row: ChatRow, secs: Long) {
        _ui.update { st -> st.copy(chats = st.chats.map { if (it.id == row.id) it.copy(muted = secs > 0) else it }) }
        scope.launch { withContext(Dispatchers.IO) { RustCore.tgMuteFor(row.spec, secs) }; load() }
    }

    fun deleteDialog(row: ChatRow) {
        _ui.update { st -> st.copy(chats = st.chats.filter { it.id != row.id }) }
        scope.launch { withContext(Dispatchers.IO) { row.deleteRemote() }; load() }
    }

    fun load() {
        _ui.update { it.copy(loading = it.chats.isEmpty(), error = null) }
        scope.launch {
            val j = withContext(Dispatchers.IO) { RustCore.tgDialogs(60) }
            val err = j.optString("error")
            if (err.isNotEmpty()) {
                _ui.update { it.copy(loading = false, error = err) }
                return@launch
            }
            val arr = j.optJSONArray("dialogs")
            val list = parse(arr) + secretRows(withContext(Dispatchers.IO) { RustCore.tgSecretList() })
            if (arr != null) prefs.edit().putString("dialogs", arr.toString()).apply()
            _ui.update { ChatsUi(loading = false, chats = sort(list)) }
        }
    }
}

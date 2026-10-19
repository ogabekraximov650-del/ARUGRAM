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
)

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

    private fun parse(arr: org.json.JSONArray?): List<ChatRow> = buildList {
        for (i in 0 until (arr?.length() ?: 0)) {
            val o = arr!!.getJSONObject(i)
            add(
                ChatRow(
                    id = o.optLong("id"), kind = o.optString("kind"), name = o.optString("name"), pinned = o.optBoolean("pinned"),
                    unread = o.optInt("unread"), muted = o.optBoolean("muted"), out = o.optBoolean("out"), date = o.optLong("date"),
                    text = o.optString("text"), bot = o.optBoolean("bot"), self = o.optBoolean("self"), verified = o.optBoolean("verified"),
                ),
            )
        }
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
            val list = parse(arr)
            if (arr != null) prefs.edit().putString("dialogs", arr.toString()).apply()
            _ui.update { ChatsUi(loading = false, chats = sort(list)) }
        }
    }
}

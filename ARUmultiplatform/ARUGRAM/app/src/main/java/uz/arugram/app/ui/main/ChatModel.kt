package uz.arugram.app.ui.main

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import uz.arugram.app.core.RustCore
import java.io.File

/** Suhbatdagi bitta xabar (Rust `msg_json`). */
data class MsgRow(
    val id: Int,
    val out: Boolean,
    val date: Long,
    val edited: Boolean,
    val text: String,
    val sender: String,
    val reply: Int,
    val fwd: String,
    val media: String,
    val kind: String,
    val pw: Int,
    val ph: Int,
    val name: String,
    val size: Long,
    val dur: Double,
    val mime: String,
    val alt: String,
    val views: Int,
    val pinned: Boolean,
    val reactions: List<Reaction>,
)

/** Reaksiya: emoji, soni, o'zim bosganmanmi. */
data class Reaction(val emoji: String, val count: Int, val mine: Boolean)

fun parseMsg(o: JSONObject): MsgRow {
    val rs = ArrayList<Reaction>()
    o.optJSONArray("reactions")?.let { a ->
        for (i in 0 until a.length()) {
            val r = a.getJSONArray(i)
            rs.add(Reaction(r.optString(0), r.optInt(1), r.optBoolean(2)))
        }
    }
    return MsgRow(
        id = o.optInt("id"), out = o.optBoolean("out"), date = o.optLong("date"), edited = o.optBoolean("edited"), text = o.optString("text"),
        sender = o.optString("sender"), reply = o.optInt("reply"), fwd = o.optString("fwd"), media = o.optString("media"), kind = o.optString("kind"),
        pw = o.optInt("pw"), ph = o.optInt("ph"), name = o.optString("name"), size = o.optLong("size"), dur = o.optDouble("dur", 0.0),
        mime = o.optString("mime"), alt = o.optString("alt"), views = o.optInt("views"), pinned = o.optBoolean("pinned"), reactions = rs,
    )
}

/**
 * Bitta suhbat holati: xabarlar (eng yangisi birinchi), eskilarini yuklash, yuborish / tahrirlash / o'chirish / uzatish / reaksiya / mahkamlash.
 * Tarmoq so'rovlari Rust yadrosida (`rust_tg_*`), bu yerda faqat holat va ketma-ketlik.
 */
class ChatModel(val chat: ChatRow, private val context: Context) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    var messages by mutableStateOf<List<MsgRow>>(emptyList())
        private set
    var loaded by mutableStateOf(false)
        private set
    var error by mutableStateOf<String?>(null)
        private set
    var pinned by mutableStateOf<MsgRow?>(null)
        private set
    var sending by mutableStateOf(0)
        private set
    var searchResults by mutableStateOf<List<MsgRow>?>(null)
        private set
    private var hasMore = true
    private var loadingMore = false

    fun close() = scope.cancel()

    private suspend fun fetch(limit: Int, offset: Int): List<MsgRow>? {
        val j = withContext(Dispatchers.IO) { RustCore.tgMessages(chat.spec, limit, offset) }
        val err = j.optString("error")
        if (err.isNotEmpty()) {
            error = err
            return null
        }
        error = null
        val arr = j.optJSONArray("messages")
        return buildList { for (i in 0 until (arr?.length() ?: 0)) add(parseMsg(arr!!.getJSONObject(i))) }
    }

    /** Eng yangi xabarlarni yangilaydi (eskilari saqlanadi). */
    fun refresh() {
        scope.launch {
            val fresh = fetch(50, 0)
            if (fresh == null) {
                loaded = true
                return@launch
            }
            val oldest = fresh.minOfOrNull { it.id } ?: Int.MAX_VALUE
            val keep = messages.filter { it.id < oldest }
            messages = (fresh + keep).distinctBy { it.id }.sortedByDescending { it.id }
            if (fresh.size < 50 && keep.isEmpty()) hasMore = false
            loaded = true
        }
    }

    fun loadOlder() {
        if (!hasMore || loadingMore || messages.isEmpty()) return
        loadingMore = true
        scope.launch {
            val oldest = messages.minOf { it.id }
            val older = fetch(50, oldest)
            if (older != null) {
                if (older.size < 50) hasMore = false
                messages = (messages + older).distinctBy { it.id }.sortedByDescending { it.id }
            }
            loadingMore = false
        }
    }

    fun loadPinned() {
        scope.launch {
            val j = withContext(Dispatchers.IO) { RustCore.tgPinned(chat.spec) }
            val arr = j.optJSONArray("messages")
            pinned = if (arr != null && arr.length() > 0) parseMsg(arr.getJSONObject(0)) else null
        }
    }

    fun markRead() {
        if (chat.unread > 0) scope.launch { withContext(Dispatchers.IO) { RustCore.tgMarkRead(chat.spec) } }
    }

    private fun after(r: JSONObject, restore: (() -> Unit)? = null) {
        val err = r.optString("error")
        if (err.isNotEmpty()) {
            error = err
            restore?.invoke()
        } else {
            refresh()
        }
    }

    fun send(text: String, replyTo: Int, onFail: (() -> Unit)? = null) {
        sending++
        scope.launch {
            val r = withContext(Dispatchers.IO) { RustCore.tgSend(chat.spec, replyTo, text) }
            sending--
            after(r, onFail)
        }
    }

    fun edit(msg: MsgRow, text: String) {
        scope.launch {
            val r = withContext(Dispatchers.IO) { RustCore.tgEdit(chat.spec, msg.id, text) }
            after(r)
        }
    }

    fun delete(ids: List<Int>) {
        messages = messages.filter { it.id !in ids }
        scope.launch {
            val r = withContext(Dispatchers.IO) { RustCore.tgDelete(chat.spec, ids) }
            after(r)
        }
    }

    fun forwardTo(target: ChatRow, ids: List<Int>) {
        scope.launch {
            val r = withContext(Dispatchers.IO) { RustCore.tgForward(chat.spec, target.spec, ids) }
            val err = r.optString("error")
            if (err.isNotEmpty()) error = err
        }
    }

    fun react(msg: MsgRow, emoji: String) {
        // Shu reaksiyam bor bo'lsa — olib tashlanadi.
        val mine = msg.reactions.any { it.mine && it.emoji == emoji }
        scope.launch {
            val r = withContext(Dispatchers.IO) { RustCore.tgReact(chat.spec, msg.id, if (mine) "" else emoji) }
            after(r)
        }
    }

    fun pin(msg: MsgRow, pin: Boolean) {
        scope.launch {
            val r = withContext(Dispatchers.IO) { RustCore.tgPinMessage(chat.spec, msg.id, pin) }
            after(r)
            loadPinned()
        }
    }

    /** Galereya/fayl tanlangach: kesh papkaga nusxalab, Rust orqali yuboradi. */
    fun sendFile(file: File, kind: String, replyTo: Int, caption: String = "", extra: String = "") {
        sending++
        scope.launch {
            val r = withContext(Dispatchers.IO) { RustCore.tgSendMedia(chat.spec, kind, file.absolutePath, replyTo, caption, extra) }
            sending--
            withContext(Dispatchers.IO) { file.delete() }
            after(r)
        }
    }

    fun sendSticker(ref: String, replyTo: Int) {
        scope.launch {
            val r = withContext(Dispatchers.IO) { RustCore.tgSendSticker(chat.spec, replyTo, ref) }
            after(r)
        }
    }

    fun search(q: String) {
        if (q.isBlank()) {
            searchResults = null
            return
        }
        scope.launch {
            val j = withContext(Dispatchers.IO) { RustCore.tgSearchInChat(chat.spec, q) }
            val arr = j.optJSONArray("messages")
            searchResults = buildList { for (i in 0 until (arr?.length() ?: 0)) add(parseMsg(arr!!.getJSONObject(i))) }
        }
    }

    fun clearSearch() {
        searchResults = null
    }

    fun clearError() {
        error = null
    }
}

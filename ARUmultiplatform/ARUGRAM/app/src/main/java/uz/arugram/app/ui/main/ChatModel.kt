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
    val poll: Poll? = null,
    val markup: Markup? = null,
    val ents: List<Ent> = emptyList(),
    val web: WebPreview? = null,
)

/** Matn bezagi: tur (`b`, `i`, `u`, `s`, `c`, `p`, `l`, `url`, `m`, `n`, `h`, `bc`, `e`, `ph`, `sp`, `q`), boshlanish, uzunlik, qo'shimcha (havola va h.k.). */
data class Ent(val type: String, val offset: Int, val length: Int, val extra: String)

/** Havola ko'rinishi (sayt, sarlavha, tavsif). */
data class WebPreview(val url: String, val site: String, val title: String, val desc: String)

/** Bot tugmalari: inline (xabar ostida) yoki javob klaviaturasi (yozish paneli ustida). */
data class Button(val text: String, val kind: String, val data: String)
data class Markup(val inline: Boolean, val rows: List<List<Button>>)

/** So'rovnoma (ko'rinishi). */
data class PollAnswer(val text: String, val option: String, val votes: Int, val chosen: Boolean)
data class Poll(val question: String, val closed: Boolean, val quiz: Boolean, val multi: Boolean, val total: Int, val answers: List<PollAnswer>)

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
        markup = o.optJSONObject("markup")?.let { mo ->
            val rr = mo.optJSONArray("rows")
            Markup(
                mo.optBoolean("inline"),
                buildList { for (i in 0 until (rr?.length() ?: 0)) { val row = rr!!.getJSONArray(i); add(buildList { for (k in 0 until row.length()) { val b = row.getJSONObject(k); add(Button(b.optString("text"), b.optString("kind"), b.optString("data"))) } }) } },
            )
        },
        ents = buildList {
            val ea = o.optJSONArray("ents")
            for (i in 0 until (ea?.length() ?: 0)) {
                val e = ea!!.getJSONArray(i)
                add(Ent(e.optString(0), e.optInt(1), e.optInt(2), e.optString(3)))
            }
        },
        web = o.optJSONObject("web")?.let { w -> WebPreview(w.optString("url"), w.optString("site"), w.optString("title"), w.optString("desc")) },
        poll = o.optJSONObject("poll")?.let { po ->
            val a = po.optJSONArray("answers")
            Poll(
                po.optString("question"), po.optBoolean("closed"), po.optBoolean("quiz"), po.optBoolean("multi"), po.optInt("total"),
                buildList { for (i in 0 until (a?.length() ?: 0)) { val x = a!!.getJSONObject(i); add(PollAnswer(x.optString("text"), x.optString("option"), x.optInt("votes"), x.optBoolean("chosen"))) } },
            )
        },
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
        val j = withContext(Dispatchers.IO) { if (chat.isSecret) RustCore.tgSecretMessages(chat.secretId) else RustCore.tgMessages(chat.spec, limit, offset) }
        val err = j.optString("error")
        if (err.isNotEmpty()) {
            error = err
            return null
        }
        error = null
        val arr = j.optJSONArray("messages")
        if (chat.isSecret) hasMore = false
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
            val keep = messages.filter { it.id in 1 until oldest }
            val pending = messages.filter { it.id <= 0 }
            messages = (pending + fresh + keep).distinctBy { it.id }.ordered()
            if (fresh.size < 50 && keep.isEmpty()) hasMore = false
            loaded = true
        }
    }

    fun loadOlder() {
        if (!hasMore || loadingMore || messages.isEmpty()) return
        loadingMore = true
        scope.launch {
            val oldest = messages.filter { it.id > 0 }.minOfOrNull { it.id } ?: run { loadingMore = false; return@launch }
            val older = fetch(50, oldest)
            if (older != null) {
                if (older.size < 50) hasMore = false
                messages = (messages + older).distinctBy { it.id }.ordered()
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
        if (chat.unread > 0) scope.launch { withContext(Dispatchers.IO) { if (chat.isSecret) RustCore.tgSecretRead(chat.secretId) else RustCore.tgMarkRead(chat.spec) } }
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

    private var pendingSeq = 0

    /** Yuborilayotgan xabarlar (id <= 0) ro'yxat tepasida, eng yangisi birinchi; keyin serverdagilar id kamayishi bo'yicha. */
    private fun List<MsgRow>.ordered(): List<MsgRow> =
        sortedWith(compareBy<MsgRow> { it.id > 0 }.thenComparator { a, b -> if (a.id <= 0) a.id.compareTo(b.id) else b.id.compareTo(a.id) })

    fun send(text: String, replyTo: Int, onFail: (() -> Unit)? = null) {
        sending++
        // Telegram kabi: xabar darhol soat belgisi bilan ko'rinadi, server tasdiqlagach almashadi.
        val temp = MsgRow(
            id = --pendingSeq, out = true, date = System.currentTimeMillis() / 1000, edited = false, text = text, sender = "", reply = replyTo,
            fwd = "", media = "", kind = "", pw = 0, ph = 0, name = "", size = 0, dur = 0.0, mime = "", alt = "", views = 0, pinned = false, reactions = emptyList(),
        )
        messages = (messages + temp).ordered()
        scope.launch {
            val r = withContext(Dispatchers.IO) { if (chat.isSecret) RustCore.tgSecretSend(chat.secretId, replyTo, text) else RustCore.tgSend(chat.spec, replyTo, text) }
            sending--
            messages = messages.filter { it.id != temp.id }
            after(r, onFail)
        }
    }

    /** Ovozsiz yoki rejalashtirilgan matn (`schedule` — unix soniya, 0 — hozir). */
    fun sendEx(text: String, replyTo: Int, silent: Boolean, schedule: Long, onFail: (() -> Unit)? = null) {
        sending++
        scope.launch {
            val r = withContext(Dispatchers.IO) { RustCore.tgSendEx(chat.spec, replyTo, silent, schedule, text) }
            sending--
            if (schedule > 0 && r.optString("error").isEmpty()) error = "Xabar rejalashtirildi"
            after(r, onFail)
        }
    }

    /** Joylashuv / kontakt / so'rovnoma. */
    fun sendSpecial(kind: String, payload: String, replyTo: Int, silent: Boolean = false, schedule: Long = 0) {
        sending++
        scope.launch {
            val r = withContext(Dispatchers.IO) { RustCore.tgSendSpecial(chat.spec, replyTo, silent, schedule, kind, payload) }
            sending--
            after(r)
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

    /** Inline tugma: url — brauzer, cb — botga yuboriladi (javob matni `onAnswer` ga). */
    fun pressButton(msg: MsgRow, b: Button, onAnswer: (String) -> Unit) {
        scope.launch {
            val r = withContext(Dispatchers.IO) { RustCore.tgCallback(chat.spec, msg.id, b.data) }
            val err = r.optString("error")
            onAnswer(if (err.isNotEmpty()) err else r.optString("message"))
            refresh()
        }
    }

    fun vote(msg: MsgRow, options: List<String>) {
        scope.launch {
            val r = withContext(Dispatchers.IO) { RustCore.tgVote(chat.spec, msg.id, options) }
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

    private var lastTyping = 0L

    /** Yozayotganingiz haqida suhbatdoshga bildiradi (4 soniyada bir martadan ko'p emas). */
    fun typing() {
        if (chat.isSecret) return
        val now = System.currentTimeMillis()
        if (now - lastTyping < 4000) return
        lastTyping = now
        scope.launch { withContext(Dispatchers.IO) { RustCore.tgTypingSend(chat.spec) } }
    }

    fun clearError() {
        error = null
    }
}

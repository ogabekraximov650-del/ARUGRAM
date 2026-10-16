package uz.arumediatv.com.data

import android.content.Context
import android.util.LruCache
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.sync.withPermit
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONObject
import uz.arumediatv.com.BuildConfig
import uz.arumediatv.com.core.SecureStore
import java.io.File
import java.nio.ByteBuffer
import java.util.concurrent.ConcurrentHashMap

object PackKind {
    const val STICKER = "sticker"
    const val EMOJI = "emoji"
    const val GIF = "gif"
    val all = listOf(STICKER, EMOJI, GIF)
    fun plural(k: String) = when (k) { EMOJI -> "Emojilar"; GIF -> "GIFlar"; else -> "Stikerlar" }
    fun single(k: String) = when (k) { EMOJI -> "Emoji"; GIF -> "GIF"; else -> "Stiker" }
}

const val PACK_ITEM_MAX_BYTES = 5L * 1024 * 1024
const val PACK_TITLE_MAX = 40
const val PACK_MAX_PER_USER = 30
const val PACK_MAX_WAITING = 100

data class PackInfo(
    val id: Long, val kind: String, val title: String, val file: String = "", val version: Int = 0,
    val items: Int = 0, val bytes: Long = 0, val ownerId: Long = 0, val ownerName: String = "", val sub: Boolean = false,
    val createdAt: Long = 0,
) {
    val usable: Boolean get() = file.isNotEmpty() && items > 0

    fun toJson(): JSONObject = JSONObject().put("id", id).put("kind", kind).put("title", title).put("file", file).put("version", version)
        .put("items", items).put("bytes", bytes).put("owner_id", ownerId).put("owner_name", ownerName).put("sub", sub).put("created_at", createdAt)

    companion object {
        fun from(o: JSONObject) = PackInfo(
            o.optLong("id"), o.optString("kind", PackKind.STICKER), o.optString("title"), o.optString("file").takeIf { it != "null" }.orEmpty(),
            o.optInt("version"), o.optInt("items"), o.optLong("bytes"), o.optLong("owner_id"), o.optString("owner_name").takeIf { it != "null" }.orEmpty(),
            o.optBoolean("sub"), o.optLong("created_at"),
        )
    }
}

/** Kutayotgan / tasdiqlangan / rad etilgan amal (server) yoki hali yuborilmagan (`queued`, telefonda). */
data class PackOp(
    val id: Long, val pack: Long, val op: String, val file: String = "", val item: Long = 0, val emoji: String = "",
    val size: Long = 0, val state: String, val reason: String = "",
) {
    val isAdd: Boolean get() = op == "add"

    companion object {
        fun from(o: JSONObject) = PackOp(
            o.optLong("id"), o.optLong("pack"), o.optString("op"), o.optString("file"), o.optLong("item"),
            o.optString("emoji"), o.optLong("size"), o.optString("state"), o.optString("reason").takeIf { it != "null" }.orEmpty(),
        )
    }
}

data class PackItem(
    val id: Long, val off: Long, val len: Long, val thumbOff: Long, val thumbLen: Long,
    val animated: Boolean, /** Ovozli MP4 (faqat GIF to'plami). */ val video: Boolean, val w: Int, val h: Int, val emoji: String,
) {
    fun toJson(): JSONObject = JSONObject().put("i", id).put("o", off).put("l", len).put("to", thumbOff).put("tl", thumbLen)
        .put("a", if (video) 2 else if (animated) 1 else 0).put("w", w).put("h", h).put("e", emoji)

    companion object {
        fun from(o: JSONObject): PackItem {
            val a = o.optInt("a")
            return PackItem(o.optLong("i"), o.optLong("o"), o.optLong("l"), o.optLong("to"), o.optLong("tl"), a >= 1, a == 2, o.optInt("w"), o.optInt("h"), o.optString("e"))
        }
    }
}

data class PackHeader(val id: Long, val kind: String, val title: String, val ver: Int, val base: Long, val items: List<PackItem>) {
    private val byId = items.associateBy { it.id }
    val thumbEnd: Long = items.maxOfOrNull { it.thumbOff + it.thumbLen } ?: 0
    fun find(itemId: Long): PackItem? = byId[itemId]

    fun toCache(): JSONObject = JSONObject().put("id", id).put("kind", kind).put("title", title).put("ver", ver).put("base", base)
        .put("items", JSONArray().also { a -> items.forEach { a.put(it.toJson()) } })

    companion object {
        fun from(j: JSONObject, base: Long): PackHeader {
            val a = j.optJSONArray("items")
            val items = (0 until (a?.length() ?: 0)).mapNotNull { a?.optJSONObject(it) }.map(PackItem::from)
            return PackHeader(j.optLong("id"), j.optString("kind"), j.optString("title"), j.optInt("ver"), base, items)
        }

        fun fromCache(j: JSONObject?): PackHeader? {
            if (j == null) return null
            val base = j.optLong("base")
            return if (base < 16) null else from(j, base)
        }
    }
}

data class PackRef(val info: PackInfo, val header: PackHeader, val item: PackItem)

/** Xabarga yuboriladigan tanlov (`media_file` = `pk_<to'plam>_<element>`). */
data class PackPick(val kind: String, val pack: Long, val item: Long, val emoji: String) {
    val ref: String get() = "pk_${pack}_$item"
}

private val refRe = Regex("^pk_(\\d{1,16})_(\\d{1,9})$")

/** `pk_<to'plam>_<element>` -> (to'plam, element). */
fun parsePackRef(name: String): Pair<Long, Long>? {
    val m = refRe.find(name) ?: return null
    val p = m.groupValues[1].toLong()
    val i = m.groupValues[2].toLong()
    return if (p > 0 && i > 0) p to i else null
}

/** Matn ichidagi maxsus emoji: `[pe:<to'plam>:<element>:<emoji>]`. */
val PACK_EMOJI_TOKEN = Regex("\\[pe:(\\d{1,16}):(\\d{1,9}):([^\\]]{0,16})\\]")

fun packEmojiToken(pack: Long, item: Long, emoji: String) = "[pe:$pack:$item:${emoji.replace("[", "").replace("]", "")}]"

data class PackLibrary(
    val mine: List<PackInfo> = emptyList(),
    val subs: List<PackInfo> = emptyList(),
    val ops: List<PackOp> = emptyList(),
    val loading: Boolean = false,
    val loaded: Boolean = false,
    val error: String? = null,
)

/**
 * Emoji, GIF va stiker to'plamlari (Flutter `PackService`). To'plam = kanaldagi BITTA shifrlangan `.arp` fayl: boshida sarlavha,
 * keyin kichik rasmlar, keyin elementlar — hech qachon to'liq yuklanmaydi, faqat kerakli BO'LAK (`Range`) mahalliy Telegram
 * manbasidan olinadi. Baytlar xotira (LRU) -> disk (shifrlangan) -> Telegram. Yozuvlar `SyncQueue` orqali.
 */
class PackStore(
    context: Context,
    private val auth: AuthService,
    private val tg: TelegramService,
    private val sync: () -> SyncQueue,
    private val client: OkHttpClient = Http.client,
    private val base: String = BuildConfig.API_BASE,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val dir = File(context.filesDir, "aru_packs").apply { mkdirs() }
    private val cacheDir = File(context.filesDir, "cache").apply { mkdirs() }
    private val _lib = MutableStateFlow(PackLibrary())
    val library: StateFlow<PackLibrary> = _lib.asStateFlow()
    private val _tick = MutableStateFlow(0)

    /** Mahalliy o'zgarish (navbat) bo'lganda ekranlar qayta chizilishi uchun. */
    val changes: StateFlow<Int> = _tick.asStateFlow()
    private fun bump() = _tick.update { it + 1 }

    @Volatile var lastError = ""
    private val known = ConcurrentHashMap<Long, PackInfo>()
    private val headers = ConcurrentHashMap<Long, PackHeader>()
    private val mem = object : LruCache<String, ByteArray>(24 * 1024 * 1024) {
        override fun sizeOf(key: String, value: ByteArray) = value.size
    }
    private val gate = Semaphore(6)
    private val hdrLock = Mutex()
    private val localSubs = ConcurrentHashMap<Long, PackInfo>()
    private val dismissed = HashSet<Long>()
    private var loadedAt = 0L

    private val token: String? get() = auth.sessionToken

    // ── Kutubxona ──
    private val libFile get() = File(cacheDir, "pack_library.json")

    private fun apply(j: JSONObject) {
        fun list(k: String) = j.optJSONArray(k)?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) } }.orEmpty()
        val mine = list("mine").map(PackInfo::from)
        val subs = list("subs").map(PackInfo::from)
        val ops = list("ops").map(PackOp::from)
        (mine + subs).forEach { known[it.id] = it }
        dismissed.retainAll(ops.map { it.id }.toSet())
        _lib.update { it.copy(mine = mine, subs = subs, ops = ops, loaded = true) }
    }

    suspend fun load(force: Boolean = false) = withContext(Dispatchers.IO) {
        if (_lib.value.loading || token == null) return@withContext
        if (!_lib.value.loaded) try { if (libFile.exists()) apply(JSONObject(libFile.readText())) } catch (_: Exception) {}
        if (!force && _lib.value.loaded && System.currentTimeMillis() - loadedAt < 10 * 60_000L) return@withContext
        _lib.update { it.copy(loading = true, error = null) }
        val r = client.callJson("$base/api/packs/library", bearer = token)
        val j = r.json
        if (r.ok && j != null) {
            apply(j)
            loadedAt = System.currentTimeMillis()
            runCatching { libFile.writeText(JSONObject().put("mine", j.optJSONArray("mine")).put("subs", j.optJSONArray("subs")).put("ops", j.optJSONArray("ops")).toString()) }
            localSubs.keys.removeAll(_lib.value.subs.map { it.id }.toSet())
            _lib.update { it.copy(loading = false) }
        } else _lib.update { it.copy(loading = false, error = if (r.code == 0) "Internet yo'q" else "Yuklab bo'lmadi (${r.code})") }
    }

    fun reset() {
        _lib.value = PackLibrary()
        known.clear(); headers.clear(); mem.evictAll(); localSubs.clear(); loadedAt = 0
        bump()
    }

    // ── Navbatdagi (hali yuborilmagan) amallar bilan birlashtirilgan ko'rinish ──
    private fun pending() = sync().pendingPacks()

    val myPacks: List<PackInfo>
        get() {
            val gone = pending().filter { it.optString("op") == "delete" }.map { it.optLong("pack") }.toSet()
            val out = _lib.value.mine.filter { it.id !in gone }.toMutableList()
            for (m in pending()) {
                if (m.optString("op") != "new") continue
                val id = m.optLong("id")
                if (id <= 0 || id in gone || out.any { it.id == id }) continue
                out.add(0, PackInfo(id, m.optString("kind"), m.optString("title"), ownerId = auth.user.value?.id ?: 0))
            }
            return out
        }

    val subPacks: List<PackInfo>
        get() {
            val off = HashSet<Long>(); val on = HashSet<Long>()
            for (m in pending()) if (m.optString("op") == "sub") (if (!m.optBoolean("on", true)) off else on).add(m.optLong("pack"))
            val out = _lib.value.subs.filter { it.id !in off }.toMutableList()
            for (e in localSubs.values) if (e.id in on && out.none { it.id == e.id }) out.add(e)
            return out
        }

    fun isSubscribed(id: Long) = subPacks.any { it.id == id }
    fun usable(kind: String): List<PackInfo> = (myPacks + subPacks).filter { it.kind == kind && it.usable }

    fun opsOf(packId: Long): List<PackOp> {
        val out = _lib.value.ops.filter { it.pack == packId && it.id !in dismissed }.toMutableList()
        for (m in pending()) if (m.optLong("pack") == packId && m.optString("op") == "add") {
            out += PackOp(-1, packId, "add", m.optString("file"), 0, m.optString("emoji"), m.optLong("size"), "queued")
        }
        return out
    }

    fun removedItems(packId: Long): Set<Long> =
        pending().filter { it.optString("op") == "remove" && it.optLong("pack") == packId }.map { it.optLong("item") }.toSet() +
            _lib.value.ops.filter { it.pack == packId && it.op == "remove" && it.state != "rejected" }.map { it.item }

    // ── Yozuvlar (SyncQueue: avval telefonda) ──
    private fun flushSoon() {
        scope.launch {
            delay(3000)
            sync().flush(force = true)
            load(force = true)
            bump()
        }
    }

    fun createPack(kind: String, title: String): PackInfo? {
        val t = title.trim()
        if (kind !in PackKind.all || t.isEmpty() || t.length > PACK_TITLE_MAX || myPacks.size >= PACK_MAX_PER_USER) return null
        val id = (java.security.SecureRandom().nextInt(0x7fffffff) + 1) * 2097152L + java.security.SecureRandom().nextInt(2097152)
        sync().putPack("p:new:$id", JSONObject().put("op", "new").put("id", id).put("kind", kind).put("title", t))
        bump(); flushSoon()
        return PackInfo(id, kind, t, ownerId = auth.user.value?.id ?: 0)
    }

    fun deletePack(id: Long) {
        sync().putPack("p:del:$id", JSONObject().put("op", "delete").put("pack", id))
        bump(); flushSoon()
    }

    fun removeItem(packId: Long, itemId: Long) {
        sync().putPack("p:rm:$packId:$itemId", JSONObject().put("op", "remove").put("pack", packId).put("item", itemId))
        bump(); flushSoon()
    }

    fun setSubscribed(p: PackInfo, on: Boolean) {
        if (on) localSubs[p.id] = p.copy(sub = true)
        sync().putPack("p:sub:${p.id}", JSONObject().put("op", "sub").put("pack", p.id).put("on", on))
        bump(); flushSoon()
    }

    fun cancelQueued(o: PackOp) {
        if (o.state != "queued" || o.file.isEmpty()) return
        sync().removeKey("p:add:${o.file}")
        bump()
    }

    fun clearOp(o: PackOp) {
        if (o.id <= 0) return
        dismissed.add(o.id)
        sync().putPack("p:clear:${o.id}", JSONObject().put("op", "clear").put("pack", o.pack).put("id", o.id))
        bump(); flushSoon()
    }

    fun retryQueued() = flushSoon()

    /** Rasm/video qo'shish: fayl Telegram'ga yuklanadi (`pki_<id>_<vaqt>_<tasodif>.bin`), keyin `add` amali navbatga tushadi. `null` — tayyor. */
    suspend fun addItem(pack: PackInfo, file: File, emoji: String = "", trimStartMs: Long = 0, trimEndMs: Long = 0, sound: Boolean = true, onProgress: ((Float) -> Unit)? = null): String? = withContext(Dispatchers.IO) {
        val uid = auth.user.value?.id ?: 0L
        if (uid <= 0) return@withContext "Avval hisobga kiring"
        val waiting = myPacks.sumOf { p -> opsOf(p.id).count { it.isAdd && (it.state == "pending" || it.state == "queued") } }
        if (waiting >= PACK_MAX_WAITING) return@withContext "Ko'rib chiqilayotgan rasmlar juda ko'p ($PACK_MAX_WAITING ta). Admin ko'rib chiqquncha kuting"
        if (!file.exists()) return@withContext "Fayl topilmadi"
        val size = file.length()
        if (size <= 0) return@withContext "Fayl bo'sh"
        if (size > PACK_ITEM_MAX_BYTES) return@withContext "Fayl 5 MB dan katta (${String.format(java.util.Locale.US, "%.1f", size / 1048576.0)} MB)"
        val head = file.inputStream().use { i -> val buf = ByteArray(32); val n = i.read(buf); if (n <= 0) ByteArray(0) else buf.copyOf(n) }
        if (sniffImage(head).isEmpty()) return@withContext "Faqat rasm (PNG, JPG, GIF, WebP) yoki video (MP4, WebM) yuklash mumkin"
        if (!tg.state.value.authorized) return@withContext "Fayl Telegram orqali yuklanadi — avval Telegram hisobini ulang"
        // Video: tanlangan bo'lak `_t<boshi>-<oxiri>`, ovozsiz bo'lsa `_m` — kesishni Actions bajaradi (Flutter bilan bir xil).
        val isVideo = sniffImage(head).let { it == "mp4" || it == "webm" }
        val trim = if (isVideo && trimEndMs > trimStartMs) "_t$trimStartMs-$trimEndMs" else ""
        val mute = if (isVideo && !sound) "_m" else ""
        val name = "pki_${uid}_${System.currentTimeMillis()}_${java.security.SecureRandom().nextInt(0xffff).toString(16)}$trim$mute.bin"
        val err = tg.uploadFile(file.absolutePath, name, "application/octet-stream", onProgress = { s, t -> if (t > 0) onProgress?.invoke(s.toFloat() / t) }, waitForClaim = false)
        if (err != null) return@withContext err
        // Yuborilgan nusxa telefonda saqlanadi: admin ko'rib chiqquncha oldindan ko'rinadi.
        runCatching { File(dir, "sent").apply { mkdirs() }.let { file.copyTo(File(it, name), overwrite = true) } }
        val e = emoji.replace("[", "").replace("]", "").take(3)
        sync().putPack("p:add:$name", JSONObject().put("op", "add").put("pack", pack.id).put("file", name).put("emoji", e).put("size", size))
        bump(); flushSoon()
        null
    }

    fun sentPreview(file: String): File? = File(File(dir, "sent"), file).takeIf { it.exists() }

    // ── Ommaviy to'plamlar ──
    suspend fun browse(kind: String = "", before: Long = 0): List<PackInfo>? = withContext(Dispatchers.IO) {
        val q = listOfNotNull(kind.takeIf { it.isNotEmpty() }?.let { "kind=$it" }, before.takeIf { it > 0 }?.let { "before=$it" }).joinToString("&")
        val r = client.callJson("$base/api/packs/public${if (q.isEmpty()) "" else "?$q"}", bearer = token)
        val arr = r.json?.optJSONArray("packs")
        if (!r.ok || arr == null) null else (0 until arr.length()).mapNotNull { arr.optJSONObject(it) }.map(PackInfo::from).also { l -> l.forEach { known[it.id] = it } }
    }

    // ── Admin ──
    suspend fun adminPending(): List<JSONObject>? = withContext(Dispatchers.IO) {
        val r = client.callJson("$base/api/packs/admin/pending", bearer = token)
        val arr = r.json?.optJSONArray("ops")
        if (!r.ok || arr == null) null else (0 until arr.length()).mapNotNull { arr.optJSONObject(it) }
    }

    suspend fun adminReview(ids: List<Long>, approve: Boolean, reason: String = ""): String? = withContext(Dispatchers.IO) {
        val r = client.callJson("$base/api/packs/admin/review", "POST", JSONObject().put("ids", JSONArray(ids)).put("approve", approve).put("reason", reason), token)
        if (r.ok) null else r.error ?: "Xato (${r.code})"
    }

    suspend fun adminDeletePack(packId: Long): String? = withContext(Dispatchers.IO) {
        val r = client.callJson("$base/api/packs/admin/delete", "POST", JSONObject().put("pack", packId), token)
        if (r.ok) null else r.error ?: "Xato (${r.code})"
    }

    /** Admin kutilayotgan rasmni ko'rishi uchun (bot chatiga nusxalanadi, baytlar qaytadi). */
    suspend fun stagingBytes(file: String): ByteArray? = tg.fetchBytes(file)

    // ── To'plam ma'lumoti va sarlavha ──
    suspend fun infoFor(packId: Long, refresh: Boolean = false): PackInfo? = withContext(Dispatchers.IO) {
        if (!refresh) known[packId]?.let { return@withContext it }
        val r = client.callJson("$base/api/packs/info?ids=$packId", bearer = token)
        val arr = r.json?.optJSONArray("packs")
        val p = (0 until (arr?.length() ?: 0)).mapNotNull { arr?.optJSONObject(it) }.map(PackInfo::from).firstOrNull { it.id == packId }
        if (p != null) known[p.id] = p
        p ?: known[packId]
    }

    private fun hdrFile(id: Long) = File(cacheDir, "pack_hdr_$id.json")

    suspend fun header(info: PackInfo): PackHeader? = withContext(Dispatchers.IO) {
        if (info.file.isEmpty()) return@withContext null
        headers[info.id]?.takeIf { it.ver >= info.version }?.let { return@withContext it }
        hdrLock.withLock {
            headers[info.id]?.takeIf { it.ver >= info.version }?.let { return@withLock it }
            val disk = try { hdrFile(info.id).takeIf { it.exists() }?.let { PackHeader.fromCache(JSONObject(it.readText())) } } catch (_: Exception) { null }
            if (disk != null && disk.ver >= info.version) { headers[info.id] = disk; return@withLock disk }
            val first = range(info.file, 0, 262_144) ?: return@withLock disk
            if (first.size < 16 || first[0] != 0x41.toByte() || first[1] != 0x52.toByte() || first[2] != 0x55.toByte() || first[3] != 0x50.toByte()) return@withLock disk
            val hlen = ByteBuffer.wrap(first, 8, 4).int.toLong() and 0xffffffffL
            if (hlen <= 0 || hlen > 16L * 1024 * 1024) return@withLock disk
            var bytes = first
            if (16 + hlen > first.size) {
                val rest = range(info.file, first.size.toLong(), 16 + hlen - first.size) ?: return@withLock disk
                bytes = first + rest
            }
            try {
                val j = JSONObject(String(bytes, 16, hlen.toInt(), Charsets.UTF_8))
                val h = PackHeader.from(j, 16 + hlen)
                if (h.id != info.id) return@withLock disk
                headers[info.id] = h
                runCatching { hdrFile(info.id).writeText(h.toCache().toString()) }
                seed(info, h, bytes)
                h
            } catch (_: Exception) {
                disk
            }
        }
    }

    /** Birinchi o'qishda kelgan baytlar ichida to'liq turgan kichik rasm va elementlar darhol keshga tushadi. */
    private fun seed(info: PackInfo, h: PackHeader, bytes: ByteArray) {
        for (it in h.items) {
            fun take(prefix: String, off: Long, len: Long) {
                if (len <= 0 || h.base + off + len > bytes.size) return
                val piece = bytes.copyOfRange((h.base + off).toInt(), (h.base + off + len).toInt())
                val k = "$prefix${info.id}_${it.id}"
                mem.put(k, piece); diskPut(info.id, k, piece)
            }
            take("t", it.thumbOff, it.thumbLen)
            take("d", it.off, it.len)
        }
    }

    suspend fun resolve(packId: Long, itemId: Long): PackRef? {
        var info = infoFor(packId) ?: return null
        var h = header(info)
        var it = h?.find(itemId)
        if (it == null) {
            val fresh = infoFor(packId, refresh = true)
            if (fresh != null && fresh.version > info.version) {
                info = fresh; h = header(info); it = h?.find(itemId)
            }
        }
        return if (h == null || it == null) null else PackRef(info, h, it)
    }

    // ── Baytlar: xotira -> disk (shifrlangan) -> Telegram (bo'lak) ──
    private val failed = ConcurrentHashMap<String, Long>()
    private val flight = ConcurrentHashMap<String, kotlinx.coroutines.Deferred<ByteArray?>>()

    private fun cooling(k: String) = failed[k]?.let { System.currentTimeMillis() - it < 5000 } == true

    suspend fun thumb(r: PackRef): ByteArray? = withContext(Dispatchers.IO) {
        val key = "t${r.info.id}_${r.item.id}"
        mem.get(key)?.let { return@withContext it }
        if (cooling(key)) return@withContext null
        diskGet(r.info.id, key)?.let { mem.put(key, it); return@withContext it }
        // Bitta so'rovda yonma-yon turgan bir nechta kichik rasm olinadi (256 KB oynasi).
        val w = 256L * 1024
        val ws = (r.item.thumbOff / w) * w
        var we = ws + w
        if (r.item.thumbOff + r.item.thumbLen > we) we = r.item.thumbOff + r.item.thumbLen
        if (we > r.header.thumbEnd) we = r.header.thumbEnd
        val blob = range(r.info.file, r.header.base + ws, we - ws)
        if (blob == null || blob.size < we - ws) { failed[key] = System.currentTimeMillis(); return@withContext null }
        var mine: ByteArray? = null
        for (o in r.header.items) {
            if (o.thumbLen <= 0 || o.thumbOff < ws || o.thumbOff + o.thumbLen > we) continue
            val piece = blob.copyOfRange((o.thumbOff - ws).toInt(), (o.thumbOff - ws + o.thumbLen).toInt())
            val k = "t${r.info.id}_${o.id}"
            mem.put(k, piece); diskPut(r.info.id, k, piece)
            if (o.id == r.item.id) mine = piece
        }
        mine
    }

    suspend fun data(r: PackRef): ByteArray? = withContext(Dispatchers.IO) {
        val key = "d${r.info.id}_${r.item.id}"
        mem.get(key)?.let { return@withContext it }
        if (cooling(key)) return@withContext null
        diskGet(r.info.id, key)?.let { mem.put(key, it); return@withContext it }
        val b = range(r.info.file, r.header.base + r.item.off, r.item.len)
        if (b == null || b.size.toLong() != r.item.len) { failed[key] = System.currentTimeMillis(); return@withContext null }
        mem.put(key, b); diskPut(r.info.id, key, b)
        b
    }

    private suspend fun range(file: String, offset: Long, len: Long): ByteArray? {
        if (len <= 0) return ByteArray(0)
        val part = 4L * 1024 * 1024
        if (len <= part) return rangeRetry(file, offset, len)
        val out = java.io.ByteArrayOutputStream()
        var pos = 0L
        while (pos < len) {
            val n = minOf(part, len - pos)
            val b = rangeRetry(file, offset + pos, n) ?: return null
            if (b.size.toLong() != n) return null
            out.write(b); pos += n
        }
        return out.toByteArray()
    }

    private suspend fun rangeRetry(file: String, offset: Long, len: Long): ByteArray? {
        for (attempt in 0 until 3) {
            rangeOnce(file, offset, len)?.let { return it }
            if (attempt < 2) delay(500L * (attempt + 1))
        }
        return null
    }

    private suspend fun rangeOnce(file: String, offset: Long, len: Long): ByteArray? = gate.withPermit {
        withContext(Dispatchers.IO) {
            val url = tg.playUrl(file) ?: run { lastError = "Telegram fayli ochilmadi ($file)"; return@withContext null }
            try {
                LocalHttp.client.newCall(Request.Builder().url(url).header("Range", "bytes=$offset-${offset + len - 1}").build()).execute().use { r ->
                    val b = r.body?.bytes()
                    if ((r.code == 206 || r.code == 200) && b != null && b.size.toLong() == len) b
                    else { lastError = "Telegram javobi ${r.code} ($file, $offset+$len)"; null }
                }
            } catch (e: Exception) {
                lastError = "O'qib bo'lmadi: ${e.message}"
                null
            }
        }
    }

    // ── Disk (shifrlangan) ──
    private fun diskFile(packId: Long, key: String) = File(File(dir, packId.toString()), key)

    private fun diskGet(packId: Long, key: String): ByteArray? = try {
        diskFile(packId, key).takeIf { it.exists() }?.let { SecureStore.open(it.readBytes()) }
    } catch (_: Exception) {
        null
    }

    private fun diskPut(packId: Long, key: String, b: ByteArray) {
        try {
            val f = diskFile(packId, key)
            f.parentFile?.mkdirs()
            f.writeBytes(SecureStore.seal(b))
        } catch (_: Exception) {
        }
    }

    fun cacheBytes(): Long = dir.walkTopDown().filter { it.isFile }.sumOf { it.length() }

    fun clearCache() {
        mem.evictAll()
        dir.listFiles()?.filter { it.name != "sent" }?.forEach { it.deleteRecursively() }
    }
}

/** Fayl boshidan turini aniqlaydi: `png`, `jpg`, `gif`, `webp`, `mp4`, `webm` yoki bo'sh. */
fun sniffImage(h: ByteArray): String {
    fun b(i: Int) = if (i < h.size) h[i].toInt() and 0xff else -1
    return when {
        b(0) == 0x89 && b(1) == 0x50 && b(2) == 0x4E && b(3) == 0x47 -> "png"
        b(0) == 0xFF && b(1) == 0xD8 -> "jpg"
        b(0) == 0x47 && b(1) == 0x49 && b(2) == 0x46 -> "gif"
        b(0) == 0x52 && b(1) == 0x49 && b(2) == 0x46 && b(3) == 0x46 && b(8) == 0x57 && b(9) == 0x45 -> "webp"
        b(4) == 0x66 && b(5) == 0x74 && b(6) == 0x79 && b(7) == 0x70 -> "mp4"
        b(0) == 0x1A && b(1) == 0x45 && b(2) == 0xDF && b(3) == 0xA3 -> "webm"
        else -> ""
    }
}

fun isPackVideo(kind: String) = kind == "mp4" || kind == "webm"

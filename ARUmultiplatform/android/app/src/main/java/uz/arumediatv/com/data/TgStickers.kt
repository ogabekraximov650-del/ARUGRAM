package uz.arumediatv.com.data

import android.content.Context
import android.util.LruCache
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import uz.arumediatv.com.core.RustCore
import uz.arumediatv.com.core.SecureStore
import java.io.File
import java.util.concurrent.ConcurrentHashMap

/**
 * VAQTINCHA: o'z to'plamlarimiz o'rniga foydalanuvchining Telegram hisobidagi stiker / emoji / GIF'lari.
 * Rust (MTProto) ro'yxat va fayllarni oladi, bu yerda diskda shifrlab keshlanadi.
 */
const val TG_STICKERS_ON = true

data class TgDoc(val id: Long, val hash: Long, val ref: String, val dc: Int, val kind: String, val size: Long, val emoji: String, val thumb: String, val animated: Boolean, val custom: Boolean) {
    /** Qurilma o'zi ko'rsata oladigan to'liq fayl: webm/mp4/gif/webp. `.tgs` — hozircha faqat kichik rasm. */
    val playable: Boolean get() = kind == "webm" || kind == "mp4" || kind == "gif" || kind == "webp"
    val isVideo: Boolean get() = kind == "webm" || kind == "mp4" || kind == "gif"
}

data class TgSet(val id: Long, val hash: Long, val title: String, val count: Int, val thumbDoc: Long)

class TgStickerStore(ctx: Context) {
    private val dir = File(ctx.filesDir, "aru_tg").apply { mkdirs() }
    private val mem = object : LruCache<String, ByteArray>(6 * 1024 * 1024) { override fun sizeOf(k: String, v: ByteArray) = v.size }
    private val sets = ConcurrentHashMap<String, List<TgSet>>()
    private val docs = ConcurrentHashMap<String, List<TgDoc>>()
    private val gate = Semaphore(3)

    private fun parseDoc(o: JSONObject) = TgDoc(
        o.optLong("id"), o.optLong("hash"), o.optString("ref"), o.optInt("dc", 2), o.optString("kind"), o.optLong("size"),
        o.optString("emoji"), o.optString("thumb"), o.optBoolean("animated"), o.optBoolean("custom"),
    )

    private fun docList(a: JSONArray?): List<TgDoc> = a?.let { (0 until it.length()).map { i -> parseDoc(it.getJSONObject(i)) } }.orEmpty()

    private val docKeyOf = ConcurrentHashMap<Long, String>()

    private fun remember(key: String, l: List<TgDoc>) { docs[key] = l; l.forEach { docKeyOf[it.id] = key } }

    suspend fun sets(kind: String): List<TgSet> = sets[kind] ?: withContext(Dispatchers.IO) {
        val r = RustCore.tgMediaList("sets:$kind")
        val a = r.optJSONArray("sets") ?: return@withContext emptyList()
        (0 until a.length()).map { val o = a.getJSONObject(it); TgSet(o.optLong("id"), o.optLong("hash"), o.optString("title"), o.optInt("count"), o.optLong("thumb_doc")) }
            .also { sets[kind] = it }
    }

    suspend fun setDocs(s: TgSet): List<TgDoc> = docs["s${s.id}"] ?: withContext(Dispatchers.IO) {
        val r = RustCore.tgMediaList("set:${s.id}:${s.hash}")
        docList(r.optJSONArray("docs")).also { remember("s${s.id}", it) }
    }

    suspend fun gifs(): List<TgDoc> = docs["gifs"] ?: withContext(Dispatchers.IO) {
        docList(RustCore.tgMediaList("gifs").optJSONArray("docs")).also { remember("gifs", it) }
    }

    /** Eskirgan havola uchun ro'yxatni qayta oladi. */
    private suspend fun refresh(d: TgDoc): TgDoc {
        val key = docKeyOf[d.id] ?: return d
        docs.remove(key)
        val l = if (key == "gifs") gifs() else {
            val id = key.removePrefix("s")
            val hash = sets.values.flatten().firstOrNull { it.id.toString() == id }?.hash ?: return d
            setDocs(TgSet(id.toLong(), hash, "", 0, 0))
        }
        return l.firstOrNull { it.id == d.id } ?: d
    }

    /** Hujjatning (yoki kichik rasmining) baytlari; diskda shifrlangan keshdan, bo'lmasa Telegram'dan. */
    suspend fun bytes(d0: TgDoc, wantThumb: Boolean): ByteArray? {
        // Kichik rasmi yo'q hujjat: webp bo'lsa o'zi rasm; boshqasi (.tgs) hozircha ko'rsatilmaydi.
        if (wantThumb && d0.thumb.isEmpty() && d0.kind != "webp") return null
        val thumb = wantThumb && d0.thumb.isNotEmpty()
        val name = "${d0.id}_${if (thumb) "t" else "f"}"
        mem.get(name)?.let { return it }
        return withContext(Dispatchers.IO) {
            val f = File(dir, name)
            if (f.exists()) runCatching { SecureStore.open(f.readBytes()) }.getOrNull()?.let { mem.put(name, it); return@withContext it }
            gate.withPermit {
                var d = d0
                for (attempt in 0..1) {
                    val tmp = File(dir, "$name.tmp")
                    val arg = JSONObject().put("id", d.id).put("hash", d.hash).put("ref", d.ref).put("dc", d.dc)
                        .put("thumb", if (thumb) d.thumb else "").put("out", tmp.absolutePath).put("max", 12L * 1024 * 1024)
                    val r = RustCore.tgMediaGet(arg.toString())
                    if (r.optBoolean("ok")) {
                        val b = runCatching { tmp.readBytes() }.getOrNull()
                        tmp.delete()
                        if (b == null || b.isEmpty()) return@withContext null
                        runCatching { f.writeBytes(SecureStore.seal(b)) }
                        mem.put(name, b)
                        return@withContext b
                    }
                    tmp.delete()
                    if (r.optString("error").contains("FILE_REFERENCE") && attempt == 0) d = refresh(d) else return@withContext null
                }
                null
            }
        }
    }

    fun clear() { dir.listFiles()?.forEach { it.delete() }; mem.evictAll(); sets.clear(); docs.clear() }
}

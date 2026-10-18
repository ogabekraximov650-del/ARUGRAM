package uz.arumediatv.com.data

import android.content.Context
import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
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

data class TgSet(val id: Long, val hash: Long, val title: String, val count: Int, val thumbDoc: Long, val thumbVer: Int = 0, val thumbDc: Int = 2)

class TgStickerStore(ctx: Context) {
    private val dir = File(ctx.filesDir, "aru_tg").apply { mkdirs() }
    private val mem = object : LruCache<String, ByteArray>(6 * 1024 * 1024) { override fun sizeOf(k: String, v: ByteArray) = v.size }
    private val sets = ConcurrentHashMap<String, List<TgSet>>()
    private val docs = ConcurrentHashMap<String, List<TgDoc>>()
    private val gate = Semaphore(3)
    /** Dekodlangan kichik rasmlar: aylantirilganda bo'sh qolmaydi, asosiy oqimda dekod qilinmaydi. */
    private val bmps = object : LruCache<String, ImageBitmap>((Runtime.getRuntime().maxMemory() / 6).toInt().coerceIn(16 shl 20, 96 shl 20)) { override fun sizeOf(k: String, v: ImageBitmap) = v.width * v.height * 4 }

    fun peekPoster(d: TgDoc): ImageBitmap? = bmps.get("p${d.id}")

    /**
     * Dekod qilingan rasm DOIMIY (diskda, shifrlangan xom piksellar) saqlanadi: qayta dekod ham, qayta yuklash ham kerak emas.
     * Tartib: xotira -> disk (xom) -> disk (asl fayl) dekod -> Telegram.
     */
    private fun loadRaw(name: String): ImageBitmap? {
        val f = File(dir, "r_$name")
        if (!f.exists()) return null
        return runCatching {
            val b = SecureStore.open(f.readBytes())
            val bb = java.nio.ByteBuffer.wrap(b)
            val w = bb.int; val h = bb.int
            val bmp = android.graphics.Bitmap.createBitmap(w, h, android.graphics.Bitmap.Config.ARGB_8888)
            bmp.copyPixelsFromBuffer(bb.slice())
            bmp.asImageBitmap()
        }.getOrNull()
    }

    private fun saveRaw(name: String, bmp: android.graphics.Bitmap) {
        if (bmp.width * bmp.height > 320 * 320) return
        runCatching {
            val bb = java.nio.ByteBuffer.allocate(8 + bmp.byteCount)
            bb.putInt(bmp.width).putInt(bmp.height)
            bmp.copyPixelsToBuffer(bb)
            File(dir, "r_$name").writeBytes(SecureStore.seal(bb.array()))
        }
    }

    private fun decode(b: ByteArray, name: String): ImageBitmap? {
        val o = BitmapFactory.Options().apply { inPreferredConfig = android.graphics.Bitmap.Config.ARGB_8888 }
        val bmp = BitmapFactory.decodeByteArray(b, 0, b.size, o) ?: return null
        saveRaw(name, bmp)
        return bmp.asImageBitmap()
    }

    suspend fun poster(d: TgDoc): ImageBitmap? {
        bmps.get("p${d.id}")?.let { return it }
        val r = withContext(Dispatchers.IO) { loadRaw("p${d.id}") }
        if (r != null) { bmps.put("p${d.id}", r); return r }
        val b = bytes(d, true) ?: return null
        return withContext(Dispatchers.Default) { decode(b, "p${d.id}") }?.also { bmps.put("p${d.id}", it) }
    }

    fun peekCover(s: TgSet): ImageBitmap? = bmps.get("c${s.id}")

    /** To'plam muqovasi (animatsiyali to'plamlarda dekod bo'lmaydi — null). */
    suspend fun cover(s: TgSet): ImageBitmap? {
        bmps.get("c${s.id}")?.let { return it }
        if (s.thumbVer == 0) return null
        withContext(Dispatchers.IO) { loadRaw("c${s.id}") }?.let { bmps.put("c${s.id}", it); return it }
        val name = "c${s.id}"
        val b = withContext(Dispatchers.IO) {
            val f = File(dir, name)
            if (f.exists()) runCatching { SecureStore.open(f.readBytes()) }.getOrNull()?.let { return@withContext it }
            gate.withPermit {
                val tmp = File(dir, "$name.tmp")
                val arg = JSONObject().put("set_id", s.id).put("set_hash", s.hash).put("set_ver", s.thumbVer).put("dc", s.thumbDc).put("out", tmp.absolutePath).put("max", 2L * 1024 * 1024)
                if (!RustCore.tgMediaGet(arg.toString()).optBoolean("ok")) { tmp.delete(); return@withPermit null }
                val x = runCatching { tmp.readBytes() }.getOrNull(); tmp.delete()
                if (x != null && x.isNotEmpty()) runCatching { File(dir, name).writeBytes(SecureStore.seal(x)) }
                x
            }
        } ?: return null
        return withContext(Dispatchers.Default) { decode(b, "c${s.id}") }?.also { bmps.put("c${s.id}", it) }
    }

    private fun parseDoc(o: JSONObject) = TgDoc(
        o.optLong("id"), o.optLong("hash"), o.optString("ref"), o.optInt("dc", 2), o.optString("kind"), o.optLong("size"),
        o.optString("emoji"), o.optString("thumb"), o.optBoolean("animated"), o.optBoolean("custom"),
    )

    private fun docList(a: JSONArray?): List<TgDoc> = a?.let { (0 until it.length()).map { i -> parseDoc(it.getJSONObject(i)) } }.orEmpty()

    private val docKeyOf = ConcurrentHashMap<Long, String>()

    private fun remember(key: String, l: List<TgDoc>) { if (l.isNotEmpty()) docs[key] = l; l.forEach { docKeyOf[it.id] = key } }

    /** Ro'yxat javobi: diskdagi nusxa (24 soatgacha yangi) bo'lsa o'sha; bo'lmasa Telegram'dan olib diskka yoziladi. */
    private fun listJson(key: String, cmd: String): JSONObject? {
        val f = File(dir, "list_$key")
        val cached = if (f.exists()) runCatching { JSONObject(String(SecureStore.open(f.readBytes()), Charsets.UTF_8)) }.getOrNull() else null
        if (cached != null && System.currentTimeMillis() - f.lastModified() < 24L * 3600 * 1000) return cached
        val r = RustCore.tgMediaList(cmd)
        if (r.optBoolean("ok")) {
            runCatching { f.writeBytes(SecureStore.seal(r.toString().toByteArray(Charsets.UTF_8))) }
            return r
        }
        return cached
    }

    suspend fun sets(kind: String): List<TgSet> = sets[kind] ?: withContext(Dispatchers.IO) {
        val r = listJson("sets_$kind", "sets:$kind") ?: return@withContext emptyList()
        val a = r.optJSONArray("sets") ?: return@withContext emptyList()
        (0 until a.length()).map { val o = a.getJSONObject(it); TgSet(o.optLong("id"), o.optLong("hash"), o.optString("title"), o.optInt("count"), o.optLong("thumb_doc"), o.optInt("thumb_ver"), o.optInt("thumb_dc", 2).let { if (it == 0) 2 else it }) }
            .also { if (it.isNotEmpty()) sets[kind] = it }
    }

    suspend fun setDocs(s: TgSet): List<TgDoc> = docs["s${s.id}"] ?: withContext(Dispatchers.IO) {
        val r = listJson("set_${s.id}", "set:${s.id}:${s.hash}")
        docList(r?.optJSONArray("docs")).also { remember("s${s.id}", it) }
    }

    suspend fun gifs(): List<TgDoc> = docs["gifs"] ?: withContext(Dispatchers.IO) {
        docList(listJson("gifs", "gifs")?.optJSONArray("docs")).also { remember("gifs", it) }
    }

    /** Eskirgan havola uchun ro'yxatni qayta oladi. */
    private suspend fun refresh(d: TgDoc): TgDoc {
        val key = docKeyOf[d.id] ?: return d
        docs.remove(key)
        File(dir, if (key == "gifs") "list_gifs" else "list_set_${key.removePrefix("s")}").delete()
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

    /** Tizim xotirani tozalashni so'raganda. */
    fun trim() { bmps.evictAll(); mem.evictAll() }

    fun clear() { dir.listFiles()?.forEach { it.delete() }; mem.evictAll(); bmps.evictAll(); sets.clear(); docs.clear() }
}

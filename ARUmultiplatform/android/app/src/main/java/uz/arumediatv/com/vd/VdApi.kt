package uz.arumediatv.com.vd

import android.content.Context
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONObject
import uz.arumediatv.com.BuildConfig
import uz.arumediatv.com.core.RustCore
import uz.arumediatv.com.core.SecureStore
import uz.arumediatv.com.data.Http
import uz.arumediatv.com.data.callJson
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit

/**
 * Video ombori (`vd`, ARUDRIVE uslubi) bilan aloqa: fayl yangi tizimda bormi (`/api/vd/info`), isitish (Telegram -> Cloudflare kesh) va
 * pleyer manzili. Kalit (bitta fayl = bitta kalit) faqat kirgan foydalanuvchiga beriladi; xotirada va Keystore bilan shifrlangan
 * SharedPreferences'da saqlanadi (tarmoqsiz ochish uchun kerak emas: ko'rilgan joy telefon keshida ochilgan holda turadi).
 */
object VdApi {
    private val base = BuildConfig.API_BASE
    private val mem = ConcurrentHashMap<String, Info>()

    class Info(val name: String, val size: Long, val key: String, val state: String, val chunks: Int, val stored: Int) {
        val ready: Boolean get() = state == "ready"
        /** Birinchi bo'lak Telegram'ga saqlangan bo'lsa pleyer ochiladi (qolganlari saqlanguncha kutadi). */
        val playable: Boolean get() = size > 0 && key.length == 32 && (ready || stored > 0)
    }

    private fun store(ctx: Context) = SecureStore(ctx.applicationContext, "aru_vdinfo")

    fun cached(name: String): Info? = mem[name]

    /** Fayl haqida ma'lumot: avval tarmoqdan (yangi holat), tarmoq yo'q bo'lsa saqlangan nusxadan. `null` — yangi tizimda yo'q (eski yo'l bilan ochiladi). */
    fun lookup(ctx: Context, name: String, bearer: String?): Info? {
        mem[name]?.takeIf { it.ready }?.let { return it }
        val r = Http.client.callJson("$base/api/vd/info", "POST", JSONObject().put("names", JSONArray().put(name)), bearer)
        if (r.ok) {
            val f = r.json?.optJSONObject("files")?.optJSONObject(name) ?: return null
            val i = Info(name, f.optLong("size"), f.optString("key"), f.optString("state"), f.optInt("chunks"), f.optInt("stored"))
            if (i.playable) {
                mem[name] = i
                store(ctx).putString(name, "${i.size}|${i.key}|${i.state}|${i.chunks}|${i.stored}")
            }
            return i.takeIf { it.playable }
        }
        if (r.code == 0) { // tarmoq yo'q — saqlangan nusxa
            val p = store(ctx).getString(name)?.split('|') ?: return null
            if (p.size < 5) return null
            return Info(name, p[0].toLongOrNull() ?: 0, p[1], p[2], p[3].toIntOrNull() ?: 0, p[4].toIntOrNull() ?: 0).takeIf { it.playable }?.also { mem[name] = it }
        }
        return null
    }

    fun forget(ctx: Context, name: String) {
        mem.remove(name)
        store(ctx).remove(name)
        BlockCache.remove(ctx, name)
    }

    // ── Isitish (Telegram -> Cloudflare kesh) ──

    const val WARM_WINDOW = VdCtr.CHUNK

    class WarmResult(val ok: Boolean, val text: String, val total: Long = 0, val pending: Boolean = false)

    fun warmDetailed(name: String, window: Long, force: Boolean = false): WarmResult {
        val path = "/api/warm/vd~$name"
        val req = Request.Builder().url("$base$path?w=$window" + if (force) "&force=1" else "").build()
        val t0 = System.currentTimeMillis()
        return try {
            Http.client.newBuilder().readTimeout(15, TimeUnit.MINUTES).callTimeout(0, TimeUnit.SECONDS).build().newCall(req).execute().use { r ->
                val body = r.body?.string().orEmpty()
                val j = runCatching { JSONObject(body) }.getOrNull()
                val st = j?.optString("status").orEmpty()
                val total = j?.optLong("total") ?: 0L
                val ok = r.isSuccessful && (st == "cached" || st == "warmed")
                if (ok && total > 0) VdWarm.totals[name] = total
                WarmResult(ok, "HTTP ${r.code} ${st.ifEmpty { body.take(60) }} · ${(System.currentTimeMillis() - t0) / 1000.0}s", total, st.startsWith("pending"))
            }
        } catch (e: Exception) {
            WarmResult(false, "Xato: ${e.javaClass.simpleName} ${e.message.orEmpty().take(80)}")
        }
    }

    /** Oyna keshga isitilguncha kutadi; bo'lak hali Telegram'ga saqlanmagan bo'lsa ("pending") 10 daqiqagacha qayta so'raydi. */
    fun warmBlocking(name: String, window: Long, force: Boolean = false): Boolean {
        val end = System.currentTimeMillis() + 10 * 60_000
        var first = force
        while (true) {
            val r = warmDetailed(name, window, first)
            first = false
            if (r.ok) return true
            if (!r.pending || System.currentTimeMillis() > end) return false
            Thread.sleep(3000)
        }
    }

    /** ExoPlayer manzili: yo'l imzolangan token bilan (Rust yadrosi, 6 soat). */
    fun playUrl(name: String): String {
        val path = "/api/play/vd~$name"
        val t = RustCore.playToken(path)
        return base + path + if (t.isEmpty()) "" else "?t=$t"
    }
}

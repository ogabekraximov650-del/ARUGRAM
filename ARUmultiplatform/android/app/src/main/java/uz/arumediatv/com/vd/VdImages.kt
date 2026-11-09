package uz.arumediatv.com.vd

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONObject
import uz.arumediatv.com.BuildConfig
import uz.arumediatv.com.data.Http
import uz.arumediatv.com.data.callJson
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Rasmlar (poster, bo'lim rasmi, profil rasmi) yangi tizimdan: kalit va hajm BIR so'rovda ko'pchilik uchun (`/api/vd/imginfo`, sessiyasiz — kirmagan
 * foydalanuvchi ham ko'radi), shifrlangan baytlar `/api/vd/img/<nom>` dan (Cloudflare keshi, 1000 kun), shifrni ilova ochadi. Yangi tizimda bo'lmagan
 * rasm uchun `null` qaytadi — [uz.arumediatv.com.data.ImageStore] eski yo'lga (Telegram) o'tadi.
 */
object VdImages {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private class Req(val name: String, val done: CompletableDeferred<VdApi.Info?>)

    private val queue = ConcurrentLinkedQueue<Req>()
    private val flushing = AtomicBoolean(false)
    private val known = ConcurrentHashMap<String, VdApi.Info>()
    /** Yangi tizimda topilmaganlar: 1 daqiqa qayta so'ralmaydi (yuklanayotgan rasm tez orada tayyor bo'ladi). */
    private val negative = ConcurrentHashMap<String, Long>()
    private const val NEGATIVE_MS = 60_000L
    private val base = BuildConfig.API_BASE

    private suspend fun info(name: String): VdApi.Info? {
        known[name]?.let { return it }
        negative[name]?.let { if (System.currentTimeMillis() - it < NEGATIVE_MS) return null }
        val r = Req(name, CompletableDeferred())
        queue.add(r)
        if (flushing.compareAndSet(false, true)) scope.launch { delay(30); flushing.set(false); flush() }
        return r.done.await()
    }

    private fun flush() {
        val batch = ArrayList<Req>()
        while (batch.size < 100) batch.add(queue.poll() ?: break)
        if (batch.isEmpty()) return
        val names = batch.map { it.name }.distinct()
        val res = Http.client.callJson("$base/api/vd/imginfo", "POST", JSONObject().put("names", JSONArray(names)))
        val files = res.json?.optJSONObject("files")
        for (r in batch) {
            val f = files?.optJSONObject(r.name)
            val i = f?.let { VdApi.Info(r.name, it.optLong("size"), it.optString("key"), it.optString("state"), it.optInt("chunks"), it.optInt("stored")) }
            if (i != null && i.ready && i.key.length == 32 && i.size > 0) {
                known[r.name] = i
                r.done.complete(i)
            } else {
                if (res.ok) negative[r.name] = System.currentTimeMillis() // tarmoq xatosida eslab qolinmaydi
                r.done.complete(null)
            }
        }
        if (!queue.isEmpty()) scope.launch { flush() }
    }

    /** Ochilgan rasm baytlari yoki `null` (yangi tizimda yo'q / tarmoq xatosi). */
    suspend fun fetch(name: String): ByteArray? {
        val i = info(name) ?: return null
        return try {
            Http.client.newBuilder().readTimeout(30, java.util.concurrent.TimeUnit.SECONDS).build()
                .newCall(Request.Builder().url("$base/api/vd/img/$name").build()).execute().use { r ->
                    if (!r.isSuccessful) return@use null
                    val enc = r.body?.bytes() ?: return@use null
                    if (enc.size.toLong() != i.size) return@use null
                    VdCtr.apply(VdCtr.hex(i.key), 0, enc)
                }
        } catch (_: Exception) {
            null
        }
    }
}

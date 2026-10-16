package uz.arumediatv.com.data

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import org.json.JSONArray
import uz.arumediatv.com.BuildConfig
import uz.arumediatv.com.core.RustCore
import java.util.concurrent.atomic.AtomicInteger

data class GateChannel(val id: Long, val kind: String, val title: String, val url: String) {
    /** Bajarilganini eslab qolish kaliti: manzil o'zgarsa qayta uriniladi. */
    val doneKey: String get() = "$kind|$url"
}

data class GateState(
    val consented: Boolean = false,
    /** `null` — hali noma'lum. */
    val channels: List<GateChannel>? = null,
) {
    /** Ruxsat so'rash kerakmi: ruxsat yo'q va ro'yxatda (yoki hali noma'lum) kanal bor. */
    val needsConsent: Boolean get() = !consented && (channels == null || channels.isNotEmpty())
}

/**
 * Bepul bo'limlar uchun majburiy kanallar (Flutter `ChannelGate`). Foydalanuvchi bir marta ruxsat bersa, ilova uning Telegram hisobi
 * bilan kanallarga BIR-BIRLAB (5-10 soniya oraliq) qo'shiladi: pleyer ochiq paytda tarmoq faqat videoniki, Telegram limiti
 * (FLOOD) bo'lsa kamida 30 daqiqa kutiladi. Ruxsat hisobda (`chan_consent`) saqlanadi.
 */
class ChannelGate(
    context: Context,
    private val auth: AuthService,
    private val tg: TelegramService,
    private val client: OkHttpClient = Http.client,
    private val base: String = BuildConfig.API_BASE,
) {
    private val prefs = context.getSharedPreferences("aru_gate", Context.MODE_PRIVATE)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val _state = MutableStateFlow(GateState())
    val state: StateFlow<GateState> = _state.asStateFlow()
    private var timer: Job? = null
    private var running = false
    private var listAt = 0L
    private val skip = HashSet<String>()

    /** Pleyer ochiq turgan ekranlar soni: shu paytda kanallarga qo'shilish to'xtaydi. */
    val busy = AtomicInteger(0)

    private fun uid() = auth.user.value?.id ?: 0L
    private fun doneSet(): MutableSet<String> = prefs.getStringSet("done_${uid()}", emptySet())!!.toMutableSet()
    private fun pauseUntil(): Long = prefs.getLong("pause_${uid()}", 0)

    /** Hisob o'zgarganda yoki ilova ochilganda: ruxsat hisobdan o'qiladi, kanallar diskdan. */
    fun sync() {
        val consent = auth.user.value?.chanConsent == true
        val cached = prefs.getString("list_${uid()}", null)?.let { parse(it) }
        _state.update { it.copy(consented = consent, channels = it.channels ?: cached) }
    }

    fun start() {
        sync()
        if (_state.value.consented) arm(5_000)
    }

    /** "Ruxsat berish" bosildi. */
    fun grant() {
        auth.updateSettings(chanConsent = true)
        _state.update { it.copy(consented = true) }
        arm(5_000)
    }

    /** Sozlamalarda o'chirildi: orqa fondagi qo'shilish to'xtaydi (qo'shilgan kanallardan chiqilmaydi). */
    fun revoke() {
        auth.updateSettings(chanConsent = false)
        _state.update { it.copy(consented = false) }
        timer?.cancel()
    }

    fun setConsent(on: Boolean) = if (on) grant() else revoke()

    private fun parse(text: String): List<GateChannel> = try {
        val a = JSONArray(text)
        (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { GateChannel(it.optLong("id"), it.optString("kind"), it.optString("title"), it.optString("url")) }
    } catch (_: Exception) {
        emptyList()
    }

    /** Ro'yxat serverdan (15 daqiqada bir marta). */
    suspend fun refresh(force: Boolean = false) = withContext(Dispatchers.IO) {
        val now = System.currentTimeMillis()
        if (!force && _state.value.channels != null && now - listAt < 15 * 60_000L) return@withContext
        val token = auth.sessionToken ?: return@withContext
        val r = client.callJson("$base/api/channels", bearer = token)
        val arr = r.json?.optJSONArray("items")
        if (r.ok && arr != null) {
            _state.update { it.copy(channels = parse(arr.toString())) }
            listAt = now
            prefs.edit().putString("list_${uid()}", arr.toString()).apply()
        }
    }

    private fun arm(afterMs: Long) {
        timer?.cancel()
        timer = scope.launch {
            delay(afterMs)
            tick()
        }
    }

    private suspend fun tick() {
        if (running || !_state.value.consented) return
        if (busy.get() > 0) return arm(10_000)
        val now = System.currentTimeMillis()
        if (now < pauseUntil()) return arm(pauseUntil() - now)
        if (!tg.state.value.authorized) return arm(EVERY)
        running = true
        var next = EVERY
        try {
            refresh()
            if (busy.get() > 0) { next = 10_000; return }
            val done = doneSet()
            val todo = (_state.value.channels ?: emptyList()).filter { it.url.isNotEmpty() && it.doneKey !in done && it.doneKey !in skip }
            if (todo.isEmpty()) { skip.clear(); return }
            val c = todo.first()
            val j = RustCore.tgJoinChannel("${c.kind}:${c.url}")
            val err = j.optString("error")
            when {
                j.optBoolean("ok") -> { done.add(c.doneKey); prefs.edit().putStringSet("done_${uid()}", done).apply() }
                err.contains("FLOOD") || err.contains("CHANNELS_TOO_MUCH") || j.has("wait") -> {
                    var wait = 30 * 60_000L
                    val secs = Regex("value:\\s*(\\d+)").find(err)?.groupValues?.get(1)?.toLongOrNull() ?: Regex("FLOOD_(?:PREMIUM_)?WAIT_(\\d+)").find(err)?.groupValues?.get(1)?.toLongOrNull() ?: j.optLong("wait")
                    if (secs > wait / 1000) wait = secs * 1000
                    prefs.edit().putLong("pause_${uid()}", System.currentTimeMillis() + wait).apply()
                    next = wait
                    return
                }
                // Havola yaroqsiz / kanal yo'q — admin tuzatmaguncha urinilmaydi.
                err.contains("INVITE_HASH") || err.contains("USERNAME_") || err.contains("noto'g'ri") || err.contains("topilmadi") -> {
                    done.add(c.doneKey); prefs.edit().putStringSet("done_${uid()}", done).apply()
                }
                else -> skip.add(c.doneKey)
            }
            next = 5_000L + (0..5_000).random()
        } finally {
            running = false
            if (_state.value.consented) arm(next)
        }
    }

    private companion object {
        const val EVERY = 5 * 60_000L
    }
}

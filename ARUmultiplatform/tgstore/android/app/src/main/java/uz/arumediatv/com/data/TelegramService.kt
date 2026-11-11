package uz.arumediatv.com.data

import android.content.Context
import android.os.Build
import kotlinx.coroutines.CompletableDeferred
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
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONObject
import uz.arumediatv.com.BuildConfig
import uz.arumediatv.com.core.RustCore
import uz.arumediatv.com.core.SecureStore
import java.io.File
import java.util.concurrent.ConcurrentHashMap

data class TgState(
    val started: Boolean = false,
    /** Server Telegram orqali ko'rishni yoqqan (api_id/hash bor). */
    val configured: Boolean = false,
    /** Telegram hisobi (MTProto sessiyasi) ulangan. */
    val authorized: Boolean = false,
    val port: Int = 0,
    val configUnreachable: Boolean = false,
    val error: String? = null,
)

/** Telegram kirish bosqichi natijasi (Rust `ok/password/hint/error/sent/wait`). */
data class TgLoginStep(
    val done: Boolean = false,
    val needPassword: Boolean = false,
    val hint: String = "",
    val error: String? = null,
    /** Kod yuborilgan joy (`type`: sms/call/...; `length` ...). */
    val sent: JSONObject? = null,
    val loggedIn: Boolean = false,
    val wait: Int = 0,
) {
    companion object {
        fun from(j: JSONObject) = TgLoginStep(
            done = j.optBoolean("ok"),
            needPassword = j.optBoolean("password"),
            hint = j.optString("hint"),
            error = j.optString("error").takeIf { it.isNotEmpty() },
            sent = j.optJSONObject("sent"),
            loggedIn = j.optBoolean("ok") && j.optJSONObject("sent") == null,
            wait = j.optInt("wait"),
        )
    }
}

/**
 * Telegram (MTProto) bilan ishlash — Flutter `TelegramService` ning Kotlin nusxasi.
 *
 * Fayl (rasm/video) ilovaga shunday keladi: worker `POST /api/tg/deliver` faylni foydalanuvchining BOT CHATIGA nusxalaydi;
 * Rust yadrosi (foydalanuvchi sifatida kirgan) uni shu chatdan oladi, shifrni ochadi va mahalliy serverda
 * (`http://127.0.0.1:<port>/tg/0/<nom>`) beradi. Fayl baytlari worker orqali O'TMAYDI.
 */
class TelegramService(
    private val context: Context,
    private val auth: AuthService,
    private val secure: SecureStore,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val base = BuildConfig.API_BASE
    private val _state = MutableStateFlow(TgState())
    val state: StateFlow<TgState> = _state.asStateFlow()

    private var bot = ""
    private var video = true
    private var channel = 0L
    private val startLock = Mutex()
    private val missing = ConcurrentHashMap<String, Long>()
    private val dir: String get() = File(context.filesDir, "tg").apply { mkdirs() }.absolutePath

    val active: Boolean get() = _state.value.let { it.authorized && it.configured && video }

    // ── Ishga tushirish ────────────────────────────────────────

    suspend fun start() {
      startLock.withLock {
        if (_state.value.started) return@withLock
        withContext(Dispatchers.IO) {
            if (!RustCore.available) {
                _state.update { it.copy(error = RustCore.loadError ?: "Yadro yuklanmadi") }
                return@withContext
            }
            RustCore.tgSetDevice(
                "${Build.MANUFACTURER} ${Build.MODEL}".trim(),
                "Android ${Build.VERSION.RELEASE}",
                "ARUMEDIATV ${BuildConfig.APP_VERSION.substringBefore('+')}",
            )
            apply(RustCore.tgInit(dir, 0, ""))
            _state.update { it.copy(started = RustCore.available) }
        }
        if (_state.value.started) {
            scope.launch {
                refreshConfig()
                checkSession()
            }
        }
      }
    }

    /** Rust holati (`configured/authorized/port`) -> `TgState`. */
    private fun apply(j: JSONObject) {
        if (j.has("error")) {
            _state.update { it.copy(error = j.optString("error")) }
            return
        }
        _state.update {
            it.copy(
                configured = j.optBoolean("configured", it.configured),
                authorized = j.optBoolean("authorized", it.authorized),
                port = j.optInt("port", it.port),
                error = null,
            )
        }
    }

    suspend fun refreshConfig(force: Boolean = false): Boolean = withContext(Dispatchers.IO) {
        if (!force) {
            val cached = secure.getString(KEY_CONFIG)?.let { runCatching { JSONObject(it) }.getOrNull() }
            val at = cached?.optLong("at") ?: 0L
            if (cached != null && System.currentTimeMillis() - at < CONFIG_TTL_MS && cached.optBoolean("enabled")) {
                applyConfig(cached)
                return@withContext _state.value.configured
            }
        }
        val r = Http.client.callJson("$base/api/tg/config")
        val j = r.json
        if (!r.ok || j == null) {
            _state.update { it.copy(configUnreachable = true) }
            return@withContext _state.value.configured
        }
        _state.update { it.copy(configUnreachable = false) }
        applyConfig(j)
        secure.putString(KEY_CONFIG, JSONObject(j.toString()).put("at", System.currentTimeMillis()).toString())
        _state.value.configured
    }

    private fun applyConfig(j: JSONObject) {
        bot = j.optString("bot", bot)
        if (bot.isNotEmpty()) RustCore.tgSetBot(bot)
        video = j.optBoolean("video", true)
        channel = j.optLong("channel", 0)
        if (j.optBoolean("enabled")) {
            val id = j.optInt("api_id")
            val hash = j.optString("api_hash")
            if (id > 0 && hash.isNotEmpty()) apply(RustCore.tgInit(dir, id, hash))
        }
    }

    suspend fun checkSession() = withContext(Dispatchers.IO) {
        if (!_state.value.started) return@withContext
        val was = _state.value.authorized
        apply(RustCore.tgCheckSession())
        if (was && !_state.value.authorized) onSessionLost()
    }

    private fun onSessionLost() {
        missing.clear()
        scope.launch { if (auth.isLoggedIn) auth.logout() }
    }

    // ── Kirish (telefon -> kod -> parol) ────────────────────────

    suspend fun requestCode(phone: String): TgLoginStep = withContext(Dispatchers.IO) {
        refreshConfig()
        if (!_state.value.configured) return@withContext TgLoginStep(error = "Telegram orqali ko'rish hozircha yoqilmagan")
        val step = TgLoginStep.from(RustCore.tgRequestCode(phone))
        if (step.loggedIn) afterLogin()
        step
    }

    suspend fun resendCode(): TgLoginStep = withContext(Dispatchers.IO) {
        val step = TgLoginStep.from(RustCore.tgResendCode())
        if (step.loggedIn) afterLogin()
        step
    }

    suspend fun signIn(code: String): TgLoginStep = withContext(Dispatchers.IO) {
        val step = TgLoginStep.from(RustCore.tgSignIn(code))
        if (step.done) afterLogin()
        step
    }

    suspend fun checkPassword(password: String): TgLoginStep = withContext(Dispatchers.IO) {
        val step = TgLoginStep.from(RustCore.tgCheckPassword(password))
        if (step.done) afterLogin()
        step
    }

    /** Ilova hisobiga kirish: yadro botga `/start <token>` yuboradi (Telegram foydalanuvchisi sifatida). `null` — muvaffaqiyat. */
    suspend fun startBot(token: String): String? = withContext(Dispatchers.IO) {
        if (bot.isEmpty()) refreshConfig(force = true)
        if (bot.isEmpty()) return@withContext "Bot nomi serverdan olinmadi"
        val j = RustCore.tgStartBot(bot, token)
        if (j.optBoolean("ok")) null else j.optString("error", "Xato")
    }

    suspend fun nearestCountry(): String? = withContext(Dispatchers.IO) { RustCore.tgNearestCountry().takeIf { it.isNotEmpty() } }

    fun resetLogin() = RustCore.tgLoginReset()

    /** QR uchun token: `{"url","expires"}`, tasdiqlansa `{"ok":true}`, parol kerak bo'lsa `{"password":true,"hint"}`. */
    suspend fun qrToken(): JSONObject = withContext(Dispatchers.IO) {
        refreshConfig()
        val j = RustCore.tgQrToken()
        if (j.optBoolean("ok")) afterLogin()
        j
    }

    fun qrAccepted(): Boolean = RustCore.tgQrAccepted()

    suspend fun logout() = withContext(Dispatchers.IO) {
        if (!_state.value.started) return@withContext
        RustCore.tgLogout()
        _state.update { it.copy(authorized = false) }
        missing.clear()
    }

    private fun afterLogin() {
        _state.update { it.copy(authorized = true) }
        missing.clear()
        clearPending = true
        scheduleClear(2000)
    }

    // ── Fayllar (rasm) ─────────────────────────────────────────

    private val batch = HashMap<String, MutableList<CompletableDeferred<ByteArray?>>>()
    private var batchJob: Job? = null
    private val batchLock = Any()

    /** Fayl nomi yoki URL oxiri -> xavfsiz nom (`[A-Za-z0-9._-]+`), aks holda bo'sh. */
    fun fileNameOf(url: String): String = Names.fileNameOf(url)

    /**
     * Rasm baytlari. 150 ms ichidagi so'rovlar BITTA `deliver` ga yig'iladi (bot chatiga bir vaqtda ko'p fayl).
     * `null` — hisob ulanmagan / fayl topilmadi.
     */
    suspend fun fetchBytes(url: String): ByteArray? {
        val name = fileNameOf(url)
        if (name.isEmpty() || !active || _state.value.port == 0 || isMissing(name)) return null
        val d = CompletableDeferred<ByteArray?>()
        synchronized(batchLock) {
            batch.getOrPut(name) { mutableListOf() }.add(d)
            if (batchJob == null) {
                batchJob = scope.launch {
                    delay(150)
                    runBatch()
                }
            }
        }
        return d.await()
    }

    /**
     * Videoni bot chatiga oldiradi va pleyer uchun mahalliy manzilni qaytaradi (Range'ni qo'llaydi).
     * `null` — Telegram ishlatib bo'lmaydi / fayl kanalda yo'q: chaqiruvchi worker yo'liga o'tadi.
     */
    suspend fun playUrl(url: String): String? = withContext(Dispatchers.IO) {
        val name = fileNameOf(url)
        val port = _state.value.port
        if (name.isEmpty() || !active || port == 0 || isMissing(name)) return@withContext null
        clearing?.let { withTimeoutOrNullCompat(8000) { it.join() } }
        delivering.incrementAndGet()
        try {
            if (name in deliver(listOf(name))) {
                "http://127.0.0.1:$port/tg/0/$name"
            } else {
                missing[name] = System.currentTimeMillis()
                null
            }
        } catch (_: Exception) {
            null
        } finally {
            scope.launch {
                delay(10_000)
                delivering.decrementAndGet()
                clearIfPending()
            }
        }
    }

    private val uploadedKeys = ConcurrentHashMap<String, String>()

    /** Yuklangan faylning ochish kaliti (worker'ga aytiladi). */
    fun keyFor(fileName: String): String = uploadedKeys[fileName].orEmpty()

    /**
     * Faylni Telegram'ga (bot chatiga) yuboradi — baytlar worker'dan O'TMAYDI; kanalga ko'chirishni bot o'zi qiladi.
     * `null` — tayyor (kanalga ko'chirilgan); aks holda xato matni. Chaqiruvchi fayl nomini `<turi>_<id>_<vaqt>.<kengaytma>` qolipida beradi.
     */
    suspend fun uploadFile(
        path: String,
        fileName: String,
        mime: String,
        onProgress: ((sent: Long, total: Long) -> Unit)? = null,
        waitForClaim: Boolean = true,
    ): String? = withContext(Dispatchers.IO) {
        if (!_state.value.authorized) return@withContext "Telegram hisobi ulanmagan"
        if (channel == 0L) refreshConfig(force = true)
        if (channel == 0L) return@withContext "Kanal sozlanmagan"
        val started = RustCore.tgUploadStart(path, fileName, mime)
        val job = started.optLong("job")
        if (job <= 0) return@withContext started.optString("error").ifEmpty { "Yuklash boshlanmadi" }
        var key = ""
        var allSentAt = 0L
        while (true) {
            delay(400)
            val st = RustCore.tgUploadStatus(job)
            val sent = st.optLong("sent")
            val total = st.optLong("total")
            if (total > 0) onProgress?.invoke(sent, total)
            if (total > 0 && sent >= total && !st.optBoolean("done")) {
                if (allSentAt == 0L) allSentAt = System.currentTimeMillis()
                if (System.currentTimeMillis() - allSentAt > 120_000) {
                    RustCore.tgUploadCancel(job)
                    return@withContext "Telegram javob bermadi — internetni tekshirib, qayta urinib ko'ring"
                }
            }
            if (st.optBoolean("done")) {
                st.optString("error").takeIf { it.isNotEmpty() && it != "null" }?.let { return@withContext it }
                key = st.optString("key").takeIf { it != "null" }.orEmpty()
                break
            }
        }
        missing.remove(fileName)
        if (key.isNotEmpty()) uploadedKeys[fileName] = key
        val s = auth.sessionToken ?: return@withContext "Ilova hisobiga kirilmagan"
        delivering.incrementAndGet()
        if (waitForClaim) awaitClaim(fileName, key, s) else scope.launch { awaitClaim(fileName, key, s) }
        null
    }

    /** Bot chatidan kanalga ko'chirilishini kutadi (odatda 1-2 s); kutish tugasa ham fayl QAYTA yuborilmaydi. */
    private suspend fun awaitClaim(fileName: String, key: String, s: String) {
        var ready = false
        for (wait in listOf(1, 1, 2, 2, 3, 4, 5, 8)) {
            delay(wait * 1000L)
            val r = Http.client.callJson(
                "$base/api/tg/claim?file=${java.net.URLEncoder.encode(fileName, "UTF-8")}&key=${java.net.URLEncoder.encode(key, "UTF-8")}",
                bearer = s,
            )
            if (r.ok && r.json?.optBoolean("ready") == true) {
                ready = true
                break
            }
        }
        if (ready) {
            delivering.decrementAndGet()
            clearPending = true
            clearIfPending()
        } else {
            scope.launch {
                delay(3 * 60_000L)
                delivering.decrementAndGet()
                clearPending = true
                clearIfPending()
            }
        }
    }

    private suspend fun runBatch() {
        val work: Map<String, List<CompletableDeferred<ByteArray?>>>
        synchronized(batchLock) {
            work = HashMap(batch)
            batch.clear()
            batchJob = null
        }
        if (work.isEmpty()) return
        fun finish(name: String, v: ByteArray?) = work[name]?.forEach { if (!it.isCompleted) it.complete(v) }
        try {
            clearing?.let { withTimeoutOrNullCompat(8000) { it.join() } }
            val names = work.keys.toList()
            delivering.incrementAndGet()
            val got = try {
                deliver(names)
            } finally {
                scope.launch {
                    delay(10_000)
                    delivering.decrementAndGet()
                    clearIfPending()
                }
            }
            names.filter { it !in got }.forEach { missing[it] = System.currentTimeMillis(); finish(it, null) }
            val port = _state.value.port
            names.filter { it in got }.chunked(4).forEach { group ->
                val jobs = group.map { n ->
                    scope.launch {
                        finish(n, localGet(port, n))
                    }
                }
                jobs.forEach { it.join() }
            }
        } catch (_: Exception) {
            work.keys.forEach { finish(it, null) }
        }
    }

    private fun localGet(port: Int, name: String): ByteArray? =
        try {
            LocalHttp.client.newCall(Request.Builder().url("http://127.0.0.1:$port/tg/0/$name").build()).execute().use { r ->
                if (r.isSuccessful) r.body?.bytes() else null
            }
        } catch (_: Exception) {
            null
        }

    /** `POST /api/tg/deliver` — qaytgan fayllar nomlari; ochish kalitlari yadroga beriladi. */
    private fun deliver(names: List<String>): Set<String> {
        val s = auth.sessionToken ?: return emptySet()
        val got = HashSet<String>()
        names.chunked(100).forEach { part ->
            val r = Http.client.callJson(
                "$base/api/tg/deliver", "POST",
                JSONObject().put("files", JSONArray(part)), s,
            )
            val j = r.json
            if (r.ok && j != null) {
                j.optJSONObject("keys")?.takeIf { it.length() > 0 }?.let { RustCore.tgSetKeys(it.toString()) }
                j.optJSONArray("files")?.let { arr -> for (i in 0 until arr.length()) arr.optString(i).takeIf { it.isNotEmpty() }?.let(got::add) }
            }
        }
        if (got.isNotEmpty()) clearPending = true
        return got
    }

    private fun isMissing(name: String): Boolean {
        val t = missing[name] ?: return false
        if (System.currentTimeMillis() - t > MISSING_TTL_MS) {
            missing.remove(name)
            return false
        }
        return true
    }

    // ── Bot chatini tozalash (yuborilgan nusxalar chatda qolib ketmasin) ──

    private val delivering = java.util.concurrent.atomic.AtomicInteger(0)

    @Volatile
    private var clearPending = false

    /** Yuklab olish ketayotgan fayllar: shu paytda bot chati tozalanmaydi (nusxa chatda turishi kerak). */
    private val holds = java.util.concurrent.ConcurrentHashMap.newKeySet<String>()

    fun hold(key: String) {
        holds.add(key)
    }

    fun unhold(key: String) {
        if (holds.remove(key) && holds.isEmpty()) {
            clearPending = true
            scheduleClear(5000)
        }
    }

    @Volatile
    private var clearing: Job? = null
    private var clearTimer: Job? = null

    /** Ekran yopilganda / foydalanuvchi chiqqanda chaqiriladi. */
    fun screenClosed() {
        if (!_state.value.authorized) return
        clearPending = true
        scheduleClear(2000)
    }

    private fun scheduleClear(afterMs: Long) {
        clearTimer?.cancel()
        clearTimer = scope.launch {
            delay(afterMs)
            clearIfPending()
        }
    }

    private suspend fun clearIfPending() {
        if (!clearPending || !_state.value.authorized) return
        if (delivering.get() > 0 || holds.isNotEmpty()) {
            scheduleClear(20_000)
            return
        }
        val job = scope.launch {
            val j = RustCore.tgClearBotChat()
            if (j.optBoolean("ok")) clearPending = false else scheduleClear(30_000)
        }
        clearing = job
        job.join()
        clearing = null
    }

    private suspend fun withTimeoutOrNullCompat(ms: Long, block: suspend () -> Unit) {
        kotlinx.coroutines.withTimeoutOrNull(ms) { block() }
    }

    companion object {
        private const val KEY_CONFIG = "tg_config_v1"
        private const val CONFIG_TTL_MS = 12 * 3600 * 1000L
        private const val MISSING_TTL_MS = 20_000L
    }
}

object Names {
    private val ok = Regex("^[A-Za-z0-9._-]+$")

    /** `https://x/api/image/ep_1.jpg?t=1` -> `ep_1.jpg`; xavfli nom -> bo'sh. */
    fun fileNameOf(url: String): String {
        val last = url.substringBefore('?').substringAfterLast('/')
        return if (ok.matches(last) && last != "." && last != "..") last else ""
    }
}

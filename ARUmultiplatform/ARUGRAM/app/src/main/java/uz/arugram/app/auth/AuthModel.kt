package uz.arugram.app.auth

import android.content.Context
import android.os.Build
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import uz.arugram.app.BuildConfig
import uz.arugram.app.core.RustCore
import java.io.File

enum class Stage { Phone, Code, Password, SignUp, Done }

/** Kod qayerga yuborildi (Rust `sent_info`): `via` — app/sms/call/..., `next` — qayta yuborish turi, `timeout` — soniya. */
data class SentInfo(val via: String, val length: Int, val next: String, val timeout: Int, val at: Long)

data class AuthUi(
    val ready: Boolean = false,
    val stage: Stage = Stage.Phone,
    /** Xalqaro raqam, faqat raqamlar (kod + raqam). */
    val phone: String = "",
    val busy: Boolean = false,
    /** Joriy sahifada ko'rsatiladigan xato matni. */
    val error: String? = null,
    /** Har yangi xatoda oshadi (sahifa "silkinishi" uchun). */
    val errorTick: Int = 0,
    val sent: SentInfo? = null,
    val hint: String = "",
    val fatal: String? = null,
    /** Kod to'g'ri bo'ldi: katakchalar yashil to'lqin animatsiyasini o'ynayapti (`animateSuccess`), so'ng keyingi sahifa. */
    val codeOk: Boolean = false,
    /** Noto'g'ri kod har gal oshadi (katakchalar silkinadi, `shakeWrongCode`). */
    val codeErrTick: Int = 0,
    /** Telegram `needShowAlert` oynasi (sarlavha "Kechirasiz" + matn); yopilganda [dismissAlert]. */
    val alert: LoginAlert? = null,
)

/** [id] har yangi oyna uchun boshqa (bir xil matn qayta chiqsa ham). */
data class LoginAlert(val title: String, val text: String, val id: Int)

private const val WRONG_CODE = "Kod noto'g'ri"
private const val CODE_EXPIRED = "Kod muddati tugadi"
private const val ALERT_TITLE = "Kechirasiz"

/**
 * Kirish holati (telefon -> kod -> parol -> tayyor). Hamma tarmoq ishi Rust yadrosida (`RustCore.tg*`), bu yerda faqat holat.
 * Bosqich Rust'da diskka saqlanadi: ilova yopilib ochilsa, kod/parol oynasidan davom etadi.
 */
class AuthModel(private val app: Context) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val _ui = MutableStateFlow(AuthUi())
    val ui: StateFlow<AuthUi> = _ui.asStateFlow()

    fun start() {
        if (_ui.value.ready) return
        scope.launch {
            val st = withContext(Dispatchers.IO) { initCore() }
            _ui.update { st }
            if (st.stage == Stage.Done) {
                // Sessiya hali tirikmi — fonda tekshiriladi (o'lgan bo'lsa kirish oynasiga qaytadi).
                val alive = withContext(Dispatchers.IO) { RustCore.tgCheckSession().optBoolean("authorized", true) }
                if (!alive) _ui.update { it.copy(stage = Stage.Phone) }
            }
        }
    }

    private fun initCore(): AuthUi {
        if (!RustCore.available) return AuthUi(ready = true, fatal = RustCore.loadError ?: "Yadro yuklanmadi")
        if (BuildConfig.TG_API_ID <= 0 || BuildConfig.TG_API_HASH.isEmpty()) {
            return AuthUi(ready = true, fatal = "Telegram API kalitlari berilmagan (TG_API_ID / TG_API_HASH)")
        }
        RustCore.tgSetDevice("${Build.MANUFACTURER} ${Build.MODEL}".trim(), "Android ${Build.VERSION.RELEASE}", "ARUGRAM ${BuildConfig.APP_VERSION}")
        val r = RustCore.tgInit(File(app.filesDir, "tg").absolutePath, BuildConfig.TG_API_ID, BuildConfig.TG_API_HASH)
        r.optString("error").takeIf { it.isNotEmpty() }?.let { return AuthUi(ready = true, fatal = it) }
        return fromLoginState(RustCore.tgLoginState())
    }

    private fun fromLoginState(j: JSONObject): AuthUi {
        val stage = when (j.optString("stage")) {
            "done" -> Stage.Done
            "code" -> Stage.Code
            "password" -> Stage.Password
            else -> Stage.Phone
        }
        return AuthUi(
            ready = true, stage = stage, phone = j.optString("phone"), hint = j.optString("hint"),
            sent = parseSent(j.optJSONObject("sent")),
        )
    }

    private fun parseSent(o: JSONObject?): SentInfo? =
        o?.let { SentInfo(it.optString("via"), it.optInt("length"), it.optString("next"), it.optInt("timeout"), it.optLong("at")) }

    private var alertSeq = 0

    /** Xato oynasi (DrKLO `needShowAlert`): toast emas, "Kechirasiz" sarlavhali dialog. */
    private fun fail(msg: String) = _ui.update { it.copy(busy = false, error = null, alert = LoginAlert(ALERT_TITLE, msg, ++alertSeq)) }

    fun dismissAlert() = _ui.update { it.copy(alert = null) }

    /** Natijani holatga o'tkazadi: kod yuborildi / parol kerak / kirildi / xato. */
    private fun apply(j: JSONObject, phone: String? = null, fromCode: Boolean = false) {
        val err = j.optString("error")
        if (fromCode) {
            when {
                err.startsWith(WRONG_CODE) -> {
                    _ui.update { it.copy(busy = false, codeOk = false, error = null, codeErrTick = it.codeErrTick + 1) }
                    return
                }
                // Kod muddati tugadi: telefon sahifasiga qaytadi va oyna chiqadi (DrKLO PHONE_CODE_EXPIRED).
                err.startsWith(CODE_EXPIRED) -> {
                    _ui.update { it.copy(busy = false, codeOk = false, stage = Stage.Phone, sent = null, hint = "", error = null, alert = LoginAlert(ALERT_TITLE, err, ++alertSeq)) }
                    return
                }
            }
        }
        if (_ui.value.codeOk) _ui.update { it.copy(codeOk = false) }
        when {
            j.optBoolean("signup") -> _ui.update { it.copy(busy = false, stage = Stage.SignUp, error = null) }
            j.optBoolean("password") -> {
                _ui.update { it.copy(busy = false, stage = Stage.Password, hint = j.optString("hint"), phone = phone ?: it.phone, error = err.ifEmpty { null }, errorTick = if (err.isEmpty()) it.errorTick else it.errorTick + 1) }
            }
            err.isNotEmpty() -> fail(err)
            j.optJSONObject("sent") != null -> _ui.update { it.copy(busy = false, stage = Stage.Code, phone = phone ?: it.phone, sent = parseSent(j.optJSONObject("sent")), error = null) }
            j.optBoolean("ok") -> _ui.update { it.copy(busy = false, stage = Stage.Done, error = null) }
            else -> fail("Kutilmagan javob")
        }
    }

    fun requestCode(phoneDigits: String) {
        if (_ui.value.busy) return
        _ui.update { it.copy(busy = true, error = null) }
        scope.launch {
            val r = withContext(Dispatchers.IO) { RustCore.tgRequestCode("+$phoneDigits") }
            apply(r, phoneDigits)
        }
    }

    fun resendCode() {
        if (_ui.value.busy) return
        _ui.update { it.copy(busy = true, error = null) }
        scope.launch { apply(withContext(Dispatchers.IO) { RustCore.tgResendCode() }) }
    }

    fun signIn(code: String) {
        if (_ui.value.busy) return
        _ui.update { it.copy(busy = true, error = null) }
        scope.launch {
            val r = withContext(Dispatchers.IO) { RustCore.tgSignIn(code) }
            val ok = r.optString("error").isEmpty() && (r.optBoolean("ok") || r.optBoolean("password") || r.optBoolean("signup"))
            if (ok) {
                // `animateSuccess`: katakchalar 75 ms oralig'ida yashil bo'ladi; hammasi tugagach (+400 ms) keyingi sahifaga o'tiladi.
                val n = (_ui.value.sent?.length ?: 0).takeIf { it in 4..8 } ?: 5
                _ui.update { it.copy(codeOk = true) }
                delay(n * 75L + 400L)
            }
            apply(r, fromCode = true)
        }
    }

    fun signUp(first: String, last: String) {
        if (_ui.value.busy || first.isBlank()) return
        _ui.update { it.copy(busy = true, error = null) }
        scope.launch { apply(withContext(Dispatchers.IO) { RustCore.tgSignUp(first.trim(), last.trim()) }) }
    }

    fun checkPassword(password: String) {
        if (_ui.value.busy || password.isEmpty()) return
        _ui.update { it.copy(busy = true, error = null) }
        scope.launch { apply(withContext(Dispatchers.IO) { RustCore.tgCheckPassword(password) }) }
    }

    fun clearError() = _ui.update { it.copy(error = null) }

    /** Orqaga: kod/parol oynasidan telefon oynasiga (yarim kirish bosqichi tashlanadi). */
    fun backToPhone() {
        scope.launch { withContext(Dispatchers.IO) { RustCore.tgLoginReset() } }
        _ui.update { it.copy(stage = Stage.Phone, busy = false, error = null, sent = null, hint = "") }
    }

    fun logout() {
        scope.launch {
            withContext(Dispatchers.IO) {
                RustCore.tgLogout()
                // Hisobdan chiqishda mahalliy kesh ham o'chadi: suhbatlar ro'yxati va avatarlar.
                app.getSharedPreferences("chats_cache", android.content.Context.MODE_PRIVATE).edit().clear().apply()
                File(app.filesDir, "avatars").deleteRecursively()
                File(app.filesDir, "media").deleteRecursively()
            }
            uz.arugram.app.notify.NotifyService.stop(app)
            app.getSharedPreferences("notify_state", android.content.Context.MODE_PRIVATE).edit().clear().apply()
            uz.arugram.app.ui.main.AvatarCache.clear()
            uz.arugram.app.ui.main.MediaCache.clear()
            uz.arugram.app.ui.main.Drafts.clearAll(app)
            uz.arugram.app.ui.main.StickerStore.clear(app)
            _ui.update { AuthUi(ready = true) }
        }
    }
}

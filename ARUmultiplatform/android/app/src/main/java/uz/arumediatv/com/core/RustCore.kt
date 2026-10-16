package uz.arumediatv.com.core

import android.util.Log
import com.sun.jna.Native
import com.sun.jna.Pointer
import org.json.JSONException
import org.json.JSONObject

/**
 * Rust yadrosiga yagona kirish nuqtasi. Hamma chaqiruv BLOKLAYDI (tarmoq/disk) — faqat `Dispatchers.IO` da chaqiring.
 * Yadro yuklanmasa (qurilma arxitekturasi mos emas va h.k.) ilova yiqilmaydi: funksiyalar bo'sh/xato natija qaytaradi.
 */
object RustCore {
    private const val TAG = "RustCore"

    @Volatile
    private var lib: RustLib? = null

    @Volatile
    var loadError: String? = null
        private set

    val available: Boolean get() = lib != null

    /** Bir marta chaqiriladi (`AruApplication`). */
    @Synchronized
    fun load(): Boolean {
        if (lib != null) return true
        return try {
            System.loadLibrary("rust_core") // soname'ni linker keshiga kiritadi (JNA keyin shuni topadi)
            lib = Native.load("rust_core", RustLib::class.java)
            true
        } catch (t: Throwable) {
            loadError = "${t.javaClass.simpleName}: ${t.message}"
            Log.e(TAG, "librust_core.so yuklanmadi: $loadError")
            false
        }
    }

    /** `*mut c_char` -> String (+ xotirani bo'shatish). */
    private fun take(p: Pointer?): String {
        val l = lib ?: return ""
        if (p == null) return ""
        return try {
            p.getString(0, "UTF-8")
        } finally {
            l.rust_free_string(p)
        }
    }

    /** JSON javob -> JSONObject; matn JSON bo'lmasa `{"error": matn}`. */
    private fun json(s: String): JSONObject =
        try {
            JSONObject(s)
        } catch (_: JSONException) {
            JSONObject().put("error", if (s.isEmpty()) "Javob yo'q" else s)
        }

    private inline fun <T> guard(default: T, block: (RustLib) -> T): T {
        val l = lib ?: return default
        return try {
            block(l)
        } catch (t: Throwable) {
            Log.e(TAG, "yadro chaqiruvi yiqildi: ${t.message}")
            default
        }
    }

    fun version(): String = guard("") { take(it.rust_core_version()) }

    fun sign(ts: String, method: String, path: String): String = guard("") { take(it.app_sign(ts, method, path)) }

    /** Pleyer manzili uchun muddatli token (`<muddat>.<hex>`); yadro imzosiz bo'lsa bo'sh. */
    fun playToken(path: String, ttlSecs: Long = 6 * 3600): String = guard("") { take(it.app_play_token(path, ttlSecs)) }

    fun setAppVersion(v: String) = guard(Unit) { it.rust_set_app_version(v) }

    fun generateMasterKey(): String = guard("") { take(it.rust_crypto_generate_key()) }

    fun setMasterKey(hex: String): Boolean = guard(false) { it.rust_crypto_set_key(hex) == 1 }

    fun secureSave(path: String, label: String, text: String): Boolean = guard(false) { it.rust_secure_save(path, label, text) == 1 }

    fun secureLoad(path: String, label: String): String = guard("") { take(it.rust_secure_load(path, label)) }

    fun secureClear(path: String) = guard(Unit) { it.rust_secure_clear(path); Unit }

    fun setUserId(id: Long) = guard(Unit) { it.rust_set_user_id(id) }

    // ── Telegram ──
    fun tgSetDevice(model: String, system: String, app: String) = guard(Unit) { it.rust_tg_set_device(model, system, app) }

    fun tgInit(dir: String, apiId: Int, apiHash: String): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_init(dir, apiId, apiHash))) }

    fun tgCheckSession(): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_check_session())) }

    fun tgStatus(): JSONObject = guard(JSONObject()) { json(take(it.rust_tg_status())) }

    fun tgRequestCode(phone: String): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_request_code(phone))) }

    fun tgResendCode(): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_resend_code())) }

    fun tgSignIn(code: String): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_sign_in(code))) }

    fun tgCheckPassword(password: String): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_check_password(password))) }

    fun tgLoginState(): JSONObject = guard(JSONObject().put("stage", "phone")) { json(take(it.rust_tg_login_state())) }

    fun tgLoginReset() = guard(Unit) { it.rust_tg_login_reset() }

    fun tgSetBot(bot: String) = guard(Unit) { it.rust_tg_set_bot(bot) }

    fun tgStartBot(bot: String, param: String): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_start_bot(bot, param))) }

    fun tgSetKeys(json: String) = guard(Unit) { it.rust_tg_set_keys(json) }

    fun tgRoute(key: String, msgId: Int): Boolean = guard(false) { it.rust_tg_route(key, msgId) == 1 }

    fun tgClearBotChat(): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_clear_bot_chat())) }

    fun tgLogout(): JSONObject = guard(JSONObject()) { json(take(it.rust_tg_logout())) }

    fun tgNearestCountry(): String = guard("") { json(take(it.rust_tg_nearest_country())).optString("country", "") }

    fun tgPlayUrl(key: String): String = guard("") { take(it.rust_tg_play_url(key)) }

    private fun errJson(msg: String) = JSONObject().put("error", msg)
}

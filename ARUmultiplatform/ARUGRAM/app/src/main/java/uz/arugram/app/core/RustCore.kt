package uz.arugram.app.core

import android.util.Log
import com.sun.jna.Native
import com.sun.jna.Pointer
import org.json.JSONException
import org.json.JSONObject

/**
 * Rust yadrosiga yagona kirish nuqtasi. Hamma chaqiruv BLOKLAYDI (tarmoq/disk) — faqat `Dispatchers.IO` da chaqiring.
 * Yadro yuklanmasa ilova yiqilmaydi: funksiyalar bo'sh/xato natija qaytaradi.
 */
object RustCore {
    private const val TAG = "RustCore"

    @Volatile
    private var lib: RustLib? = null

    @Volatile
    var loadError: String? = null
        private set

    val available: Boolean get() = lib != null

    /** Intro GL sahnasi uchun (faqat GL oqimidan chaqiriladi). */
    internal val intro: RustLib? get() = lib

    @Synchronized
    fun load(): Boolean {
        if (lib != null) return true
        return try {
            System.loadLibrary("rust_core")
            lib = Native.load("rust_core", RustLib::class.java)
            true
        } catch (t: Throwable) {
            loadError = "${t.javaClass.simpleName}: ${t.message}"
            Log.e(TAG, "librust_core.so yuklanmadi: $loadError")
            false
        }
    }

    private fun take(p: Pointer?): String {
        val l = lib ?: return ""
        if (p == null) return ""
        return try {
            p.getString(0, "UTF-8")
        } finally {
            l.rust_free_string(p)
        }
    }

    private fun json(s: String): JSONObject =
        try {
            JSONObject(s)
        } catch (_: JSONException) {
            JSONObject().put("error", if (s.isEmpty()) "Javob yo'q" else s)
        }

    private fun errJson(msg: String) = JSONObject().put("error", msg)

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

    fun generateMasterKey(): String = guard("") { take(it.rust_crypto_generate_key()) }

    fun setMasterKey(hex: String): Boolean = guard(false) { it.rust_crypto_set_key(hex) == 1 }

    fun tgSetDevice(model: String, system: String, app: String) = guard(Unit) { it.rust_tg_set_device(model, system, app) }

    fun tgInit(dir: String, apiId: Int, apiHash: String): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_init(dir, apiId, apiHash))) }

    fun tgCheckSession(): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_check_session())) }

    fun tgStatus(): JSONObject = guard(JSONObject()) { json(take(it.rust_tg_status())) }

    fun tgRequestCode(phone: String): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_request_code(phone))) }

    fun tgResendCode(): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_resend_code())) }

    fun tgSignIn(code: String): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_sign_in(code))) }

    fun tgCheckPassword(password: String): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_check_password(password))) }

    fun tgDialogs(limit: Int = 40): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_dialogs(limit.toString()))) }

    fun tgMessages(chatId: Long, limit: Int = 40): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_messages("$chatId:$limit"))) }

    fun tgSendText(chatId: Long, text: String): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_send_text("$chatId\n$text"))) }

    fun tgLoginState(): JSONObject = guard(JSONObject().put("stage", "phone")) { json(take(it.rust_tg_login_state())) }

    fun tgLoginReset() = guard(Unit) { it.rust_tg_login_reset() }

    fun tgLogout(): JSONObject = guard(JSONObject()) { json(take(it.rust_tg_logout())) }
}

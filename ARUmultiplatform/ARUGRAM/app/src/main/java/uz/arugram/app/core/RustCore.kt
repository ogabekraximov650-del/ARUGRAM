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

    fun tgMessages(spec: String, limit: Int = 40, offsetId: Int = 0): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_messages("$spec:$limit:$offsetId"))) }

    fun tgSendText(chatId: Long, text: String): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_send_text("$chatId\n$text"))) }

    fun tgAvatars(ids: List<Long>): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_avatars(ids.joinToString(",")))) }

    fun tgMarkRead(spec: String): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_mark_read(spec))) }

    fun tgMe(): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_me())) }

    fun tgContacts(): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_contacts())) }

    /** Kelgan yangilanishlar soni; o'zgarsa yangi xabar/holat bor. */
    fun tgUpdateSeq(): Long = guard(0L) { it.rust_tg_update_seq() }

    fun tgUserAvatars(arg: String): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_user_avatars(arg))) }

    fun tgPhoto(spec: String, msgId: Int): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(it.rust_tg_photo("$spec:$msgId"))) }

    private fun call(block: (RustLib) -> Pointer?): JSONObject = guard(errJson("Yadro yuklanmagan")) { json(take(block(it))) }

    fun tgSend(spec: String, replyId: Int, text: String) = call { it.rust_tg_send("$spec\n$replyId\n$text") }

    fun tgEdit(spec: String, msgId: Int, text: String) = call { it.rust_tg_edit("$spec\n$msgId\n$text") }

    fun tgDelete(spec: String, ids: List<Int>) = call { it.rust_tg_delete("$spec\n${ids.joinToString(",")}") }

    fun tgForward(from: String, to: String, ids: List<Int>) = call { it.rust_tg_forward("$from\n$to\n${ids.joinToString(",")}") }

    fun tgPinMessage(spec: String, msgId: Int, pin: Boolean) = call { it.rust_tg_pin_message("$spec\n$msgId\n${if (pin) 1 else 0}") }

    fun tgPinDialog(spec: String, pin: Boolean) = call { it.rust_tg_pin_dialog("$spec\n${if (pin) 1 else 0}") }

    fun tgMute(spec: String, mute: Boolean) = call { it.rust_tg_mute("$spec\n${if (mute) 1 else 0}") }

    fun tgDeleteDialog(spec: String) = call { it.rust_tg_delete_dialog(spec) }

    fun tgSendMedia(spec: String, kind: String, path: String, replyId: Int, caption: String, extra: String = "") =
        call { it.rust_tg_send_media("$spec\n$kind\n$path\n$replyId\n$extra\n$caption") }

    fun tgMediaFile(spec: String, msgId: Int, dir: String) = call { it.rust_tg_media_file("$spec\n$msgId\n$dir") }

    fun tgSearch(q: String) = call { it.rust_tg_search(q) }

    fun tgTyping(): JSONObject = guard(JSONObject()) { json(take(it.rust_tg_typing())) }

    fun tgSessions() = call { it.rust_tg_sessions() }

    fun tgTerminate(hash: String) = call { it.rust_tg_terminate(hash) }

    fun tgProfile() = call { it.rust_tg_profile() }

    fun tgUpdateProfile(first: String, last: String, about: String) = call { it.rust_tg_update_profile("$first\n$last\n$about") }

    fun tgUpdateUsername(name: String) = call { it.rust_tg_update_username(name) }

    fun tgSearchInChat(spec: String, q: String) = call { it.rust_tg_search_in_chat("$spec\n$q") }

    fun tgPinned(spec: String) = call { it.rust_tg_pinned(spec) }

    fun tgReact(spec: String, msgId: Int, emoji: String) = call { it.rust_tg_react("$spec\n$msgId\n$emoji") }

    fun tgStickerSets() = call { it.rust_tg_sticker_sets() }

    fun tgStickerSet(id: String, hash: String) = call { it.rust_tg_sticker_set("$id:$hash") }

    fun tgStickerFile(ref: String) = call { it.rust_tg_sticker_file(ref) }

    fun tgSendSticker(spec: String, replyId: Int, ref: String) = call { it.rust_tg_send_sticker("$spec\n$replyId\n$ref") }

    fun tgResolve(name: String) = call { it.rust_tg_resolve(name) }

    fun tgJoin(spec: String) = call { it.rust_tg_join(spec) }

    fun tgFolders() = call { it.rust_tg_folders() }

    fun tgCreateGroup(title: String, members: String) = call { it.rust_tg_create_group("$title\n$members") }

    fun tgCreateChannel(title: String, about: String) = call { it.rust_tg_create_channel("$title\n$about") }

    fun tgPeerInfo(spec: String) = call { it.rust_tg_peer_info(spec) }

    fun tgSignUp(first: String, last: String) = call { it.rust_tg_sign_up("$first\n$last") }

    fun tgLoginState(): JSONObject = guard(JSONObject().put("stage", "phone")) { json(take(it.rust_tg_login_state())) }

    fun tgLoginReset() = guard(Unit) { it.rust_tg_login_reset() }

    fun tgLogout(): JSONObject = guard(JSONObject()) { json(take(it.rust_tg_logout())) }
}

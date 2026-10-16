package uz.arumediatv.com.core

import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Pointer

/**
 * Rust yadrosining (`ARUmultiplatform/android/rust`, `librust_core.so`) C funksiyalari — JNA orqali, JNI yozmasdan.
 * Imzolar `rust/src/` fayllari dagi `extern "C"` bilan AYNAN bir xil: `*const c_char` -> String, `*mut c_char` -> Pointer
 * (matn o'qilgach `rust_free_string` bilan bo'shatiladi), `i32` -> Int, `i64` -> Long.
 */
@Suppress("FunctionName")
interface RustLib : Library {
    fun rust_core_version(): Pointer?
    fun rust_free_string(p: Pointer?)

    // ── Imzo va versiya ──
    fun app_sign(ts: String, method: String, path: String): Pointer?
    fun app_play_token(path: String, ttlSecs: Long): Pointer?
    fun rust_set_app_version(v: String)

    // ── Shifrlash va xavfsiz saqlash ──
    fun rust_crypto_generate_key(): Pointer?
    fun rust_crypto_set_key(hex: String): Int
    fun rust_secure_save(path: String, label: String, text: String): Int
    fun rust_secure_load(path: String, label: String): Pointer?
    fun rust_secure_clear(path: String): Int

    // ── Telegram (MTProto) ──
    fun rust_tg_set_device(model: String, system: String, app: String)
    fun rust_tg_init(dir: String, apiId: Int, apiHash: String): Pointer?
    fun rust_tg_check_session(): Pointer?
    fun rust_tg_status(): Pointer?
    fun rust_tg_request_code(phone: String): Pointer?
    fun rust_tg_resend_code(): Pointer?
    fun rust_tg_sign_in(code: String): Pointer?
    fun rust_tg_check_password(password: String): Pointer?
    fun rust_tg_login_state(): Pointer?
    fun rust_tg_login_reset()
    fun rust_tg_set_bot(bot: String)
    fun rust_tg_start_bot(bot: String, param: String): Pointer?
    fun rust_tg_set_keys(json: String)
    fun rust_tg_route(key: String, msgId: Int): Int
    fun rust_tg_clear_bot_chat(): Pointer?
    fun rust_tg_logout(): Pointer?
    fun rust_tg_nearest_country(): Pointer?
    fun rust_tg_play_url(key: String): Pointer?
    fun rust_tg_upload_start(path: String, name: String, mime: String, channelId: Long): Pointer?
    fun rust_tg_upload_status(job: Long): Pointer?
    fun rust_tg_upload_cancel(job: Long)
    fun rust_tg_join_channel(arg: String): Pointer?
    fun rust_tg_channel_info(arg: String): Pointer?
    fun rust_tg_read_pinned(arg: String): Pointer?
    fun rust_tg_make_bot_admin(arg: String): Pointer?

    // ── Video kesh (yuklab olish, mahalliy server) ──
    fun rust_video_cache_start(dir: String): Int
    fun rust_video_cache_stats(urlsJson: String): Pointer?
    fun rust_video_cache_ranges(url: String): Pointer?
    fun rust_video_cache_complete(url: String): Int
    fun rust_video_cache_download(url: String): Int
    fun rust_video_cache_pause(url: String): Int
    fun rust_video_cache_delete(url: String): Int
    fun rust_video_cache_wipe(): Int
    fun rust_video_cache_total(url: String): Long
    fun rust_video_cache_net_bytes(): Long
    fun rust_video_cache_served_bytes(): Long

    // ── Ichki hisob ──
    fun rust_set_user_id(id: Long)
}

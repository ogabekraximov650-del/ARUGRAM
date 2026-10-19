package uz.arugram.app.core

import com.sun.jna.Library
import com.sun.jna.Pointer

/**
 * Rust yadrosining (`ARUGRAM/rust`, `librust_core.so`) C funksiyalari — JNA orqali. Imzolar `rust/src/` dagi `extern "C"` bilan
 * AYNAN bir xil: `*const c_char` -> String, `*mut c_char` -> Pointer (matn o'qilgach `rust_free_string` bilan bo'shatiladi).
 */
@Suppress("FunctionName")
interface RustLib : Library {
    fun rust_core_version(): Pointer?
    fun rust_free_string(p: Pointer?)

    // ── Shifrlash (sessiya diskka shifrlab yoziladi) ──
    fun rust_crypto_generate_key(): Pointer?
    fun rust_crypto_set_key(hex: String): Int

    // ── Telegram (MTProto, grammers) ──
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
    fun rust_tg_dialogs(limit: String): Pointer?
    fun rust_tg_messages(arg: String): Pointer?
    fun rust_tg_send_text(arg: String): Pointer?
    fun rust_tg_avatars(arg: String): Pointer?
    fun rust_tg_mark_read(arg: String): Pointer?
    fun rust_tg_logout(): Pointer?

    // ── Kirish (intro) sahnasi: Telegram `IntroRenderer.c` ning Rust'ga o'tkazilgan nusxasi (OpenGL ES 2) ──
    fun rust_intro_set_textures(ids: IntArray, n: Int)
    fun rust_intro_surface_created(): Int
    fun rust_intro_surface_changed(w: Int, h: Int, scale: Float, a1: Int)
    fun rust_intro_set_background(r: Float, g: Float, b: Float, a: Float)
    fun rust_intro_set_page(page: Int)
    fun rust_intro_set_scroll_offset(v: Float)
    fun rust_intro_set_date(v: Float)
    fun rust_intro_draw_frame(deltaMs: Int)
    fun rust_intro_destroy()
}

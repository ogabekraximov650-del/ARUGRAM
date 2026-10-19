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
    fun rust_tg_me(): Pointer?
    fun rust_tg_contacts(): Pointer?
    fun rust_tg_update_seq(): Long
    fun rust_tg_send(arg: String): Pointer?
    fun rust_tg_edit(arg: String): Pointer?
    fun rust_tg_delete(arg: String): Pointer?
    fun rust_tg_forward(arg: String): Pointer?
    fun rust_tg_pin_message(arg: String): Pointer?
    fun rust_tg_pin_dialog(arg: String): Pointer?
    fun rust_tg_mute(arg: String): Pointer?
    fun rust_tg_delete_dialog(arg: String): Pointer?
    fun rust_tg_send_media(arg: String): Pointer?
    fun rust_tg_media_file(arg: String): Pointer?
    fun rust_tg_media_read(arg: String): Pointer?
    fun rust_tg_search(arg: String): Pointer?
    fun rust_tg_typing(): Pointer?
    fun rust_tg_sessions(): Pointer?
    fun rust_tg_terminate(arg: String): Pointer?
    fun rust_tg_profile(): Pointer?
    fun rust_tg_update_profile(arg: String): Pointer?
    fun rust_tg_update_username(arg: String): Pointer?
    fun rust_tg_sign_up(arg: String): Pointer?
    fun rust_tg_peer_info(arg: String): Pointer?
    fun rust_tg_search_in_chat(arg: String): Pointer?
    fun rust_tg_pinned(arg: String): Pointer?
    fun rust_tg_react(arg: String): Pointer?
    fun rust_tg_folders(): Pointer?
    fun rust_tg_edit_chat(arg: String): Pointer?
    fun rust_tg_add_members(arg: String): Pointer?
    fun rust_tg_set_photo(arg: String): Pointer?
    fun rust_tg_clear_history(arg: String): Pointer?
    fun rust_tg_invite_link(arg: String): Pointer?
    fun rust_tg_callback(arg: String): Pointer?
    fun rust_tg_vote(arg: String): Pointer?
    fun rust_tg_typing_send(arg: String): Pointer?
    fun rust_tg_set_online(arg: String): Pointer?
    fun rust_tg_block(arg: String): Pointer?
    fun rust_tg_add_contact(arg: String): Pointer?
    fun rust_tg_import_invite(arg: String): Pointer?
    fun rust_tg_members(arg: String): Pointer?
    fun rust_tg_shared(arg: String): Pointer?
    fun rust_tg_sticker_sets(): Pointer?
    fun rust_tg_sticker_set(arg: String): Pointer?
    fun rust_tg_sticker_file(arg: String): Pointer?
    fun rust_tg_send_sticker(arg: String): Pointer?
    fun rust_tg_resolve(arg: String): Pointer?
    fun rust_tg_join(arg: String): Pointer?
    fun rust_tg_create_group(arg: String): Pointer?
    fun rust_tg_create_channel(arg: String): Pointer?
    fun rust_tg_photo(arg: String): Pointer?
    fun rust_tg_user_avatars(arg: String): Pointer?
    fun rust_tg_stories(): Pointer?
    fun rust_tg_story_file(arg: String): Pointer?
    fun rust_tg_story_read(arg: String): Pointer?
    fun rust_tg_story_send(arg: String): Pointer?
    fun rust_tg_calls(): Pointer?
    fun rust_tg_premium(): Pointer?
    fun rust_tg_stars(): Pointer?
    fun rust_tg_secret_list(): Pointer?
    fun rust_tg_secret_messages(arg: String): Pointer?
    fun rust_tg_secret_send(arg: String): Pointer?
    fun rust_tg_secret_start(arg: String): Pointer?
    fun rust_tg_secret_discard(arg: String): Pointer?
    fun rust_tg_secret_read(arg: String): Pointer?
    fun rust_tg_archived(): Pointer?
    fun rust_tg_archive(arg: String): Pointer?
    fun rust_tg_send_ex(arg: String): Pointer?
    fun rust_tg_send_special(arg: String): Pointer?
    fun rust_tg_scheduled(arg: String): Pointer?
    fun rust_tg_scheduled_act(arg: String): Pointer?
    fun rust_tg_bot_menu(arg: String): Pointer?
    fun rust_tg_webview(arg: String): Pointer?
    fun rust_tg_translate(arg: String): Pointer?
    fun rust_tg_member_act(arg: String): Pointer?
    fun rust_tg_folder_save(arg: String): Pointer?
    fun rust_tg_folder_delete(arg: String): Pointer?
    fun rust_tg_privacy_get(arg: String): Pointer?
    fun rust_tg_privacy_set(arg: String): Pointer?
    fun rust_tg_sticker_install(arg: String): Pointer?
    fun rust_tg_sticker_uninstall(arg: String): Pointer?
    fun rust_tg_saved_gifs(): Pointer?
    fun rust_tg_doc_file(arg: String): Pointer?
    fun rust_tg_save_gif(arg: String): Pointer?
    fun rust_tg_contact_edit(arg: String): Pointer?
    fun rust_tg_profile_photos(arg: String): Pointer?
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

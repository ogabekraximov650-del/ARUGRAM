import { Keyboard } from "grammy";

export const BTN_PICK_ADMIN = "🤖 Botni kanalga admin qilish";
export const BTN_PICK_ID = "🆔 Kanal IDsi botga yuborish";
export const BTN_PICK_BACK = "⟨ Orqaga";

export interface ChatPickerIds {
  reqAdminId: number;
  reqIdOnlyId: number;
}

const FULL_BOT_ADMIN_RIGHTS = {
  is_anonymous: false,
  can_manage_chat: true,
  can_change_info: false,
  can_post_messages: true,
  can_edit_messages: true,
  can_delete_messages: true,
  can_invite_users: true,
  can_restrict_members: false,
  can_promote_members: false,
  can_manage_video_chats: false,
  can_post_stories: false,
  can_edit_stories: false,
  can_delete_stories: false,
} as const;

const FULL_USER_ADMIN_RIGHTS = {
  is_anonymous: false,
  can_manage_chat: true,
  can_change_info: false,
  can_post_messages: true,
  can_edit_messages: true,
  can_delete_messages: true,
  can_invite_users: true,
  can_restrict_members: false,
  can_promote_members: true,
  can_manage_video_chats: false,
  can_post_stories: false,
  can_edit_stories: false,
  can_delete_stories: false,
} as const;

const READONLY_USER_ADMIN_RIGHTS = {
  is_anonymous: false,
  can_manage_chat: true,
  can_change_info: false,
  can_post_messages: true,
  can_edit_messages: true,
  can_delete_messages: true,
  can_invite_users: false,
  can_restrict_members: false,
  can_promote_members: false,
  can_manage_video_chats: false,
  can_post_stories: false,
  can_edit_stories: false,
  can_delete_stories: false,
} as const;

/**
 * Reply-keyboard with two `request_chat` buttons:
 *  - 🤖 Botni kanalga admin qilish — botni tanlangan kanalga admin sifatida qoʻshadi
 *  - 🆔 Kanal IDsi botga yuborish — faqat tanlangan kanalning chat_id'sini botga yuboradi
 *  - ⟨ Orqaga — qaytish (alohida hears handler bilan tutiladi)
 */
export function chatPickerKb(ids: ChatPickerIds): Keyboard {
  return new Keyboard()
    .requestChat(BTN_PICK_ADMIN, ids.reqAdminId, {
      chat_is_channel: true,
      bot_is_member: false,
      user_administrator_rights: FULL_USER_ADMIN_RIGHTS,
      bot_administrator_rights: FULL_BOT_ADMIN_RIGHTS,
    })
    .row()
    .requestChat(BTN_PICK_ID, ids.reqIdOnlyId, {
      chat_is_channel: true,
      user_administrator_rights: READONLY_USER_ADMIN_RIGHTS,
    })
    .row()
    .text(BTN_PICK_BACK)
    .resized()
    .persistent();
}

/** Tushuntirish matni — userning so'ragan formatida */
export function chatPickerHelpText(): string {
  return (
    "1. Botni kanalingizga admin qiling\n" +
    "2. Kanaldagi video, fayl, surat yoki matnli postni botga yuboring — bot oʻzi avtomatik kanalga ulanadi\n" +
    "3. Agar siz bunday narsalarga tushunmasangiz, pastdagi koʻrsatmalarni bajaring.\n\n" +
    "🤖 <b>Botni kanalga admin qilish</b>\n\n" +
    "Tugmasini bosing, admin qilmoqchi boʻlgan kanalni tanlang va botni admin qiling.\n\n" +
    "Keyin esa ortga qaytib\n\n" +
    "🆔 <b>Kanal IDsi botga yuborish</b>\n\n" +
    "Tugmasini bosing, kanalni tanlang va yuborish tugmasini bosing — bot oʻzi avtomatik kanal IDsini oladi."
  );
}

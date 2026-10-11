// Admin ekranlari uchun umumiy yordamchilar.

import * as tgMedia from '../tg/media.js';

export const fileOf = (url) => `${url ?? ''}`.trim().split('?')[0].split('/').pop() || '';

/** Rasm: eng uzun tomoni 1600 px, JPEG 85% (ilovadagi `imageQuality: 85`). */
export function toJpeg(file) {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      const k = Math.min(1, 1600 / Math.max(img.naturalWidth, img.naturalHeight));
      const c = document.createElement('canvas');
      c.width = Math.max(1, Math.round(img.naturalWidth * k));
      c.height = Math.max(1, Math.round(img.naturalHeight * k));
      c.getContext('2d').drawImage(img, 0, 0, c.width, c.height);
      c.toBlob((b) => { URL.revokeObjectURL(url); b ? resolve(b) : reject(new Error('rasm_xato')); }, 'image/jpeg', 0.85);
    };
    img.onerror = () => { URL.revokeObjectURL(url); reject(new Error('rasm_ochilmadi')); };
    img.src = url;
  });
}

/** Rasmni Telegram orqali yuklaydi (ilovadagi `TelegramService.uploadFile`); bazaga yoziladigan fayl nomini qaytaradi. */
export async function uploadImage(blob, prefix) {
  if (!(await tgMedia.ensureTelegram())) throw new Error("Telegram ulanmagan — rasm yuklab bo'lmaydi");
  const name = `${prefix}_${Date.now()}.jpg`;
  const res = await tgMedia.uploadFile(blob, name);
  return res?.name || name;
}

export const fmtSize = (b) => `${(Number(b || 0) / (1024 * 1024)).toFixed(2)} MB`;

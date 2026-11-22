// Fayl yuklash (Android `vd/VdUpload.kt` nusxasi), Telegram hisobi KERAK EMAS.
//   * KICHIK fayl (<= 18 MiB: rasm...) — `PUT /api/vd/small`: worker o'zi shifrlab Telegram'ga saqlaydi;
//   * KATTA fayl (video) — brauzer YANGI tasodifiy kalit bilan AES-128-CTR shifrlab 18 MiB bo'laklarni KETMA-KET worker orqali yuboradi
//     (`/api/vd/cup/init`, `/cup/chunk`, `/cup/complete`); worker har bo'lakni Telegram'ga saqlaydi. R2 va Actions KERAK EMAS.
// `uploadFile(file, name, {onProgress(sent,total)})` -> `{ name, key }` (tg/media.js bilan bir xil shakl).

import { api, apiPost } from '../api.js';
import { ctrApply } from '../tg/ctr.js';

const CHUNK = 18 * 1024 * 1024;

const hex = (u8) => [...u8].map((b) => b.toString(16).padStart(2, '0')).join('');

/** Kichik fayl: ochiq baytlar workerga (u shifrlaydi). `null` — bu yo'l ishlamadi (R2 yo'liga o'tiladi). */
async function small(file, name, onProgress) {
  let j = null;
  try {
    const r = await api(`/api/vd/small?name=${encodeURIComponent(name)}`, { method: 'PUT', body: file, headers: { 'Content-Type': 'application/octet-stream' } });
    j = r;
  } catch (e) {
    if (e?.status === 401 || e?.status === 403) throw new Error(e?.body?.error || "Ruxsat yo'q");
    return null;
  }
  if (j?.ok && `${j.key}`.length === 32) { onProgress?.(file.size, file.size); return { name, key: j.key }; }
  return null;
}

export async function uploadFile(file, name, { onProgress } = {}) {
  if (!file || !file.size) throw new Error("Fayl o'qilmadi");
  if (file.size <= CHUNK) { const r = await small(file, name, onProgress); if (r) return r; }
  // Katta fayl: brauzer YANGI tasodifiy kalit bilan har 18 MiB bo'lakni shifrlaydi va KETMA-KET WORKER orqali yuboradi (R2 kerak emas);
  // worker har bo'lakni Telegram'ga saqlaydi. Uzilsa shu joydan davom etadi.
  const key = crypto.getRandomValues(new Uint8Array(16));
  const keyHex = hex(key);
  const init = await apiPost('/api/vd/cup/init', { name, size: file.size, key: keyHex });
  if (!init?.ok) throw new Error('Yuklash boshlanmadi');
  const total = Math.ceil(file.size / CHUNK);
  let done = Number(init.done) || 0;
  if (init.ready) { onProgress?.(file.size, file.size); return { name, key: keyHex }; }
  onProgress?.(Math.min(file.size, done * CHUNK), file.size);
  while (done < total) {
    const o = done * CHUNK;
    const plain = new Uint8Array(await file.slice(o, Math.min(file.size, o + CHUNK)).arrayBuffer());
    const enc = await ctrApply(keyHex, o, plain);
    let ok = false; let last = 'xato';
    for (let attempt = 0; attempt < 5 && !ok; attempt++) {
      try {
        const r = await api(`/api/vd/cup/chunk?name=${encodeURIComponent(name)}&i=${done}`, { method: 'PUT', body: enc, headers: { 'Content-Type': 'application/octet-stream' } });
        if (r?.ok) { ok = true; done = Math.max(done + 1, Number(r.done) || 0); } else last = r?.error || 'xato';
      } catch (e) {
        last = e?.body?.error || e?.message || 'xato';
        if (e?.status === 409 && e?.body?.done != null) { done = Number(e.body.done); ok = true; break; } // boshqa urinish allaqachon yozgan
        if (e?.status === 401 || e?.status === 403) throw new Error(e?.body?.error || "Ruxsat yo'q");
      }
      if (!ok) await new Promise((r) => setTimeout(r, 2000 * (attempt + 1)));
    }
    if (!ok) throw new Error(`Yuklab bo'lmadi: ${last}`);
    onProgress?.(Math.min(file.size, done * CHUNK), file.size);
  }
  await apiPost('/api/vd/cup/complete', { name });
  return { name, key: keyHex };
}

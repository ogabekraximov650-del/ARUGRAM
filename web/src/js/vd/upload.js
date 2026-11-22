// Fayl yuklash (Android `vd/VdUpload.kt` nusxasi), Telegram hisobi KERAK EMAS.
//   * KICHIK fayl (<= 18 MiB: rasm...) — `PUT /api/vd/small`: worker o'zi shifrlab Telegram'ga saqlaydi;
//   * KATTA fayl (video) — brauzer YANGI tasodifiy kalit bilan AES-128-CTR shifrlaydi (18 MiB bo'laklar bilan) va R2 ga
//     imzolangan havola orqali to'g'ridan-to'g'ri yuboradi (worker orqali bayt o'tmaydi): `/api/vd/up/init` -> PUT -> `/api/vd/up/complete`;
//     keyin worker saqlash workflow'ini (`vdstore.yml`) uyg'otadi.
// `uploadFile(file, name, {onProgress(sent,total)})` -> `{ name, key }` (tg/media.js bilan bir xil shakl).

import { api, apiPost } from '../api.js';
import { ctrApply } from '../tg/ctr.js';

const CHUNK = 18 * 1024 * 1024;

const hex = (u8) => [...u8].map((b) => b.toString(16).padStart(2, '0')).join('');

function xhrPut(url, body, onProgress, headers = {}) {
  return new Promise((resolve, reject) => {
    const x = new XMLHttpRequest();
    x.open('PUT', url);
    x.timeout = 0;
    Object.entries(headers).forEach(([k, v]) => x.setRequestHeader(k, v));
    x.upload.onprogress = (e) => { if (e.lengthComputable) onProgress?.(e.loaded, e.total); };
    x.onload = () => resolve(x);
    x.onerror = () => reject(new Error('network'));
    x.ontimeout = () => reject(new Error('timeout'));
    x.send(body);
  });
}

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
  const key = crypto.getRandomValues(new Uint8Array(16));
  const keyHex = hex(key);
  const init = await apiPost('/api/vd/up/init', { name, size: file.size, key: keyHex });
  const url = init?.url;
  if (!url) throw new Error('Yuklash boshlanmadi');
  // Shifrlash: bo'laklab (xotira to'lmasin), Blob'larga yig'iladi (brauzer ularni diskka chiqara oladi).
  const parts = [];
  for (let o = 0; o < file.size; o += CHUNK) {
    const plain = new Uint8Array(await file.slice(o, Math.min(file.size, o + CHUNK)).arrayBuffer());
    parts.push(new Blob([await ctrApply(keyHex, o, plain)]));
  }
  const body = new Blob(parts);
  let last = 'xato';
  for (let attempt = 0; attempt < 4; attempt++) {
    try {
      const x = await xhrPut(url, body, onProgress);
      if (x.status >= 200 && x.status < 300) {
        await apiPost('/api/vd/up/complete', { name });
        return { name, key: keyHex };
      }
      last = `R2 HTTP ${x.status}`;
    } catch (e) { last = e?.message || 'xato'; }
    await new Promise((r) => setTimeout(r, 2000 * (attempt + 1)));
  }
  throw new Error(`Yuklab bo'lmadi: ${last}`);
}

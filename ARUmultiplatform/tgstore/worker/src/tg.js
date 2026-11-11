// UZ: Telegram Bot API qatlami (R2 yo'q). 429 bo'lsa retry_after kutib qayta uriniladi.
// EN: Telegram Bot API layer (no R2). On 429 waits retry_after and retries.
// RU: sloy Telegram Bot API (bez R2). Pri 429 zhdet retry_after i povtoryaet.

const API = 'https://api.telegram.org';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function call(env, method, body, isForm = false) {
  for (let attempt = 0; attempt < 4; attempt++) {
    const r = await fetch(`${API}/bot${env.BOT_TOKEN}/${method}`, isForm
      ? { method: 'POST', body }
      : { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) });
    const j = await r.json().catch(() => ({ ok: false, description: 'bad json ' + r.status }));
    if (j.ok) return j.result;
    if (r.status === 429 && j.parameters?.retry_after) {
      await sleep(Math.min(j.parameters.retry_after, 20) * 1000 + 100);
      continue;
    }
    throw new Error(`tg ${method}: ${j.description || r.status}`);
  }
  throw new Error(`tg ${method}: too many retries`);
}

/** UZ: hujjat yuboradi -> {file_id, message_id} / EN: sends a document / RU: otpravlyaet dokument */
export async function sendDoc(env, chat, name, bytes) {
  const fd = new FormData();
  fd.set('chat_id', String(chat));
  fd.set('disable_notification', 'true');
  fd.set('document', new Blob([bytes], { type: 'application/octet-stream' }), name);
  const m = await call(env, 'sendDocument', fd, true);
  return { file_id: m.document.file_id, message_id: m.message_id };
}

export async function copyMsg(env, toChat, fromChat, messageId) {
  const r = await call(env, 'copyMessage', { chat_id: toChat, from_chat_id: fromChat, message_id: messageId, disable_notification: true });
  return r.message_id;
}

/** UZ: forward -> yangi xabardagi file_id (tiklash uchun) / EN: forward -> file_id of new message (restore) / RU: forward -> file_id (vosstanovlenie) */
export async function forwardMsg(env, toChat, fromChat, messageId) {
  const m = await call(env, 'forwardMessage', { chat_id: toChat, from_chat_id: fromChat, message_id: messageId, disable_notification: true });
  return { file_id: m.document?.file_id, message_id: m.message_id };
}

export async function deleteMsg(env, chat, messageId) {
  try { await call(env, 'deleteMessage', { chat_id: chat, message_id: messageId }); return true; }
  catch (e) { return /not found|can't be deleted|message to delete/i.test(String(e)); }
}

export async function pinMsg(env, chat, messageId) {
  await call(env, 'pinChatMessage', { chat_id: chat, message_id: messageId, disable_notification: true });
}

export async function getChat(env, chat) {
  return call(env, 'getChat', { chat_id: chat });
}

/** UZ: file_id bo'yicha baytlar (<=20 MB) / EN: bytes by file_id (<=20 MB) / RU: bayty po file_id (<=20 MB) */
export async function download(env, fileId) {
  const f = await call(env, 'getFile', { file_id: fileId });
  const r = await fetch(`${API}/file/bot${env.BOT_TOKEN}/${f.file_path}`);
  if (!r.ok) throw new Error('tg download ' + r.status);
  return new Uint8Array(await r.arrayBuffer());
}

export async function getMe(env) {
  return call(env, 'getMe', {});
}

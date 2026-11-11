// UZ: TableDO ni Telegram'siz sinash: SQLite = node:sqlite, Telegram = soxta fetch.
// EN: Tests TableDO offline: SQLite = node:sqlite, Telegram = fake fetch.
// RU: Test TableDO offlayn: SQLite = node:sqlite, Telegram = fetch-zaglushka.
import test from 'node:test';
import assert from 'node:assert/strict';
import { DatabaseSync } from 'node:sqlite';
import { TableDO } from '../src/table_do.js';

function fakeTelegram() {
  const files = new Map(); let nextMsg = 100; const msgs = new Map(); let pinned = null; const log = [];
  const orig = globalThis.fetch;
  globalThis.fetch = async (url, init = {}) => {
    const u = String(url);
    const m = /api\.telegram\.org\/bot[^/]+\/(\w+)/.exec(u);
    const reply = (result) => new Response(JSON.stringify({ ok: true, result }));
    if (/api\.telegram\.org\/file\//.test(u)) {
      const id = u.split('/file/bot')[1].split('/').slice(1).join('/');
      return files.has(id) ? new Response(files.get(id)) : new Response('', { status: 404 });
    }
    const method = m[1];
    log.push(method);
    if (method === 'sendDocument') {
      const fd = init.body; const f = fd.get('document'); const bytes = new Uint8Array(await f.arrayBuffer());
      const id = 'F' + nextMsg; files.set(id, bytes);
      const msg = { message_id: nextMsg++, chat: fd.get('chat_id'), document: { file_id: id, file_name: f.name } };
      msgs.set(msg.message_id, msg); return reply(msg);
    }
    const body = JSON.parse(init.body);
    if (method === 'getFile') return files.has(body.file_id) ? reply({ file_path: body.file_id }) : new Response(JSON.stringify({ ok: false, description: 'no file' }), { status: 400 });
    const dup = (o, chat) => { const id = nextMsg++; const nf = 'F' + id; files.set(nf, files.get(o.document.file_id)); const msg = { ...o, message_id: id, chat, document: { ...o.document, file_id: nf } }; msgs.set(id, msg); return msg; };
    if (method === 'copyMessage') return reply({ message_id: dup(msgs.get(body.message_id), body.chat_id).message_id });
    if (method === 'forwardMessage') return reply(dup(msgs.get(body.message_id), body.chat_id));
    if (method === 'deleteMessage') { const mm = msgs.get(body.message_id); if (mm?.document) files.delete(mm.document.file_id); msgs.delete(body.message_id); return reply(true); }
    if (method === 'pinChatMessage') { pinned = msgs.get(body.message_id); return reply(true); }
    if (method === 'getChat') return reply({ pinned_message: pinned });
    return new Response(JSON.stringify({ ok: false, description: 'unknown ' + method }), { status: 400 });
  };
  return { files, msgs, log, restore: () => { globalThis.fetch = orig; } };
}

function makeDO(env) {
  const db = new DatabaseSync(':memory:');
  const sql = { exec: (q, ...a) => { const st = db.prepare(q); const rows = /^\s*select/i.test(q) ? st.all(...a) : (st.run(...a), []); return { toArray: () => rows }; } };
  let alarm = null;
  const ctx = { storage: { sql, transactionSync: (fn) => { db.exec('BEGIN'); try { const r = fn(); db.exec('COMMIT'); return r; } catch (e) { db.exec('ROLLBACK'); throw e; } }, getAlarm: async () => alarm, setAlarm: async (t) => { alarm = t; } } };
  const d = new TableDO(ctx, env);
  const call = async (path, body) => (await d.fetch(new Request('https://do' + path, { method: body ? 'POST' : 'GET', headers: { 'x-table': 't1' }, body: body ? JSON.stringify(body) : undefined }))).json();
  return { d, call, db };
}
const env = { BOT_TOKEN: 'T', CHANNEL_ID: '-1', MIRROR_CHANNEL_ID: '-2' };

test('write -> locate -> chunk bytes; group commit batches concurrent writes', async () => {
  const tgs = fakeTelegram();
  try {
    const { call } = makeDO(env);
    const rs = await Promise.all([0, 1, 2, 3].map((i) => call('/write', { ops: [{ op: 'put', k: 'a' + i, v: { i } }] })));
    assert.ok(rs.every((r) => r.ok));
    assert.ok(Math.max(...rs.map((r) => r.batched)) > 1, 'at least some were grouped');
    const sends = tgs.log.filter((m) => m === 'sendDocument').length;
    assert.ok(sends < 4, 'fewer uploads than requests: ' + sends);
    const loc = await call('/locate?k=a2');
    assert.ok(loc.found && loc.no === 1);
    const st = await call('/stats');
    assert.equal(st.keys, 4);
  } finally { tgs.restore(); }
});

test('edit creates new version, old is GC-ed, mirror copy deleted with it', async () => {
  const tgs = fakeTelegram();
  try {
    const { call } = makeDO(env);
    await call('/write', { ops: [{ op: 'put', k: 'x', v: 1 }] });
    const v1 = (await call('/locate?k=x')).ver;
    await call('/write', { ops: [{ op: 'put', k: 'x', v: 2 }] });
    const v2 = (await call('/locate?k=x')).ver;
    assert.equal(v2, v1 + 1);
    assert.equal((await call(`/chunk?no=1&ver=${v1}`)).current, false, 'old version still readable until GC');
    const filesBefore = tgs.files.size;
    const g = await call('/gc', {});
    assert.equal(g.deleted, 1);
    assert.ok(tgs.files.size < filesBefore);
    assert.equal((await call(`/chunk?no=1&ver=${v1}`)).found, false);
  } finally { tgs.restore(); }
});

test('failed Telegram upload leaves index untouched', async () => {
  const tgs = fakeTelegram();
  try {
    const { call } = makeDO(env);
    await call('/write', { ops: [{ op: 'put', k: 'keep', v: 1 }] });
    const real = globalThis.fetch;
    globalThis.fetch = async (u, i) => (/sendDocument/.test(String(u)) ? new Response(JSON.stringify({ ok: false, description: 'boom' }), { status: 400 }) : real(u, i));
    const r = await call('/write', { ops: [{ op: 'put', k: 'new', v: 2 }, { op: 'del', k: 'keep' }] });
    assert.ok(r.error, 'write failed');
    globalThis.fetch = real;
    assert.equal((await call('/stats')).keys, 1);
    assert.ok((await call('/locate?k=keep')).found);
    assert.ok(!(await call('/locate?k=new')).found);
    const ok = await call('/write', { ops: [{ op: 'put', k: 'new', v: 2 }] });
    assert.ok(ok.ok, 'works again after failure');
  } finally { tgs.restore(); }
});

test('backup then restore into a fresh DO rebuilds keys', async () => {
  const tgs = fakeTelegram();
  try {
    const a = makeDO(env);
    await a.call('/write', { ops: [{ op: 'put', k: 'p', v: { n: 1 } }, { op: 'put', k: 'q', v: { n: 2 } }] });
    await a.call('/write', { ops: [{ op: 'del', k: 'p' }] });
    const b = await a.call('/backup', {});
    assert.ok(b.ok, JSON.stringify(b));
    const fresh = makeDO(env);
    const r = await fresh.call('/restore', {});
    assert.ok(r.ok, JSON.stringify(r));
    assert.equal(r.keys, 1);
    assert.ok((await fresh.call('/locate?k=q')).found);
    assert.ok(!(await fresh.call('/locate?k=p')).found);
  } finally { tgs.restore(); }
});

test('restore falls back to mirror copy when main channel message is gone', async () => {
  const tgs = fakeTelegram();
  try {
    const a = makeDO(env);
    await a.call('/write', { ops: [{ op: 'put', k: 'z', v: 'hello' }] });
    await a.call('/backup', {});
    // UZ: asosiy kanaldagi bo'lak fayli yo'qoladi / EN: main-channel chunk file disappears / RU: fayl chanka v osnovnom kanale ischezaet
    const main = [...tgs.msgs.values()].find((m) => String(m.chat) === '-1' && m.document?.file_name?.startsWith('DBP_t1_1_v'));
    tgs.files.delete(main.document.file_id);
    const fresh = makeDO(env);
    const r = await fresh.call('/restore', {});
    assert.ok(r.ok, JSON.stringify(r));
    assert.equal(r.keys, 1);
  } finally { tgs.restore(); }
});

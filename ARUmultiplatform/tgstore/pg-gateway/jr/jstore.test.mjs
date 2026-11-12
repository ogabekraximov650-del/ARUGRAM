import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import { JStore } from './jstore.mjs';
import { makeTelegram } from '../archive.mjs';

const I = (v) => ({ type: 'integer', value: String(v) });
const T = (v) => ({ type: 'text', value: v });
const tmp = () => fs.mkdtempSync(path.join(os.tmpdir(), 'jr-'));
const key = () => crypto.randomBytes(32).toString('hex');
const val = (r) => r.result.rows.map((row) => row.map((c) => c.value));

test('SQLite SQL runs unchanged and results are Turso-shaped', () => {
  const s = new JStore({ dir: tmp() }).init();
  let r = s.exec([{ sql: 'CREATE TABLE IF NOT EXISTS u (id INTEGER PRIMARY KEY AUTOINCREMENT, n TEXT, t INTEGER DEFAULT 0)' }]);
  assert.equal(r[0].ok, true);
  r = s.exec([{ sql: 'INSERT INTO u (n,t) VALUES (?,?)', args: [T('ali'), I(1762900000000)] }, { sql: 'INSERT INTO u (n) VALUES (?1)', args: [T('vali')] }]);
  assert.equal(r[0].result.last_insert_rowid, '1'); assert.equal(r[1].result.rows_written, 1);
  assert.doesNotThrow(() => JSON.stringify(r), 'natija JSON ga aylanishi kerak (BigInt yo\'q)');
  r = s.exec([{ sql: 'SELECT id, n, MAX(t, ?) AS m FROM u ORDER BY id', args: [I(5)] }]);
  assert.deepEqual(r[0].result.cols.map((c) => c.name), ['id', 'n', 'm']);
  assert.deepEqual(val(r[0]), [['1', 'ali', '1762900000000'], ['2', 'vali', '5']]);
  r = s.exec([{ sql: "INSERT INTO u (n) VALUES ('x') RETURNING id" }]);
  assert.deepEqual(val(r[0]), [['3']]);
  r = s.exec([{ sql: 'WITH used(n) AS (SELECT 1) SELECT n FROM used WHERE 5 NOT GLOB \'*[^0-9]*\'' }]);
  assert.deepEqual(val(r[0]), [['1']]);
});

test('a failing statement rolls the whole batch back', () => {
  const s = new JStore({ dir: tmp() }).init();
  s.exec([{ sql: 'CREATE TABLE a (id INTEGER PRIMARY KEY, v TEXT)' }]);
  const r = s.exec([{ sql: "INSERT INTO a VALUES (1,'x')" }, { sql: "INSERT INTO a VALUES (1,'dup')" }]);
  assert.equal(r[0].ok, false);
  assert.deepEqual(val(s.exec([{ sql: 'SELECT COUNT(*) FROM a' }])[0]), [['0']]);
});

test('writes go to db_N_vV journal folders and rotate at the chunk size', () => {
  const dir = tmp(); const s = new JStore({ dir, chunkBytes: 400 }).init();
  s.exec([{ sql: 'CREATE TABLE a (id INTEGER PRIMARY KEY, v TEXT)' }]);
  for (let i = 1; i <= 12; i++) s.exec([{ sql: 'INSERT INTO a VALUES (?,?)', args: [I(i), T('qiymat-' + i)] }]);
  const folders = fs.readdirSync(dir).filter((n) => /^db_\d+_v1$/.test(n)).sort();
  assert.ok(folders.length >= 3, folders.join());
  assert.ok(fs.existsSync(path.join(dir, 'db_1_v1', 'journal.jsonl')));
  const sealed = JSON.parse(fs.readFileSync(path.join(dir, 'db_1_v1', 'meta.json'), 'utf8'));
  assert.equal(sealed.sealed, true);
  assert.equal(s.seq, 13);
});

test('replicate + snapshot + recover rebuilds the database from Telegram', async () => {
  const k = key(), tg = makeTelegram({ mock: true });
  const dir = tmp(); const s = new JStore({ dir, tg, key: k, chunkBytes: 400 }).init();
  s.exec([{ sql: 'CREATE TABLE a (id INTEGER PRIMARY KEY, v TEXT)' }]);
  for (let i = 1; i <= 5; i++) s.exec([{ sql: 'INSERT INTO a VALUES (?,?)', args: [I(i), T('a' + i)] }]);
  await s.replicate();
  await s.snapshot();                       // surat: 6 ta yozuv
  for (let i = 6; i <= 10; i++) s.exec([{ sql: 'INSERT INTO a VALUES (?,?)', args: [I(i), T('b' + i)] }]);
  s.exec([{ sql: "UPDATE a SET v='yangi' WHERE id=2" }, { sql: 'DELETE FROM a WHERE id=3' }]);
  await s.replicate();
  const before = val(s.exec([{ sql: 'SELECT id, v FROM a ORDER BY id' }])[0]);
  // disk yo'qoldi
  const dir2 = tmp();
  const { store, replayed } = await JStore.recover({ dir: dir2, tg, key: k, chunkBytes: 400 });
  assert.ok(replayed >= 1);
  assert.deepEqual(val(store.exec([{ sql: 'SELECT id, v FROM a ORDER BY id' }])[0]), before);
  assert.equal(store.seq, s.seq);
  // tiklangandan keyin yozish davom etadi
  store.exec([{ sql: 'INSERT INTO a VALUES (99,?)', args: [T('davom')] }]);
  assert.equal(store.seq, s.seq + 1);
});

test('marks bump monotonically and usage reports size/percent', () => {
  const s = new JStore({ dir: tmp(), limitMb: 1 }).init();
  assert.deepEqual(s.marks({ bump: ['t'], at: 100, get: ['t', 'x'] }).v, { t: 100 });
  assert.deepEqual(s.marks({ bump: ['t'], at: 50, get: ['t'] }).v, { t: 101 });
  const u = s.usage(); assert.ok(u.used_bytes > 0 && u.percent > 0 && u.limit_bytes === 1048576);
});

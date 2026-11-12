import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { toCsv, parseCsv, csvObjects, seal, open, keyOf, SPECS, makeTelegram, packFiles, unpackFiles } from './archive.mjs';

test('CSV keeps NULL, empty string, quotes, commas and newlines', () => {
  const cols = ['id', 'body', 'note'];
  const rows = [{ id: '1', body: 'salom, "dunyo"', note: null }, { id: '2', body: '', note: 'a\nb' }];
  const p = parseCsv(toCsv(cols, rows));
  assert.deepEqual(p[0], ['id', 'body', 'note']);
  assert.deepEqual(p[1], ['1', 'salom, "dunyo"', null]);
  assert.deepEqual(p[2], ['2', '', 'a\nb']);
  assert.deepEqual(csvObjects(toCsv(cols, rows))[1], { id: '2', body: '', note: 'a\nb' });
});

test('seal/open round trip, wrong key and tampering fail', () => {
  const key = crypto.randomBytes(32), plain = Buffer.from('maxfiy ma\'lumot');
  const s = seal(key, plain);
  assert.deepEqual(open(key, s), plain);
  assert.throws(() => open(crypto.randomBytes(32), s));
  s[s.length - 1] ^= 1;
  assert.throws(() => open(key, s));
});

test('composite keys join with colon in spec order', () => {
  const w = SPECS.find((s) => s.table === 'watch_history_db');
  assert.equal(keyOf(w, { user_id: 1, anime_id: 2, season_id: 3, epizod_id: 4 }), '1:2:3:4');
});

test('mock telegram stores and returns files', async () => {
  const tg = makeTelegram({ mock: true });
  const r = await tg.send('x', Buffer.from('abc'));
  assert.equal((await tg.get(r.file_id)).toString(), 'abc');
  await tg.del(r.msg_id);
  await assert.rejects(() => tg.get(r.file_id));
});

test('chunk container packs and unpacks named files', () => {
  const files = { 'billing_log.csv': Buffer.from('id\n"1"\n'), 'reports_db.csv': Buffer.from('') };
  const out = unpackFiles(packFiles(files));
  assert.deepEqual(Object.keys(out).sort(), ['billing_log.csv', 'reports_db.csv']);
  assert.equal(out['billing_log.csv'].toString(), 'id\n"1"\n');
  assert.equal(out['reports_db.csv'].length, 0);
});

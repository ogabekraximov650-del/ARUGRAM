// UZ: core.js sinovlari / EN: core.js tests / RU: testy core.js
import test from 'node:test';
import assert from 'node:assert/strict';
import { chunkName, parseChunkName, encodeChunk, decodeChunk, sizeOf, planBatch, validTable, timingSafeEq, sha256Hex } from '../src/core.js';

// kichik test holati / small test state / nebolshoe testovoe sostoyanie
function world(max, sealedSet = new Set()) {
  const chunks = new Map(); // no -> Map
  const idx = new Map();
  let openNo = 1;
  const run = (ops) => {
    const r = planBatch({
      ops, max, openNo,
      locate: (k) => (idx.has(k) ? idx.get(k) : null),
      load: (no) => chunks.get(no) || new Map(),
      sealed: (no) => sealedSet.has(no),
    });
    for (const [no, m] of r.touched) chunks.set(no, m);
    for (const [k, no] of r.assign) no == null ? idx.delete(k) : idx.set(k, no);
    for (const no of r.newlySealed) sealedSet.add(no);
    openNo = r.openNo;
    return r;
  };
  return { chunks, idx, run, sealedSet, get openNo() { return openNo; } };
}

test('chunk name round trip', () => {
  assert.equal(chunkName('episodes', 3, 12), 'DBP_episodes_3_v12');
  assert.deepEqual(parseChunkName('DBP_episodes_3_v12'), { table: 'episodes', no: 3, ver: 12 });
  assert.equal(parseChunkName('junk'), null);
  assert.ok(validTable('ep-1') && !validTable('a_b') && !validTable(''));
});

test('encode/decode keeps unicode and sizes match', () => {
  const m = new Map([['a', { t: 'Salom dunyo ўзбек' }], ['b', [1, 2, 3]]]);
  const b = encodeChunk(m);
  assert.deepEqual([...decodeChunk(b)], [...m]);
  assert.equal(b.length, sizeOf(m));
});

test('new keys go to the open chunk; full chunk gets sealed and next opens', () => {
  const w = world(200);
  const big = 'x'.repeat(60);
  w.run([{ op: 'put', k: 'a', v: big }, { op: 'put', k: 'b', v: big }, { op: 'put', k: 'c', v: big }, { op: 'put', k: 'd', v: big }]);
  assert.ok(w.sealedSet.has(1), 'chunk 1 sealed');
  assert.ok(w.idx.get('a') === 1 && w.idx.get('d') >= 2);
  for (const m of w.chunks.values()) assert.ok(sizeOf(m) <= 200);
  assert.equal(w.openNo, Math.max(...w.chunks.keys()));
});

test('edit in sealed chunk stays there if it fits', () => {
  const w = world(200);
  const big = 'x'.repeat(60);
  w.run([{ op: 'put', k: 'a', v: big }, { op: 'put', k: 'b', v: big }, { op: 'put', k: 'c', v: big }, { op: 'put', k: 'd', v: big }]);
  const before = w.idx.get('a');
  w.run([{ op: 'put', k: 'a', v: 'small' }]);
  assert.equal(w.idx.get('a'), before);
  assert.equal(w.chunks.get(before).get('a'), 'small');
});

test('edit that grows beyond sealed chunk moves to open chunk', () => {
  const w = world(200);
  const big = 'x'.repeat(60);
  w.run([{ op: 'put', k: 'a', v: big }, { op: 'put', k: 'b', v: big }, { op: 'put', k: 'c', v: big }, { op: 'put', k: 'd', v: big }]);
  const grown = 'y'.repeat(140);
  w.run([{ op: 'put', k: 'a', v: grown }]);
  assert.notEqual(w.idx.get('a'), 1);
  assert.ok(!w.chunks.get(1).has('a'));
  for (const m of w.chunks.values()) assert.ok(sizeOf(m) <= 200);
});

test('delete removes from chunk and index; deleting unknown is a no-op', () => {
  const w = world(1000);
  w.run([{ op: 'put', k: 'a', v: 1 }, { op: 'put', k: 'b', v: 2 }]);
  w.run([{ op: 'del', k: 'a' }, { op: 'del', k: 'zzz' }]);
  assert.ok(!w.idx.has('a') && w.idx.has('b'));
  assert.ok(!w.chunks.get(1).has('a'));
});

test('same key twice in one batch: last wins', () => {
  const w = world(1000);
  w.run([{ op: 'put', k: 'a', v: 1 }, { op: 'put', k: 'a', v: 2 }, { op: 'put', k: 'b', v: 3 }, { op: 'del', k: 'b' }]);
  assert.equal(w.chunks.get(1).get('a'), 2);
  assert.ok(!w.idx.has('b'));
});

test('too large record and bad ops throw', () => {
  const w = world(100);
  assert.throws(() => w.run([{ op: 'put', k: 'a', v: 'x'.repeat(500) }]), /too large/);
  assert.throws(() => w.run([{ op: 'zzz', k: 'a' }]), /bad op/);
  assert.throws(() => w.run([{ op: 'put', k: '', v: 1 }]), /bad key/);
});

test('many records: every chunk within limit, all keys present exactly once', () => {
  const w = world(1000);
  const ops = [];
  for (let i = 0; i < 500; i++) ops.push({ op: 'put', k: 'k' + i, v: { i, s: 'v'.repeat(i % 40) } });
  w.run(ops);
  const seen = new Set();
  for (const [no, m] of w.chunks) {
    assert.ok(sizeOf(m) <= 1000, 'chunk ' + no);
    for (const k of m.keys()) { assert.ok(!seen.has(k)); seen.add(k); assert.equal(w.idx.get(k), no); }
  }
  assert.equal(seen.size, 500);
});

test('helpers', async () => {
  assert.ok(timingSafeEq('abc', 'abc') && !timingSafeEq('abc', 'abd') && !timingSafeEq('a', 'ab'));
  assert.equal((await sha256Hex('a')).length, 64);
});

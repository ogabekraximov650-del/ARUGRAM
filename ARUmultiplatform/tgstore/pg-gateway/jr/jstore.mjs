// UZ: JURNAL-BAZA (Ample): asosiy ma'lumot SQLite'da (production SQL o'zgarishsiz ishlaydi), har yozuv `db_N_vV/` papkalaridagi jurnalga qo'shiladi;
//     papka 1 MB ga yetgach yopiladi va keyingisi ochiladi. Jurnal bo'laklari (va vaqti-vaqti bilan to'liq surat) shifrlanib Telegram'ga yuklanadi:
//     server diski yo'qolsa, Telegram'dan surat + jurnallar bilan baza qayta tiklanadi. Hamma o'qish va yozish SHU BITTA joydan o'tadi, shuning uchun
//     eskirgan (yangilanmagan) ma'lumot hech qayerdan chiqmaydi.
// EN: JOURNAL STORE (Ample): live data in SQLite (production SQL runs unchanged); every write is appended to a journal inside `db_N_vV/` folders that
//     rotate at 1 MB. Journal chunks (plus periodic full snapshots) are encrypted and uploaded to Telegram; if the disk is lost the DB is rebuilt from the
//     snapshot + journals. All reads and writes go through this one node, so no stale copy can ever be served.
// RU: ЖУРНАЛЬНАЯ БАЗА (Ample): данные в SQLite, каждая запись добавляется в журнал в папках db_N_vV/ (ротация по 1 МБ); чанки и снимки шифруются и уходят
//     в Telegram — при потере диска база восстанавливается. Все чтения и записи идут через один узел: устаревших данных нет.
import { DatabaseSync } from 'node:sqlite';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { seal, open, packFiles, unpackFiles } from '../archive.mjs';

const MB = 1024 * 1024;
const WRITE_RX = /^\s*(insert|update|delete|replace|create|drop|alter|vacuum|reindex)\b/i;

const toParam = (a) => {
  if (a === null || a === undefined || a.type === 'null') return null;
  if (a.type === 'integer') return BigInt(a.value);
  if (a.type === 'float') return Number(a.value);
  if (a.type === 'blob') return Buffer.from(a.base64 || a.value, 'base64');
  return String(a.value);
};
const toCell = (v) => {
  if (v === null || v === undefined) return { type: 'null' };
  if (typeof v === 'bigint') return { type: 'integer', value: v.toString() };
  if (typeof v === 'number') return Number.isInteger(v) && Math.abs(v) < 2 ** 53 ? { type: 'integer', value: String(v) } : { type: 'float', value: v };
  if (v instanceof Uint8Array) return { type: 'blob', base64: Buffer.from(v).toString('base64') };
  return { type: 'text', value: String(v) };
};

export class JStore {
  constructor({ dir, tg = null, key = null, chunkBytes = MB, limitMb = 1024, log = () => {} }) {
    this.dir = dir; this.tg = tg; this.key = key ? Buffer.from(key, 'hex') : null; this.chunkBytes = chunkBytes; this.limitMb = limitMb; this.log = log;
    this.stmts = new Map(); this.dirty = new Set(); this.busy = false; this.snap = null; this.skipJournal = false; this.manifestMsg = null;
  }

  // ── ochish ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  init() {
    fs.mkdirSync(this.dir, { recursive: true });
    this.dbPath = path.join(this.dir, 'main.db');
    this.db = new DatabaseSync(this.dbPath);
    this.db.exec('pragma journal_mode=wal; pragma synchronous=full; pragma foreign_keys=off;');
    this.db.exec("create table if not exists jr_state(k text primary key, v integer); insert or ignore into jr_state values('seq',0);");
    this.db.exec('create table if not exists kv_marks(k text primary key, v integer not null default 0);');
    this.seq = Number(this.db.prepare("select v from jr_state where k='seq'").get().v);
    try { const m = JSON.parse(fs.readFileSync(path.join(this.dir, 'jr_snap.json'), 'utf8')); this.snap = m.snap; this.manifestMsg = m.manifest_msg; } catch { /* surat hali yo'q */ }
    this.#scan();
    return this;
  }
  #folders() { return fs.readdirSync(this.dir).map((n) => /^db_(\d+)_v(\d+)$/.exec(n)).filter(Boolean).map((m) => ({ no: +m[1], ver: +m[2], d: path.join(this.dir, m[0]) })).sort((a, b) => a.no - b.no); }
  #meta(d) { try { return JSON.parse(fs.readFileSync(path.join(d, 'meta.json'), 'utf8')); } catch { return null; } }
  #saveMeta(c) { fs.writeFileSync(path.join(c.d, 'meta.json'), JSON.stringify({ no: c.no, ver: c.ver, sealed: c.sealed, first_seq: c.first_seq, last_seq: c.last_seq, file_id: c.file_id, msg_id: c.msg_id, key: c.key })); }
  #scan() {
    this.chunks = this.#folders().map((f) => {
      const m = this.#meta(f.d) || { no: f.no, ver: f.ver, sealed: false, first_seq: 0, last_seq: 0 };
      const c = { ...m, d: f.d };
      try { // oxirgi qator raqami (meta eskirgan bo'lishi mumkin). EN/RU: recompute last_seq from the file
        const lines = fs.readFileSync(path.join(f.d, 'journal.jsonl'), 'utf8').trim().split('\n').filter(Boolean);
        if (lines.length) { c.first_seq = JSON.parse(lines[0]).q; c.last_seq = JSON.parse(lines[lines.length - 1]).q; }
      } catch { /* bo'sh */ }
      return c;
    });
    this.cur = [...this.chunks].reverse().find((c) => !c.sealed) || null;
    if (!this.cur) this.#openNext();
    // meta yo'qolgan/eskirgan holatda bazadagi `seq` jurnaldan oldinda bo'lsa — to'liq suratdan so'ng tuzaladi. EN/RU: DB ahead of journal -> heal via snapshot.
    this.gap = this.seq > Math.max(0, ...this.chunks.map((c) => c.last_seq || 0));
  }
  #openNext() {
    const no = this.chunks.length ? Math.max(...this.chunks.map((c) => c.no)) + 1 : 1;
    const d = path.join(this.dir, `db_${no}_v1`); fs.mkdirSync(d, { recursive: true });
    this.cur = { no, ver: 1, sealed: false, first_seq: 0, last_seq: 0, d };
    this.chunks.push(this.cur); this.#saveMeta(this.cur);
  }

  // ── so'rovlar ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  #prep(sql) {
    let s = this.stmts.get(sql);
    if (!s) { s = this.db.prepare(sql); s.setReadBigInts(true); s.setReturnArrays(true); if (this.stmts.size > 500) this.stmts.clear(); this.stmts.set(sql, s); }
    return s;
  }
  #one(st) {
    const s = this.#prep(st.sql); const args = (st.args || []).map(toParam);
    const write = WRITE_RX.test(st.sql);
    if (s.columns().length) {
      const cols = s.columns().map((c) => ({ name: c.name }));
      const rows = s.all(...args).map((r) => r.map(toCell));
      return { write, res: { cols, rows, affected_row_count: rows.length, rows_read: rows.length, rows_written: write ? rows.length : 0 } };
    }
    const r = s.run(...args);
    return { write, res: { cols: [], rows: [], affected_row_count: Number(r.changes), rows_read: 0, rows_written: write ? Number(r.changes) : 0, last_insert_rowid: String(r.lastInsertRowid) } };
  }

  /** Bir paket = BITTA tranzaksiya (xato bo'lsa hammasi qaytariladi). EN/RU: one batch = one transaction. */
  exec(stmts, { journal = true } = {}) {
    const results = []; let wrote = false; const writes = [];
    this.db.exec('begin immediate');
    try {
      for (const st of stmts) {
        const r = this.#one(st);
        results.push({ ok: true, result: r.res });
        if (r.write) { wrote = true; writes.push([st.sql, st.args || []]); }
      }
      if (wrote) { this.seq += 1; this.db.prepare("update jr_state set v=? where k='seq'").run(this.seq); }
      this.db.exec('commit');
    } catch (e) {
      try { this.db.exec('rollback'); } catch { /* */ }
      this.stmts.clear();
      return [{ ok: false, error: String((e && e.message) || e) }];
    }
    if (wrote && journal && !this.skipJournal) this.#append(this.seq, writes);
    return results;
  }

  #append(q, writes) {
    const c = this.cur;
    fs.appendFileSync(path.join(c.d, 'journal.jsonl'), JSON.stringify({ q, t: Date.now(), s: writes }) + '\n');
    if (!c.first_seq) c.first_seq = q;
    c.last_seq = q; this.dirty.add(c.no);
    if (fs.statSync(path.join(c.d, 'journal.jsonl')).size >= this.chunkBytes) { c.sealed = true; this.#saveMeta(c); this.#openNext(); }
  }

  // ── belgilar va hajm ───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  marks({ bump = [], get = [], at = 0 }) {
    for (const k of bump) this.db.prepare('insert into kv_marks(k,v) values(?,?) on conflict(k) do update set v=max(excluded.v, kv_marks.v+1)').run(k, BigInt(Math.max(Number(at), 1)));
    const v = {};
    for (const k of get) { const r = this.db.prepare('select v from kv_marks where k=?').get(k); if (r) v[k] = Number(r.v); }
    return { v };
  }
  #dirBytes() { let n = 0; for (const f of fs.readdirSync(this.dir)) { const p = path.join(this.dir, f); const st = fs.statSync(p); n += st.isDirectory() ? fs.readdirSync(p).reduce((a, x) => a + fs.statSync(path.join(p, x)).size, 0) : st.size; } return n; }
  usage() {
    const used = this.#dirBytes(); const limit = this.limitMb * MB;
    let top = []; try { top = this.db.prepare("select name, sum(pgsize) as b from dbstat group by name order by b desc limit 8").all().map((r) => ({ table: r.name, bytes: Number(r.b) })); } catch { /* dbstat yo'q */ }
    return { used_bytes: used, limit_bytes: limit, percent: limit ? (used / limit) * 100 : 0, top, seq: this.seq, chunks: this.chunks.length };
  }

  // ── Telegram'ga nusxa ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  async #upload(name, plain, keyHex) {
    const sealed = seal(Buffer.from(keyHex, 'hex'), plain);
    const up = await this.tg.send(name, sealed);
    return { ...up, bytes: sealed.length };
  }
  async #manifest() {
    const m = { v: 1, at: Date.now(), seq: this.seq, snapshot: this.snap, chunks: this.chunks.filter((c) => c.file_id).map((c) => ({ no: c.no, ver: c.ver, sealed: c.sealed, first_seq: c.first_seq, last_seq: c.last_seq, file_id: c.file_id, msg_id: c.msg_id, key: c.key })) };
    const sealed = seal(this.key, Buffer.from(JSON.stringify(m)));
    const up = await this.tg.send('manifest.bin', sealed);
    await this.tg.pin(up.msg_id);
    if (this.manifestMsg) await this.tg.del(this.manifestMsg);
    this.manifestMsg = up.msg_id; this.#saveSnapMeta();
  }
  #saveSnapMeta() { fs.writeFileSync(path.join(this.dir, 'jr_snap.json'), JSON.stringify({ snap: this.snap, manifest_msg: this.manifestMsg })); }

  /** O'zgargan jurnal bo'laklarini Telegram'ga yuklaydi (eski xabar o'chadi) va ro'yxatni yangilaydi. EN/RU: replicate dirty journal chunks. */
  async replicate() {
    if (!this.tg || !this.key || this.busy) return { skipped: true };
    this.busy = true;
    try {
      let n = 0;
      for (const no of [...this.dirty]) {
        const c = this.chunks.find((x) => x.no === no); if (!c) { this.dirty.delete(no); continue; }
        const plain = packFiles({ 'journal.jsonl': fs.readFileSync(path.join(c.d, 'journal.jsonl')) });
        const keyHex = crypto.randomBytes(32).toString('hex');
        const up = await this.#upload(`db_${c.no}_v${c.ver}.bin`, plain, keyHex);
        const old = c.msg_id;
        Object.assign(c, { file_id: up.file_id, msg_id: up.msg_id, key: keyHex }); this.#saveMeta(c);
        this.dirty.delete(no); n++;
        if (old) await this.tg.del(Number(old));
      }
      if (n || !this.manifestMsg) await this.#manifest();
      return { uploaded: n };
    } finally { this.busy = false; }
  }

  /** To'liq surat: baza fayli 1 MB bo'laklarga bo'linib yuklanadi; undan oldingi jurnal bo'laklari o'chiriladi. EN/RU: full snapshot in 1 MB parts. */
  async snapshot() {
    if (!this.tg || !this.key) return { skipped: true };
    const tmp = path.join(this.dir, 'snap.tmp');
    fs.rmSync(tmp, { force: true });
    const seq = this.seq; // ayni sinxron blokda: orada yozuv bo'lmaydi. EN/RU: same sync block, no write can interleave
    this.db.prepare('vacuum into ?').run(tmp);
    const buf = fs.readFileSync(tmp); fs.rmSync(tmp, { force: true });
    const keyHex = crypto.randomBytes(32).toString('hex'); const parts = [];
    for (let i = 0, p = 0; i < Math.max(buf.length, 1); i += MB, p++) {
      const up = await this.#upload(`snap_${seq}_${p}.bin`, buf.subarray(i, i + MB), keyHex);
      parts.push({ file_id: up.file_id, msg_id: up.msg_id });
    }
    const old = this.snap;
    this.snap = { seq, at: Date.now(), key: keyHex, parts, bytes: buf.length };
    // Surat tayyor: undan oldingi yopilgan jurnal bo'laklari kerak emas.
    for (const c of [...this.chunks]) {
      if (c.sealed && c.last_seq <= seq) {
        if (c.msg_id) await this.tg.del(Number(c.msg_id));
        fs.rmSync(c.d, { recursive: true, force: true }); this.chunks = this.chunks.filter((x) => x !== c); this.dirty.delete(c.no);
      }
    }
    await this.#manifest();
    if (old) for (const p of old.parts) await this.tg.del(Number(p.msg_id));
    this.gap = false;
    return { seq, parts: parts.length, bytes: buf.length };
  }
  needSnapshot() { return !this.snap || this.gap || this.seq - this.snap.seq > 5000 || Date.now() - this.snap.at > 24 * 3600_000; }

  /** Disk yo'qolgan bo'lsa: Telegram'dagi surat + jurnallardan bazani tiklaydi. EN/RU: disaster recovery from Telegram. */
  static async recover({ dir, tg, key, chunkBytes, limitMb, log = () => {} }) {
    const pinned = await tg.getPinned();
    if (!pinned) throw new Error('Telegram\'da ro\'yxat (pin) topilmadi');
    const mk = Buffer.from(key, 'hex');
    const manifest = JSON.parse(open(mk, await tg.get(pinned.file_id)).toString('utf8'));
    fs.mkdirSync(dir, { recursive: true });
    const sk = Buffer.from(manifest.snapshot.key, 'hex');
    const parts = []; for (const p of manifest.snapshot.parts) parts.push(open(sk, await tg.get(p.file_id)));
    fs.writeFileSync(path.join(dir, 'main.db'), Buffer.concat(parts));
    const st = new JStore({ dir, tg, key, chunkBytes, limitMb, log });
    // Jurnal bo'laklarini o'z nomli papkalarga ochamiz va suratdan keyingi yozuvlarni qayta o'ynaymiz.
    st.skipJournal = true;
    let replayed = 0;
    for (const c of manifest.chunks.sort((a, b) => a.no - b.no)) {
      const d = path.join(dir, `db_${c.no}_v${c.ver}`); fs.mkdirSync(d, { recursive: true });
      const files = unpackFiles(open(Buffer.from(c.key, 'hex'), await tg.get(c.file_id)));
      fs.writeFileSync(path.join(d, 'journal.jsonl'), files['journal.jsonl']);
      fs.writeFileSync(path.join(d, 'meta.json'), JSON.stringify({ no: c.no, ver: c.ver, sealed: c.sealed, first_seq: c.first_seq, last_seq: c.last_seq, file_id: c.file_id, msg_id: c.msg_id, key: c.key }));
    }
    st.init();
    const lines = []; for (const c of st.chunks) { try { for (const l of fs.readFileSync(path.join(c.d, 'journal.jsonl'), 'utf8').split('\n').filter(Boolean)) lines.push(JSON.parse(l)); } catch { /* */ } }
    lines.sort((a, b) => a.q - b.q);
    for (const e of lines) {
      if (e.q <= st.seq) continue;
      const r = st.exec(e.s.map(([sql, args]) => ({ sql, args })), { journal: false });
      if (r[0] && r[0].ok === false) throw new Error(`jurnalni qayta o'ynash ${e.q}: ${r[0].error}`);
      st.db.prepare("update jr_state set v=? where k='seq'").run(e.q); st.seq = e.q; replayed++;
    }
    st.skipJournal = false; st.snap = manifest.snapshot; st.manifestMsg = pinned.msg_id; st.#saveSnapMeta();
    return { store: st, replayed, seq: st.seq };
  }
}

// UZ: DOIRA TIZIMI (Ample tomonida). Baza 70% to'lganda sovuq qatorlar PostgreSQL `COPY ... CSV HEADER` formatidagi shifrlangan fayllarga yig'ilib
//     Telegram'ga yuklanadi va bazadan o'chiriladi; kerak bo'lganda arxivdan o'qiladi (`lookup`) yoki bazaga qaytariladi (`restore`, vaqtincha "isitiladi").
//     Hammasini shu server qiladi (Worker'ning chaqiruv/CPU cheklovlari yo'q); ishlar bazada saqlanadi, server qayta yonsa ham davom etadi.
// EN: CIRCLE SYSTEM (Ample side). When the DB passes 70%, cold rows are packed into AES-GCM sealed `COPY ... CSV HEADER` files, uploaded to Telegram and deleted
//     from Postgres; archived rows can be read (`lookup`) or restored (`restore`, kept warm for a while). Runs on this long-lived server, no Worker limits.
// RU: СИСТЕМА КРУГА (на стороне Ample). При 70% заполнения холодные строки уходят в зашифрованные CSV-файлы (формат COPY) в Telegram и удаляются из Postgres.
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';

export const THRESHOLD = 70;
export const BATCH = 500;
/** UZ: bitta bo'lak (papka) hajmi: shunga yetguncha yoziladi, keyin keyingi raqamli papka ochiladi. EN/RU: chunk size target. */
export const CHUNK_BYTES = Number(process.env.ARCH_CHUNK_BYTES || 1024 * 1024);
const DAY = 86_400_000;

/** Sovuq jadvallar: kalit ustunlari, "sovuq" sharti ($1 = kesim vaqti, ms), necha kundan keyin. EN/RU: cold tables. */
export const SPECS = [
  { table: 'billing_log', keys: ['id'], cold: 'created_at < $1', days: 180 },
  { table: 'reports_db', keys: ['id'], cold: 'created_at < $1', days: 180 },
  { table: 'comments_db', keys: ['id'], cold: 'deleted = 1 AND created_at < $1', days: 90 },
  { table: 'sessions_db', keys: ['id'], cold: 'last_seen_at < $1', days: 90 },
  { table: 'watch_history_db', keys: ['user_id', 'anime_id', 'season_id', 'epizod_id'], cold: 'deleted_at > 0 AND deleted_at < $1', days: 90 },
];
const specOf = (t) => SPECS.find((s) => s.table === t);

// ── CSV (PostgreSQL COPY ... CSV HEADER): NULL = bo'sh, bo'sh satr = "" ───────────────────────────────────────────────────────────────────────
const cell = (v) => (v === null || v === undefined ? '' : `"${String(v).replace(/"/g, '""')}"`);
export function toCsv(cols, rows) {
  return cols.join(',') + '\n' + rows.map((r) => cols.map((c) => cell(r[c])).join(',')).join('\n') + (rows.length ? '\n' : '');
}
export function parseCsv(text) {
  const rows = []; let row = [], cur = '', quoted = false, inQ = false;
  const push = () => { row.push(cur === '' && !quoted ? null : cur); cur = ''; quoted = false; };
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (inQ) { if (c === '"') { if (text[i + 1] === '"') { cur += '"'; i++; } else inQ = false; } else cur += c; }
    else if (c === '"') { inQ = true; quoted = true; }
    else if (c === ',') push();
    else if (c === '\n') { push(); rows.push(row); row = []; }
    else if (c !== '\r') cur += c;
  }
  if (cur !== '' || quoted || row.length) { push(); rows.push(row); }
  return rows;
}
/** CSV -> [{col: value|null}] (sarlavha bo'yicha). EN/RU: CSV -> objects. */
export function csvObjects(text) {
  const [head, ...rest] = parseCsv(text);
  if (!head) return [];
  return rest.map((r) => Object.fromEntries(head.map((h, i) => [h, r[i] ?? null])));
}

// ── AES-256-GCM: iv(12) | tag(16) | shifrlangan matn ──────────────────────────────────────────────────────────────────────────────────────
export function seal(key, plain) {
  const iv = crypto.randomBytes(12);
  const c = crypto.createCipheriv('aes-256-gcm', key, iv);
  const ct = Buffer.concat([c.update(plain), c.final()]);
  return Buffer.concat([iv, c.getAuthTag(), ct]);
}
export function open(key, sealed) {
  const d = crypto.createDecipheriv('aes-256-gcm', key, sealed.subarray(0, 12));
  d.setAuthTag(sealed.subarray(12, 28));
  return Buffer.concat([d.update(sealed.subarray(28)), d.final()]);
}

export const keyOf = (spec, row) => spec.keys.map((k) => String(row[k])).join(':');
const keySql = (spec, alias) => `concat_ws(':', ${spec.keys.map((k) => `${alias ? alias + '.' : ''}${k}::text`).join(', ')})`;

// ── Telegram (bot API) ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
export function makeTelegram({ token, channel, mock }) {
  if (mock) { // UZ: sinov uchun xotirada. EN/RU: in-memory for tests.
    const files = new Map(); let n = 1;
    return { mock: true, files,
      async send(name, buf) { const id = n++; files.set('F' + id, buf); return { file_id: 'F' + id, msg_id: 1000 + id }; },
      async get(fid) { const b = files.get(fid); if (!b) throw new Error('mock: fayl yo\'q'); return b; },
      async del(mid) { files.delete('F' + (mid - 1000)); return true; } };
  }
  const api = (m) => `https://api.telegram.org/bot${token}/${m}`;
  async function call(method, make) {
    for (let a = 0; a < 6; a++) {
      let r, j;
      try { r = await fetch(api(method), make()); j = await r.json(); } catch (e) { await sleep(1000 * (a + 1)); continue; }
      if (j.ok) return j.result;
      if (j.error_code === 429) { await sleep(((j.parameters && j.parameters.retry_after) || 5) * 1000 + 250); continue; }
      if (j.error_code >= 500) { await sleep(1000 * (a + 1)); continue; }
      throw new Error(`telegram ${method}: ${j.description || r.status}`);
    }
    throw new Error(`telegram ${method}: qayta urinishlar tugadi`);
  }
  return {
    async send(name, buf) {
      const res = await call('sendDocument', () => { const fd = new FormData(); fd.set('chat_id', String(channel)); fd.set('document', new Blob([buf]), name); return { method: 'POST', body: fd }; });
      return { file_id: res.document.file_id, msg_id: res.message_id };
    },
    async get(fid) {
      const f = await call('getFile', () => ({ method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ file_id: fid }) }));
      const r = await fetch(`https://api.telegram.org/file/bot${token}/${f.file_path}`);
      if (!r.ok) throw new Error(`telegram yuklash ${r.status}`);
      return Buffer.from(await r.arrayBuffer());
    },
    async del(mid) { try { await call('deleteMessage', () => ({ method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ chat_id: channel, message_id: mid }) })); return true; } catch { return false; } },
  };
}


// ── Bo'lak konteyneri: [u32 sarlavha uzunligi][sarlavha JSON [{n,l}]][fayllar ketma-ket] ───────────────────────────────────────────────────────
export function packFiles(files) {
  const names = Object.keys(files).sort();
  const header = Buffer.from(JSON.stringify(names.map((n) => ({ n, l: files[n].length }))));
  const len = Buffer.alloc(4); len.writeUInt32BE(header.length);
  return Buffer.concat([len, header, ...names.map((n) => files[n])]);
}
export function unpackFiles(buf) {
  const hl = buf.readUInt32BE(0);
  const list = JSON.parse(buf.subarray(4, 4 + hl).toString());
  let off = 4 + hl; const out = {};
  for (const { n, l } of list) { out[n] = buf.subarray(off, off + l); off += l; }
  return out;
}

export class Archive {
  /** @param pool pg.Pool  @param tg makeTelegram(...)  @param opts { usage: async()=>percent, dir } */
  constructor(pool, tg, opts = {}) {
    this.pool = pool; this.tg = tg; this.usage = opts.usage || (async () => 0); this.busy = false;
    // UZ: papkalar shu yerda: `<ildiz>/db_1_v1/`, `db_2_v1/` ... Release papkasi emas, server diskida doimiy joy. EN/RU: chunk folders live here.
    let root = opts.dir || process.env.ARCH_DIR || '/var/lib/tgstore-arch';
    try { fs.mkdirSync(root, { recursive: true }); } catch { root = path.join(os.tmpdir(), 'tgstore-arch'); fs.mkdirSync(root, { recursive: true }); }
    this.root = root;
  }

  async init() {
    await this.pool.query(`create table if not exists arch_files(id bigint generated by default as identity primary key, tbl text, name text, file_id text, msg_id bigint,
      key text, rows bigint, size bigint, created_at bigint, restored_at bigint default 0)`);
    for (const c of ['no bigint', 'ver bigint default 1', 'sealed boolean default false']) await this.pool.query(`alter table arch_files add column if not exists ${c}`);
    await this.pool.query('create table if not exists arch_keys(tbl text not null, k text not null, fid bigint not null, primary key(tbl, k))');
    await this.pool.query('create table if not exists arch_warm(tbl text not null, k text not null, until bigint not null, primary key(tbl, k))');
  }

  dirOf(no, ver) { return path.join(this.root, `db_${no}_v${ver}`); }

  /** Bo'lak papkasini ochadi: papka yangi bo'lsa shu qaydda o'qiydi, aks holda Telegram'dan olib, o'z nomidagi papkaga joylaydi. EN/RU: open a chunk folder (download+unpack if missing/stale). */
  async #load(row) {
    const dir = this.dirOf(row.no, row.ver);
    try {
      const meta = JSON.parse(fs.readFileSync(path.join(dir, 'meta.json'), 'utf8'));
      if (String(meta.msg_id) === String(row.msg_id)) {
        const files = {};
        for (const n of fs.readdirSync(dir)) if (n.endsWith('.csv')) files[n] = fs.readFileSync(path.join(dir, n));
        return files;
      }
    } catch { /* papka yo'q yoki eskirgan */ }
    const sealed = await this.tg.get(row.file_id);
    const files = unpackFiles(open(Buffer.from(row.key, 'hex'), sealed));
    this.#write(row, files);
    return files;
  }
  #write(row, files) {
    const dir = this.dirOf(row.no, row.ver);
    fs.rmSync(dir, { recursive: true, force: true });
    fs.mkdirSync(dir, { recursive: true });
    for (const [n, b] of Object.entries(files)) fs.writeFileSync(path.join(dir, n), b);
    fs.writeFileSync(path.join(dir, 'meta.json'), JSON.stringify({ no: Number(row.no), ver: Number(row.ver), msg_id: String(row.msg_id), sealed: !!row.sealed }));
    this.#trim();
  }
  #trim(limit = 200 * 1024 * 1024) { // yopilgan papkalar keshi 200 MB dan oshmasin (eng eskisi o'chadi). EN/RU: bounded cache of sealed folders.
    const size = (d) => fs.readdirSync(d).reduce((a, n) => a + fs.statSync(path.join(d, n)).size, 0);
    const list = fs.readdirSync(this.root).filter((n) => /^db_\d+_v\d+$/.test(n)).map((n) => { const d = path.join(this.root, n); const st = fs.statSync(d); let sealed = true; try { sealed = JSON.parse(fs.readFileSync(path.join(d, 'meta.json'), 'utf8')).sealed; } catch { /* */ } return { d, t: st.mtimeMs, sz: size(d), sealed }; }).sort((a, b) => a.t - b.t);
    let total = list.reduce((a, x) => a + x.sz, 0);
    for (const x of list) { if (total <= limit) break; if (!x.sealed) continue; fs.rmSync(x.d, { recursive: true, force: true }); total -= x.sz; }
  }

  /** Bitta jadvaldan BIR bo'lak qatorni OCHIQ bo'lak (db_N_vV) jurnaliga qo'shadi; 1 MB ga yetsa bo'lak yopiladi, keyingisi ochiladi. EN/RU: append one batch to the open chunk. */
  async archiveOne(spec, { dry = false, now = Date.now() } = {}) {
    const cut = now - spec.days * DAY;
    const q = `select * from ${spec.table} t where ${spec.cold} and not exists (select 1 from arch_warm w where w.tbl = $2 and w.k = ${keySql(spec, 't')} and w.until > $3)
               order by ${spec.keys.map((k) => 't.' + k).join(', ')} limit ${BATCH}`;
    const r = await this.pool.query(q, [cut, spec.table, now]);
    if (!r.rows.length || dry) return { table: spec.table, found: r.rows.length, archived: 0, dry };
    const cols = r.fields.map((f) => f.name);
    // Ochiq bo'lak (yopilmagan oxirgisi) yoki yangisi.
    const open_ = (await this.pool.query('select * from arch_files where sealed = false order by id desc limit 1')).rows[0];
    let no, ver = 1, files = {};
    if (open_) { no = Number(open_.no); ver = Number(open_.ver || 1); files = await this.#load(open_); }
    else no = Number((await this.pool.query('select coalesce(max(no),0)+1 as n from arch_files')).rows[0].n);
    const fname = `${spec.table}.csv`;
    const body = toCsv(cols, r.rows);
    return await this.#flush({ spec, open_, no, ver, files, fname, body, rows: r.rows, cut, now });
  }

  async #flush({ spec, open_, no, ver, files, fname, body, rows, cut, now }) {
    // Mavjud faylga qo'shish (sarlavhasiz) yoki yangi fayl (sarlavha bilan).
    const prev = files[fname];
    files[fname] = Buffer.from((prev ? prev.toString('utf8') : '') + (prev ? body.slice(body.indexOf('\n') + 1) : body), 'utf8');
    const total = Object.values(files).reduce((a, b) => a + b.length, 0);
    const sealedNow = total >= CHUNK_BYTES;
    const keyHex = crypto.randomBytes(32).toString('hex');
    const sealed = seal(Buffer.from(keyHex, 'hex'), packFiles(files));
    const name = `db_${no}_v${ver}.bin`;
    const up = await this.tg.send(name, sealed);
    const keys = rows.map((row) => keyOf(spec, row));
    const client = await this.pool.connect();
    let row;
    try {
      await client.query('begin');
      if (open_) {
        row = (await client.query('update arch_files set file_id=$1, msg_id=$2, key=$3, rows=coalesce(rows,0)+$4, size=$5, sealed=$6, name=$7 where id=$8 returning *',
          [up.file_id, up.msg_id, keyHex, rows.length, sealed.length, sealedNow, name, open_.id])).rows[0];
      } else {
        row = (await client.query('insert into arch_files(tbl,name,file_id,msg_id,key,rows,size,created_at,no,ver,sealed) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) returning *',
          ['mixed', name, up.file_id, up.msg_id, keyHex, rows.length, sealed.length, now, no, ver, sealedNow])).rows[0];
      }
      // UZ: o'chirishda sovuqlik sharti qayta tekshiriladi: ayni shu orada yangilangan qator o'chmaydi. EN/RU: re-check coldness so a just-updated row survives.
      const del = await client.query(`delete from ${spec.table} t where ${keySql(spec, 't')} = any($1::text[]) and ${spec.cold.replace(/\$1/g, '$2')} returning ${keySql(spec, 't')} as k`, [keys, cut]);
      const gone = del.rows.map((x) => x.k);
      await client.query('insert into arch_keys(tbl,k,fid) select $1, unnest($2::text[]), $3 on conflict (tbl,k) do update set fid = excluded.fid', [spec.table, gone, row.id]);
      await client.query('commit');
      // Faqat endi papka yoziladi va eski Telegram xabari o'chiriladi.
      this.#write(row, files);
      if (open_) await this.tg.del(Number(open_.msg_id));
      return { table: spec.table, found: rows.length, archived: gone.length, chunk: `db_${no}_v${ver}`, sealed: sealedNow, bytes: sealed.length, dry: false };
    } catch (e) {
      await client.query('rollback').catch(() => {});
      await this.tg.del(up.msg_id); // yetim fayl qolmasin
      throw e;
    } finally { client.release(); }
  }

  /** 70% dan keyin ishlaydi. `max` — bir chaqiruvda eng ko'pi bilan nechta bo'lak. EN/RU: one scheduler tick. */
  async run({ force = false, dry = false, max = 5 } = {}) {
    if (this.busy) return { skipped: 'band' };
    this.busy = true;
    try {
      await this.init();
      const pct = await this.usage();
      if (pct < THRESHOLD && !force) return { active: false, percent: pct, threshold: THRESHOLD };
      await this.pool.query('delete from login_tokens where expires_at < $1', [Date.now() - DAY]).catch(() => {});
      await this.pool.query('delete from arch_warm where until < $1', [Date.now()]).catch(() => {});
      const steps = [];
      for (let i = 0; i < max; i++) {
        let did = false;
        for (const spec of SPECS) {
          const s = await this.archiveOne(spec, { dry });
          steps.push(s);
          if (s.archived || (dry && s.found)) { did = true; break; }
        }
        if (!did || dry) break;
        if (!force && (await this.usage()) < THRESHOLD - 5) break; // 65% ga tushsa to'xtaydi. EN/RU: stop once back under 65%.
      }
      return { active: true, percent: pct, threshold: THRESHOLD, steps };
    } finally { this.busy = false; }
  }

  async files() {
    await this.init();
    return (await this.pool.query('select id, no, ver, sealed, name, rows, size, created_at, restored_at from arch_files order by id desc limit 100')).rows;
  }

  /** Arxivdagi bitta qator (yoki `null`): bo'lak papkasi ochiladi (kerak bo'lsa Telegram'dan olinadi), jadval jurnalidan qator topiladi. EN/RU: one archived row. */
  async lookup(tbl, key) {
    const spec = specOf(tbl); if (!spec) throw new Error('noma\'lum jadval');
    const k = await this.pool.query('select fid from arch_keys where tbl=$1 and k=$2', [tbl, key]);
    if (!k.rows[0]) return null;
    const f = (await this.pool.query('select * from arch_files where id=$1', [k.rows[0].fid])).rows[0];
    if (!f) return null;
    const files = await this.#load(f);
    const csv = files[`${tbl}.csv`];
    return csv ? csvObjects(csv.toString('utf8')).find((o) => keyOf(spec, o) === key) || null : null;
  }

  /** Bo'lakdagi hamma qatorni bazaga qaytaradi (mavjudlariga tegmaydi) va `warmDays` kunga qayta arxivlanishdan himoya qiladi. EN/RU: restore a chunk + keep warm. */
  async restore(id, { warmDays = 7 } = {}) {
    const f = (await this.pool.query('select * from arch_files where id=$1', [id])).rows[0];
    if (!f) throw new Error('fayl topilmadi');
    const files = await this.#load(f);
    const client = await this.pool.connect();
    let restored = 0; const tables = [];
    try {
      await client.query('begin');
      const until = Date.now() + warmDays * DAY;
      for (const [n, buf] of Object.entries(files)) {
        const spec = specOf(n.replace(/\.csv$/, '')); if (!spec) continue;
        const objs = csvObjects(buf.toString('utf8')); if (!objs.length) continue;
        const ins = await client.query(`insert into ${spec.table} select * from json_populate_recordset(null::${spec.table}, $1::json) on conflict do nothing`, [JSON.stringify(objs)]);
        await client.query('insert into arch_warm(tbl,k,until) select $1, unnest($2::text[]), $3 on conflict (tbl,k) do update set until = excluded.until', [spec.table, objs.map((o) => keyOf(spec, o)), until]);
        restored += ins.rowCount; tables.push(spec.table);
      }
      await client.query('update arch_files set restored_at=$1 where id=$2', [Date.now(), id]);
      await client.query('commit');
      return { restored, tables, warm_until: Date.now() + warmDays * DAY };
    } catch (e) { await client.query('rollback').catch(() => {}); throw e; } finally { client.release(); }
  }

  /** Kalit bo'yicha: qator qaysi bo'lakda bo'lsa, o'sha bo'lakni isitadi. EN/RU: warm by key. */
  async warm(tbl, key, opts) {
    const k = await this.pool.query('select fid from arch_keys where tbl=$1 and k=$2', [tbl, key]);
    if (!k.rows[0]) return { restored: 0, found: false };
    return { found: true, ...(await this.restore(k.rows[0].fid, opts)) };
  }

  /** Bo'lakni butunlay o'chiradi (Telegram xabari, papka, kalit indeksi, ro'yxat) — ma'lumot arxivda QOLMAYDI. Faqat sinov/tozalash. EN/RU: remove a chunk entirely. */
  async purge(id) {
    const f = (await this.pool.query('select * from arch_files where id=$1', [id])).rows[0];
    if (!f) return { purged: false };
    const okTg = await this.tg.del(Number(f.msg_id));
    await this.pool.query('delete from arch_keys where fid=$1', [id]);
    await this.pool.query('delete from arch_files where id=$1', [id]);
    fs.rmSync(this.dirOf(f.no, f.ver), { recursive: true, force: true });
    return { purged: true, telegram_deleted: okTg };
  }
}

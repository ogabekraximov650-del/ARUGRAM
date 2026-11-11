// UZ: Bitta jadval = bitta Durable Object (SQLite). U INDEKSni (kalit -> bo'lak) ushlaydi va yozuvlarni
//     "guruh-commit" bilan Telegram bo'laklariga yozadi. R2 YO'Q.
// EN: One table = one Durable Object (SQLite). It holds the INDEX (key -> chunk) and writes records to
//     Telegram chunks with group commit. NO R2.
// RU: Odna tablitsa = odin Durable Object (SQLite). Derzhit INDEKS (klyuch -> chank) i pishet zapisi v
//     chanki Telegram gruppovym kommitom. BEZ R2.
import { chunkName, encodeChunk, decodeChunk, planBatch, validTable, MAX_CHUNK } from './core.js';
import * as tg from './tg.js';

const GC_DELAY_MS = 120_000;      // UZ: eski versiya shuncha turib o'chiriladi / EN: old versions deleted after / RU: starye versii udalyayutsya cherez
const BACKUP_DELAY_MS = 15_000;   // UZ: indeks nusxasi (zerkal kanal) / EN: index dump delay / RU: zaderzhka dampa indeksa
const json = (o, status = 200) => new Response(JSON.stringify(o), { status, headers: { 'content-type': 'application/json' } });

export class TableDO {
  constructor(ctx, env) {
    this.ctx = ctx;
    this.env = env;
    this.sql = ctx.storage.sql;
    this.mem = new Map();      // no -> {ver, map}  (tezkor xotira / fast memory / bystraya pamyat)
    this.pending = [];
    this.flushing = false;
    this.sql.exec(`CREATE TABLE IF NOT EXISTS idx(k TEXT PRIMARY KEY, no INTEGER NOT NULL)`);
    this.sql.exec(`CREATE TABLE IF NOT EXISTS chunks(no INTEGER PRIMARY KEY, ver INTEGER NOT NULL, file_id TEXT, msg_id INTEGER, mirror_id INTEGER, size INTEGER, sealed INTEGER NOT NULL DEFAULT 0)`);
    this.sql.exec(`CREATE TABLE IF NOT EXISTS gc(msg_id INTEGER PRIMARY KEY, mirror_id INTEGER, no INTEGER, ver INTEGER, file_id TEXT, due INTEGER)`);
    this.sql.exec(`CREATE TABLE IF NOT EXISTS meta(k TEXT PRIMARY KEY, v TEXT)`);
  }

  one(q, ...a) { return this.sql.exec(q, ...a).toArray()[0]; }
  all(q, ...a) { return this.sql.exec(q, ...a).toArray(); }
  meta(k) { return this.one('SELECT v FROM meta WHERE k=?', k)?.v; }
  setMeta(k, v) { this.sql.exec('INSERT INTO meta(k,v) VALUES(?,?) ON CONFLICT(k) DO UPDATE SET v=excluded.v', k, String(v)); }

  async fetch(req) {
    const u = new URL(req.url);
    const table = req.headers.get('x-table') || '';
    if (!validTable(table)) return json({ error: 'bad table' }, 400);
    this.table = table;
    if (this.meta('table') !== table) this.setMeta('table', table);
    try {
      switch (u.pathname) {
        case '/write': return json(await this.write((await req.json()).ops || []));
        case '/locate': return json(this.locate(u.searchParams.get('k')));
        case '/chunk': return json(this.chunkInfo(+u.searchParams.get('no'), +u.searchParams.get('ver')));
        case '/chunks': return json({ chunks: this.all('SELECT no,ver,size,sealed FROM chunks ORDER BY no'), count: this.one('SELECT COUNT(*) c FROM idx').c });
        case '/keys': return json(this.keys(u.searchParams.get('after') || '', Math.min(+u.searchParams.get('limit') || 100, 1000)));
        case '/gc': return json(await this.runGc(true));
        case '/backup': return json(await this.backup());
        case '/restore': return json(await this.restore());
        case '/stats': return json(this.stats());
        default: return json({ error: 'not found' }, 404);
      }
    } catch (e) {
      return json({ error: String(e?.message || e) }, 500);
    }
  }

  locate(k) {
    const r = this.one('SELECT i.no, c.ver FROM idx i JOIN chunks c ON c.no=i.no WHERE i.k=?', k);
    if (!r) return { found: false };
    const c = this.chunkInfo(r.no, r.ver);
    return { found: true, no: r.no, ver: r.ver, file_id: c.file_id };
  }

  /** UZ: joriy yoki hali o'chirilmagan eski versiya file_id'si / EN: file_id of current or not-yet-deleted old version / RU: file_id tekushchey ili ne udalennoy versii */
  chunkInfo(no, ver) {
    const c = this.one('SELECT file_id, ver FROM chunks WHERE no=? AND ver=?', no, ver);
    if (c) return { found: true, file_id: c.file_id, current: true };
    const g = this.one('SELECT file_id FROM gc WHERE no=? AND ver=?', no, ver);
    return g ? { found: true, file_id: g.file_id, current: false } : { found: false };
  }

  keys(after, limit) {
    const rows = this.all('SELECT i.k, i.no, c.ver FROM idx i JOIN chunks c ON c.no=i.no WHERE i.k>? ORDER BY i.k LIMIT ?', after, limit);
    return { keys: rows, next: rows.length === limit ? rows[rows.length - 1].k : null };
  }

  stats() {
    const c = this.one('SELECT COUNT(*) n, COALESCE(SUM(size),0) bytes, COALESCE(SUM(sealed),0) sealed FROM chunks');
    return {
      table: this.table, keys: this.one('SELECT COUNT(*) c FROM idx').c, chunks: c.n, bytes: c.bytes, sealed: c.sealed,
      gc_pending: this.one('SELECT COUNT(*) c FROM gc').c, commits: +(this.meta('commits') || 0), last_backup: +(this.meta('last_backup') || 0),
    };
  }

  // ── YOZISH: guruh-commit / WRITE: group commit / ZAPIS: gruppovoy kommit ─────────────────────────────
  write(ops) {
    return new Promise((resolve, reject) => {
      this.pending.push({ ops, resolve, reject });
      if (!this.flushing) { this.flushing = true; this.flushLoop(); }
    });
  }

  async flushLoop() {
    try {
      while (this.pending.length) {
        const group = this.pending.splice(0);
        const all = group.flatMap((g) => g.ops);
        try {
          const final = await this.commit(all);
          for (const g of group) {
            const res = {};
            for (const op of g.ops) res[op.k] = final.get(op.k) ?? null;
            g.resolve({ ok: true, batched: group.length, results: res });
          }
        } catch (e) {
          // UZ: guruh yiqilsa har bir so'rov alohida qayta uriniladi (bitta yomon so'rov boshqasini o'ldirmasin)
          // EN: if the group fails every request is retried alone (one bad request must not kill the others)
          // RU: esli gruppa upala, kazhdyy zapros probuetsya otdelno
          if (group.length === 1) { group[0].reject(e); continue; }
          for (const g of group) {
            try {
              const f = await this.commit(g.ops);
              const res = {};
              for (const op of g.ops) res[op.k] = f.get(op.k) ?? null;
              g.resolve({ ok: true, batched: 1, results: res });
            } catch (e2) { g.reject(e2); }
          }
        }
      }
    } finally {
      this.flushing = false;
    }
  }

  async loadChunk(no) {
    const c = this.one('SELECT ver, file_id FROM chunks WHERE no=?', no);
    if (!c) return new Map();
    const m = this.mem.get(no);
    if (m && m.ver === c.ver) return m.map;
    const map = decodeChunk(await tg.download(this.env, c.file_id));
    this.mem.set(no, { ver: c.ver, map });
    return map;
  }

  /** UZ: bitta commit: reja -> yangi versiyalarni yuklash -> SQLite'da bir tranzaksiya / EN / RU: see above */
  async commit(ops) {
    // UZ: planBatch sinxron, shuning uchun kerakli bo'laklarni oldindan yuklab olamiz
    // EN: planBatch is synchronous so we preload the chunks it may need
    // RU: planBatch sinkhronen, poetomu zaranee gruzim nuzhnye chanki
    const need = new Set();
    const open = this.openNo();
    need.add(open);
    for (const op of ops) { const r = this.one('SELECT no FROM idx WHERE k=?', op.k); if (r) need.add(r.no); }
    const loaded = new Map();
    for (const no of need) loaded.set(no, await this.loadChunk(no));
    const plan = planBatch({
      ops, openNo: open, max: MAX_CHUNK,
      locate: (k) => this.one('SELECT no FROM idx WHERE k=?', k)?.no ?? null,
      load: (no) => loaded.get(no) || new Map(),
      sealed: (no) => !!this.one('SELECT sealed FROM chunks WHERE no=?', no)?.sealed,
    });

    const chat = this.env.CHANNEL_ID;
    const mirror = this.env.MIRROR_CHANNEL_ID;
    const uploads = [];
    for (const [no, map] of plan.touched) {
      const cur = this.one('SELECT ver FROM chunks WHERE no=?', no);
      const ver = (cur?.ver || 0) + 1;
      const bytes = encodeChunk(map);
      uploads.push((async () => {
        const up = await tg.sendDoc(this.env, chat, chunkName(this.table, no, ver), bytes);
        let mirror_id = null;
        if (mirror) { try { mirror_id = await tg.copyMsg(this.env, mirror, chat, up.message_id); } catch { /* zerkal ixtiyoriy / mirror optional / zerkalo neobyazatelno */ } }
        return { no, ver, map, size: bytes.length, ...up, mirror_id };
      })());
    }
    const done = await Promise.all(uploads);

    const now = Date.now();
    this.ctx.storage.transactionSync(() => {
      for (const u of done) {
        const old = this.one('SELECT ver, file_id, msg_id, mirror_id FROM chunks WHERE no=?', u.no);
        if (old?.msg_id) this.sql.exec('INSERT OR REPLACE INTO gc(msg_id,mirror_id,no,ver,file_id,due) VALUES(?,?,?,?,?,?)', old.msg_id, old.mirror_id, u.no, old.ver, old.file_id, now + GC_DELAY_MS);
        this.sql.exec(
          `INSERT INTO chunks(no,ver,file_id,msg_id,mirror_id,size,sealed) VALUES(?,?,?,?,?,?,0)
           ON CONFLICT(no) DO UPDATE SET ver=excluded.ver,file_id=excluded.file_id,msg_id=excluded.msg_id,mirror_id=excluded.mirror_id,size=excluded.size`,
          u.no, u.ver, u.file_id, u.message_id, u.mirror_id, u.size);
      }
      for (const no of plan.newlySealed) this.sql.exec('UPDATE chunks SET sealed=1 WHERE no=?', no);
      for (const [k, no] of plan.assign) {
        if (no == null) this.sql.exec('DELETE FROM idx WHERE k=?', k);
        else this.sql.exec('INSERT INTO idx(k,no) VALUES(?,?) ON CONFLICT(k) DO UPDATE SET no=excluded.no', k, no);
      }
      this.setMeta('commits', +(this.meta('commits') || 0) + 1);
      this.setMeta('dirty', 1);
    });
    for (const u of done) this.mem.set(u.no, { ver: u.ver, map: u.map });

    const final = new Map();
    for (const [k, no] of plan.assign) {
      if (no == null) final.set(k, null);
      else final.set(k, { no, ver: this.one('SELECT ver FROM chunks WHERE no=?', no).ver });
    }
    await this.scheduleAlarm();
    return final;
  }

  openNo() {
    const r = this.one('SELECT no FROM chunks WHERE sealed=0 ORDER BY no DESC LIMIT 1');
    if (r) return r.no;
    const m = this.one('SELECT MAX(no) m FROM chunks');
    return (m?.m || 0) + 1;
  }

  // ── GC va zaxira nusxa / GC and backup / GC i bekap ───────────────────────────────────────────────────
  async scheduleAlarm() {
    const cur = await this.ctx.storage.getAlarm();
    if (!cur) await this.ctx.storage.setAlarm(Date.now() + Math.min(GC_DELAY_MS, BACKUP_DELAY_MS) + 1000);
  }

  async alarm() {
    // `table` alarmda yo'q — meta'dan olamiz / table is absent in alarm — take from meta / table net v alarm — berem iz meta
    this.table = this.table || this.meta('table');
    try { await this.runGc(false); } catch { /* keyingi alarmda / next alarm / sleduyushchiy alarm */ }
    try { if (this.meta('dirty') === '1' && this.env.MIRROR_CHANNEL_ID) await this.backup(); } catch { /* idem */ }
    if (this.one('SELECT COUNT(*) c FROM gc').c > 0 || this.meta('dirty') === '1') await this.ctx.storage.setAlarm(Date.now() + 30_000);
  }

  async runGc(force) {
    const now = Date.now();
    const rows = force ? this.all('SELECT * FROM gc') : this.all('SELECT * FROM gc WHERE due<=?', now);
    let deleted = 0;
    for (const r of rows) {
      const ok = await tg.deleteMsg(this.env, this.env.CHANNEL_ID, r.msg_id);
      if (r.mirror_id && this.env.MIRROR_CHANNEL_ID) await tg.deleteMsg(this.env, this.env.MIRROR_CHANNEL_ID, r.mirror_id);
      if (ok) { this.sql.exec('DELETE FROM gc WHERE msg_id=?', r.msg_id); deleted++; }
    }
    return { deleted, left: this.one('SELECT COUNT(*) c FROM gc').c };
  }

  /** UZ: bo'laklar jadvali (indeks emas — u bo'laklardan tiklanadi) zerkal kanalga yuboriladi va qadab qo'yiladi.
   *  EN: the chunk table (not the key index — it is rebuilt from chunks) is sent to the mirror channel and pinned.
   *  RU: tablitsa chankov (a ne indeks klyuchey — on vosstanavlivaetsya iz chankov) shlyotsya v zerkalo i zakreplyaetsya. */
  async backup() {
    const mirror = this.env.MIRROR_CHANNEL_ID;
    if (!mirror) return { skipped: 'no mirror channel' };
    this.setMeta('table', this.table);
    const dump = { table: this.table, at: Date.now(), chunks: this.all('SELECT no,ver,file_id,msg_id,mirror_id,size,sealed FROM chunks ORDER BY no') };
    const bytes = new TextEncoder().encode(JSON.stringify(dump));
    const up = await tg.sendDoc(this.env, mirror, `DBP_${this.table}_index.json`, bytes);
    await tg.pinMsg(this.env, mirror, up.message_id);
    const prev = this.meta('index_msg');
    if (prev) await tg.deleteMsg(this.env, mirror, +prev);
    this.setMeta('index_msg', up.message_id);
    this.setMeta('dirty', 0);
    this.setMeta('last_backup', Date.now());
    return { ok: true, chunks: dump.chunks.length, bytes: bytes.length, message_id: up.message_id };
  }

  /** UZ: DO bo'sh bo'lsa zerkal kanaldagi qadalgan indeksdan tiklaydi va har bir bo'lakdan kalit indeksini qayta quradi.
   *  EN: restores from the pinned index in the mirror channel and rebuilds the key index from every chunk.
   *  RU: vosstanavlivaet iz zakreplennogo indeksa v zerkale i perestraivaet indeks klyuchey po kazhdomu chanku. */
  async restore() {
    const mirror = this.env.MIRROR_CHANNEL_ID;
    if (!mirror) throw new Error('no mirror channel');
    const chat = await tg.getChat(this.env, mirror);
    const fid = chat.pinned_message?.document?.file_id;
    if (!fid) throw new Error('no pinned index');
    const dump = JSON.parse(new TextDecoder().decode(await tg.download(this.env, fid)));
    if (dump.table !== this.table) throw new Error('index is for table ' + dump.table);
    this.sql.exec('DELETE FROM idx'); this.sql.exec('DELETE FROM chunks'); this.sql.exec('DELETE FROM gc');
    this.mem.clear();
    let keys = 0;
    for (const c of dump.chunks) {
      let file_id = c.file_id, msg_id = c.msg_id;
      let bytes;
      try { bytes = await tg.download(this.env, file_id); }
      catch {
        // UZ: asosiy kanal yo'qolgan bo'lsa zerkal nusxasidan olinadi / EN: if main channel is lost use the mirror copy / RU: esli osnovnoy kanal poteryan — kopiya iz zerkala
        const f = await tg.forwardMsg(this.env, mirror, mirror, c.mirror_id);
        file_id = f.file_id; msg_id = null;
        bytes = await tg.download(this.env, file_id);
        await tg.deleteMsg(this.env, mirror, f.message_id);
      }
      const map = decodeChunk(bytes);
      this.sql.exec('INSERT INTO chunks(no,ver,file_id,msg_id,mirror_id,size,sealed) VALUES(?,?,?,?,?,?,?)', c.no, c.ver, file_id, msg_id, c.mirror_id, c.size, c.sealed);
      for (const k of map.keys()) this.sql.exec('INSERT OR REPLACE INTO idx(k,no) VALUES(?,?)', k, c.no);
      keys += map.size;
      this.mem.set(c.no, { ver: c.ver, map });
    }
    this.setMeta('table', this.table);
    return { ok: true, chunks: dump.chunks.length, keys };
  }
}

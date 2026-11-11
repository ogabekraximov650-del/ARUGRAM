// UZ: sof mantiq (tarmoqsiz) — bo'lak nomi, kodlash, yozuvlarni bo'laklarga joylash. Node'da sinaladi.
// EN: pure logic (no network) — chunk names, encoding, placing records into chunks. Unit-tested in Node.
// RU: chistaya logika (bez seti) — imena chankov, kodirovanie, razmeshchenie zapisey po chankam.

export const MAX_CHUNK = 1000 * 1024; // ~1 MB: UZ: shundan oshsa bo'lak "muhrlanadi" / EN: sealed when exceeded / RU: pri prevyshenii chank zapechatyvaetsya
const enc = new TextEncoder();
const dec = new TextDecoder();

export function chunkName(table, no, ver) {
  return `DBP_${table}_${no}_v${ver}`;
}

export function parseChunkName(name) {
  const m = /^DBP_(.+)_(\d+)_v(\d+)$/.exec(name);
  return m ? { table: m[1], no: +m[2], ver: +m[3] } : null;
}

export function validTable(t) {
  return typeof t === 'string' && /^[A-Za-z0-9-]{1,40}$/.test(t);
}

const lineOf = (k, v) => JSON.stringify([k, v]) + '\n';
const bytesOf = (s) => enc.encode(s).length;

/** UZ: Map -> bayt (har qator: [kalit, qiymat]) / EN: Map -> bytes / RU: Map -> bayty */
export function encodeChunk(map) {
  let s = '';
  for (const [k, v] of map) s += lineOf(k, v);
  return enc.encode(s);
}

export function decodeChunk(bytes) {
  const map = new Map();
  const text = dec.decode(bytes);
  for (const line of text.split('\n')) {
    if (!line) continue;
    const [k, v] = JSON.parse(line);
    map.set(k, v);
  }
  return map;
}

export function sizeOf(map) {
  let n = 0;
  for (const [k, v] of map) n += bytesOf(lineOf(k, v));
  return n;
}

/**
 * UZ: Bir paket amalni bo'laklarga joylaydi.
 *  ops: [{op:'put',k,v} | {op:'del',k}]
 *  locate(k) -> bo'lak raqami yoki null; load(no) -> Map (yo'q bo'lak uchun bo'sh Map);
 *  openNo — hozir yozishga ochiq bo'lak; sealed(no) -> bool.
 *  Qoidalar: yangi kalit ochiq bo'lakka; to'lsa bo'lak muhrlanadi, keyingisi ochiladi;
 *  muhrlangan bo'lakda faqat tahrir/o'chirish; tahrir bo'lakka sig'masa kalit ochiq bo'lakka ko'chadi.
 * EN: Places a batch of ops into chunks (rules above, same in all three languages).
 * RU: Razmeshchaet paket operatsiy po chankam (pravila vyshe).
 * Returns { touched: Map(no->Map), assign: Map(key->no|null), newlySealed: Set, openNo }
 */
export function planBatch({ ops, locate, load, openNo, sealed, max = MAX_CHUNK }) {
  const touched = new Map();
  const sizes = new Map();
  const assign = new Map();
  const newlySealed = new Set();
  const sealedNow = (no) => newlySealed.has(no) || sealed(no);
  let cur = openNo;
  const nextNo = () => Math.max(cur, ...touched.keys(), ...newlySealed) + 1;

  const get = (no) => {
    let m = touched.get(no);
    if (!m) {
      m = new Map(load(no));
      touched.set(no, m);
      sizes.set(no, sizeOf(m));
    }
    return m;
  };
  const removeFrom = (no, k) => {
    const m = get(no);
    if (m.has(k)) {
      sizes.set(no, sizes.get(no) - bytesOf(lineOf(k, m.get(k))));
      m.delete(k);
    }
  };
  const addTo = (no, k, v) => {
    const m = get(no);
    m.set(k, v);
    sizes.set(no, sizes.get(no) + bytesOf(lineOf(k, v)));
  };

  for (const op of ops) {
    const k = op.k;
    if (typeof k !== 'string' || !k) throw new Error('bad key');
    if (op.op === 'del') {
      const no = assign.has(k) ? assign.get(k) : locate(k);
      if (no != null) removeFrom(no, k);
      assign.set(k, null);
      continue;
    }
    if (op.op !== 'put') throw new Error('bad op');
    const line = bytesOf(lineOf(k, op.v));
    if (line > max) throw new Error('record too large');
    let no = assign.has(k) ? assign.get(k) : locate(k);
    if (no != null) {
      // UZ: tahrir — avval eski qiymat olib tashlanadi / EN: edit — drop the old value first / RU: pravka — snachala ubiraem staroe
      removeFrom(no, k);
      if (sizes.get(no) + line <= max) {
        addTo(no, k, op.v);
        assign.set(k, no);
        continue;
      }
      // sig'madi -> ochiq bo'lakka ko'chadi / does not fit -> moves to the open chunk / ne vlezlo -> v otkrytyy chank
    }
    if (sealedNow(cur)) cur = nextNo();
    get(cur);
    if (sizes.get(cur) > 0 && sizes.get(cur) + line > max) {
      newlySealed.add(cur);
      cur = nextNo();
      get(cur);
    }
    addTo(cur, k, op.v);
    assign.set(k, cur);
  }
  return { touched, assign, newlySealed, openNo: cur };
}

// ── UZ: kichik yordamchilar / EN: small helpers / RU: melkie pomoshchniki ──
export async function sha256Hex(s) {
  const d = await crypto.subtle.digest('SHA-256', enc.encode(s));
  return [...new Uint8Array(d)].map((b) => b.toString(16).padStart(2, '0')).join('');
}

export function timingSafeEq(a, b) {
  if (typeof a !== 'string' || typeof b !== 'string' || a.length !== b.length) return false;
  let r = 0;
  for (let i = 0; i < a.length; i++) r |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return r === 0;
}

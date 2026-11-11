// UZ: SQLite -> PostgreSQL tarjimon. Production (Turso) SQL buyruqlari SQLite lahjasida; bu modul ularni Postgres'ga o'giradi (kodni o'zgartirmasdan).
// EN: SQLite -> PostgreSQL translator. The production SQL is written for SQLite; this turns it into Postgres SQL without touching the app code.
// RU: Perevodchik SQLite -> PostgreSQL.

/** Satr literallari va izohlarni vaqtincha olib tashlaydi (ichidagi `?` va kalit so'zlarga tegmaslik uchun). */
export function mask(sql) {
  const strs = [];
  let out = '';
  for (let i = 0; i < sql.length;) {
    const c = sql[i];
    if (c === "'" || c === '"') {
      let j = i + 1;
      while (j < sql.length) { if (sql[j] === c) { if (sql[j + 1] === c) { j += 2; continue; } break; } j++; }
      strs.push(sql.slice(i, j + 1));
      out += `\u0001${strs.length - 1}\u0001`;
      i = j + 1;
    } else if (c === '-' && sql[i + 1] === '-') {
      while (i < sql.length && sql[i] !== '\n') i++;
    } else if (c === '/' && sql[i + 1] === '*') {
      const j = sql.indexOf('*/', i + 2);
      i = j < 0 ? sql.length : j + 2;
      out += ' ';
    } else { out += c; i++; }
  }
  return { text: out, strs };
}
export const unmask = (text, strs) => text.replace(/\u0001(\d+)\u0001/g, (_, i) => {
  const s = strs[+i];
  // SQLite "..." (ikki tirnoq) qiymat bo'lib qolgan joylar uchun: Postgres'da bu identifikator. Bo'sh/oddiy matnni bir tirnoqqa o'giramiz.
  return s;
});

/** `name(` dan keyingi mos yopuvchi qavs indeksi. */
function matchParen(t, open) {
  let d = 0;
  for (let i = open; i < t.length; i++) { if (t[i] === '(') d++; else if (t[i] === ')') { d--; if (d === 0) return i; } }
  return -1;
}
function splitArgs(s) {
  const out = []; let d = 0, cur = '';
  for (const ch of s) {
    if (ch === '(') d++; else if (ch === ')') d--;
    if (ch === ',' && d === 0) { out.push(cur); cur = ''; } else cur += ch;
  }
  out.push(cur);
  return out;
}

/** Skalyar MAX(a,b[,..]) / MIN(a,b[,..]) -> GREATEST / LEAST (agregat MAX(x) o'zgarishsiz). */
export function scalarMinMax(t) {
  const re = /\b(MAX|MIN)\s*\(/gi;
  let out = '', last = 0, m;
  while ((m = re.exec(t))) {
    const open = m.index + m[0].length - 1;
    const close = matchParen(t, open);
    if (close < 0) break;
    const inner = scalarMinMax(t.slice(open + 1, close));
    const args = splitArgs(inner);
    out += t.slice(last, m.index);
    out += args.length > 1 ? `${m[1].toUpperCase() === 'MAX' ? 'GREATEST' : 'LEAST'}(${inner})` : `${m[1]}(${inner})`;
    last = close + 1;
    re.lastIndex = close + 1;
  }
  return out + t.slice(last);
}

/** GROUP_CONCAT(x[, sep]) -> string_agg((x)::text, sep). */
export function groupConcat(t) {
  const re = /\bGROUP_CONCAT\s*\(/gi;
  let out = '', last = 0, m;
  while ((m = re.exec(t))) {
    const open = m.index + m[0].length - 1;
    const close = matchParen(t, open);
    if (close < 0) break;
    const args = splitArgs(t.slice(open + 1, close));
    out += t.slice(last, m.index) + `string_agg((${args[0].trim().replace(/^DISTINCT\s+/i, '')})::text, ${args[1] ? args[1].trim() : "','"})`;
    last = close + 1; re.lastIndex = close + 1;
  }
  return out + t.slice(last);
}


/** CAST(x AS INTEGER): SQLite matn bo'lsa 0 beradi, Postgres esa xato — shuning uchun xavfsiz shakl. EN/RU: SQLite-style lenient CAST to integer. */
export function castInteger(t) {
  const re = /\bCAST\s*\(/gi;
  let out = '', last = 0, m;
  while ((m = re.exec(t))) {
    const open = m.index + m[0].length - 1;
    const close = matchParen(t, open);
    if (close < 0) break;
    const inner = t.slice(open + 1, close);
    const mm = inner.match(/^([\s\S]*)\s+AS\s+INTEGER\s*$/i);
    if (!mm) continue;
    const x = castInteger(mm[1]);
    out += t.slice(last, m.index) + `(CASE WHEN (${x})::text ~ '^-{0,1}[0-9]{1,18}$' THEN (${x})::text::bigint ELSE 0 END)`;
    last = close + 1; re.lastIndex = close + 1;
  }
  return out + t.slice(last);
}

/** x [NOT] GLOB 'pattern' -> x [!~|~] '^regex$' (andoza satri `strs` ichida almashtiriladi). EN/RU: GLOB -> POSIX regex. */
export function globToRegex(p) {
  let out = '^', inBr = false;
  for (const ch of p) {
    if (inBr) { out += ch; if (ch === ']') inBr = false; continue; }
    if (ch === '[') { inBr = true; out += ch; }
    else if (ch === '*') out += '.*';
    else if (ch === '?') out += '.';
    else if ('.+(){}|^$\\'.includes(ch)) out += '\\' + ch;
    else out += ch;
  }
  return out + '$';
}
export function globOps(t, strs) {
  return t.replace(/\b(NOT\s+)?GLOB\s+\u0001(\d+)\u0001/gi, (_, not, i) => {
    const lit = strs[+i];
    strs[+i] = "'" + globToRegex(lit.slice(1, -1).replace(/''/g, "'")).replace(/'/g, "''") + "'";
    return `${not ? '!~' : '~'} \u0001${i}\u0001`;
  });
}

/** strftime('fmt', expr, 'unixepoch') -> to_char(to_timestamp(expr) AT TIME ZONE 'UTC', 'fmt'). EN/RU: SQLite strftime for unix seconds. */
export function strftimeOps(t, strs) {
  const re = /\bstrftime\s*\(/gi;
  let out = '', last = 0, m;
  while ((m = re.exec(t))) {
    const open = m.index + m[0].length - 1;
    const close = matchParen(t, open);
    if (close < 0) break;
    const args = splitArgs(t.slice(open + 1, close));
    const fm = args[0] && args[0].trim().match(/^\u0001(\d+)\u0001$/);
    if (!fm || args.length < 3) continue;
    const fmt = strs[+fm[1]].slice(1, -1).replace(/%Y/g, 'YYYY').replace(/%m/g, 'MM').replace(/%d/g, 'DD').replace(/%H/g, 'HH24').replace(/%M/g, 'MI').replace(/%S/g, 'SS').replace(/T/g, '"T"');
    strs.push("'" + fmt.replace(/'/g, "''") + "'");
    out += t.slice(last, m.index) + `to_char(to_timestamp(${args[1].trim()}) AT TIME ZONE 'UTC', \u0001${strs.length - 1}\u0001)`;
    last = close + 1; re.lastIndex = close + 1;
  }
  return out + t.slice(last);
}

const firstWord = (t) => (t.trim().match(/^[A-Za-z]+/) || [''])[0].toUpperCase();

/** CREATE TABLE / ALTER TABLE turlari: INTEGER -> BIGINT (millisekund vaqtlar 32 bitga sig'maydi), INTEGER PRIMARY KEY -> identity. */
export function translateDdl(t) {
  let s = t;
  s = s.replace(/\bINTEGER\s+PRIMARY\s+KEY(\s+AUTOINCREMENT)?/gi, 'BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY');
  s = s.replace(/\bAUTOINCREMENT\b/gi, '');
  s = s.replace(/\bINTEGER\b/gi, 'BIGINT').replace(/\bREAL\b/gi, 'DOUBLE PRECISION').replace(/\bBLOB\b/gi, 'BYTEA');
  s = s.replace(/\)\s*WITHOUT\s+ROWID/gi, ')');
  return s;
}

function parseInsertHead(t) {
  const m = t.match(/\bINSERT\s+INTO\s+([A-Za-z_][\w]*)\s*(\(([^)]*)\))?/i);
  if (!m) return null;
  return { table: m[1].toLowerCase(), cols: m[3] ? m[3].split(',').map((c) => c.trim().toLowerCase()) : null };
}

/** `ON CONFLICT(...) DO UPDATE SET a = a + excluded.a` — Postgres'da o'ng tomondagi `a` noaniq: jadval nomi bilan to'ldiriladi. */
export function qualifyConflict(t, table, cols) {
  const m = t.match(/\bON\s+CONFLICT\b[\s\S]*?\bDO\s+UPDATE\s+SET\b/i);
  if (!m) return t;
  const start = m.index + m[0].length;
  const tail = t.slice(start);
  const endM = tail.match(/\b(WHERE|RETURNING)\b/i);
  const clause = endM ? tail.slice(0, endM.index) : tail;
  const rest = endM ? tail.slice(endM.index) : '';
  const colset = new Set(cols);
  const fixed = splitArgs(clause).map((asg) => {
    const eq = asg.indexOf('=');
    if (eq < 0) return asg;
    const lhs = asg.slice(0, eq), rhs = asg.slice(eq + 1);
    const q = rhs.replace(/(\.\s*)?\b([A-Za-z_][\w]*)\b(?!\s*\()/g, (full, dot, id) => (dot || !colset.has(id.toLowerCase()) ? full : `${table}.${id}`));
    return `${lhs}=${q}`;
  });
  return t.slice(0, start) + fixed.join(',') + ' ' + rest;
}

/**
 * Asosiy tarjima. `schema.cols(table)` / `schema.pk(table)` — jadval ustunlari va birlamchi kalit (Postgres'dan o'qilgan).
 * Qaytaradi: { text, strs, kind, table }.
 */
export function translate(sql, schema) {
  const { text: masked, strs } = mask(sql);
  let t = masked.trim().replace(/;+\s*$/, '');
  const kind = firstWord(t);
  let table = null;
  if (kind === 'CREATE' || kind === 'ALTER') {
    t = translateDdl(t);
    return { text: numberParams(t), strs, kind, table };
  }
  t = scalarMinMax(t);
  t = groupConcat(t);
  t = castInteger(t);
  t = globOps(t, strs);
  t = strftimeOps(t, strs);
  t = t.replace(/\bLIKE\b/gi, 'ILIKE').replace(/\bNOT\s+ILIKE\b/gi, 'NOT ILIKE');
  t = t.replace(/\binstr\s*\(([^,()]+),([^()]+)\)/gi, 'strpos($1,$2)').replace(/\bifnull\s*\(/gi, 'COALESCE(');
  // UZ: SQLite'da `(instr(..)>0)+(instr(..)>0)` mantiqiy qiymatlarni qo'shadi; Postgres'da boolean + boolean yo'q -> ::int. EN/RU: boolean arithmetic.
  t = t.replace(/\((strpos\([^()]*\)\s*>\s*0)\)(?=\s*\+)/gi, '(($1)::int)').replace(/(\+\s*)\((strpos\([^()]*\)\s*>\s*0)\)/gi, '$1(($2)::int)');
  if (/^\s*(INSERT|REPLACE)\b/i.test(t)) {
    let ignore = false, replace = false;
    t = t.replace(/^\s*INSERT\s+OR\s+IGNORE\s+INTO\b/i, () => { ignore = true; return 'INSERT INTO'; });
    t = t.replace(/^\s*(INSERT\s+OR\s+REPLACE|REPLACE)\s+INTO\b/i, () => { replace = true; return 'INSERT INTO'; });
    const head = parseInsertHead(t);
    if (head) table = head.table;
    const ret = t.match(/\bRETURNING\b[\s\S]*$/i);
    const body = ret ? t.slice(0, ret.index) : t;
    const tailRet = ret ? ' ' + ret[0] : '';
    if (ignore) t = body + ' ON CONFLICT DO NOTHING' + tailRet;
    else if (replace && head) {
      const pk = schema.pk(head.table), cols = head.cols || schema.cols(head.table);
      const sets = cols.filter((c) => !pk.includes(c)).map((c) => `${c}=EXCLUDED.${c}`).join(',');
      t = body + ` ON CONFLICT (${pk.join(',')}) ` + (sets ? `DO UPDATE SET ${sets}` : 'DO NOTHING') + tailRet;
    } else if (head && /\bON\s+CONFLICT\b[\s\S]*\bDO\s+UPDATE\b/i.test(t)) {
      t = qualifyConflict(t, head.table, schema.cols(head.table));
    }
  } else if (/^\s*(UPDATE|DELETE)\b/i.test(t)) {
    const m = t.match(/^\s*(?:UPDATE(?:\s+OR\s+\w+)?|DELETE\s+FROM)\s+([A-Za-z_]\w*)/i);
    if (m) table = m[1].toLowerCase();
    t = t.replace(/^\s*UPDATE\s+OR\s+\w+\s+/i, 'UPDATE ');
  }
  return { text: numberParams(t), strs, kind, table };
}

/** `?` -> `$1, $2, ...` (satr literallari allaqachon olib tashlangan). */
export function numberParams(t) { let n = 0; return t.replace(/\?(\d+)?/g, (_, d) => (d ? `$${d}` : `$${++n}`)); }

export const finalSql = (r) => unmask(r.text, r.strs);

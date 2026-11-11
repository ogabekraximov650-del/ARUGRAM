//! UZ: kvdb — Telegram bo'laklari ustidagi baza qatlami. Ro'yxat (Supabase) faqat nom/file_id/kalitni biladi; ma'lumot shifrlangan fayllarda.
//!     O'qish: worker xotirasi -> Cloudflare keshi -> Telegram. Yozish: bo'lakni ochish -> o'zgartirish -> qayta shifrlab yuklash -> ro'yxatda CAS.
//! EN: kvdb — the database layer over Telegram chunks. The registry (Supabase) knows only name/file_id/key; data lives in encrypted files.
//!     Read: isolate memory -> Cloudflare cache -> Telegram. Write: open chunk -> change -> re-encrypt + upload -> CAS in the registry.
//! RU: kvdb — sloy bazy nad chankami Telegram. Reestr (Supabase) znaet tolko imya/file_id/klyuch; dannye v shifrovannykh faylakh.

use crate::kvfmt::{self, Op, View, TARGET_CHUNK};
use crate::kvreg::{route, ChunkMeta, TableMeta};
use crate::kvsb::{self, CommitResult};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use worker::*;

const HOT_MAX: usize = 48;

thread_local! {
    /// UZ: ochilgan bo'laklar (nom -> ko'rinish); nom o'zgarmas versiyali, shuning uchun eskirmaydi. EN: opened chunks by immutable name. RU: otkrytye chanki.
    static HOT: RefCell<HashMap<String, Rc<View>>> = RefCell::new(HashMap::new());
    /// UZ: ro'yxat nusxasi (jadval -> (ro'yxat, olingan vaqt)). O'qish shundan; yozish HAR DOIM yangisini oladi. EN/RU: registry cache for reads.
    static PATHS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    static METAS: RefCell<HashMap<String, (TableMeta, i64)>> = RefCell::new(HashMap::new());
}

/// UZ: o'qishda ro'yxat nusxasi shuncha ms eskirishi mumkin (Supabase'ga kamroq borish uchun). EN/RU: max staleness of the registry copy for reads.
const META_TTL_MS: i64 = 3000;
fn now_ms() -> i64 { Date::now().as_millis() as i64 }

fn token(env: &Env) -> String { crate::tg_secret(env, "TELEGRAM_BOT_TOKEN") }
fn channel(env: &Env) -> i64 { crate::tg_secret(env, "KV_CHANNEL_ID").parse().unwrap_or(0) }

fn hex_key(h: &str) -> Result<[u8; 32]> {
    if h.len() != 64 { return Err(Error::RustError("kalit uzunligi".into())); }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).map_err(|_| Error::RustError("kalit hex".into()))?;
    }
    Ok(out)
}

fn err<T>(m: impl Into<String>) -> Result<T> { Err(Error::RustError(m.into())) }

/// UZ: HAMMA jadvallar BITTA tartiblangan kalit fazosida: kalit = `<jadval>\u{1e}<qator kaliti>`. Bo'laklar shu fazoni ketma-ket bo'laklarga bo'ladi.
/// EN: ALL tables share ONE ordered keyspace: key = `<table>\u{1e}<row key>`; chunks split that space into consecutive ranges. RU: odno prostranstvo klyuchey.
pub fn key_of(table: &str, key: &str) -> String { format!("{table}\u{1e}{key}") }
/// [lo, hi): jadvalning hamma kalitlari / all keys of the table / vse klyuchi tablitsy
pub fn range_of(table: &str) -> (String, String) { (format!("{table}\u{1e}"), format!("{table}\u{1f}")) }
pub fn strip(table: &str, key: &str) -> String { key.get(table.len() + 1..).unwrap_or("").to_string() }

/// UZ: bo'lak nomi: `db_<raqam>_v<versiya>` (1 MB ga to'lsa keyingi raqam). EN: chunk name. RU: imya chanka.
pub fn chunk_label(no: u64, ver: u64) -> String { format!("db_{no}_v{ver}") }

/// UZ: kesh/xotira kaliti: nom + kalitning boshi (tozalashdan keyin nom takrorlansa ham eski kesh olinmasin). EN/RU: cache key = name + key prefix.
fn cache_name(c: &ChunkMeta) -> String { format!("{}_{}", c.name, c.key.get(..8).unwrap_or("")) }

/// UZ: ro'yxat, HAR DOIM yangi (yozish uchun). EN: always fresh (for writes). RU: vsegda svezhiy (dlya zapisi).
pub async fn meta_all(env: &Env, fresh: bool) -> Result<TableMeta> {
    if !fresh {
        if let Some(m) = METAS.with(|c| c.borrow().get("*").filter(|(_, at)| now_ms() - *at < META_TTL_MS).map(|(m, _)| m.clone())) {
            return Ok(m);
        }
    }
    let t = kvsb::list(env).await?;
    METAS.with(|c| c.borrow_mut().insert("*".to_string(), (t.clone(), now_ms())));
    Ok(t)
}

async fn download(env: &Env, file_id: &str) -> Result<Vec<u8>> {
    let token = token(env);
    // UZ: `getFile` natijasi (fayl yo'li) ~1 soat yaroqli — izolyatda eslab qolinadi (ichki chaqiruvlar chegarasini tejaydi). EN/RU: cache getFile paths.
    let cached = PATHS.with(|p| p.borrow().get(file_id).cloned());
    let path = match cached {
        Some(p) => p,
        None => {
            let f = crate::tg_api_tok(&token, "getFile", json!({"file_id": file_id})).await?;
            let p = f["file_path"].as_str().unwrap_or("").to_string();
            if !p.is_empty() { PATHS.with(|c| { let mut c = c.borrow_mut(); if c.len() > 512 { c.clear(); } c.insert(file_id.to_string(), p.clone()); }); }
            p
        }
    };
    if path.is_empty() { return err("Telegram fayl yo'lini bermadi"); }
    let mut r = Fetch::Request(Request::new(&format!("https://api.telegram.org/file/bot{token}/{path}"), Method::Get)?).send().await?;
    if r.status_code() != 200 { return err(format!("Telegram yuklash {}", r.status_code())); }
    r.bytes().await
}

fn cache_req(name: &str) -> Result<Request> { Request::new(&format!("https://tgstore-kv.internal/c/{name}"), Method::Get) }

async fn cache_put(name: &str, sealed: &[u8]) {
    let Ok(req) = cache_req(name) else { return };
    let Ok(resp) = Response::from_bytes(sealed.to_vec()) else { return };
    let h = Headers::new();
    let _ = h.set("Cache-Control", "public, max-age=31536000, immutable");
    let _ = Cache::default().put(&req, resp.with_headers(h)).await;
}

fn hot_put(name: &str, v: Rc<View>) {
    HOT.with(|h| {
        let mut h = h.borrow_mut();
        if h.len() >= HOT_MAX { h.clear(); }
        h.insert(name.to_string(), v);
    });
}

/// UZ: bo'lakni ochadi: xotira -> kesh -> Telegram. EN: opens a chunk. RU: otkryvaet chank.
pub async fn load_chunk(env: &Env, c: &ChunkMeta) -> Result<(Rc<View>, &'static str)> {
    let cname = cache_name(c);
    if let Some(v) = HOT.with(|h| h.borrow().get(&cname).cloned()) { return Ok((v, "isolate")); }
    let key = hex_key(&c.key)?;
    let (sealed, src) = match Cache::default().get(&cache_req(&cname)?, false).await? {
        Some(mut hit) => (hit.bytes().await?, "cache"),
        None => {
            let b = download(env, &c.file_id).await?;
            cache_put(&cname, &b).await;
            (b, "telegram")
        }
    };
    let plain = kvfmt::open(&key, &sealed).map_err(Error::RustError)?;
    let v = Rc::new(View::parse(plain).map_err(Error::RustError)?);
    hot_put(&cname, v.clone());
    Ok((v, src))
}

/// UZ: bitta yozuv — faqat kerakli bo'lak ochiladi. EN: one record, only its chunk is opened. RU: odna zapis.
pub async fn get(env: &Env, table: &str, key: &str) -> Result<Option<Value>> {
    let m = meta_all(env, false).await?;
    let k = key_of(table, key);
    let Some(i) = route(&m.chunks, &k) else { return Ok(None) };
    let (v, _) = load_chunk(env, &m.chunks[i]).await?;
    v.get(&k).map_err(Error::RustError)
}

/// UZ: jadval ichidagi [lo, hi) oralig'i (bir nechta bo'lakni kesib o'tishi mumkin). EN: range scan inside a table. RU: diapazon vnutri tablitsy.
pub async fn scan(env: &Env, table: &str, lo: &str, hi: Option<&str>, limit: usize, rev: bool) -> Result<Vec<(String, Value)>> {
    let m = meta_all(env, false).await?;
    let (tlo, thi) = range_of(table);
    let lo_k = if lo.is_empty() { tlo } else { key_of(table, lo) };
    let hi_k = hi.map(|h| key_of(table, h)).unwrap_or(thi);
    let mut idx: Vec<usize> = (0..m.chunks.len()).filter(|&i| m.chunks[i].max.as_str() >= lo_k.as_str() && m.chunks[i].min.as_str() < hi_k.as_str()).collect();
    if rev { idx.reverse(); }
    let mut out = Vec::new();
    for i in idx {
        if out.len() >= limit { break; }
        let (v, _) = load_chunk(env, &m.chunks[i]).await?;
        out.extend(v.range(&lo_k, Some(hi_k.as_str()), limit - out.len(), rev).map_err(Error::RustError)?.into_iter().map(|(k, x)| (strip(table, &k), x)));
    }
    Ok(out)
}

/// UZ: jadvalning HAMMA qatori. EN: all rows of a table. RU: vse stroki.
pub async fn all_rows(env: &Env, table: &str) -> Result<Vec<(String, Value)>> {
    scan(env, table, "", None, usize::MAX, false).await
}

struct Upload {
    no: u64,
    ver: u64,
    name: String,
    sealed: Vec<u8>,
    key_hex: String,
    min: String,
    max: String,
    rows: u32,
    plain: Vec<u8>,
}

async fn upload(env: &Env, u: Upload) -> Result<(ChunkMeta, Vec<u8>)> {
    let (file_id, msg_id) = crate::vd::tg_send_document(&token(env), channel(env), &u.name, "", &u.sealed).await?;
    let cm = ChunkMeta { no: u.no, ver: u.ver, name: u.name, file_id, msg_id, key: u.key_hex, min: u.min, max: u.max, rows: u.rows, size: u.sealed.len() as u64 };
    cache_put(&cache_name(&cm), &u.sealed).await;
    Ok((cm, u.plain))
}

/// UZ: bitta jadvalga yozish (to'qnashuvda qayta uriniladi). EN: write to one table, retried on conflict. RU: zapis v odnu tablitsu.
pub async fn commit(env: &Env, table: &str, ops: Vec<Op>) -> Result<()> {
    for _attempt in 0..4 {
        if commit_multi(env, vec![TableOps { table: table.to_string(), ops: ops.clone(), read: None }]).await? { return Ok(()); }
    }
    err("yozish: to'qnashuv (4 urinish)")
}

pub struct TableOps {
    pub table: String,
    /// UZ: kalitlar jadval ichidagi (prefikssiz). EN/RU: keys are table-local (no prefix).
    pub ops: Vec<Op>,
    /// UZ: o'qilgan bo'laklar nomlari (SQL ko'prigi): ular hali tirik bo'lishi shart, aks holda `false` (qayta hisoblash kerak). EN/RU: read-set chunk names.
    pub read: Option<Vec<String>>,
}

/// UZ: bir nechta jadvalga ATOMIK yozish: o'zgargan bo'laklar BITTA yuklashda (kichik jadvallar bitta bo'lakni baham ko'radi), keyin ro'yxat BITTA tranzaksiyada almashadi.
/// `false` — to'qnashuv (boshqa yozuvchi o'qilgan/o'zgargan bo'lakni almashtirgan). EN: atomic multi-table write; `false` on conflict. RU: atomarnaya zapis.
pub async fn commit_multi(env: &Env, items: Vec<TableOps>) -> Result<bool> {
    let mut read: Vec<String> = Vec::new();
    let mut ops: Vec<Op> = Vec::new();
    for it in items {
        if let Some(r) = it.read { for n in r { if !read.contains(&n) { read.push(n); } } }
        for op in it.ops {
            ops.push(match op { Op::Put(k, v) => Op::Put(key_of(&it.table, &k), v), Op::Del(k) => Op::Del(key_of(&it.table, &k)) });
        }
    }
    if ops.is_empty() { return Ok(true); }
    let m = meta_all(env, true).await?;
    // UZ: o'qilgan bo'laklardan biri allaqachon almashtirilgan bo'lsa — yuklamasdan to'xtaymiz. EN/RU: stale read-set -> stop before uploading.
    if read.iter().any(|n| !m.chunks.iter().any(|c| &c.name == n)) { return Ok(false); }
    let mut groups: BTreeMap<usize, Vec<Op>> = BTreeMap::new();
    for op in &ops {
        let k = match op { Op::Put(k, _) | Op::Del(k) => k };
        groups.entry(route(&m.chunks, k).unwrap_or(usize::MAX)).or_default().push(op.clone());
    }
    let mut remove: Vec<String> = Vec::new();
    let mut specs: Vec<Upload> = Vec::new();
    let mut next_no = m.next_no.max(1);
    for (idx, gops) in groups {
        let (base, old) = if idx == usize::MAX {
            (Rc::new(View::parse(kvfmt::encode(&[]).map_err(Error::RustError)?).map_err(Error::RustError)?), None)
        } else {
            (load_chunk(env, &m.chunks[idx]).await?.0, Some(&m.chunks[idx]))
        };
        let outs = base.apply(&gops, TARGET_CHUNK).map_err(Error::RustError)?;
        if let Some(o) = old { remove.push(o.name.clone()); }
        for (i, (plain, min, max)) in outs.into_iter().enumerate() {
            // UZ: birinchi qism eski raqamni saqlaydi (db_1_v1 -> db_1_v2), 1 MB dan oshib bo'linsa keyingi raqam (db_2_v1) ochiladi. EN/RU: split -> next number.
            let (no, ver) = match (i, old) {
                (0, Some(o)) => (o.no, o.ver + 1),
                _ => { let n = next_no; next_no += 1; (n, 1) }
            };
            let key_hex = crate::random_hex(64);
            let key = hex_key(&key_hex)?;
            let sealed = kvfmt::seal(&key, &plain).map_err(Error::RustError)?;
            let rows = View::parse(plain.clone()).map_err(Error::RustError)?.len() as u32;
            specs.push(Upload { no, ver, name: chunk_label(no, ver), sealed, key_hex, min, max, rows, plain });
        }
    }
    let results = futures_util::future::join_all(specs.into_iter().map(|u| upload(env, u))).await;
    let mut adds: Vec<ChunkMeta> = Vec::new();
    let mut views: Vec<(String, Vec<u8>)> = Vec::new();
    let mut failed: Option<Error> = None;
    for r in results {
        match r { Ok((cm, plain)) => { views.push((cache_name(&cm), plain)); adds.push(cm); } Err(e) => failed = Some(e) }
    }
    if let Some(e) = failed {
        let ids: Vec<i64> = adds.iter().map(|a| a.msg_id).collect();
        tg_delete_many(env, &ids).await;
        return Err(e);
    }
    let uploaded: Vec<i64> = adds.iter().map(|a| a.msg_id).collect();
    let committed = kvsb::commit(env, now_ms(), &read, &remove, &adds).await;
    // UZ: to'qnashuv yoki xato — yuklangan fayllar ro'yxatga kirmadi: DARHOL o'chiriladi (yetim fayl qolmasin). EN/RU: delete uploads that never made it into the registry.
    if !matches!(committed, Ok(CommitResult::Ok(_))) { tg_delete_many(env, &uploaded).await; }
    match committed? {
        CommitResult::Ok(list) => {
            METAS.with(|c| { c.borrow_mut().insert("*".to_string(), (list, now_ms())); });
            for (name, plain) in views {
                if let Ok(v) = View::parse(plain) { hot_put(&name, Rc::new(v)); }
            }
            Ok(true)
        }
        CommitResult::Conflict => Ok(false),
    }
}

/// UZ: bir nechta jadvalning to'liq surati BITTA ro'yxatdan; har bo'lak bir marta ochiladi. Qaytadi: o'qilgan bo'laklar nomlari (yozishda tekshiriladi) va har jadval qatorlari.
///     `fresh=false` — o'qish (keshlangan ro'yxat), `true` — yozishdan oldin. EN/RU: consistent snapshot of several tables.
pub async fn snapshot_tables(env: &Env, tables: &[String], fresh: bool) -> Result<(Vec<String>, Vec<Vec<(String, Value)>>)> {
    let m = meta_all(env, fresh).await?;
    let mut loaded: HashMap<usize, Rc<View>> = HashMap::new();
    let mut read: Vec<String> = Vec::new();
    let mut out = Vec::with_capacity(tables.len());
    for t in tables {
        let (lo, hi) = range_of(t);
        let mut rows = Vec::new();
        for (i, c) in m.chunks.iter().enumerate() {
            if c.max.as_str() < lo.as_str() || c.min.as_str() >= hi.as_str() { continue; }
            let v = match loaded.get(&i) {
                Some(v) => v.clone(),
                None => { let (v, _) = load_chunk(env, c).await?; loaded.insert(i, v.clone()); read.push(c.name.clone()); v }
            };
            rows.extend(v.range(&lo, Some(hi.as_str()), usize::MAX, false).map_err(Error::RustError)?.into_iter().map(|(k, x)| (strip(t, &k), x)));
        }
        out.push(rows);
    }
    Ok((read, out))
}

/// UZ: bir nechta xabarni BITTA chaqiruvda o'chiradi (`deleteMessages`, 100 tagacha) — ichki chaqiruvlarni tejaydi. Xato bo'lsa `false`. EN/RU: bulk delete.
pub async fn tg_delete_many(env: &Env, ids: &[i64]) -> bool {
    let ids: Vec<i64> = ids.iter().copied().filter(|i| *i != 0).collect();
    let mut ok = true;
    for batch in ids.chunks(100) {
        let r = crate::tg_api_tok(&token(env), "deleteMessages", json!({"chat_id": channel(env), "message_ids": batch})).await;
        if let Err(e) = r {
            let s = e.to_string();
            if !(s.contains("not found") || s.contains("can't be deleted")) { ok = false; }
        }
    }
    ok
}

/// UZ: almashtirilgan eski versiyalarni Telegram'dan o'chiradi (cron har 10 daqiqada). EN: deletes replaced versions. RU: udalyaet starye versii.
pub async fn gc(env: &Env) -> Result<Value> {
    let mut deleted = 0usize;
    let mut pending = 0i64;
    for _ in 0..10 {
        let due = kvsb::gc_take(env, now_ms()).await?;
        if due.is_empty() { break; }
        if !tg_delete_many(env, &due).await { return Ok(json!({"deleted": deleted, "error": "telegram"})); }
        pending = kvsb::gc_done(env, &due).await?;
        deleted += due.len();
    }
    Ok(json!({"deleted": deleted, "pending": pending}))
}

/// UZ: izolyat keshlarini tozalaydi (ro'yxat nusxasi, ochilgan bo'laklar, fayl yo'llari). EN/RU: clears isolate caches.
pub fn clear_caches() {
    HOT.with(|h| h.borrow_mut().clear());
    METAS.with(|m| m.borrow_mut().clear());
    PATHS.with(|p| p.borrow_mut().clear());
}

/// UZ: BAZANI TO'LIQ TOZALASH: Telegram'dagi hamma bo'lak xabarlari o'chiriladi, keyin ro'yxat jadvallari bo'shatiladi. Bir chaqiruvda cheklangan miqdor
///     ishlanadi (`done:false` bo'lsa qayta chaqiriladi). Jadvallar va fayllar keyin ilova ishlatilganda O'ZI qayta yaratiladi.
/// EN: FULL WIPE — deletes every chunk message in Telegram, then empties the registry tables; repeat while `done:false`. Everything is recreated on first use.
/// RU: POLNAYA OCHISTKA — udalyaet vse soobshcheniya chankov v Telegram, zatem ochishchaet reestr; povtoryat poka `done:false`.
pub async fn wipe(env: &Env) -> Result<Value> {
    let mut deleted = 0usize;
    for _ in 0..30 {
        let ids = kvsb::wipe_take(env, 100).await?;
        if ids.is_empty() {
            kvsb::wipe_done(env, &[], true).await?;
            clear_caches();
            return Ok(json!({"done": true, "deleted": deleted}));
        }
        if !tg_delete_many(env, &ids).await { return Ok(json!({"done": false, "deleted": deleted, "error": "telegram"})); }
        kvsb::wipe_done(env, &ids, false).await?;
        deleted += ids.len();
    }
    Ok(json!({"done": false, "deleted": deleted}))
}

/// UZ: bitta jadvalning hamma qatorini o'chiradi (yangi bo'laklar bilan; eski versiyalar GC navbatiga tushadi). EN/RU: deletes all rows of a table.
pub async fn drop_table(env: &Env, table: &str) -> Result<usize> {
    let rows = all_rows(env, table).await?;
    let n = rows.len();
    if n > 0 { commit(env, table, rows.into_iter().map(|(k, _)| Op::Del(k)).collect()).await?; }
    Ok(n)
}

/// UZ: `/__kv/selftest` — yozish, o'qish, tahrir, o'chirish, bo'linish, GC; faqat hisoblar (soat so'rov ichida to'xtab turadi).
/// EN: end-to-end self-test of the layer. RU: samoproverka sloya.
pub async fn selftest(env: &Env) -> Result<Value> {
    let table = format!("selftest{}", crate::random_hex(6));
    let mut steps: Vec<Value> = Vec::new();
    let mut rec = |name: &str, ok: bool, note: String| steps.push(json!({"step": name, "ok": ok, "note": note}));

    // 1) yozish: 300 qator x ~5 KB = ~1.5 MB -> kamida 2 bo'lak
    let pad = "x".repeat(5000);
    let ops: Vec<Op> = (0..300).map(|i| Op::Put(format!("{i:08}"), json!({"i": i, "pad": pad, "name": format!("Qism {i} ўзбек")}))).collect();
    let w = commit(env, &table, ops).await;
    rec("yozish 300 qator", w.is_ok(), format!("{:?}", w.as_ref().err().map(|e| e.to_string())));
    let all = meta_all(env, true).await?;
    let (tlo, thi) = range_of(&table);
    let m = TableMeta { rev: 0, next_no: all.next_no, chunks: all.chunks.iter().filter(|c| c.max.as_str() >= tlo.as_str() && c.min.as_str() < thi.as_str()).cloned().collect() };
    rec("bo'laklarga bo'linish", m.chunks.len() >= 2, format!("{} bo'lak: {}", m.chunks.len(), m.chunks.iter().map(|c| c.name.clone()).collect::<Vec<_>>().join(",")));
    let ordered = m.chunks.windows(2).all(|w| w[0].max < w[1].min);
    rec("kalit oraliqlari kesishmaydi", ordered, m.chunks.iter().map(|c| format!("{}:{}..{}", c.no, c.min, c.max)).collect::<Vec<_>>().join(" "));

    // 2) o'qish
    let g = get(env, &table, "00000123").await;
    rec("bitta yozuvni o'qish", matches!(&g, Ok(Some(v)) if v["i"] == json!(123)), format!("{:?}", g.as_ref().map(|v| v.is_some()).map_err(|e| e.to_string())));
    let miss = get(env, &table, "99999999").await;
    rec("yo'q kalit -> None", matches!(miss, Ok(None)), String::new());

    // 3) qidiruv oralig'i, bo'laklarni kesib o'tadi
    let s = scan(env, &table, "00000100", Some("00000200"), 1000, false).await;
    rec("oraliq [100,200)", matches!(&s, Ok(v) if v.len() == 100 && v[0].0 == "00000100" && v[99].0 == "00000199"), format!("{:?}", s.as_ref().map(|v| v.len()).map_err(|e| e.to_string())));
    let s2 = scan(env, &table, "", None, 3, true).await;
    rec("oxirgi 3 ta (teskari)", matches!(&s2, Ok(v) if v.len() == 3 && v[0].0 == "00000299"), String::new());

    // 4) tahrir va o'chirish
    let e = commit(env, &table, vec![Op::Put("00000007".into(), json!({"edited": true})), Op::Del("00000008".into())]).await;
    rec("tahrir + o'chirish", e.is_ok(), format!("{:?}", e.as_ref().err().map(|e| e.to_string())));
    let a = get(env, &table, "00000007").await;
    let b = get(env, &table, "00000008").await;
    rec("tahrir ko'rinadi, o'chirilgan yo'q", matches!(&a, Ok(Some(v)) if v["edited"] == json!(true)) && matches!(b, Ok(None)), String::new());
    let all = all_rows(env, &table).await;
    rec("jami qatorlar 299", matches!(&all, Ok(v) if v.len() == 299), format!("{:?}", all.as_ref().map(|v| v.len()).map_err(|e| e.to_string())));

    // 5) xotirasiz o'qish (kesh/Telegram'dan) — ko'rinishlar tozalanadi
    HOT.with(|h| h.borrow_mut().clear());
    let m2 = meta_all(env, true).await?;
    let (_, src) = load_chunk(env, &m2.chunks[0]).await?;
    rec("xotira tozalangach qayta ochish", true, format!("manba: {src}"));

    // 6) GC: eski versiyalar navbatda (due kelajakda), shuning uchun hozircha 0 ta o'chadi
    let g = gc(env).await;
    rec("GC chaqiruvi", g.is_ok(), format!("{:?}", g.as_ref().map(|v| v.to_string()).map_err(|e| e.to_string())));

    // tozalash: sinov jadvali va uning fayllari o'chiriladi / cleanup: the test table and its files are dropped / ochistka
    let dropped = drop_table(env, &table).await;
    rec("sinov jadvali o'chirildi", dropped.is_ok(), format!("{:?}", dropped.as_ref().map_err(|e| e.to_string())));
    let ok = steps.iter().all(|s| s["ok"] == json!(true));
    Ok(json!({"ok": ok, "table": table, "steps": steps}))
}

//! UZ: kvdb — Telegram bo'laklari ustidagi baza qatlami. Ro'yxat (DO) faqat nom/file_id/kalitni biladi; ma'lumot shifrlangan fayllarda.
//!     O'qish: worker xotirasi -> Cloudflare keshi -> Telegram. Yozish: bo'lakni ochish -> o'zgartirish -> qayta shifrlab yuklash -> ro'yxatda CAS.
//! EN: kvdb — the database layer over Telegram chunks. The registry (DO) knows only name/file_id/key; data lives in encrypted files.
//!     Read: isolate memory -> Cloudflare cache -> Telegram. Write: open chunk -> change -> re-encrypt + upload -> CAS in the registry.
//! RU: kvdb — sloy bazy nad chankami Telegram. Reestr (DO) znaet tolko imya/file_id/klyuch; dannye v shifrovannykh faylakh.

use crate::kvfmt::{self, Op, View, TARGET_CHUNK};
use crate::kvreg::{route, ChunkMeta, TableMeta};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use worker::*;

const HOT_MAX: usize = 48;

thread_local! {
    /// UZ: ochilgan bo'laklar (nom -> ko'rinish); nom o'zgarmas versiyali, shuning uchun eskirmaydi. EN: opened chunks by immutable name. RU: otkrytye chanki.
    static HOT: RefCell<HashMap<String, Rc<View>>> = RefCell::new(HashMap::new());
}

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

async fn reg(env: &Env, body: Value) -> Result<Value> {
    let stub = env.durable_object("KV_REG")?.get_by_name("reg")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post).with_body(Some(body.to_string().into()));
    let mut r = stub.fetch_with_request(Request::new_with_init("https://kv-reg.internal/", &init)?).await?;
    if r.status_code() != 200 { return err(format!("registry {}", r.status_code())); }
    r.json().await
}

pub async fn meta(env: &Env, table: &str) -> Result<TableMeta> {
    let v = reg(env, json!({"op": "get", "table": table})).await?;
    serde_json::from_value(v).map_err(|e| Error::RustError(format!("meta: {e}")))
}

async fn download(env: &Env, file_id: &str) -> Result<Vec<u8>> {
    let token = token(env);
    let f = crate::tg_api_tok(&token, "getFile", json!({"file_id": file_id})).await?;
    let path = f["file_path"].as_str().unwrap_or("").to_string();
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
    if let Some(v) = HOT.with(|h| h.borrow().get(&c.name).cloned()) { return Ok((v, "isolate")); }
    let key = hex_key(&c.key)?;
    let (sealed, src) = match Cache::default().get(&cache_req(&c.name)?, false).await? {
        Some(mut hit) => (hit.bytes().await?, "cache"),
        None => {
            let b = download(env, &c.file_id).await?;
            cache_put(&c.name, &b).await;
            (b, "telegram")
        }
    };
    let plain = kvfmt::open(&key, &sealed).map_err(Error::RustError)?;
    let v = Rc::new(View::parse(plain).map_err(Error::RustError)?);
    hot_put(&c.name, v.clone());
    Ok((v, src))
}

/// UZ: bitta yozuv — faqat kerakli bo'lak ochiladi. EN: one record, only its chunk is opened. RU: odna zapis.
pub async fn get(env: &Env, table: &str, key: &str) -> Result<Option<Value>> {
    let m = meta(env, table).await?;
    let Some(i) = route(&m.chunks, key) else { return Ok(None) };
    let (v, _) = load_chunk(env, &m.chunks[i]).await?;
    v.get(key).map_err(Error::RustError)
}

/// UZ: [lo, hi) oralig'i (bir nechta bo'lakni kesib o'tishi mumkin). EN: range scan across chunks. RU: diapazon po chankam.
pub async fn scan(env: &Env, table: &str, lo: &str, hi: Option<&str>, limit: usize, rev: bool) -> Result<Vec<(String, Value)>> {
    let m = meta(env, table).await?;
    let mut idx: Vec<usize> = (0..m.chunks.len())
        .filter(|&i| m.chunks[i].max.as_str() >= lo && hi.map_or(true, |h| m.chunks[i].min.as_str() < h))
        .collect();
    if rev { idx.reverse(); }
    let mut out = Vec::new();
    for i in idx {
        if out.len() >= limit { break; }
        let (v, _) = load_chunk(env, &m.chunks[i]).await?;
        out.extend(v.range(lo, hi, limit - out.len(), rev).map_err(Error::RustError)?);
    }
    Ok(out)
}

/// UZ: jadvalning HAMMA qatori (SQL ko'prigi uchun). EN: all rows (for the SQL bridge). RU: vse stroki.
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

async fn upload(env: &Env, table: &str, u: Upload) -> Result<(ChunkMeta, Vec<u8>)> {
    let (file_id, msg_id) = crate::vd::tg_send_document(&token(env), channel(env), &u.name, "", &u.sealed).await?;
    let _ = table;
    cache_put(&u.name, &u.sealed).await;
    Ok((ChunkMeta { no: u.no, ver: u.ver, name: u.name, file_id, msg_id, key: u.key_hex, min: u.min, max: u.max, rows: u.rows, size: u.sealed.len() as u64 }, u.plain))
}

/// UZ: amallarni yozadi (put/del). Bo'lak qayta yig'iladi, shifrlanadi, yuklanadi; ro'yxatda CAS — to'qnashuvda qayta uriniladi.
/// EN: applies ops. Chunk is rebuilt, encrypted, uploaded; CAS in the registry, retried on conflict. RU: primenyaet operatsii.
pub async fn commit(env: &Env, table: &str, ops: Vec<Op>) -> Result<()> {
    for _attempt in 0..4 {
        if commit_try(env, table, ops.clone(), None).await? { return Ok(()); }
    }
    err("yozish: ro'yxat to'qnashuvi (4 urinish)")
}

/// UZ: bitta urinish. `expect` berilsa va ro'yxat `rev` i boshqacha bo'lsa — `false` (ma'lumot o'zgarib ketgan, chaqiruvchi qayta hisoblashi kerak).
/// EN: one attempt; with `expect` a differing registry `rev` returns `false`. RU: odna popytka; pri neskhozhdenii `rev` — `false`.
pub async fn commit_try(env: &Env, table: &str, ops: Vec<Op>, expect: Option<u64>) -> Result<bool> {
    if ops.is_empty() { return Ok(true); }
    {
        let m = meta(env, table).await?;
        if let Some(e) = expect { if m.rev != e { return Ok(false); } }
        let mut groups: BTreeMap<usize, Vec<Op>> = BTreeMap::new();
        for op in &ops {
            let k = match op { Op::Put(k, _) | Op::Del(k) => k };
            groups.entry(route(&m.chunks, k).unwrap_or(usize::MAX)).or_default().push(op.clone());
        }
        let mut remove: Vec<u64> = Vec::new();
        let mut specs: Vec<Upload> = Vec::new();
        let mut next_no = m.next_no.max(1);
        for (idx, gops) in groups {
            let (base, old) = if idx == usize::MAX {
                (Rc::new(View::parse(kvfmt::encode(&[]).map_err(Error::RustError)?).map_err(Error::RustError)?), None)
            } else {
                (load_chunk(env, &m.chunks[idx]).await?.0, Some(&m.chunks[idx]))
            };
            let outs = base.apply(&gops, TARGET_CHUNK).map_err(Error::RustError)?;
            if let Some(o) = old { remove.push(o.no); }
            for (i, (plain, min, max)) in outs.into_iter().enumerate() {
                let (no, ver) = match (i, old) {
                    (0, Some(o)) => (o.no, o.ver + 1),
                    _ => { let n = next_no; next_no += 1; (n, 1) }
                };
                let key_hex = crate::random_hex(64);
                let key = hex_key(&key_hex)?;
                let sealed = kvfmt::seal(&key, &plain).map_err(Error::RustError)?;
                let rows = View::parse(plain.clone()).map_err(Error::RustError)?.len() as u32;
                specs.push(Upload { no, ver, name: kvfmt::chunk_name(table, no, ver), sealed, key_hex, min, max, rows, plain });
            }
        }
        let results = futures_util::future::join_all(specs.into_iter().map(|u| upload(env, table, u))).await;
        let mut add: Vec<ChunkMeta> = Vec::new();
        let mut views: Vec<(String, Vec<u8>)> = Vec::new();
        for r in results {
            let (cm, plain) = r?;
            views.push((cm.name.clone(), plain));
            add.push(cm);
        }
        let res = reg(env, json!({"op": "commit", "table": table, "rev": m.rev, "remove": remove, "add": add})).await?;
        if res["ok"] == json!(true) {
            for (name, plain) in views {
                if let Ok(v) = View::parse(plain) { hot_put(&name, Rc::new(v)); }
            }
            return Ok(true);
        }
        // UZ: to'qnashuv — boshqa worker yozib ulgurdi; yuklangan fayllar ro'yxatga kirmadi (yetim). EN/RU: conflict.
        Ok(false)
    }
}

/// UZ: jadvalning bir xil `rev` dagi to'liq surati (ro'yxat bir marta o'qiladi). EN: a consistent snapshot of a table. RU: snimok tablitsy.
pub async fn snapshot(env: &Env, table: &str) -> Result<(u64, Vec<(String, Value)>)> {
    let m = meta(env, table).await?;
    let mut out = Vec::new();
    for c in &m.chunks {
        let (v, _) = load_chunk(env, c).await?;
        out.extend(v.range("", None, usize::MAX, false).map_err(Error::RustError)?);
    }
    Ok((m.rev, out))
}

/// UZ: eski versiyalarni Telegram'dan o'chiradi. EN: deletes replaced versions from Telegram. RU: udalyaet starye versii.
pub async fn gc(env: &Env) -> Result<Value> {
    let r = reg(env, json!({"op": "gc_take"})).await?;
    let due: Vec<Value> = r["due"].as_array().cloned().unwrap_or_default();
    let mut done: Vec<i64> = Vec::new();
    for g in &due {
        let id = g["msg_id"].as_i64().unwrap_or(0);
        let res = crate::tg_api_tok(&token(env), "deleteMessage", json!({"chat_id": channel(env), "message_id": id})).await;
        let gone = match &res { Ok(_) => true, Err(e) => { let s = e.to_string(); s.contains("not found") || s.contains("can't be deleted") } };
        if gone { done.push(id); }
    }
    let left = reg(env, json!({"op": "gc_done", "ids": done})).await?;
    Ok(json!({"deleted": done.len(), "pending": left["pending"]}))
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
    let m = meta(env, &table).await?;
    rec("bo'laklarga bo'linish", m.chunks.len() >= 2, format!("{} bo'lak, rev {}", m.chunks.len(), m.rev));
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
    let m2 = meta(env, &table).await?;
    let (_, src) = load_chunk(env, &m2.chunks[0]).await?;
    rec("xotira tozalangach qayta ochish", true, format!("manba: {src}"));

    // 6) GC: eski versiyalar navbatda (due kelajakda), shuning uchun hozircha 0 ta o'chadi
    let g = gc(env).await;
    rec("GC chaqiruvi", g.is_ok(), format!("{:?}", g.as_ref().map(|v| v.to_string()).map_err(|e| e.to_string())));

    let ok = steps.iter().all(|s| s["ok"] == json!(true));
    Ok(json!({"ok": ok, "table": table, "steps": steps}))
}

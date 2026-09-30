// worker/src/packs.rs — EMOJI, GIF VA STIKER TO'PLAMLARI
//
// TALAB (foydalanuvchi): Telegram'ning premium emoji, GIF va stikerlari
// kuchsiz telefonlarda og'irlik qildi — ilova o'z tizimini yasasin;
// foydalanuvchilar o'zlari yasaganini yuklasin (Telegram'dagidek).
//
// ── NIMA QANDAY ISHLAYDI ───────────────────────────────────────
//
//   * TO'PLAM = bitta shifrlangan fayl (`pk_<id>_<versiya>.arp`, kanalda).
//     Fayl boshida to'liq ro'yxat (sarlavha) turadi, keyin elementlar
//     (`tool/encode/arupack.py` — format izohi shu yerda). Emoji, GIF va
//     stiker uchun ALOHIDA to'plamlar: turi to'plam yaratilganda tanlanadi.
//     Bitta fayl <= 1 GB, bitta element <= 5 MB.
//   * ELEMENT QO'SHISH: ilova rasmni shifrlab bot chatiga yuklaydi
//     (`pki_<hisob>_...`), bot uni kanalga ko'chiradi (`tg_user_media`);
//     ilova `SyncQueue` orqali `add` amalini yuboradi -> `pack_ops` da
//     `pending`. Admin ko'rib chiqadi (`admin/review`): tasdiqlansa
//     `approved`, aks holda `rejected` (+ sabab). Tasdiqlangan amallarni
//     GitHub Actions (`tool/encode/packs_run.py`, kodlash run'ining ichida)
//     to'plam fayliga qo'shadi va yangi versiyani yuklaydi.
//   * HAMMA YOZUV `POST /api/sync` (SyncQueue) orqali: `sync_stmts`. Turso
//     yozuvlari pul turadi — shu sabab to'plamning ichidagi elementlar
//     bazada EMAS, faylning sarlavhasida; bazada faqat to'plam qatori va
//     kutayotgan amallar.
//   * Fayl baytlari worker'dan o'tmaydi: ilova ularni `/api/tg/deliver`
//     orqali bot chatiga olib, mahalliy manbadan bo'lak-bo'lak o'qiydi.
//
// ── KETMA-KETLIK ───────────────────────────────────────────────
//
// Bir to'plamni bir vaqtda faqat BITTA run o'zgartiradi (`pack_db.runner`
// + `lease_until`). Run o'lib qolsa ijara 25 daqiqada tugaydi va keyingisi
// davom etadi. Run yangi faylni yuklab bo'lib, `finish` yubormay o'lib
// qolsa — kanalda bitta ortiqcha (bazada yo'q) post qoladi; bu zararsiz.

use super::*;

const KINDS: [&str; 3] = ["sticker", "emoji", "gif"];
const MAX_PACKS_PER_USER: i64 = 30;
const MAX_PENDING_PER_USER: i64 = 100;
const MAX_SUBS: i64 = 200;
const MAX_ITEM_BYTES: i64 = 5 * 1024 * 1024;
const TITLE_MAX: usize = 40;
const EMOJI_MAX: usize = 16;
const JOB_LEASE_MS: i64 = 25 * 60 * 1000;
const JOB_MAX_ATTEMPTS: i64 = 3;
const JOB_MAX_OPS: i64 = 40;
/// Bot kanalga ko'chirib ulgurmagan fayl shuncha kutiladi.
const STAGING_WAIT_MS: i64 = 2 * 60 * 60 * 1000;
/// Rad etilgan amallar egasiga shuncha vaqt ko'rsatiladi.
const REJECTED_SHOW_MS: i64 = 30 * 24 * 60 * 60 * 1000;
/// `pack_ops.reason` — to'plam o'chirilgani uchun bekor bo'lgan amallar
/// (egasiga ko'rsatilmaydi).
const REASON_DELETED: &str = "pack_deleted";

pub(crate) const DDL: [&str; 6] = [
    // `file` — joriy fayl nomi (`pk_<id>_<versiya>.arp`), bo'sh — hali
    // elementi yo'q. `runner` + `lease_until` — Actions ijarasi.
    "CREATE TABLE IF NOT EXISTS pack_db (
        id INTEGER PRIMARY KEY,
        owner_id INTEGER NOT NULL,
        kind TEXT NOT NULL,
        title TEXT NOT NULL,
        file TEXT DEFAULT '',
        version INTEGER DEFAULT 0,
        items INTEGER DEFAULT 0,
        bytes INTEGER DEFAULT 0,
        state TEXT DEFAULT 'active',
        runner TEXT DEFAULT '',
        lease_until INTEGER DEFAULT 0,
        attempts INTEGER DEFAULT 0,
        created_at INTEGER NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_pack_owner ON pack_db(owner_id, state)",
    "CREATE INDEX IF NOT EXISTS idx_pack_public ON pack_db(state, kind, created_at)",
    // Kutayotgan / tasdiqlangan / rad etilgan amallar. `op`: add | remove.
    // `state`: pending (admin ko'rishi kerak) | approved (Actions qo'shadi)
    // | rejected. Bajarilgan amal O'CHIRILADI.
    "CREATE TABLE IF NOT EXISTS pack_ops (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        pack_id INTEGER NOT NULL,
        owner_id INTEGER NOT NULL,
        op TEXT NOT NULL,
        file TEXT DEFAULT '',
        item_id INTEGER DEFAULT 0,
        emoji TEXT DEFAULT '',
        size INTEGER DEFAULT 0,
        state TEXT DEFAULT 'pending',
        reason TEXT DEFAULT '',
        created_at INTEGER NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_pack_ops_state ON pack_ops(state, pack_id)",
    "CREATE TABLE IF NOT EXISTS pack_subs (
        user_id INTEGER NOT NULL,
        pack_id INTEGER NOT NULL,
        PRIMARY KEY (user_id, pack_id)
    )",
];

// ── SOF YORDAMCHILAR (testlanadi) ───────────────────────────────

/// `pk_<to'plam>_<element>` -> (to'plam, element).
pub(crate) fn parse_ref(s: &str) -> Option<(i64, i64)> {
    let rest = s.strip_prefix("pk_")?;
    let (a, b) = rest.split_once('_')?;
    if a.is_empty() || b.is_empty() || a.len() > 16 || b.len() > 9 {
        return None;
    }
    if !a.bytes().all(|c| c.is_ascii_digit()) || !b.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let (p, i) = (a.parse::<i64>().ok()?, b.parse::<i64>().ok()?);
    if p > 0 && i > 0 { Some((p, i)) } else { None }
}

fn clean_title(s: &str) -> Option<String> {
    let t: String = s.trim().chars().filter(|c| !c.is_control()).take(TITLE_MAX).collect();
    let t = t.trim().to_string();
    if t.is_empty() { None } else { Some(t) }
}

fn clean_emoji(s: &str) -> String {
    s.trim().chars().filter(|c| !c.is_control() && *c != '[' && *c != ']')
        .take(EMOJI_MAX).collect()
}

/// Ilova yaratadigan to'plam raqami: tasodifiy 52 bit (JSON'da aniq).
fn valid_pack_id(id: i64) -> bool {
    id >= 1_000_000 && id < (1i64 << 53)
}

fn arg_i(v: i64) -> TursoArg { TursoArg::int(v) }
fn arg_s(v: &str) -> TursoArg { TursoArg::text(v) }

/// `SyncQueue` yuborgan `packs` amallari uchun buyruqlar.
///
/// Har bir buyruq O'ZI tekshiradi (egasi, chegaralar, takror) — yaroqsiz
/// amal jimgina tashlanadi va butun paketni yiqitmaydi. Qaytadi:
/// (buyruqlar, o'chirilgan to'plamlar, olib tashlash amallari bormi).
pub(crate) fn sync_stmts(
    me: i64,
    packs: &[Value],
    now: i64,
) -> (Vec<(&'static str, Vec<TursoArg>)>, Vec<i64>, bool) {
    let mut out: Vec<(&'static str, Vec<TursoArg>)> = Vec::new();
    let mut deleted = Vec::new();
    let mut removes = false;
    for p in packs {
        let pack = jint(p, "pack");
        match p["op"].as_str().unwrap_or("") {
            "new" => {
                let id = jint(p, "id");
                let kind = p["kind"].as_str().unwrap_or("");
                let Some(title) = clean_title(p["title"].as_str().unwrap_or("")) else { continue };
                if !valid_pack_id(id) || !KINDS.contains(&kind) { continue; }
                out.push((
                    "INSERT OR IGNORE INTO pack_db (id,owner_id,kind,title,created_at)
                     SELECT ?,?,?,?,?
                      WHERE (SELECT COUNT(*) FROM pack_db WHERE owner_id=? AND state='active') < ?",
                    vec![arg_i(id), arg_i(me), arg_s(kind), arg_s(&title), arg_i(now),
                         arg_i(me), arg_i(MAX_PACKS_PER_USER)],
                ));
            }
            "add" => {
                let file = p["file"].as_str().unwrap_or("").trim().to_string();
                let size = jint(p, "size");
                let prefix = format!("pki_{me}_");
                if pack <= 0 || !tg_safe_name(&file) || !file.starts_with(&prefix)
                    || size <= 0 || size > MAX_ITEM_BYTES { continue; }
                out.push((
                    "INSERT INTO pack_ops (pack_id,owner_id,op,file,emoji,size,state,created_at)
                     SELECT ?,?,'add',?,?,?,'pending',?
                      WHERE EXISTS (SELECT 1 FROM pack_db WHERE id=? AND owner_id=? AND state='active')
                        AND NOT EXISTS (SELECT 1 FROM pack_ops WHERE file=? AND op='add')
                        AND (SELECT COUNT(*) FROM pack_ops WHERE owner_id=? AND state='pending') < ?",
                    vec![arg_i(pack), arg_i(me), arg_s(&file), arg_s(&clean_emoji(p["emoji"].as_str().unwrap_or(""))),
                         arg_i(size), arg_i(now), arg_i(pack), arg_i(me), arg_s(&file),
                         arg_i(me), arg_i(MAX_PENDING_PER_USER)],
                ));
            }
            "remove" => {
                let item = jint(p, "item");
                if pack <= 0 || item <= 0 { continue; }
                removes = true;
                out.push((
                    "INSERT INTO pack_ops (pack_id,owner_id,op,item_id,state,created_at)
                     SELECT ?,?,'remove',?,'approved',?
                      WHERE EXISTS (SELECT 1 FROM pack_db WHERE id=? AND owner_id=? AND state='active')
                        AND NOT EXISTS (SELECT 1 FROM pack_ops
                                         WHERE pack_id=? AND op='remove' AND item_id=? AND state='approved')",
                    vec![arg_i(pack), arg_i(me), arg_i(item), arg_i(now),
                         arg_i(pack), arg_i(me), arg_i(pack), arg_i(item)],
                ));
            }
            "delete" => {
                if pack <= 0 { continue; }
                deleted.push(pack);
                out.push((
                    "UPDATE pack_ops SET state='rejected', reason=?
                      WHERE pack_id=? AND owner_id=? AND state IN ('pending','approved')",
                    vec![arg_s(REASON_DELETED), arg_i(pack), arg_i(me)],
                ));
                out.push((
                    "UPDATE pack_db SET state='deleted' WHERE id=? AND owner_id=? AND state='active'",
                    vec![arg_i(pack), arg_i(me)],
                ));
            }
            // Rad etilgan yuborishni egasi qo'lda tozalaydi (bitta `id` yoki
            // to'plamdagi hammasi). Faqat o'zining `rejected` yozuvlari.
            "clear" => {
                let id = jint(p, "id");
                if id <= 0 && pack <= 0 { continue; }
                out.push((
                    "DELETE FROM pack_ops WHERE owner_id=? AND state='rejected' AND op='add'
                        AND ((?>0 AND id=?) OR (?<=0 AND pack_id=?))",
                    vec![arg_i(me), arg_i(id), arg_i(id), arg_i(id), arg_i(pack)],
                ));
            }
            "sub" => {
                if pack <= 0 { continue; }
                if p["on"].as_bool().unwrap_or(true) {
                    out.push((
                        "INSERT OR IGNORE INTO pack_subs (user_id,pack_id)
                         SELECT ?,?
                          WHERE EXISTS (SELECT 1 FROM pack_db WHERE id=? AND state='active' AND items>0)
                            AND (SELECT COUNT(*) FROM pack_subs WHERE user_id=?) < ?",
                        vec![arg_i(me), arg_i(pack), arg_i(pack), arg_i(me), arg_i(MAX_SUBS)],
                    ));
                } else {
                    out.push((
                        "DELETE FROM pack_subs WHERE user_id=? AND pack_id=?",
                        vec![arg_i(me), arg_i(pack)],
                    ));
                }
            }
            _ => {}
        }
    }
    (out, deleted, removes)
}

/// 30 kundan oshgan rad etilgan yozuvlarni o'chiradi. Cron har 10 daqiqada
/// chaqiradi, lekin ish kuniga BIR marta (03:00-03:10 UTC) bajariladi —
/// Turso o'qishlari tejalsin.
pub(crate) async fn cleanup_rejected(env: &Env) {
    let now = now_ms();
    let in_day = (now / 60_000) % (24 * 60);
    if !(180..190).contains(&in_day) { return; }
    let _ = turso_batch(env, &[(
        "DELETE FROM pack_ops WHERE state='rejected' AND created_at<?",
        vec![arg_i(now - REJECTED_SHOW_MS)],
    )]).await;
}

// ── SYNC'DAN KEYIN ──────────────────────────────────────────────

/// O'chirilgan to'plamning kanaldagi fayllari va kutayotgan elementlari
/// o'chadi; olib tashlash amali bo'lsa Actions ishga tushadi.
pub(crate) async fn after_sync(env: &Env, me: i64, deleted: &[i64], removes: bool) {
    for id in deleted.iter().take(10) {
        forget_pack_files(env, *id, Some(me)).await;
    }
    if removes {
        let _ = kick(env).await;
    }
}

/// To'plam (o'chirilgan holatda) faylini va vaqtinchalik fayllarini kanaldan
/// o'chiradi. `owner` berilsa faqat shu odamniki.
async fn forget_pack_files(env: &Env, id: i64, owner: Option<i64>) {
    let Ok(res) = turso_many(env, &[
        ("SELECT file FROM pack_db WHERE id=? AND state='deleted' AND (?=0 OR owner_id=?)",
         vec![arg_i(id), arg_i(owner.unwrap_or(0)), arg_i(owner.unwrap_or(0))]),
        ("SELECT file FROM pack_ops WHERE pack_id=? AND op='add' AND file<>'' AND reason=?",
         vec![arg_i(id), arg_s(REASON_DELETED)]),
    ]).await else { return };
    let mut names: Vec<String> = Vec::new();
    for r in rows_of(&res[0]).iter().chain(rows_of(&res[1]).iter()) {
        if let Some(f) = r["file"].as_str() {
            if !f.is_empty() { names.push(f.to_string()); }
        }
    }
    for n in &names {
        tg_forget_file(env, n).await;
    }
    let _ = turso_batch(env, &[
        ("UPDATE pack_db SET file='' WHERE id=? AND state='deleted'", vec![arg_i(id)]),
        ("DELETE FROM pack_ops WHERE pack_id=? AND reason=?", vec![arg_i(id), arg_s(REASON_DELETED)]),
    ]).await;
}

/// Tasdiqlangan amali bor to'plamlar soni (Actions navbati).
///
/// FAQAT `approved`: admin ko'rib chiqmagan (`pending`) va rad etilgan
/// rasmlar navbatda TURADI, lekin Actions ularni ishlamaydi.
const PENDING_SQL: &str =
    "SELECT COUNT(DISTINCT o.pack_id) AS n FROM pack_ops o JOIN pack_db p ON p.id=o.pack_id
      WHERE o.state='approved' AND p.state='active'";

/// To'plamlar workflow'i (yangi akkauntdagi repoda; kodlash `encode.yml`
/// dan ALOHIDA).
const GH_PACKS_WORKFLOW: &str = "packs.yml";

/// Ishlanadigan to'plam bo'lsa va workflow ishlamayotgan bo'lsa — `packs.yml`
/// ni ishga tushiradi. Kerak: `GH_ACTIONS_TOKEN`, `GH_REPO`. Natija matni.
pub(crate) async fn kick(env: &Env) -> String {
    let n = match turso_exec(env, PENDING_SQL, vec![]).await {
        Ok(r) => first_row(&r).map(|r| jint(&r, "n")).unwrap_or(0),
        Err(_) => return "Navbatni o'qib bo'lmadi".into(),
    };
    if n == 0 {
        return "To'plam navbati bo'sh".into();
    }
    let repo = tg_secret(env, "GH_REPO");
    if !repo.contains('/') {
        return "GH_REPO o'rnatilmagan".into();
    }
    for st in ["queued", "in_progress", "waiting", "requested", "pending"] {
        match gh_api(env, Method::Get,
            &format!("/repos/{repo}/actions/workflows/{GH_PACKS_WORKFLOW}/runs?status={st}&per_page=1"), None).await {
            Ok((200, v)) if v["total_count"].as_i64().unwrap_or(0) > 0 => {
                return "To'plamlar workflow'i ishlayapti yoki kutmoqda".into();
            }
            Ok((200, _)) => {}
            Ok((code, _)) => return format!("GitHub javobi {code}"),
            Err(e) => return format!("Actions ishga tushmadi: {e}"),
        }
    }
    // Ikki so'rov bir vaqtda ikkita run ochmasin: ishga tushirish huquqi
    // bazada ATOMIK olinadi (oxirgisidan 3 daqiqa o'tmagan bo'lsa — tegmaydi).
    let now = now_ms();
    let claim = turso_exec(env,
        "INSERT INTO app_config (cfg_key,cfg_value) VALUES ('packs_kicked_at', ?)
         ON CONFLICT(cfg_key) DO UPDATE SET cfg_value=excluded.cfg_value
           WHERE CAST(app_config.cfg_value AS INTEGER) < ?
         RETURNING cfg_key",
        vec![arg_s(&now.to_string()), arg_i(now - 3 * 60 * 1000)]).await;
    if !claim.ok().and_then(|r| first_row(&r)).is_some() {
        return "To'plamlar workflow'i hozirgina ishga tushirilgan".into();
    }
    match gh_api(env, Method::Post,
        &format!("/repos/{repo}/actions/workflows/{GH_PACKS_WORKFLOW}/dispatches"),
        Some(json!({"ref": "main"}))).await {
        Ok((204, _)) => "To'plamlar workflow'i ishga tushirildi".into(),
        Ok((code, _)) => format!("Actions ishga tushmadi (GitHub {code})"),
        Err(e) => format!("Actions ishga tushmadi: {e}"),
    }
}

// ── XABARDA ISHLATISH ───────────────────────────────────────────

/// Xabar/izohga biriktiriladigan stiker yoki GIF haqiqatan mavjud
/// to'plamga tegishlimi (`media_type`: sticker | gif).
pub(crate) async fn valid_ref(env: &Env, media_file: &str, media_type: &str) -> bool {
    let Some((pack, _item)) = parse_ref(media_file) else { return false };
    let Ok(res) = turso_exec(env,
        "SELECT kind FROM pack_db WHERE id=? AND state='active' AND items>0",
        vec![arg_i(pack)]).await else { return false };
    first_row(&res)
        .and_then(|r| r["kind"].as_str().map(|k| k == media_type))
        .unwrap_or(false)
}

// ── ROUTER ──────────────────────────────────────────────────────

fn pack_json(o: &Value) -> Value {
    json!({
        "id": jint(o, "id"),
        "kind": o["kind"].as_str().unwrap_or(""),
        "title": o["title"].as_str().unwrap_or(""),
        "file": o["file"].as_str().unwrap_or(""),
        "version": jint(o, "version"),
        "items": jint(o, "items"),
        "bytes": jint(o, "bytes"),
        "owner_id": jint(o, "owner_id"),
    })
}

fn query_param(req: &Request, name: &str) -> String {
    req.url().ok()
        .and_then(|u| u.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.to_string()))
        .unwrap_or_default()
}

pub(crate) async fn route(mut req: Request, env: &Env, path: &str, method: Method) -> Result<Response> {
    // ── ACTIONS (maxfiy kalit bilan) ─────────────────────────
    if path.starts_with("/api/packs/job/") {
        if !encode_token_ok(&req, env) {
            return json_resp(&json!({"error": "unauthorized"}), 401);
        }
        if method != Method::Post {
            return err404("topilmadi");
        }
        let b: Value = req.json().await.unwrap_or(json!({}));
        return match path {
            "/api/packs/job/peek" => {
                let n = turso_exec(env, PENDING_SQL, vec![]).await.ok()
                    .and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(0);
                ok(json!({"pending": n}))
            }
            "/api/packs/job/claim" => job_claim(env, &b).await,
            "/api/packs/job/heartbeat" => job_heartbeat(env, &b).await,
            "/api/packs/job/finish" => job_finish(env, &b).await,
            _ => err404("topilmadi"),
        };
    }

    let Some(u) = session_user(env, &bearer(&req)).await? else {
        return json_resp(&json!({"error": "unauthorized"}), 401);
    };
    let me = u["id"].as_i64().unwrap_or(0);
    if me <= 0 {
        return json_resp(&json!({"error": "unauthorized"}), 401);
    }
    let now = now_ms();

    match (method, path) {
        // Mening to'plamlarim, obunalarim va kutayotgan amallarim — BITTA so'rov.
        (Method::Get, "/api/packs/library") => {
            let res = turso_many(env, &[
                ("SELECT id,owner_id,kind,title,file,version,items,bytes FROM pack_db
                   WHERE owner_id=? AND state='active' ORDER BY created_at DESC LIMIT 60",
                 vec![arg_i(me)]),
                ("SELECT p.id,p.owner_id,p.kind,p.title,p.file,p.version,p.items,p.bytes
                    FROM pack_subs s JOIN pack_db p ON p.id=s.pack_id
                   WHERE s.user_id=? AND p.state='active' AND p.items>0 AND p.owner_id<>?
                   ORDER BY p.title LIMIT 200",
                 vec![arg_i(me), arg_i(me)]),
                ("SELECT id,pack_id,op,file,item_id,emoji,size,state,reason,created_at FROM pack_ops
                   WHERE owner_id=? AND reason<>? AND (state IN ('pending','approved')
                         OR (state='rejected' AND created_at>?))
                   ORDER BY id DESC LIMIT 200",
                 vec![arg_i(me), arg_s(REASON_DELETED), arg_i(now - REJECTED_SHOW_MS)]),
            ]).await?;
            let ops: Vec<Value> = rows_of(&res[2]).iter().map(|o| json!({
                "id": jint(o, "id"), "pack": jint(o, "pack_id"),
                "op": o["op"].as_str().unwrap_or(""),
                "file": o["file"].as_str().unwrap_or(""),
                "item": jint(o, "item_id"),
                "emoji": o["emoji"].as_str().unwrap_or(""),
                "size": jint(o, "size"),
                "state": o["state"].as_str().unwrap_or(""),
                "reason": o["reason"].as_str().unwrap_or(""),
                "at": jint(o, "created_at"),
            })).collect();
            ok_nostore(json!({
                "mine": rows_of(&res[0]).iter().map(pack_json).collect::<Vec<_>>(),
                "subs": rows_of(&res[1]).iter().map(pack_json).collect::<Vec<_>>(),
                "ops": ops,
            }))
        }

        // Ommaviy to'plamlar (yangilaridan boshlab; `before` — sahifalash).
        (Method::Get, "/api/packs/public") => {
            let kind = query_param(&req, "kind");
            let before = query_param(&req, "before").parse::<i64>().unwrap_or(0);
            let mut sql = String::from(
                "SELECT p.id,p.owner_id,p.kind,p.title,p.file,p.version,p.items,p.bytes,p.created_at,
                        u.first_name AS owner_name,
                        (SELECT 1 FROM pack_subs s WHERE s.user_id=? AND s.pack_id=p.id) AS sub
                   FROM pack_db p LEFT JOIN users_db u ON u.id=p.owner_id
                  WHERE p.state='active' AND p.items>0");
            let mut args = vec![arg_i(me)];
            if KINDS.contains(&kind.as_str()) {
                sql.push_str(" AND p.kind=?");
                args.push(arg_s(&kind));
            }
            if before > 0 {
                sql.push_str(" AND p.created_at<?");
                args.push(arg_i(before));
            }
            sql.push_str(" ORDER BY p.created_at DESC LIMIT 30");
            let res = turso_exec(env, &sql, args).await?;
            let list: Vec<Value> = rows_of(&res).iter().map(|o| {
                let mut j = pack_json(o);
                j["owner_name"] = json!(o["owner_name"].as_str().unwrap_or(""));
                j["sub"] = json!(jint(o, "sub") == 1);
                j["created_at"] = json!(jint(o, "created_at"));
                j
            }).collect();
            ok_nostore(json!({"packs": list}))
        }

        // Xabarda uchragan to'plamlarning ma'lumoti (ko'ruvchi fayl nomini bilsin).
        (Method::Get, "/api/packs/info") => {
            let ids: Vec<i64> = query_param(&req, "ids").split(',')
                .filter_map(|s| s.trim().parse::<i64>().ok()).filter(|i| *i > 0).take(20).collect();
            if ids.is_empty() {
                return ok_nostore(json!({"packs": []}));
            }
            let sql = format!(
                "SELECT id,owner_id,kind,title,file,version,items,bytes FROM pack_db
                  WHERE state='active' AND id IN ({})", in_marks(ids.len()));
            let res = turso_exec(env, &sql, ids.iter().map(|i| arg_i(*i)).collect()).await?;
            ok_nostore(json!({"packs": rows_of(&res).iter().map(pack_json).collect::<Vec<_>>()}))
        }

        // ── ADMIN ────────────────────────────────────────────
        (Method::Get, "/api/packs/admin/pending") => {
            if !is_admin(&u) {
                return json_resp(&json!({"error": "forbidden"}), 403);
            }
            let res = turso_exec(env,
                "SELECT o.id,o.pack_id,o.file,o.emoji,o.size,o.created_at,o.owner_id,
                        p.kind,p.title,u.first_name,u.username
                   FROM pack_ops o JOIN pack_db p ON p.id=o.pack_id
                   LEFT JOIN users_db u ON u.id=o.owner_id
                  WHERE o.state='pending' AND o.op='add' AND p.state='active'
                  ORDER BY o.id ASC LIMIT 60", vec![]).await?;
            let list: Vec<Value> = rows_of(&res).iter().map(|o| json!({
                "id": jint(o, "id"), "pack": jint(o, "pack_id"),
                "file": o["file"].as_str().unwrap_or(""),
                "emoji": o["emoji"].as_str().unwrap_or(""),
                "size": jint(o, "size"), "at": jint(o, "created_at"),
                "owner_id": jint(o, "owner_id"),
                "owner": o["first_name"].as_str().filter(|s| !s.is_empty())
                    .or(o["username"].as_str()).unwrap_or(""),
                "kind": o["kind"].as_str().unwrap_or(""),
                "title": o["title"].as_str().unwrap_or(""),
            })).collect();
            ok_nostore(json!({"ops": list}))
        }

        (Method::Post, "/api/packs/admin/review") => {
            if !is_admin(&u) {
                return json_resp(&json!({"error": "forbidden"}), 403);
            }
            let b: Value = req.json().await.unwrap_or(json!({}));
            let ids: Vec<i64> = b["ids"].as_array().map(|a| a.iter().filter_map(|v| v.as_i64())
                .filter(|i| *i > 0).take(60).collect()).unwrap_or_default();
            if ids.is_empty() {
                return json_resp(&json!({"error": "ids"}), 400);
            }
            let approve = b["approve"].as_bool().unwrap_or(false);
            let reason: String = b["reason"].as_str().unwrap_or("").trim().chars().take(120).collect();
            let reason = if reason.is_empty() { "Admin tasdiqlamadi".to_string() } else { reason };
            let marks = in_marks(ids.len());
            let id_args = || ids.iter().map(|i| arg_i(*i)).collect::<Vec<_>>();
            if approve {
                let sql = format!(
                    "UPDATE pack_ops SET state='approved', reason=''
                      WHERE id IN ({marks}) AND state='pending' AND op='add'
                        AND pack_id IN (SELECT id FROM pack_db WHERE state='active')");
                turso_exec(env, &sql, id_args()).await?;
                let _ = kick(env).await;
                return ok_nostore(json!({"ok": true, "approved": ids.len()}));
            }
            let sql = format!("SELECT id,file FROM pack_ops WHERE id IN ({marks}) AND state='pending' AND op='add'");
            let files: Vec<String> = rows_of(&turso_exec(env, &sql, id_args()).await?).iter()
                .filter_map(|o| o["file"].as_str().map(String::from)).collect();
            let sql = format!(
                "UPDATE pack_ops SET state='rejected', reason=?
                  WHERE id IN ({marks}) AND state='pending' AND op='add'");
            let mut args = vec![arg_s(&reason)];
            args.extend(id_args());
            turso_exec(env, &sql, args).await?;
            for f in &files {
                tg_forget_file(env, f).await;
            }
            ok_nostore(json!({"ok": true, "rejected": ids.len()}))
        }

        // Admin istalgan to'plamni o'chira oladi (shikoyat, nomaqbul kontent).
        (Method::Post, "/api/packs/admin/delete") => {
            if !is_admin(&u) {
                return json_resp(&json!({"error": "forbidden"}), 403);
            }
            let b: Value = req.json().await.unwrap_or(json!({}));
            let id = jint(&b, "pack");
            if id <= 0 {
                return json_resp(&json!({"error": "pack"}), 400);
            }
            turso_batch(env, &[
                ("UPDATE pack_ops SET state='rejected', reason=? WHERE pack_id=? AND state IN ('pending','approved')",
                 vec![arg_s(REASON_DELETED), arg_i(id)]),
                ("UPDATE pack_db SET state='deleted' WHERE id=? AND state='active'", vec![arg_i(id)]),
            ]).await?;
            forget_pack_files(env, id, None).await;
            ok_nostore(json!({"ok": true}))
        }

        _ => err404("topilmadi"),
    }
}

// ── ACTIONS: navbat ─────────────────────────────────────────────

/// Ishni bo'shatadi (urinish sanalmaydi).
async fn release(env: &Env, pack: i64, runner: &str, refund: bool) {
    let sql = if refund {
        "UPDATE pack_db SET runner='', lease_until=0, attempts=MAX(attempts-1,0) WHERE id=? AND runner=?"
    } else {
        "UPDATE pack_db SET runner='', lease_until=0 WHERE id=? AND runner=?"
    };
    let _ = turso_exec(env, sql, vec![arg_i(pack), arg_s(runner)]).await;
}

async fn job_claim(env: &Env, b: &Value) -> Result<Response> {
    let runner = b["runner"].as_str().unwrap_or("").trim().to_string();
    if runner.is_empty() || runner.len() > 64 {
        return json_resp(&json!({"error": "runner"}), 400);
    }
    let now = now_ms();
    let mut skip: Vec<i64> = Vec::new();
    for _ in 0..8 {
        let sql = format!(
            "SELECT o.pack_id AS pid FROM pack_ops o JOIN pack_db p ON p.id=o.pack_id
              WHERE o.state='approved' AND p.state='active' AND p.lease_until<=?
                AND o.pack_id NOT IN ({})
              ORDER BY o.id ASC LIMIT 1", in_marks(skip.len()));
        let mut args = vec![arg_i(now)];
        args.extend(skip.iter().map(|i| arg_i(*i)));
        let Some(r) = first_row(&turso_exec(env, &sql, args).await?) else {
            return ok(json!({"none": true}));
        };
        let pid = jint(&r, "pid");
        let got = turso_exec(env,
            "UPDATE pack_db SET runner=?, lease_until=?, attempts=attempts+1
              WHERE id=? AND state='active' AND lease_until<=?
              RETURNING id,kind,title,file,version,attempts",
            vec![arg_s(&runner), arg_i(now + JOB_LEASE_MS), arg_i(pid), arg_i(now)]).await?;
        let Some(p) = first_row(&got) else {
            skip.push(pid);
            continue;
        };
        let title = p["title"].as_str().unwrap_or("").to_string();
        // Ko'p marta yiqilgan to'plam navbatni to'smasin.
        if jint(&p, "attempts") > JOB_MAX_ATTEMPTS {
            let _ = turso_batch(env, &[
                ("UPDATE pack_ops SET state='rejected', reason=? WHERE pack_id=? AND state='approved'",
                 vec![arg_s("Texnik xato: to'plamni yangilab bo'lmadi"), arg_i(pid)]),
                ("UPDATE pack_db SET runner='', lease_until=0, attempts=0 WHERE id=?", vec![arg_i(pid)]),
            ]).await;
            encode_notify(env, &format!(
                "\u{274C} To'plam #{pid} («{}») {JOB_MAX_ATTEMPTS} marta yangilanmadi — amallar rad etildi.",
                html_escape(&title))).await;
            skip.push(pid);
            continue;
        }
        let ops_res = turso_exec(env,
            "SELECT id,op,file,item_id,emoji,created_at FROM pack_ops
              WHERE pack_id=? AND state='approved' ORDER BY id ASC LIMIT ?",
            vec![arg_i(pid), arg_i(JOB_MAX_OPS)]).await?;
        let ops = rows_of(&ops_res);
        let cur_file = p["file"].as_str().unwrap_or("").to_string();
        let mut names: Vec<String> = ops.iter()
            .filter(|o| o["op"].as_str() == Some("add"))
            .filter_map(|o| o["file"].as_str().map(String::from)).collect();
        if !cur_file.is_empty() {
            names.push(cur_file.clone());
        }
        let mut found: std::collections::HashMap<String, (i64, String)> = std::collections::HashMap::new();
        if !names.is_empty() {
            let sql = format!("SELECT file_name,msg_id,file_key FROM tg_files WHERE file_name IN ({})", in_marks(names.len()));
            let res = turso_exec(env, &sql, names.iter().map(|n| arg_s(n)).collect()).await?;
            for o in rows_of(&res) {
                found.insert(o["file_name"].as_str().unwrap_or("").to_string(),
                    (jint(&o, "msg_id"), o["file_key"].as_str().unwrap_or("").to_string()));
            }
        }
        // Joriy fayl kanalda topilmasa — qayta yozib bo'lmaydi (elementlar
        // yo'qolardi): urinish sanaladi, bir necha marta keyin rad etiladi.
        let cur = if cur_file.is_empty() { None } else { found.get(&cur_file).cloned() };
        if !cur_file.is_empty() && !cur.as_ref().map(|c| c.0 > 0 && valid_file_key(&c.1)).unwrap_or(false) {
            release(env, pid, &runner, false).await;
            skip.push(pid);
            continue;
        }
        let mut out_ops: Vec<Value> = Vec::new();
        let mut lost: Vec<i64> = Vec::new();
        for o in &ops {
            let id = jint(o, "id");
            if o["op"].as_str() == Some("remove") {
                out_ops.push(json!({"id": id, "op": "remove", "item_id": jint(o, "item_id")}));
                continue;
            }
            let file = o["file"].as_str().unwrap_or("");
            match found.get(file) {
                Some((msg, key)) if *msg > 0 && valid_file_key(key) => out_ops.push(json!({
                    "id": id, "op": "add", "file": file, "msg_id": msg, "key": key,
                    "emoji": o["emoji"].as_str().unwrap_or(""),
                })),
                _ if now - jint(o, "created_at") > STAGING_WAIT_MS => lost.push(id),
                _ => {} // bot hali kanalga ko'chirmagan — keyingi safar
            }
        }
        if !lost.is_empty() {
            let sql = format!(
                "UPDATE pack_ops SET state='rejected', reason=? WHERE id IN ({}) AND state='approved'",
                in_marks(lost.len()));
            let mut args = vec![arg_s("Fayl kanalda topilmadi")];
            args.extend(lost.iter().map(|i| arg_i(*i)));
            let _ = turso_exec(env, &sql, args).await;
        }
        if out_ops.is_empty() {
            release(env, pid, &runner, true).await;
            skip.push(pid);
            continue;
        }
        let (msg_id, key) = cur.unwrap_or((0, String::new()));
        return ok(json!({
            "job": {
                "pack": {
                    "id": pid, "kind": p["kind"].as_str().unwrap_or(""), "title": title,
                    "file": cur_file, "version": jint(&p, "version"),
                    "msg_id": msg_id, "key": key,
                },
                "ops": out_ops,
            },
            "channel": tg_channel_id(env),
        }));
    }
    ok(json!({"none": true}))
}

async fn job_heartbeat(env: &Env, b: &Value) -> Result<Response> {
    let runner = b["runner"].as_str().unwrap_or("");
    let res = turso_exec(env,
        "UPDATE pack_db SET lease_until=? WHERE id=? AND runner=? RETURNING id",
        vec![arg_i(now_ms() + JOB_LEASE_MS), arg_i(jint(b, "pack")), arg_s(runner)]).await?;
    if first_row(&res).is_none() {
        return json_resp(&json!({"error": "job_lost"}), 409);
    }
    ok(json!({"ok": true}))
}

async fn job_finish(env: &Env, b: &Value) -> Result<Response> {
    let pid = jint(b, "pack");
    let runner = b["runner"].as_str().unwrap_or("").to_string();
    if pid <= 0 || runner.is_empty() {
        return json_resp(&json!({"error": "pack"}), 400);
    }
    let res = turso_exec(env,
        "SELECT state,file,version FROM pack_db WHERE id=? AND runner=?",
        vec![arg_i(pid), arg_s(&runner)]).await?;
    let Some(row) = first_row(&res) else {
        return json_resp(&json!({"error": "job_lost"}), 409);
    };
    let state = row["state"].as_str().unwrap_or("").to_string();
    let old_file = row["file"].as_str().unwrap_or("").to_string();
    let old_version = jint(&row, "version");

    if b["ok"].as_bool() != Some(true) {
        release(env, pid, &runner, false).await;
        return ok(json!({"ok": true, "retry": true}));
    }

    let changed = b["changed"].as_bool().unwrap_or(false);
    let new_file = b["file"].as_str().unwrap_or("").to_string();
    let key = b["key"].as_str().unwrap_or("").to_ascii_lowercase();
    let msg_id = jint(b, "msg_id");
    let version = jint(b, "version");
    if changed {
        if new_file != format!("pk_{pid}_{version}.arp") || version != old_version + 1
            || !valid_file_key(&key) || msg_id <= 0
        {
            return json_resp(&json!({"error": "fayl"}), 400);
        }
        // Kalit va xabar raqami DARHOL yoziladi (o'chirilgan to'plamda ham:
        // yuklangan fayl kanaldan tozalanishi uchun).
        turso_exec(env,
            "INSERT INTO tg_files (file_name, msg_id, file_key) VALUES (?, ?, ?)
             ON CONFLICT(file_name) DO UPDATE SET msg_id=excluded.msg_id, file_key=excluded.file_key",
            vec![arg_s(&new_file), arg_i(msg_id), arg_s(&key)]).await?;
    }
    if state != "active" {
        // To'plam ish paytida o'chirilgan — yangi fayl kerak emas.
        if changed {
            tg_forget_file(env, &new_file).await;
        }
        release(env, pid, &runner, false).await;
        return ok(json!({"ok": true, "deleted": true}));
    }

    // Natijalar: bajarilgan amal o'chadi, rad etilgani sabab bilan qoladi.
    let results = b["results"].as_array().cloned().unwrap_or_default();
    let ids: Vec<i64> = results.iter().map(|r| jint(r, "id")).filter(|i| *i > 0).take(200).collect();
    let mut staging: Vec<String> = Vec::new();
    if !ids.is_empty() {
        let sql = format!(
            "SELECT file FROM pack_ops WHERE pack_id=? AND op='add' AND file<>'' AND id IN ({})",
            in_marks(ids.len()));
        let mut args = vec![arg_i(pid)];
        args.extend(ids.iter().map(|i| arg_i(*i)));
        staging = rows_of(&turso_exec(env, &sql, args).await?).iter()
            .filter_map(|o| o["file"].as_str().map(String::from)).collect();
    }
    let mut stmts: Vec<(&str, Vec<TursoArg>)> = Vec::new();
    if changed {
        stmts.push((
            "UPDATE pack_db SET file=?, version=?, items=?, bytes=?, runner='', lease_until=0, attempts=0
              WHERE id=? AND runner=?",
            vec![arg_s(&new_file), arg_i(version), arg_i(jint(b, "items").max(0)),
                 arg_i(jint(b, "bytes").max(0)), arg_i(pid), arg_s(&runner)],
        ));
    } else {
        stmts.push((
            "UPDATE pack_db SET runner='', lease_until=0, attempts=0 WHERE id=? AND runner=?",
            vec![arg_i(pid), arg_s(&runner)],
        ));
    }
    for r in &results {
        let id = jint(r, "id");
        if id <= 0 { continue; }
        if r["ok"].as_bool() == Some(true) {
            stmts.push((
                "DELETE FROM pack_ops WHERE id=? AND pack_id=? AND state='approved'",
                vec![arg_i(id), arg_i(pid)],
            ));
        } else {
            let why: String = r["reason"].as_str().unwrap_or("Rad etildi").chars().take(200).collect();
            stmts.push((
                "UPDATE pack_ops SET state='rejected', reason=? WHERE id=? AND pack_id=? AND state='approved'",
                vec![arg_s(&why), arg_i(id), arg_i(pid)],
            ));
        }
    }
    // Eski rad etilgan yozuvlar (30 kundan keyin) tozalanadi — jadval o'smasin.
    stmts.push((
        "DELETE FROM pack_ops WHERE state='rejected' AND created_at<?",
        vec![arg_i(now_ms() - REJECTED_SHOW_MS)],
    ));
    turso_batch(env, &stmts).await?;

    if changed && !old_file.is_empty() && old_file != new_file {
        tg_forget_file(env, &old_file).await;
    }
    for f in &staging {
        tg_forget_file(env, f).await;
    }
    ok(json!({"ok": true}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn havola_tahlili() {
        assert_eq!(parse_ref("pk_123_7"), Some((123, 7)));
        assert_eq!(parse_ref("pk_123_"), None);
        assert_eq!(parse_ref("pk__7"), None);
        assert_eq!(parse_ref("pk_12a_7"), None);
        assert_eq!(parse_ref("pk_0_7"), None);
        assert_eq!(parse_ref("pk_5_0"), None);
        assert_eq!(parse_ref("xx_5_1"), None);
        assert_eq!(parse_ref("pk_5_1_2"), None);
        assert_eq!(parse_ref("pk_99999999999999999_1"), None);
    }

    #[test]
    fn nom_va_emoji_tozalanadi() {
        assert_eq!(clean_title("  Mushuk\u{0007}lar "), Some("Mushuklar".to_string()));
        assert_eq!(clean_title("   "), None);
        assert_eq!(clean_title(&"x".repeat(100)).unwrap().chars().count(), TITLE_MAX);
        assert_eq!(clean_emoji(" [😀] "), "😀");
    }

    fn op(v: Value) -> Vec<Value> { vec![v] }

    #[test]
    fn yaroqsiz_amallar_tashlanadi() {
        // Begona hisobning fayli, katta hajm, noto'g'ri tur, kichik raqam.
        let bad = vec![
            json!({"op": "add", "pack": 5, "file": "pki_2_a.bin", "size": 100}),
            json!({"op": "add", "pack": 5, "file": "pki_1_a.bin", "size": 6 * 1024 * 1024}),
            json!({"op": "add", "pack": 5, "file": "../pki_1_a", "size": 10}),
            json!({"op": "add", "pack": 0, "file": "pki_1_a.bin", "size": 10}),
            json!({"op": "new", "id": 5, "kind": "sticker", "title": "x"}),
            json!({"op": "new", "id": 5_000_000, "kind": "video", "title": "x"}),
            json!({"op": "new", "id": 5_000_000, "kind": "gif", "title": "  "}),
            json!({"op": "remove", "pack": 5, "item": 0}),
            json!({"op": "bilmadim"}),
        ];
        let (s, d, r) = sync_stmts(1, &bad, 10);
        assert!(s.is_empty() && d.is_empty() && !r);
    }

    #[test]
    fn yaroqli_amallar_buyruqqa_aylanadi() {
        let (s, d, r) = sync_stmts(1, &op(json!({"op": "new", "id": 5_000_000, "kind": "gif", "title": "G"})), 10);
        assert_eq!(s.len(), 1);
        assert!(d.is_empty() && !r);
        let (s, _, _) = sync_stmts(1, &op(json!({"op": "add", "pack": 5, "file": "pki_1_a.bin", "size": 100, "emoji": "😀"})), 10);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].1.len(), 11);
        let (s, d, r) = sync_stmts(1, &op(json!({"op": "remove", "pack": 5, "item": 3})), 10);
        assert_eq!((s.len(), d.len(), r), (1, 0, true));
        let (s, d, _) = sync_stmts(1, &op(json!({"op": "delete", "pack": 5})), 10);
        assert_eq!((s.len(), d), (2, vec![5]));
        let (s, _, _) = sync_stmts(1, &op(json!({"op": "sub", "pack": 5, "on": false})), 10);
        assert_eq!(s.len(), 1);
        let (s, _, _) = sync_stmts(1, &op(json!({"op": "clear", "pack": 5, "id": 9})), 10);
        assert_eq!(s.len(), 1);
        let (s, _, _) = sync_stmts(1, &op(json!({"op": "clear"})), 10);
        assert!(s.is_empty());
    }
}

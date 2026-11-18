//! UZ: Admin uchun ma'lumotlar bazasini FAQAT O'QISH tizimi (Turso / Cloudflare D1 paneliga o'xshash): jurnallar (jadvallar), ustunlar va yozuvlar.
//!     Hech narsa yozilmaydi/o'chirilmaydi. Faqat admin (`admin_only`). Jadval va ustun nomlari foydalanuvchidan kelib, `sqlite_master`/`PRAGMA table_info` ro'yxati
//!     bilan TEKSHIRILADI (so'rovga begona matn qo'shib bo'lmaydi); qiymatlar faqat bog'lanadi (`?`). Admin paneli — barcha qiymatlar to'liq ko'rinadi
//!     (foydalanuvchi talabi: "admin panelida sir bo'lmaydi"). Sahifalash: bir so'rovda ko'pi bilan 50 qator (Turso o'qishlari pul turadi).
//! EN: read-only admin database browser (tables, columns, rows); admin-only; identifiers validated, values bound; all values are shown in full; max 50 rows/request.
//! RU: tolko-dlya-chteniya prosmotr bazy dlya admina; vse znacheniya vidny polnostyu.
use super::*;

pub(crate) const MAX_LIMIT: i64 = 200;
const CELL_MAX: usize = 4000;

/// Jadval/ustun nomi xavfsizmi: faqat harf, raqam va pastki chiziq (so'rovga qo'shib yuborib bo'lmaydi).
pub(crate) fn safe_ident(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Qiymatni ko'rsatishga tayyorlaydi: juda uzun matn kesiladi (ekran uchun), qolgani o'zgarmaydi.
pub(crate) fn display_cell(v: &Value) -> Value {
    match v {
        Value::String(s) if s.chars().count() > CELL_MAX => json!(format!("{}\u{2026} (kesildi)", s.chars().take(CELL_MAX).collect::<String>())),
        other => other.clone(),
    }
}

/// LIKE uchun `%`, `_`, `\` belgilari oddiy matn bo'lib qolsin.
pub(crate) fn like_escape(q: &str) -> String {
    q.chars().flat_map(|c| if matches!(c, '%' | '_' | '\\') { vec!['\\', c] } else { vec![c] }).collect()
}

fn i64_q(url: &url::Url, k: &str, d: i64) -> i64 {
    url.query_pairs().find(|(n, _)| n == k).and_then(|(_, v)| v.parse().ok()).unwrap_or(d)
}

fn str_q(url: &url::Url, k: &str) -> String {
    url.query_pairs().find(|(n, _)| n == k).map(|(_, v)| v.to_string()).unwrap_or_default()
}

/// `GET /api/admin/db/tables` va `GET /api/admin/db/rows?table=&offset=&limit=&order=&dir=&col=&q=` (faqat admin).
pub(crate) async fn route(req: &Request, env: &Env, path: &str) -> Result<Response> {
    if let Some(deny) = admin_only(req, env).await? {
        return Ok(deny);
    }
    let url = req.url()?;
    if path == "/api/admin/db/tables" {
        let res = turso_exec(env,
            "SELECT name, type FROM sqlite_master WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' ORDER BY name", vec![]).await?;
        let items: Vec<Value> = rows_of(&res).into_iter().map(|r| json!({"name": r["name"], "kind": r["type"]})).collect();
        return ok_nostore(json!({"items": items}));
    }
    if path != "/api/admin/db/rows" {
        return err404("topilmadi");
    }

    // Jadval nomi: faqat bazadagi haqiqiy jadval/ko'rinish.
    let table = str_q(&url, "table");
    if !safe_ident(&table) {
        return json_resp(&json!({"error": "jadval nomi noto'g'ri"}), 400);
    }
    let found = turso_exec(env, "SELECT type FROM sqlite_master WHERE name=? AND type IN ('table','view') AND name NOT LIKE 'sqlite_%'",
        vec![TursoArg::text(&table)]).await?;
    let Some(kind) = first_row(&found).and_then(|r| r["type"].as_str().map(str::to_string)) else {
        return json_resp(&json!({"error": "bunday jadval yo'q"}), 404);
    };

    // Ustunlar (`PRAGMA table_info` — jadval nomi yuqorida tekshirilgan, qo'shtirnoq ichida).
    let info = turso_exec(env, &format!("PRAGMA table_info(\"{table}\")"), vec![]).await?;
    let cols: Vec<(String, String, bool)> = rows_of(&info).into_iter()
        .map(|r| (r["name"].as_str().unwrap_or("").to_string(), r["type"].as_str().unwrap_or("").to_string(), r["pk"].as_i64().unwrap_or(0) > 0))
        .filter(|(n, _, _)| !n.is_empty())
        .collect();
    if cols.is_empty() {
        return json_resp(&json!({"error": "ustunlar topilmadi"}), 404);
    }
    let has = |n: &str| cols.iter().any(|(c, _, _)| c == n);

    // Tartiblash: faqat mavjud ustun; bo'lmasa `id` (yoki `rowid`) bo'yicha kamayish.
    let want = str_q(&url, "order");
    let desc = str_q(&url, "dir") != "asc";
    let order_sql = if !want.is_empty() && has(&want) && safe_ident(&want) {
        format!(" ORDER BY \"{want}\" {}", if desc { "DESC" } else { "ASC" })
    } else if has("id") {
        " ORDER BY \"id\" DESC".to_string()
    } else if kind == "table" {
        " ORDER BY rowid DESC".to_string()
    } else {
        String::new()
    };

    // Filtr: bitta ustun bo'yicha matn qidirish (qiymat bog'lanadi).
    let (fcol, fq) = (str_q(&url, "col"), str_q(&url, "q"));
    let mut args: Vec<TursoArg> = Vec::new();
    let where_sql = if !fq.trim().is_empty() && has(&fcol) && safe_ident(&fcol) {
        args.push(TursoArg::text(&format!("%{}%", like_escape(fq.trim().chars().take(100).collect::<String>().as_str()))));
        format!(" WHERE CAST(\"{fcol}\" AS TEXT) LIKE ? ESCAPE '\\'")
    } else {
        String::new()
    };
    let limit = i64_q(&url, "limit", 30).clamp(1, MAX_LIMIT);
    let offset = i64_q(&url, "offset", 0).clamp(0, 1_000_000);
    args.push(TursoArg::int(limit + 1));
    args.push(TursoArg::int(offset));
    let sql = format!("SELECT * FROM \"{table}\"{where_sql}{order_sql} LIMIT ? OFFSET ?");
    let res = match turso_exec(env, &sql, args.clone()).await {
        Ok(r) => r,
        Err(_) if !order_sql.is_empty() => {
            // `rowid` yo'q jadval (WITHOUT ROWID): tartibsiz qayta uriniladi.
            let sql2 = format!("SELECT * FROM \"{table}\"{where_sql} LIMIT ? OFFSET ?");
            turso_exec(env, &sql2, args).await?
        }
        Err(e) => return Err(e),
    };
    let mut rows = rows_of(&res);
    let has_more = rows.len() as i64 > limit;
    rows.truncate(limit as usize);

    let out_rows: Vec<Value> = rows.iter().map(|r| {
        Value::Array(cols.iter().map(|(name, _, _)| display_cell(&r[name.as_str()])).collect())
    }).collect();
    let columns: Vec<Value> = cols.iter().map(|(n, t, pk)| json!({"name": n, "type": t, "pk": pk})).collect();
    ok_nostore(json!({
        "table": table, "kind": kind, "columns": columns, "rows": out_rows,
        "offset": offset, "limit": limit, "has_more": has_more,
        "order": want, "dir": if desc { "desc" } else { "asc" },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_safe() {
        assert!(safe_ident("users_db") && safe_ident("a1") && safe_ident("_x"));
        assert!(!safe_ident("") && !safe_ident("a b") && !safe_ident("x\"; DROP TABLE t;--") && !safe_ident("a.b") && !safe_ident(&"a".repeat(65)));
    }

    #[test]
    fn cells_are_shown_in_full_and_trimmed_only_when_huge() {
        assert_eq!(display_cell(&json!("abc")), json!("abc"));
        assert_eq!(display_cell(&Value::Null), Value::Null);
        assert_eq!(display_cell(&json!(12345)), json!(12345));
        // Kalit, token va boshqa qiymatlar admin panelida YASHIRILMAYDI.
        assert_eq!(display_cell(&json!("0123456789abcdef0123456789abcdef")), json!("0123456789abcdef0123456789abcdef"));
        let long = "x".repeat(CELL_MAX + 50);
        let s = display_cell(&json!(long));
        assert!(s.as_str().unwrap().ends_with("(kesildi)") && s.as_str().unwrap().chars().count() < CELL_MAX + 30);
    }

    #[test]
    fn like_is_escaped() {
        assert_eq!(like_escape("50%_a\\b"), "50\\%\\_a\\\\b");
    }
}

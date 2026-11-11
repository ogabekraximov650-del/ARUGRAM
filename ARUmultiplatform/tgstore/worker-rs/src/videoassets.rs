// worker/src/videoassets.rs — "Ilova haqida video" (tool/video) uchun namunaviy poster va qism ma'lumoti.
//
// Faqat GitHub Actions chaqiradi (`ENCODE_TOKEN` sarlavhasi, `encode_token_ok`). Faqat O'QIYDI: hech narsa yozmaydi.
// Javob: eng ko'p ko'rilgan bo'limlarning posterlari (kanaldagi xabar raqami + AES-128-CTR kaliti) va bitta kichik
// sifatli tayyor qism. Fayl baytlari worker orqali O'TMAYDI — Actions ularni Telegram'dan o'zi oladi va ochadi.

use super::*;

/// Namunaviy qism uchun eng katta hajm (Actions tez yuklab olsin).
const MAX_EPISODE_BYTES: i64 = 400 * 1024 * 1024;

async fn tg_file(env: &Env, file: &str) -> Option<(i64, String)> {
    if file.is_empty() {
        return None;
    }
    let r = turso_exec(env, "SELECT msg_id, file_key FROM tg_files WHERE file_name=?", vec![TursoArg::text(file)]).await.ok()?;
    let row = first_row(&r)?;
    let key = row["file_key"].as_str().unwrap_or("").to_ascii_lowercase();
    let id = jint(&row, "msg_id");
    if id > 0 && valid_file_key(&key) { Some((id, key)) } else { None }
}

pub(crate) async fn route(req: Request, env: &Env, path: &str, method: Method) -> Result<Response> {
    if method != Method::Get || path != "/api/video/assets" {
        return err404("topilmadi");
    }
    if !encode_token_ok(&req, env) {
        return json_resp(&json!({"error": "unauthorized"}), 401);
    }
    let r = turso_exec(env,
        "SELECT s.anime_id AS aid, s.season_id AS sid, s.bolim_id AS bolim, s.nomi AS nomi, s.photo_url AS photo,
                s.epizod_count AS eps, s.views_total AS views, s.rating_sum AS rsum, s.rating_count AS rcnt, a.name AS anime_name
           FROM season_db s LEFT JOIN anime_db a ON a.id = s.anime_id
          WHERE s.photo_url <> '' ORDER BY s.views_total DESC LIMIT 16", vec![]).await?;
    let mut posters: Vec<Value> = Vec::new();
    let mut seasons: Vec<(i64, i64, String)> = Vec::new();
    for row in rows_of(&r) {
        let file = bare_name(row["photo"].as_str().unwrap_or(""));
        let Some((msg_id, key)) = tg_file(env, &file).await else { continue };
        let anime = row["anime_name"].as_str().unwrap_or("").to_string();
        let name = if anime.is_empty() { row["nomi"].as_str().unwrap_or("").to_string() } else { anime };
        posters.push(json!({
            "name": &name, "section": jint(&row, "bolim"), "episodes": jint(&row, "eps"), "views": jint(&row, "views"),
            "rating_sum": jint(&row, "rsum"), "rating_count": jint(&row, "rcnt"),
            "file": file, "msg_id": msg_id, "key": key,
        }));
        seasons.push((jint(&row, "aid"), jint(&row, "sid"), name));
        if posters.len() >= 8 {
            break;
        }
    }

    // Namunaviy qism: poster bo'limlaridan birinchisi, eng kichik sifatda tayyor bo'lgan eng birinchi qism.
    let mut episode = Value::Null;
    'seasons: for (aid, sid, name) in &seasons {
        let e = turso_exec(env,
            "SELECT epizod_number AS num, epizod_name AS ename,
                    url_360p, size_360p, url_480p, size_480p
               FROM epizod_db WHERE anime_id=? AND season_id=? ORDER BY epizod_number ASC LIMIT 6",
            vec![TursoArg::int(*aid), TursoArg::int(*sid)]).await?;
        for ep in rows_of(&e) {
            for q in ["360p", "480p"] {
                let file = bare_name(ep[format!("url_{q}").as_str()].as_str().unwrap_or(""));
                let size = jint(&ep, &format!("size_{q}"));
                if file.is_empty() || (size > 0 && size > MAX_EPISODE_BYTES) {
                    continue;
                }
                if let Some((msg_id, key)) = tg_file(env, &file).await {
                    episode = json!({
                        "anime": name, "number": jint(&ep, "num"), "title": ep["ename"], "quality": q,
                        "file": file, "msg_id": msg_id, "key": key, "size": size,
                    });
                    break 'seasons;
                }
            }
        }
    }
    ok(json!({"channel": tg_channel_id(env), "posters": posters, "episode": episode}))
}

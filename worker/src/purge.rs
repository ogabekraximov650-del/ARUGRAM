// ══════════════════════════════════════════════════════════════
//  BIR MARTALIK TOZALASH: bo'limdagi qismlar oralig'ini bazadan VA kanaldan o'chirish
// ══════════════════════════════════════════════════════════════
//
// UZ: `app_config.ep_purge` = `<anime>/<bo'lim>/<dan>/<gacha>` (ilovadagi qism RAQAMLARI, ikkalasi ham kiradi) qo'yilsa, cron har safar
//     (har 10 daqiqada) shu oraliqdagi qismlarni admin "qismni o'chirish" yo'li bilan BIR XIL tozalaydi: baza (qism, kodlash navbati, tomosha tarixi,
//     kadrlar navbati, hisoblagich) AVVAL, so'ng kanal fayllari (sifatlar, asl video, kadrlar) ish yurituvchiga (`job_enqueue_cleanup`) topshiriladi.
//     Bir tikda 5 tadan ko'p qism olinmaydi (Cloudflare so'rov chegarasi). Qism qolmagach belgi o'chadi, Anibla navbatidagi shu anime ishlari
//     (`ep_purge_q` = nomdan bir bo'lak, masalan "One Piece") tozalanadi va admin'ga xabar ketadi. Belgi bo'lmasa HECH NARSA qilinmaydi.
// EN: A one-time range purge driven by `app_config.ep_purge`; reuses the admin episode-delete path (DB first, then channel files via the cleanup job).

use super::*;

const PER_TICK: i64 = 5;

pub(crate) async fn tick(env: &Env) {
    let Some(m) = config_get(env, "ep_purge").await.filter(|v| !v.trim().is_empty()) else { return };
    let p: Vec<i64> = m.trim().split('/').filter_map(|x| x.parse().ok()).collect();
    if p.len() != 4 || p[2] <= 0 || p[3] < p[2] {
        console_log!("ep_purge noto'g'ri belgi: {m}");
        return;
    }
    let (a, s, lo, hi) = (p[0], p[1], p[2], p[3]);
    let res = turso_exec(env,
        "SELECT url_360p,url_480p,url_720p,url_1080p,origin_video, anime_id, season_id, epizod_id, epizod_number, collage_msg,
                q360_vd_msgs,q480_vd_msgs,q720_vd_msgs,q1080_vd_msgs
           FROM epizod_db WHERE anime_id=? AND season_id=? AND epizod_number BETWEEN ? AND ? ORDER BY epizod_number LIMIT ?",
        vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(lo), TursoArg::int(hi), TursoArg::int(PER_TICK)]).await;
    let Ok(res) = res else { return };
    let rows = rows_of(&res);
    if rows.is_empty() {
        // Hammasi o'chdi: belgini olib tashlaymiz, shu anime'ning kutayotgan Anibla ishlarini ham.
        config_put(env, "ep_purge", "").await;
        let mut dropped = 0usize;
        if let Some(like) = config_get(env, "ep_purge_q").await.filter(|v| !v.trim().is_empty()) {
            if let Ok(r) = turso_exec(env, "DELETE FROM anibla_jobs WHERE LOWER(caption) LIKE ? RETURNING id",
                vec![TursoArg::text(&format!("%{}%", like.trim().to_lowercase()))]).await {
                dropped = rows_of(&r).len();
            }
            config_put(env, "ep_purge_q", "").await;
        }
        free_seasons_forget().await;
        encode_notify(env, &format!(
            "\u{1F9F9} Tozalash tugadi: anime {a}, bo'lim {s}, {lo}\u{2013}{hi}-qismlar bazadan o'chirildi (kanal fayllari ish yurituvchi orqali o'chmoqda). \
             Anibla navbatidan olib tashlangan ishlar: {dropped}.")).await;
        return;
    }
    let mut gone = Vec::new();
    for row in &rows {
        let eid = jint(row, "epizod_id");
        let pk = || vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(eid)];
        let db = turso_batch(env, &[
            ("DELETE FROM epizod_db WHERE anime_id=? AND season_id=? AND epizod_id=?", pk()),
            ("DELETE FROM encode_jobs WHERE anime_id=? AND season_id=? AND epizod_id=?", pk()),
            ("DELETE FROM encode_q WHERE anime_id=? AND season_id=? AND epizod_id=?", pk()),
            ("DELETE FROM watch_history_db WHERE anime_id=? AND season_id=? AND epizod_id=?", pk()),
            ("DELETE FROM frames_q WHERE anime_id=? AND season_id=? AND epizod_id=?", pk()),
            ("DELETE FROM announced WHERE anime_id=? AND season_id=? AND epizod_id=?", pk()),
            ("UPDATE season_db SET epizod_count=(SELECT COUNT(*) FROM epizod_db WHERE anime_id=? AND season_id=?) WHERE anime_id=? AND season_id=?",
             vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(a), TursoArg::int(s)]),
        ]).await;
        if let Err(e) = db {
            console_log!("ep_purge: {eid} o'chmadi: {e}");
            continue;
        }
        let mut items = cleanup_items(&[row.clone()], &[("url_360p", "q360_vd_msgs"), ("url_480p", "q480_vd_msgs"), ("url_720p", "q720_vd_msgs"), ("url_1080p", "q1080_vd_msgs")]);
        let ov = row["origin_video"].as_str().unwrap_or("");
        if ov.starts_with("orig_") {
            items.push(json!({"n": ov, "m": ""}));
        }
        items.extend(collage_items(&[row.clone()]));
        job_enqueue_cleanup(env, items).await;
        gone.push(jint(row, "epizod_number"));
    }
    free_seasons_forget().await;
    console_log!("ep_purge: o'chirildi {gone:?}");
}

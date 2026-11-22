SELECT (SELECT COUNT(*) FROM tg_files) AS tg, (SELECT COUNT(*) FROM vd_queue) AS vdq, (SELECT COUNT(*) FROM vd_files) AS vdf, (SELECT COUNT(*) FROM epizod_db) AS ep, (SELECT COUNT(*) FROM anime_db) AS an, (SELECT COUNT(*) FROM season_db) AS se, (SELECT COUNT(*) FROM users_db) AS us;
SELECT anime_id, season_id, photo_url, photo_vd_chunks, length(photo_vd_ids) AS ids_len, length(photo_vd_key) AS key_len FROM season_db ORDER BY season_id DESC LIMIT 5;
SELECT name, state, size, chunks, length(chunk_ids) AS ids_len, vkey IS NOT NULL AS has_key FROM vd_queue ORDER BY created DESC LIMIT 8;

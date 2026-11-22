SELECT anime_id, season_id, photo_url, photo_vd_chunks, length(photo_vd_ids) AS ids_len, length(photo_vd_key) AS key_len FROM season_db ORDER BY season_id DESC LIMIT 12;
SELECT id, photo_url, photo_vd_chunks, length(photo_vd_ids) AS ids_len FROM anime_db ORDER BY id DESC LIMIT 12;
SELECT name, state, size, chunks, length(chunk_ids) AS ids_len, datetime(created/1000+18000,'unixepoch') AS t FROM vd_queue ORDER BY created DESC LIMIT 15;

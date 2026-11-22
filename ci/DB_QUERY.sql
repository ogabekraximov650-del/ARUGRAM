SELECT anime_id, season_id, epizod_id, datetime(at/1000+18000,'unixepoch') AS e_lon_vaqti FROM announced ORDER BY at DESC LIMIT 12;
SELECT anime_id, season_id, epizod_id, epizod_number, collage_msg, length(collage_file_id) AS fid_len, length(collage_key) AS key_len, datetime(ready_at/1000+18000,'unixepoch') AS tayyor FROM epizod_db WHERE ready_at > 0 ORDER BY ready_at DESC LIMIT 12;
SELECT anime_id, season_id, epizod_id, quality, state, attempts, substr(error,1,150) AS error, datetime(queued_at/1000+18000,'unixepoch') AS navbat FROM encode_q ORDER BY queued_at DESC LIMIT 24;
SELECT anime_id, season_id, epizod_id, state, attempts, substr(error,1,120) AS error, substr(done,1,40) AS done FROM encode_jobs ORDER BY queued_at DESC LIMIT 12;

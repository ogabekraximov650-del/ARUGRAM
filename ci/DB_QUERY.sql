SELECT anime_id,season_id,epizod_id,state,attempts,substr(error,1,120) AS error,done,datetime(queued_at/1000+18000,'unixepoch') AS qoshilgan,runner,lease_until FROM encode_jobs ORDER BY queued_at;
SELECT anime_id,season_id,epizod_id,quality,state,attempts,substr(error,1,140) AS error FROM encode_q ORDER BY anime_id,epizod_id,quality;
SELECT anime_id,season_id,epizod_id,epizod_number,origin_size,origin_height,substr(origin_video,1,30) AS origin FROM epizod_db ORDER BY created_at DESC LIMIT 12;

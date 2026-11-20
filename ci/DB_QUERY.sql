SELECT anime_id,season_id,epizod_id,state,attempts,done,substr(error,1,150) AS error,substr(origin,1,45) AS origin,datetime(queued_at/1000+18000,'unixepoch') AS qoshilgan FROM encode_jobs ORDER BY queued_at; -- 1795187903
SELECT anime_id,epizod_id,quality,state,attempts,substr(error,1,120) AS error FROM encode_q ORDER BY epizod_id,quality;
SELECT epizod_id,epizod_number,origin_size,substr(origin_video,1,45) AS origin_video FROM epizod_db WHERE anime_id=4;
SELECT file_name,msg_id FROM tg_files WHERE file_name LIKE 'orig%' ORDER BY msg_id DESC LIMIT 12;
SELECT id,kind,state,payload FROM async_jobs ORDER BY id DESC LIMIT 6;

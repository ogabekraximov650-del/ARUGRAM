SELECT file_name,msg_id FROM tg_files WHERE file_name LIKE 'orig%';
SELECT anime_id,season_id,epizod_id,quality,state,attempts,substr(error,1,160) AS error FROM encode_q;
SELECT anime_id,season_id,epizod_id,origin,state,done,attempts,substr(error,1,160) AS error FROM encode_jobs;
SELECT anime_id,season_id,epizod_id,epizod_number,origin_video,origin_size,url_360p,url_480p,url_720p,url_1080p FROM epizod_db;
SELECT id,kind,state,substr(payload,1,160) AS payload FROM async_jobs ORDER BY id DESC LIMIT 15;

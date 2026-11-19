SELECT anime_id,season_id,epizod_id,epizod_number,origin_video,origin_size,origin_height,url_360p,url_480p,url_720p,url_1080p,ready_at FROM epizod_db WHERE anime_id=4;
SELECT * FROM encode_jobs;
SELECT * FROM encode_q;
SELECT file_name,msg_id FROM tg_files WHERE file_name LIKE 'orig%';
SELECT cfg_key,substr(cfg_value,1,120) AS v FROM app_config WHERE cfg_key LIKE 'anibla%' OR cfg_key LIKE 'encode%';

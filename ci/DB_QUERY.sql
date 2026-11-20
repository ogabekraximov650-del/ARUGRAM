SELECT id,kind,state,attempts,substr(error,1,140) AS error,datetime(created_at/1000+18000,'unixepoch') AS yaratilgan FROM async_jobs ORDER BY id DESC LIMIT 15;
SELECT name,state,substr(msg_ids,1,40) AS msgs,datetime(created_at/1000+18000,'unixepoch') AS t FROM vd_queue ORDER BY created_at DESC LIMIT 15;
SELECT file_name,msg_id FROM tg_files ORDER BY rowid DESC LIMIT 12;
SELECT COUNT(*) AS epizod FROM epizod_db;

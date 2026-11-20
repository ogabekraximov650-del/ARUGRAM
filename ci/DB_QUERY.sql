SELECT COUNT(*) AS epizodlar FROM epizod_db;
SELECT COUNT(*) AS encode_jobs FROM encode_jobs;
SELECT COUNT(*) AS orig_tg_files FROM tg_files WHERE file_name LIKE 'orig%';
SELECT (SELECT COUNT(*) FROM anime_db) AS animelar, (SELECT COUNT(*) FROM season_db) AS bolimlar, (SELECT COUNT(*) FROM watch_history_db) AS tarix;

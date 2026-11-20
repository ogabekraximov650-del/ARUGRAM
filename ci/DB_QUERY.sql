SELECT substr(file_name,1,5) AS pre, COUNT(*) AS n FROM tg_files GROUP BY pre ORDER BY n DESC LIMIT 15;
SELECT substr(name,1,5) AS pre, COUNT(*) AS n FROM vd_queue GROUP BY pre ORDER BY n DESC LIMIT 15;
SELECT COUNT(*) AS epizodlar FROM epizod_db;
SELECT COUNT(*) AS q_jobs FROM encode_jobs;
SELECT COUNT(*) AS anibla_jobs FROM anibla_jobs;

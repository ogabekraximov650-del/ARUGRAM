SELECT name FROM sqlite_master WHERE type='table' ORDER BY name;
SELECT id,state,attempts,substr(error,1,160) AS error,runner,lease_until,datetime(created_at/1000+18000,'unixepoch') AS yaratilgan FROM anibla_jobs ORDER BY id DESC LIMIT 20;

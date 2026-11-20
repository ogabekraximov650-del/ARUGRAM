SELECT id,state,attempts,runner,lease_until,substr(error,1,200) AS error,substr(caption,1,60) AS caption FROM anibla_jobs ORDER BY id DESC LIMIT 10;
SELECT * FROM anibla_jobs ORDER BY id DESC LIMIT 1;

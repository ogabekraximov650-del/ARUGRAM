SELECT id,state,attempts,runner,datetime(lease_until/1000+18000,'unixepoch') AS ijara,substr(error,1,200) AS error FROM anibla_jobs ORDER BY id;
# 1795176149

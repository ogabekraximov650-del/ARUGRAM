SELECT cfg_key, length(cfg_value) AS len, substr(cfg_value, 1, 300) AS qiymat FROM app_config WHERE cfg_key LIKE 'sess%' OR cfg_key LIKE '%pool%';

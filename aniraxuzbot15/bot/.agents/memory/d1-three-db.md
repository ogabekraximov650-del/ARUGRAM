---
name: D1 3-baza arxitekturasi
description: Cloudflare D1 uch bazaga bo'lish qarorlari va routing qoidasi
---

## Baza bo'lish
- DB_ANIME (fda65bb9): anime_db, season_db, epizod_db
- DB_STAT  (f9179d01): stat_downloads, stat_like, stat_dislike, stat_save, stat_coment, stat_share
- DB_MAIN  (6a9aa9e4): user_db, sessions, bot_settings, post_queue, chanel_info_db, request_chanel_db, tashqi_link_db, kanstruktor_info_db

## Routing
db_d1.ts:detectDbKey() SQL dagi birinchi FROM/INTO/UPDATE/JOIN/TABLE tokenidan jadval nomini oladi.

## Cross-DB muammo
checkAndRecordDownload() STAT + ANIME bazalarini tegadi — d1Batch emas, Promise.all(d1Query, d1QueryBatch) ishlatiladi.
getAnimeInfoStats() va getUserActivityStats() — barchasi bir xil bazada, d1BatchRead batch bilan.

## Turso o'chirildi
db_turso.ts, scripts/reset_db.ts o'chirildi. @libsql/client package.json dan olib tashlandi. TursoSessionAdapter → D1SessionAdapter.

**Why:** Turso sekin (~20-40ms round-trip), Cloudflare D1 ~1-3ms. D1 xavfsizligi ham yuqori.

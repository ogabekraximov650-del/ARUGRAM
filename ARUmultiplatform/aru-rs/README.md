# ARU — media platforma (Rust + uzcoders)

Noldan, faqat `uzcoders` kutubxonasida yozilgan ilova (boshqa kutubxona yo'q). Uslub: qora plita, oq (teskari) faol belgi, bitta signal rangi (to'q sariq), logotipdagi 22% burchak.

| Fayl | Nima |
|---|---|
| `src/lib.rs`, `src/app.rs` | kirish nuqtasi, pastki menyu; ilova holati va mantiq (kirish, like, obuna, fikr, pleyer, yuklash) |
| `src/home.rs`, `src/reels.rs`, `src/watch.rs`, `src/profile.rs`, `src/create.rs` | Bosh/Obunalar/Qidiruv, Oqim, Ko'rish, Profil, Yaratish + sarlavha + yuklash holati + birinchi kirish |
| `src/feed.rs`, `src/thumbs.rs`, `src/model.rs`, `src/json.rs`, `src/net.rs`, `src/media.rs`, `src/ident.rs` | ro'yxat sahifalash, muqova rasmlari, modellar, JSON, tarmoq (Java orqali), pleyer/yuklash ko'prigi, kanal kimligi |
| `java/.../AruActivity.java` | HTTP, bo'lakli yuklash (muqova va davomiylik shu yerda olinadi), MediaPlayer (oqimli manba) — tashqi kutubxonasiz |
| `assets/` | 3 ta kichraytirilgan shrift (~86 KB) va logotip (4 KB) |
| `bot.enc`, `db.enc` | Telegram bot/kanal va Supabase kalitlari (ENCODE_TOKEN bilan shifrlangan) |

Namuna ma'lumot YO'Q: hamma narsa worker'dan. Testlarda faqat tarmoq o'rniga sinov javobi (`src/tests.rs`).

Yig'ish va testlar FAQAT GitHub Actions'da: `.github/workflows/aru.yml`: `app` (testlar, ekran rasmlari artifact `aru-ekranlar-<run>`, APK `aru-<run>`) va `worker` (deploy + E2E).

## Worker (`worker/`)
Cloudflare worker `aru-api` (Rust -> WASM) + Supabase (alohida loyiha `ARU`; sxema `worker/schema.sql` izohida, kalitlar `db.enc`). Videolar ≤19 MiB bo'laklarda kanalga yoziladi; yozish so'rovlari `X-App-Token` (`APP_SIGN_SECRET`) bilan. `GET /api/videos`, `GET /api/video/<id>`, `GET /api/video/<id>/stream` (Range), `POST /api/upload`, `PUT /api/upload/<id>/<n>`, `POST /api/upload/<id>/done`, `DELETE /api/video/<id>`.

## Bot va kanal
`bot.enc` — Telegram bot tokeni va kanal ID'si (ikki qator), AES-256-CBC + PBKDF2 (200 000), kalit — `ENCODE_TOKEN` (loyihadagi boshqa `.enc` fayllar kabi). Ochish: `openssl enc -d -aes-256-cbc -pbkdf2 -iter 200000 -pass env:K -in bot.enc`. Ochiq matnda hech qayerga yozilmaydi.

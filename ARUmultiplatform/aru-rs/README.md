# ARU — media platforma (Rust + uzcoders)

Noldan, faqat `uzcoders` kutubxonasida yozilgan ilova (boshqa kutubxona yo'q). Uslub: qora plita, oq (teskari) faol belgi, bitta signal rangi (to'q sariq), logotipdagi 22% burchak.

| Fayl | Nima |
|---|---|
| `src/lib.rs` | ilova holati, pastki menyu, orqaga tugmasi, testlar |
| `src/home.rs` | Bosh (mozaika, ro'yxat) va Obunalar |
| `src/reels.rs` | Oqim (tik video, amallar pastda, fikrlar suzib yuradi) |
| `src/watch.rs` | Video ko'rish (vaqtga bog'langan fikrlar) |
| `src/me.rs`, `src/create.rs` | Profil (kanal) va "Yaratish" oynasi |
| `src/art.rs` | namuna rasmlar (kodda chiziladi, fayl yo'q) |
| `src/data.rs` | namuna kontent — keyin Telegram bot va kanal bilan almashtiriladi |
| `assets/` | 3 ta kichraytirilgan shrift (Latin + Kirill, ~86 KB) va logotip (4 KB) |

Yig'ish va testlar FAQAT GitHub Actions'da: `.github/workflows/aru.yml`: `app` (testlar, ekran rasmlari artifact `aru-ekranlar-<run>`, APK `aru-<run>`) va `worker` (deploy + E2E).

## Worker (`worker/`)
Cloudflare worker `aru-api` (Rust -> WASM) + Supabase (alohida loyiha `ARU`; sxema `worker/schema.sql` izohida, kalitlar `db.enc`). Videolar ≤19 MiB bo'laklarda kanalga yoziladi; yozish so'rovlari `X-App-Token` (`APP_SIGN_SECRET`) bilan. `GET /api/videos`, `GET /api/video/<id>`, `GET /api/video/<id>/stream` (Range), `POST /api/upload`, `PUT /api/upload/<id>/<n>`, `POST /api/upload/<id>/done`, `DELETE /api/video/<id>`.

## Bot va kanal
`bot.enc` — Telegram bot tokeni va kanal ID'si (ikki qator), AES-256-CBC + PBKDF2 (200 000), kalit — `ENCODE_TOKEN` (loyihadagi boshqa `.enc` fayllar kabi). Ochish: `openssl enc -d -aes-256-cbc -pbkdf2 -iter 200000 -pass env:K -in bot.enc`. Ochiq matnda hech qayerga yozilmaydi.

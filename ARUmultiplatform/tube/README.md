# ARUTUBE — DoodStream videolari uchun YouTube'ga o'xshash ilova

Kotlin + Jetpack Compose (ilova) + Rust (yadro) + alohida Rust Cloudflare Worker. Bazasiz: ro'yxat DoodStream'dan jonli olinadi.

| Papka | Nima |
|---|---|
| `worker/` | Cloudflare Worker `arutube-api` (Rust → WASM). DoodStream API kaliti FAQAT shu yerda (secret `DOOD_KEY`). `GET /api/videos`, `/api/video/<kod>`, `/api/upload-ticket`, `POST /api/remote` |
| `android/app/` | Compose ilova: bosh sahifa (muqova + davomiylik + ko'rishlar), pleyer (iframe `/e/<kod>`), yuklash (fayl yoki havola) |
| `android/rust/` | `libtube_core.so`: HTTP (ureq/rustls) va faylni DoodStream'ga oqim bilan yuklash (xotiraga to'liq o'qilmaydi, jarayon foizi) |

## Sozlash (bir marta, qo'lda)
GitHub → Settings → Environments → `secret` ichiga **`DOOD_KEY`** (DoodStream API kaliti) qo'shing. Worker deploy shu kalitni Cloudflare secret qilib qo'yadi.
`APP_SIGN_SECRET` (bor) — ilova tokeni: APK ichiga va worker `APP_TOKEN` ga bir xil qiymat qo'yiladi.

## Yig'ish
Faqat GitHub Actions: `tube-android.yml` (Rust testlari → .so → Kotlin testlari → APK, artifact `arutube-<run>`), `tube-worker.yml` (testlar + deploy).
Worker manzili `app/build.gradle.kts` dagi `API_BASE` (`https://arutube-api.<subdomen>.workers.dev`).

## Cheklovlar
- Yuklash kaliti ilovaga faqat yuklash paytida (`/api/upload-ticket`, `X-App-Token` bilan) beriladi: DoodStream'ning yuklash serveri kalitni so'raydi.
- Pleyer — DoodStream iframe'i; reklama/yo'naltirishlar bloklanadi (asosiy sahifani almashtirish).

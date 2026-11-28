# GigaCraft

Noldan yozilgan voxel o'yin: **Rust + OpenGL ES 2.0, tashqi kutubxonasiz**. Android (NativeActivity), APK ~1 MB.

## Asosiy g'oyalar
- **1 blok = 25 sm**, o'yinchi bo'yi 7 blok (1.75 m). Minecraft'dan 4 baravar mayda bloklar.
- **Cheksiz dunyo**: `gen.rs` xeshga asoslangan — ixtiyoriy joy mustaqil hosil bo'ladi. Koordinatalar `f64`/`i64`, kamera atrofida "suzuvchi boshlang'ich nuqta" bilan chiziladi (uzoqda titramaydi).
- **Uzoqdagi relyef yirik kataklar bilan** (`lod.rs`): yaqinda bloklar, undan keyin har daraja 2 baravar yirik plitalar (halqalar), tutashgan joylarda "etak" yoriqlarni yopadi.
- **Faqat sirt chiziladi** (`mesh.rs`): havoga tegmaydigan yuzlar yo'q, burchaklarda AO.
- **Fon oqimlarida yuklash** (`stream.rs`): kadr qotmaydi; tahrirlangan ustun darhol qayta quriladi.
- **Kun/tun**, yulduzlar, oy, kechqurun binafsha-to'q sariq osmon (`sky.rs`); suv yarim shaffof.
- **Boshqaruv**: suzuvchi joystik, qarash uchun surish, Sakrash/Uchish/Buzish/Qo'yish tugmalari, 9 katakli panel, blok tanlash oynasi, pauza menyusi (sifat, vaqt, sezgirlik).
- **Uchish** 4 tezlikda (1x, 3x, 10x, 30x); 3x dan yuqorisida devorlardan o'tadi.

## Tuzilish
`src/`: `gen` → `world` → `mesh` / `lod` → `stream` → `render`; `player`, `ray`, `game`, `ui`, `hud`, `text`, `font/` (TTF o'quvchi), `sky`, `app` (hammasini birlashtiradi), `egl`, `gl`, `android` (NativeActivity qobig'i), `png` (testlardagi rasmlar).

## Yig'ish va testlar
Faqat GitHub Actions: `.github/workflows/android.yml` — birlik testlari, Mesa (llvmpipe) bilan **haqiqiy OpenGL kadrlari** (`tests/shots.rs` rasmlari artifact'da), keyin APK (arm64-v8a + armeabi-v7a), imzolanadi va **Releases → latest** ga yuklanadi.

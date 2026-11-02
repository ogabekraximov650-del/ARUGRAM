# DRKLO — ARUGRAM Android (Telegram Android asosida)

UZ: Bu papka DrKLO/Telegram (GPL-2.0-or-later) kodining nusxasi (asos: commit `3de37c8cfb039452be7398909461da6288babf15`). ARUGRAM o'zgarishlari shu papkada qilinadi: kuchsiz qurilmalarda tezlashtirish va "glass" linza effekti; maqsad — Android 10 (32-bit) da silliq ishlash.
EN: This folder is a copy of DrKLO/Telegram (GPL-2.0-or-later), base commit `3de37c8cfb039452be7398909461da6288babf15`. ARUGRAM changes live here: speedups for weak devices and a "glass" lens effect; goal — smooth on Android 10 (32-bit).
RU: Эта папка — копия DrKLO/Telegram (GPL-2.0-or-later), базовый коммит `3de37c8cfb039452be7398909461da6288babf15`. Здесь делаются изменения ARUGRAM: ускорение для слабых устройств и эффект «стеклянной» линзы; цель — плавная работа на Android 10 (32-bit).

## O'zgarishlar / Changes / Изменения
- `x86`, `x86_64` prebuilt `libwallet_engine.a` olib tashlandi (hajm); ABI: `armeabi-v7a`, `arm64-v8a` / x86 prebuilt wallet libs removed (size); ABIs: armeabi-v7a, arm64-v8a / удалены x86-prebuilt (размер).
- Build: `.github/workflows/drklo-build.yml` (faqat Actions'da / Actions only / только Actions).

# ARUmultiplatform — har platforma uchun ALOHIDA kodli ARUMEDIATV

Flutter loyihasi (`lib/`, `pubspec.yaml`) to'xtatildi (tegilmaydi, ishlayverishi uchun qoldi). Yangi ilovalar har platforma uchun
o'z tilida yoziladi:

| Papka | Texnologiya | Holat |
|---|---|---|
| `android/` | Kotlin + Jetpack Compose + Rust (JNI) | **Bosh sahifa** tayyor; qolgan ekranlar bosqichma-bosqich |
| `ios/` | — | keyin |

## Android

- Paket: `uz.arumediatv.com`, ilova nomi **ARUMEDIATV**, worker: `https://arumediatv.uzcom.workers.dev` (`app/build.gradle.kts` -> `API_BASE`).
- `rust/` — Rust yadrosi (`libaru_core.so`): hozircha so'rov imzosi (`X-App-Sig`). Keyin Telegram (MTProto), shifrlangan kesh, pleyer manbasi.
- `app/` — Compose ilovasi: `ui/` (ekranlar), `data/` (server va kesh), `core/` (JNI ko'prigi).
- Imzo kaliti Flutter ilovasi bilan bir xil (`ci/release.keystore`) — yangi APK eskisi ustiga o'rnatiladi.
- **Yig'ish va testlar FAQAT GitHub Actions'da** (`.github/workflows/android-native.yml`): Rust testlari, Kotlin birlik testlari, APK
  (artifact: `arumediatv-native-<run>`). Mahalliy muhitga hech narsa o'rnatilmaydi.

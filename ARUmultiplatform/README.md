# ARUmultiplatform — har platforma uchun ALOHIDA kodli ARUMEDIATV

Flutter loyihasi (`lib/`, `pubspec.yaml`) to'xtatildi (tegilmaydi, ishlayverishi uchun qoldi). Yangi ilovalar har platforma uchun
o'z tilida yoziladi:

| Papka | Texnologiya | Holat |
|---|---|---|
| `android/` | Kotlin + Jetpack Compose + Rust (JNI) | **Bosh sahifa** tayyor; qolgan ekranlar bosqichma-bosqich |
| `ios/` | — | keyin |

## Android

- Paket: `uz.arumediatv.com`, ilova nomi **ARUMEDIATV**, worker: `https://arumediatv.uzcom.workers.dev` (`app/build.gradle.kts` -> `API_BASE`).
- `rust/` — Rust yadrosi (`librust_core.so`, asosiy `rust/` nusxasi): Telegram (MTProto), shifrlangan kesh, imzo. Kotlin JNA orqali chaqiradi (`core/RustCore.kt`). Ilova: Bosh sahifa, Qidiruv, Katalog, Profil (Telegram login, rasmlar bot chati orqali).
- `app/` — Compose ilovasi: `ui/` (ekranlar), `data/` (server va kesh), `core/` (JNI ko'prigi).
- Imzo kaliti Flutter ilovasi bilan bir xil (`ci/release.keystore`) — yangi APK eskisi ustiga o'rnatiladi.
- **Yig'ish va testlar FAQAT GitHub Actions'da** (`.github/workflows/android-native.yml`): Rust testlari, Kotlin birlik testlari, APK
  (artifact: `arumediatv-native-<run>`). Mahalliy muhitga hech narsa o'rnatilmaydi.

## Telegram (DrKLO manba kodi)

`ARUmultiplatform/Telegram/` — [DrKLO/Telegram](https://github.com/DrKLO/Telegram) (Telegram for Android) manba kodi (13.0.2, GPLv2; `LICENSE` shu papkada). Submodullar repoda gitlink sifatida (`/.gitmodules`), CI'da `submodules: recursive` bilan olinadi. Yig'ish FAQAT GitHub Actions'da: `.github/workflows/telegram-android.yml` (`:TMessagesProj_AppStandalone:assembleAfatStandalone`, faqat arm64-v8a + armeabi-v7a). O'zgarish: ABI ro'yxati qisqartirildi (paket nomi `org.telegram.messenger.web` — rasmiy Telegram bilan to'qnashmaydi).

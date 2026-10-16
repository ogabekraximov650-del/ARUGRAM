# ARUmultiplatform

ARUGRAM'ning native nusxasi. Hozirgi Flutter ilovasi (`lib/`) o'zgarmaydi; bu yerda har platforma uchun alohida,
o'sha platformaning o'z vositasida yoziladi, Rust yadro esa umumiy.

| Papka | Nima |
|---|---|
| `android/` | Kotlin + Jetpack Compose ilovasi va Rust yadro (`android/rust`) |

## Android

```
cd ARUmultiplatform/android
./gradlew :app:assembleDebug     # APK: app/build/outputs/apk/debug/
./gradlew :app:testDebugUnitTest # Kotlin testlari
cd rust && cargo test --lib      # Rust testlari
```

Kerak: JDK 17+, Android SDK (platform 35), NDK (Rust uchun), `rustup target add aarch64-linux-android armv7-linux-androideabi`.
Server manzili `-PAPI_BASE=https://...` bilan beriladi (CI: `NATIVE_API_BASE` secret). Sukut qiymat yo'q — eski ilovaning serveriga tegilmaydi.

Joylashuv va ranglar Flutter ilovadan ko'chirilgan — manba fayllari har bir Kotlin faylining izohida ko'rsatilgan.
Ko'chirish tartibi va nima hali yo'qligi: `KEYINGI_VAZIFA.md` oxirgi bo'lim.

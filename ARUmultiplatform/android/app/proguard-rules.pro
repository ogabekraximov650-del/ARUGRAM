# R8 (kodni qisqartirish) — XAVFSIZ rejim: nomlar o'zgartirilmaydi (`-dontobfuscate`, xato izlari o'qiladigan bo'lib qoladi),
# ilovaning o'z kodi (`uz.arumediatv.com`) butunlay saqlanadi, faqat ishlatilmagan KUTUBXONA kodi olib tashlanadi.

-dontobfuscate
-keepattributes *Annotation*, InnerClasses, EnclosingMethod, Signature, Exceptions, RuntimeVisibleAnnotations, RuntimeInvisibleAnnotations

# Ilovaning o'z kodi: JNA interfeyslari (`RustLib`), JNI (`AruDataSource` native metodlari), serialization, reflection.
-keep class uz.arumediatv.com.** { *; }
-keep class io.flutter.plugins.videoplayer.** { *; }

# JNA (Rust yadrosi bilan bog'lanish): reflection va native kutubxona
-keep class com.sun.jna.** { *; }
-keep class * extends com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.Structure { <fields>; }
-dontwarn java.awt.**
-dontwarn com.sun.jna.**

# kotlinx.serialization
-keepattributes *Annotation*
-keep,includedescriptorclasses class uz.arumediatv.com.**$$serializer { *; }
-keepclassmembers class uz.arumediatv.com.** { *** Companion; }
-keepclasseswithmembers class uz.arumediatv.com.** { kotlinx.serialization.KSerializer serializer(...); }

# Media3 / ExoPlayer: reflection orqali yuklanadigan qismlar
-keep class androidx.media3.** { *; }
-dontwarn androidx.media3.**

# OkHttp / Okio / org.json: ixtiyoriy kutubxonalar haqidagi ogohlantirishlar
-dontwarn okhttp3.internal.platform.**
-dontwarn org.bouncycastle.**
-dontwarn org.conscrypt.**
-dontwarn org.openjsse.**
-dontwarn javax.annotation.**

# AGP 9: standart fayl endi "-dontoptimize" ni o'z ichiga olmaydi; avvalgi xatti-harakatni saqlash uchun (R8 optimizatsiyasini yoqish alohida, sinab ko'rib qilinadi).
# AGP 9: the default file no longer carries -dontoptimize; keep the previous behaviour (enabling R8 optimization is a separate, tested step).
-dontoptimize

# AdMob (Google Mobile Ads SDK): R8 ishga tushirish komponenti (`MobileAdsInitProvider`) va reklama sinflarini kesib tashlamasin (release'da ilova ochilmay qolgan edi).
# AdMob: keep the init provider and ad classes so R8 cannot strip them (the release build crashed on launch).
-keep class com.google.android.gms.ads.** { *; }
-keep class com.google.android.gms.internal.ads.** { *; }
-dontwarn com.google.android.gms.ads.**
-dontwarn com.google.android.gms.internal.ads.**

# WorkManager + Room + androidx.startup (AdMob kutubxonasi olib keladi va ilova ochilishida ishga tushiradi): Room `WorkDatabase_Impl` ni nomi bo'yicha (reflection) yaratadi.
# R8 uning konstruktorini kesib tashlasa ilova ochilmasdan yopiladi: "Failed to create an instance of androidx.work.impl.WorkDatabase" (build 189, Xiaomi, Android 10).
# WorkManager + Room + startup: Room creates `WorkDatabase_Impl` by name; if R8 strips its constructor the app dies at launch.
-keep class androidx.startup.** { *; }
-keep class * extends androidx.startup.Initializer { *; }
-keep class androidx.work.** { *; }
-keep class androidx.room.** { *; }
-keep class androidx.sqlite.** { *; }
-keep class * extends androidx.room.RoomDatabase { *; }
-dontwarn androidx.work.**
-dontwarn androidx.room.**

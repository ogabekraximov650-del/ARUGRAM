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

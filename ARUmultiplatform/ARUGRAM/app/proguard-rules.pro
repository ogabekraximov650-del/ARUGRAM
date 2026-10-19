# R8 — xavfsiz rejim: nomlar o'zgartirilmaydi, ilovaning o'z kodi saqlanadi, faqat ishlatilmagan kutubxona kodi olib tashlanadi.
-dontobfuscate
-keepattributes *Annotation*, InnerClasses, EnclosingMethod, Signature, Exceptions

-keep class uz.arugram.app.** { *; }

# JNA (Rust yadrosi bilan bog'lanish)
-keep class com.sun.jna.** { *; }
-keep class * extends com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.Structure { <fields>; }
-dontwarn java.awt.**
-dontwarn com.sun.jna.**

# Lottie -> okio: `javax.annotation.Nullable` Android'da yo'q (faqat izoh), ogohlantirish xato bo'lib qolmasin.
-dontwarn javax.annotation.**
-dontwarn org.codehaus.mojo.animal_sniffer.**

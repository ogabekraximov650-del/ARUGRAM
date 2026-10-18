plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "uz.arumediatv.game"
    compileSdk = 35

    defaultConfig {
        applicationId = "uz.arumediatv.game"
        minSdk = 26
        targetSdk = 35
        versionCode = (System.getenv("VERSION_CODE") ?: "1").toIntOrNull() ?: 1
        versionName = "0.1.0"
    }

    // Tajriba bosqichi: debug kalit bilan imzolanadi (telefonga to'g'ridan-to'g'ri o'rnatiladi).
    buildTypes {
        debug { isMinifyEnabled = false }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }

    packaging { jniLibs.useLegacyPackaging = false }
}

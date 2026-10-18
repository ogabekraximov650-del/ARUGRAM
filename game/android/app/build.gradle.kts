plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

// Doimiy imzo: ARUMEDIATV bilan bir xil `ci/release.keystore` (CI'da Secret bo'lsa u). Har build bir xil kalit — yangilanish ustiga o'rnatiladi.
val ciKeystore = System.getenv("KS_FILE") ?: "${rootDir}/../../ci/release.keystore"
val ciStorePass = System.getenv("KS_PASS")?.takeIf { it.isNotBlank() } ?: "arumedia"

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

    signingConfigs {
        create("fixed") {
            storeFile = file(ciKeystore)
            storePassword = ciStorePass
            keyAlias = "arumedia"
            keyPassword = ciStorePass
        }
    }

    buildTypes {
        debug {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("fixed")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }

    packaging { jniLibs.useLegacyPackaging = false }
}

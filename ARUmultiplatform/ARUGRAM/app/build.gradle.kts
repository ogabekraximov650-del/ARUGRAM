plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

// CI beradi (`.github/workflows/arugram-android.yml`); mahalliy yig'ishda — standart qiymatlar.
val ciVersionCode = (System.getenv("VERSION_CODE") ?: "1").toIntOrNull() ?: 1
val ciKeystore = System.getenv("KS_FILE") ?: "${rootDir}/../../ci/release.keystore"
val ciStorePass = System.getenv("KS_PASS")?.takeIf { it.isNotBlank() } ?: "arumedia"
// Telegram API kalitlari (my.telegram.org) — GitHub Secrets (`TG_API_ID`, `TG_API_HASH`); kodga yozilmaydi.
val tgApiId = (System.getenv("TG_API_ID") ?: "").trim().toIntOrNull() ?: 0
val tgApiHash = (System.getenv("TG_API_HASH") ?: "").trim()

android {
    namespace = "uz.arugram.app"
    compileSdk = 35

    defaultConfig {
        applicationId = "uz.arugram.app"
        minSdk = 24
        targetSdk = 35
        versionCode = ciVersionCode
        versionName = "0.1.0"

        buildConfigField("int", "TG_API_ID", "$tgApiId")
        buildConfigField("String", "TG_API_HASH", "\"$tgApiHash\"")
        buildConfigField("String", "APP_VERSION", "\"0.1.0\"")

        ndk {
            abiFilters += listOf("armeabi-v7a", "arm64-v8a")
        }
    }

    signingConfigs {
        create("release") {
            storeFile = file(ciKeystore)
            storePassword = ciStorePass
            keyAlias = "arumedia"
            keyPassword = ciStorePass
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.getByName("release")
        }
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }

    packaging {
        resources.excludes += "/META-INF/{AL2.0,LGPL2.1}"
        jniLibs.useLegacyPackaging = false
    }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)

    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.compose.foundation)
    implementation(libs.compose.material3)

    implementation("androidx.profileinstaller:profileinstaller:1.4.1")
    // JNA: Rust yadrosining C funksiyalarini (`librust_core.so`) to'g'ridan-to'g'ri chaqiradi.
    implementation("net.java.dev.jna:jna:5.17.0@aar")
    implementation(libs.kotlinx.coroutines.android)

    testImplementation(libs.junit)
}

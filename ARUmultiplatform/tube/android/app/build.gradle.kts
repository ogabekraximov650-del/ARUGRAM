plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

// CI beradi (`.github/workflows/tube-android.yml`); mahalliy yig'ishda — standart qiymatlar.
val ciVersionCode = (System.getenv("VERSION_CODE") ?: "1").toIntOrNull() ?: 1
val ciKeystore = System.getenv("KS_FILE") ?: "${rootDir}/../../../ci/release.keystore"
val ciStorePass = System.getenv("KS_PASS")?.takeIf { it.isNotBlank() } ?: "arumedia"

android {
    namespace = "uz.arutube.app"
    compileSdk = 35

    defaultConfig {
        applicationId = "uz.arutube.app"
        minSdk = 24
        targetSdk = 35
        versionCode = ciVersionCode
        versionName = "0.1.0"

        // Worker (alohida, DoodStream kaliti faqat shu yerda). `uzcom` — Cloudflare akkaunt subdomeni (arumediatv bilan bir xil).
        buildConfigField("String", "API_BASE", "\"https://arutube-api.uzcom.workers.dev\"")
        // Worker bilan umumiy ilova tokeni (CI'da `APP_SIGN_SECRET`); bo'sh bo'lsa worker ham himoyasiz.
        buildConfigField("String", "APP_TOKEN", "\"${System.getenv("APP_SIGN_SECRET") ?: ""}\"")

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
        // 16 KB sahifali qurilmalar uchun: .so siqilmagan va tekislangan holda qadoqlanadi.
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
    implementation(libs.compose.foundation)
    implementation(libs.compose.material3)

    implementation("androidx.profileinstaller:profileinstaller:1.4.1")
    // Muqova (thumbnail) rasmlari
    implementation("io.coil-kt:coil-compose:2.7.0")
    // JNA: Rust yadrosining C funksiyalarini (`libtube_core.so`) chaqiradi.
    implementation("net.java.dev.jna:jna:5.17.0@aar")
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.kotlinx.coroutines.android)

    testImplementation(libs.junit)
}

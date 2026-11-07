plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

// CI beradi (`.github/workflows/drive-android.yml`); mahalliy yig'ishda — standart qiymatlar.
val ciVersionCode = (System.getenv("VERSION_CODE") ?: "1").toIntOrNull() ?: 1
val ciKeystore = System.getenv("KS_FILE") ?: "${rootDir}/../../../ci/release.keystore"
val ciStorePass = System.getenv("KS_PASS")?.takeIf { it.isNotBlank() } ?: "arumedia"
// So'rov imzosi siri (worker `APP_SIGN_SECRET` bilan bir xil) — faqat CI muhit o'zgaruvchisidan, kodga yozilmaydi.
val appSecret = System.getenv("APP_SIGN_SECRET") ?: ""

android {
    namespace = "uz.arudrive.app"
    compileSdk = 35

    defaultConfig {
        applicationId = "uz.arudrive.app"
        minSdk = 24
        targetSdk = 35
        versionCode = ciVersionCode
        versionName = "0.1.0"
        buildConfigField("String", "API_BASE", "\"https://arudrive.uzcom.workers.dev\"")
        buildConfigField("String", "APP_SECRET", "\"$appSecret\"")
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
            isMinifyEnabled = false
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
    kotlinOptions { jvmTarget = "17" }
    packaging { resources.excludes += "/META-INF/{AL2.0,LGPL2.1}" }
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
    implementation(libs.compose.material.icons.core)

    implementation("androidx.media3:media3-exoplayer:1.5.1")
    implementation("androidx.media3:media3-ui:1.5.1")
    implementation("io.coil-kt:coil-compose:2.7.0")
    implementation(libs.okhttp)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.kotlinx.coroutines.android)

    testImplementation(libs.junit)
}

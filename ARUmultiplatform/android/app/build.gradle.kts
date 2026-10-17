plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

// CI beradi (`.github/workflows/android-native.yml`); mahalliy yig'ishda — standart qiymatlar.
val ciVersionCode = (System.getenv("VERSION_CODE") ?: "1").toIntOrNull() ?: 1
val ciKeystore = System.getenv("KS_FILE") ?: "${rootDir}/../../ci/release.keystore"
val ciStorePass = System.getenv("KS_PASS")?.takeIf { it.isNotBlank() } ?: "arumedia"

// `-PonlyAbi=armeabi-v7a` — faqat 32 bitli APK (CI'da "Admin 32 bit" uchun); bo'lmasa 32+64 bit.
val onlyAbi = (project.findProperty("onlyAbi") as String?)?.takeIf { it.isNotBlank() }

android {
    namespace = "uz.arumediatv.com"
    compileSdk = 35

    defaultConfig {
        applicationId = "uz.arumediatv.com"
        minSdk = 24
        targetSdk = 35
        versionCode = ciVersionCode
        versionName = "0.1.0"

        // Server manzili BITTA joyda. Yangi worker nomi: `arumediatv`.
        buildConfigField("String", "API_BASE", "\"https://arumediatv.uzcom.workers.dev\"")
        // Serverga (`X-App-Version`) boradigan versiya: `<versionName>+<versionCode % 1000>` (worker `version_rank` shuni tushunadi).
        buildConfigField("String", "APP_VERSION", "\"0.1.0+${ciVersionCode % 1000}\"")

        ndk {
            abiFilters += if (onlyAbi != null) listOf(onlyAbi) else listOf("armeabi-v7a", "arm64-v8a")
        }
    }

    // user — oddiy foydalanuvchi ilovasi; admin — bir xil paket (`uz.arumediatv.com`), faqat `BuildConfig.IS_ADMIN` farq qiladi.
    flavorDimensions += "role"
    productFlavors {
        create("user") {
            dimension = "role"
            buildConfigField("boolean", "IS_ADMIN", "false")
        }
        create("admin") {
            dimension = "role"
            buildConfigField("boolean", "IS_ADMIN", "true")
        }
    }

    signingConfigs {
        // Flutter ilovasi bilan BIR XIL imzo kaliti (`ci/release.keystore`) — yangilanish ustiga o'rnatiladi.
        create("release") {
            storeFile = file(ciKeystore)
            storePassword = ciStorePass
            keyAlias = "arumedia"
            keyPassword = ciStorePass
        }
    }

    buildTypes {
        release {
            // Birinchi bosqichda qisqartirish (R8) o'chiq: telefonda sinab ko'rilmagan JNI/serialization qoidalari
            // sababli ilova jimgina yiqilmasligi uchun. Barqaror bo'lgach yoqiladi.
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
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.compose.foundation)
    implementation(libs.compose.material3)
    implementation(libs.compose.material.icons.core)

    // Compose kutubxonalarining tayyor "baseline profile"larini o'rnatadi: birinchi ochilish va aylantirish qotmaydi.
    implementation("androidx.profileinstaller:profileinstaller:1.4.1")
    // Video: Media3 ExoPlayer (Range so'rovlar Rust mahalliy serveriga yoki worker keshiga ketadi).
    // Baseline profil (ilovaniki va Compose/Media3 kutubxonalariniki) o'rnatilgach shu orqali ART ga beriladi.
    implementation("androidx.profileinstaller:profileinstaller:1.4.1")
    implementation("androidx.media3:media3-exoplayer:1.5.1")
    implementation("androidx.media3:media3-ui:1.5.1")
    // QR kod (Telegram QR orqali kirish)
    implementation("com.google.zxing:core:3.5.3")
    implementation(libs.okhttp)
    // JNA: Rust yadrosining C funksiyalarini (`librust_core.so`) JNI yozmasdan to'g'ridan-to'g'ri chaqiradi.
    implementation("net.java.dev.jna:jna:5.17.0@aar")
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.kotlinx.coroutines.android)

    testImplementation(libs.junit)
}

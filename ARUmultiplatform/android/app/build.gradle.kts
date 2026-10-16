plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.kotlin.plugin.serialization")
}

android {
    namespace = "uz.arugram.soft"
    compileSdk = 35

    defaultConfig {
        // Eski (Flutter) ilova bilan yonma-yon o'rnatilishi uchun
        // `.native` qo'shimchasi: paket `uz.arugram.soft.native`.
        applicationId = "uz.arugram.soft.native"
        minSdk = 24
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"

        // Server manzili. ATAYLAB sukut qiymat YO'Q: bu ilova eski (Flutter)
        // ilovaning serveriga tegmasligi kerak — manzil CI'dan beriladi
        // (`-PAPI_BASE=...`, `NATIVE_API_BASE` secret'i). Bo'sh bo'lsa ilova
        // so'rov yubormaydi, "internet yo'q" holatida ishlaydi.
        val apiBase = (project.findProperty("API_BASE") as String?) ?: ""
        buildConfigField("String", "API_BASE", "\"$apiBase\"")

        // Rust yadro faqat shu arxitekturalar uchun yig'iladi.
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a") }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }

    buildFeatures {
        compose = true
        buildConfig = true
    }
}

// ── Rust yadroni yig'ish ─────────────────────────────────────────
//
// `rust/` -> `librust_core.so` (arm64-v8a, armeabi-v7a) -> `build/rustJniLibs`.
// Android NDK kerak (`ANDROID_NDK_HOME` yoki `$ANDROID_HOME/ndk/<versiya>`) va
// `rustup target add aarch64-linux-android armv7-linux-androideabi`.
// NDK yoki cargo topilmasa vazifa O'TKAZIB YUBORILADI: ilova yiqilmaydi,
// `RustCore.loaded == false` bo'ladi (keshsiz ishlaydi).
val rustDir = rootProject.file("rust")
val rustOut = layout.buildDirectory.dir("rustJniLibs")

val ndkHome: String? = System.getenv("ANDROID_NDK_HOME")
    ?: (System.getenv("ANDROID_HOME") ?: android.sdkDirectory.path).let { sdk ->
        File(sdk, "ndk").listFiles()?.filter { it.isDirectory }?.maxByOrNull { it.name }?.path
    }

data class RustAbi(val abi: String, val triple: String, val clang: String)

val rustAbis = listOf(
    RustAbi("arm64-v8a", "aarch64-linux-android", "aarch64-linux-android24-clang"),
    RustAbi("armeabi-v7a", "armv7-linux-androideabi", "armv7a-linux-androideabi24-clang"),
)

val buildRust = tasks.register("buildRust") {
    group = "rust"
    description = "rust/ ni Android uchun yig'adi (librust_core.so)"
    inputs.dir(rustDir.resolve("src"))
    inputs.file(rustDir.resolve("Cargo.toml"))
    outputs.dir(rustOut)
    doLast {
        if (ndkHome == null) {
            logger.warn("Android NDK topilmadi — Rust yig'ilmadi (librust_core.so yo'q)")
            return@doLast
        }
        val bin = File(ndkHome, "toolchains/llvm/prebuilt/linux-x86_64/bin")
        for (a in rustAbis) {
            val env = a.triple.uppercase().replace('-', '_')
            val clang = File(bin, a.clang).path
            val ar = File(bin, "llvm-ar").path
            project.exec {
                workingDir = rustDir
                environment("CARGO_TARGET_${env}_LINKER", clang)
                environment("CC_${a.triple.replace('-', '_')}", clang)
                environment("AR_${a.triple.replace('-', '_')}", ar)
                commandLine("cargo", "build", "--release", "--lib", "--target", a.triple)
            }
            val so = rustDir.resolve("target/${a.triple}/release/librust_core.so")
            val dest = rustOut.get().dir(a.abi).asFile.apply { mkdirs() }
            so.copyTo(File(dest, "librust_core.so"), overwrite = true)
        }
    }
}

android.sourceSets.getByName("main").jniLibs.srcDir(rustOut)
tasks.matching { it.name.startsWith("merge") && it.name.endsWith("JniLibFolders") }
    .configureEach { dependsOn(buildRust) }

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2025.04.01")
    implementation(composeBom)
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.compose.ui:ui-tooling-preview")
    debugImplementation("androidx.compose.ui:ui-tooling")

    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")

    // Rasmlar (Flutter'dagi `cached_network_image` o'rnida).
    implementation("io.coil-kt.coil3:coil-compose:3.1.0")
    implementation("io.coil-kt.coil3:coil-network-okhttp:3.1.0")

    // Tarmoq va JSON.
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.8.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.1")

    testImplementation("junit:junit:4.13.2")
}

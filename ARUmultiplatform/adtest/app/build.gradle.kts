plugins {
    id("com.android.application")
}

android {
    namespace = "uz.arumediatv.adtest"
    compileSdk = 37

    defaultConfig {
        applicationId = "uz.arumediatv.adtest"
        minSdk = 24
        targetSdk = 35
        versionCode = 1
        versionName = "1.0"
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    // Asosiy ilovadagi bilan bir xil versiya.
    implementation("com.google.android.gms:play-services-ads:23.6.0")
}

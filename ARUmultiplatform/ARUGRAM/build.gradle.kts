// Ildiz Gradle fayli: plaginlar versiyalari `gradle/libs.versions.toml` da, modul sozlamalari `app/build.gradle.kts` da.
plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.kotlin.android) apply false
    alias(libs.plugins.kotlin.compose) apply false
    alias(libs.plugins.kotlin.serialization) apply false
}

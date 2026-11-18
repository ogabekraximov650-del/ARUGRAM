// UZ: reklamani Actions'da (emulyatorda) sinash uchun juda kichik ilova. Faqat AdMob yuklashni tekshiradi, asosiy ilovaga tegmaydi.
// EN: tiny app used only to check AdMob loading on a CI emulator; independent from the main app.
// RU: malenkoe prilozhenie tolko dlya proverki zagruzki AdMob na emulyatore v CI.
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}
rootProject.name = "ADTEST"
include(":app")

package uz.arumediatv.com.core

/**
 * Rust yadrosi (`ARUmultiplatform/android/rust`, `libaru_core.so`) bilan JNI ko'prigi.
 *
 * Hozircha: so'rov imzosi (`X-App-Sig`). Telegram, shifrlangan kesh va pleyer manbasi keyingi bosqichlarda shu yerga ulanadi.
 */
object AruCore {
    /** `.so` bir marta yuklanadi; yuklanmasa (masalan, qurilma arxitekturasi mos emas) ilova yiqilmaydi — imzo bo'sh qaytadi. */
    private val loaded: Boolean by lazy {
        try {
            System.loadLibrary("aru_core")
            true
        } catch (_: Throwable) {
            false
        }
    }

    /** `v2.<vaqt>.<hex>` yoki bo'sh satr (yadro yuklanmagan / imzo kaliti yo'q). */
    fun sign(ts: String, method: String, path: String): String {
        if (!loaded) return ""
        return try {
            nativeSign(ts, method, path)
        } catch (_: Throwable) {
            ""
        }
    }

    /** Diagnostika matni. */
    fun info(): String {
        if (!loaded) return "aru_core yuklanmadi"
        return try {
            nativeInfo()
        } catch (_: Throwable) {
            "aru_core xato"
        }
    }

    // `internal` EMAS: Kotlin internal funksiya nomini o'zgartiradi (mangling) va JNI uni topa olmaydi.
    @JvmStatic
    external fun nativeSign(ts: String, method: String, path: String): String

    @JvmStatic
    external fun nativeInfo(): String
}

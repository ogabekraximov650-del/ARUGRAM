package uz.arugram.soft.rust

import android.util.Log

/**
 * Rust yadro (`librust_core.so`) bilan JNI ko'prigi.
 *
 * Flutter ilovadagi `RustCore` (`lib/services/rust_bridge.dart`) ning
 * Kotlin ko'rinishi. Hozircha faqat kesh funksiyalari ulangan
 * (`rust/src/jni_bridge.rs`); qolganlari (Telegram, video kesh) ekranlar
 * bilan birga ulanadi.
 *
 * `.so` yuklanmasa (masalan Rust hali yig'ilmagan) ilova YIQILMAYDI:
 * [loaded] `false` bo'ladi va kesh xotirasiz ishlaydi.
 */
object RustCore {
    private const val TAG = "RustCore"

    @Volatile
    var loaded: Boolean = false
        private set

    init {
        loaded = try {
            System.loadLibrary("rust_core")
            true
        } catch (e: UnsatisfiedLinkError) {
            Log.w(TAG, "librust_core.so yuklanmadi: ${e.message}")
            false
        }
    }

    @JvmStatic private external fun nativeVersion(): String?
    @JvmStatic private external fun nativeCacheGet(path: String): String?
    @JvmStatic private external fun nativeCacheIsFresh(path: String): Boolean
    @JvmStatic private external fun nativeCacheSave(path: String, json: String): Boolean
    @JvmStatic private external fun nativeCacheClear(path: String)

    fun version(): String? = if (loaded) nativeVersion() else null

    /** Keshdagi JSON matni (yoki `null`). */
    fun cacheGet(path: String): String? = if (loaded) nativeCacheGet(path) else null

    /** Kesh 5 daqiqadan yangimi (fon-yangilash kerakmi). */
    fun cacheIsFresh(path: String): Boolean = loaded && nativeCacheIsFresh(path)

    fun cacheSave(path: String, json: String): Boolean = loaded && nativeCacheSave(path, json)

    fun cacheClear(path: String) {
        if (loaded) nativeCacheClear(path)
    }
}

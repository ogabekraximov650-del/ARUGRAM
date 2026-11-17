package uz.arumediatv.com

import android.app.Application
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

class AruApplication : Application() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        if (!AppServices.ready) return
        if (level >= TRIM_MEMORY_RUNNING_LOW) runCatching { AppServices.tgStickers.trim() }
        if (level >= TRIM_MEMORY_UI_HIDDEN) AppServices.images.trim(true)
        else if (level >= TRIM_MEMORY_RUNNING_LOW) AppServices.images.trim(false)
    }

    override fun onCreate() {
        super.onCreate()
        // UZ: Kotlin/Java halokatlari ham `last_crash.txt` ga yoziladi — keyingi ochilishda sababi ko'rsatiladi (Rust halokatlari bilan bir joyda). Ilova ochilmay yopilsa ham iz qoladi.
        // EN: Kotlin/Java crashes are also written to `last_crash.txt` and shown on the next launch.
        // RU: Java/Kotlin-padeniya takzhe pishutsya v `last_crash.txt`.
        val prev = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { t, e ->
            try {
                val text = "${t.name}: " + android.util.Log.getStackTraceString(e).take(4000)
                java.io.File(filesDir, "last_crash.txt").writeText(text)
            } catch (_: Throwable) {
            }
            prev?.uncaughtException(t, e)
        }
        AppServices.init(this)
        scope.launch {
            AppServices.auth.restore()
            AppServices.billing.restore()
            AppServices.gate.start()
            AppServices.billing.loadIfStale()
            sweepTemp()
            AppServices.telegram.start()
        }
    }

    /** Vaqtinchasi 30 daqiqadan eski fayllar o'chadi (Flutter `StorageJanitor.sweep`); rasm keshi (`aru_img`) qoladi. */
    private fun sweepTemp() {
        try {
            val now = System.currentTimeMillis()
            cacheDir.listFiles()?.forEach { f ->
                if (f.name == "aru_img") return@forEach
                if (now - f.lastModified() < 30 * 60_000L) return@forEach
                if (f.isDirectory) f.deleteRecursively() else f.delete()
            }
        } catch (_: Exception) {
        }
    }
}

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
        AppServices.init(this)
        scope.launch {
            AppServices.auth.restore()
            AppServices.billing.restore()
            AppServices.gate.start()
            AppServices.billing.load(force = true)
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

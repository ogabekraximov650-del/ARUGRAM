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

    /**
     * UZ: halokat yozuvchisi ENG ERTA nuqtada (`attachBaseContext`) o'rnatiladi — kutubxonalarning ishga tushish komponentlari (masalan AdMob `MobileAdsInitProvider`) `onCreate` dan OLDIN ishga tushadi,
     * `onCreate` da o'rnatilsa ularning halokati yozilmay qolardi. Kotlin/Java halokatlari `last_crash.txt` ga va Download papkasiga yoziladi.
     * EN: the crash logger is installed at the earliest point so provider-init crashes (e.g. AdMob) are captured too.
     * RU: zapis padeniy ustanavlivaetsya v `attachBaseContext`, do provajderov bibliotek.
     */
    override fun attachBaseContext(base: android.content.Context) {
        super.attachBaseContext(base)
        val prev = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { t, e ->
            try {
                val text = "${t.name}: " + android.util.Log.getStackTraceString(e).take(4000)
                java.io.File(filesDir, "last_crash.txt").writeText(text)
                saveCrashToDownloads(text)
            } catch (_: Throwable) {
            }
            prev?.uncaughtException(t, e)
        }
    }

    override fun onCreate() {
        super.onCreate()
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

    /**
     * UZ: halokat matni telefonning **Yuklashlar (Download)** papkasiga ham saqlanadi (`arumediatv_xato_<sana>.txt`) — ilova ochilmasa ham fayl menejeridan topib, menga yuborish mumkin.
     * Android 10+ da ruxsat so'ralmaydi (MediaStore); eski Android'da ilovaning o'z yuklashlar papkasiga (`Android/data/uz.arumediatv.com/files/Download`) yoziladi.
     * EN: the crash text is also saved into the public Download folder so it can be found with a file manager and sent over.
     * RU: tekst padeniya sokhranyaetsya i v obshchuyu papku Download.
     */
    private fun saveCrashToDownloads(text: String) {
        val stamp = java.text.SimpleDateFormat("yyyyMMdd_HHmmss", java.util.Locale.US).format(java.util.Date())
        val name = "arumediatv_xato_$stamp.txt"
        val body = ("ARUMEDIATV xato hisoboti\n" +
            "Versiya: ${uz.arumediatv.com.BuildConfig.APP_VERSION}\n" +
            "Qurilma: ${android.os.Build.MANUFACTURER} ${android.os.Build.MODEL}, Android ${android.os.Build.VERSION.RELEASE} (SDK ${android.os.Build.VERSION.SDK_INT})\n" +
            "Vaqt: $stamp\n\n" + text).toByteArray()
        if (android.os.Build.VERSION.SDK_INT >= 29) {
            val cv = android.content.ContentValues().apply {
                put(android.provider.MediaStore.Downloads.DISPLAY_NAME, name)
                put(android.provider.MediaStore.Downloads.MIME_TYPE, "text/plain")
                put(android.provider.MediaStore.Downloads.RELATIVE_PATH, android.os.Environment.DIRECTORY_DOWNLOADS)
            }
            val uri = contentResolver.insert(android.provider.MediaStore.Downloads.EXTERNAL_CONTENT_URI, cv) ?: return
            contentResolver.openOutputStream(uri)?.use { it.write(body) }
        } else {
            val dir = getExternalFilesDir(android.os.Environment.DIRECTORY_DOWNLOADS) ?: return
            java.io.File(dir, name).writeBytes(body)
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

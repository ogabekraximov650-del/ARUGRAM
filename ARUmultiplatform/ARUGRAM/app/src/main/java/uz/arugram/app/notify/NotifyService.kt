package uz.arugram.app.notify

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import uz.arugram.app.MainActivity
import uz.arugram.app.core.RustCore

/** Ilova ochiq (ko'rinib turgan) yoki yo'qligini bildiradi: ochiq vaqtda bildirishnoma chiqmaydi. */
object AppVisible {
    @Volatile var foreground = false
}

/**
 * Orqa fondagi xabar bildirishnomalari. Google xizmatlarisiz (FCM) Telegram o'zi ham shunday ishlaydi: ilova o'zining doimiy ulanishini
 * "oldingi reja xizmati" sifatida ushlab turadi, Rust yadrosining `update_seq` hisoblagichi o'zgarsa suhbatlar ro'yxatini olib, yangi kiruvchi
 * xabarlar uchun bildirishnoma chiqaradi.
 */
class NotifyService : Service() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var job: Job? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        createChannels(this)
        val n = NotificationCompat.Builder(this, CH_SERVICE)
            .setSmallIcon(android.R.drawable.stat_notify_chat)
            .setContentTitle("ARUGRAM")
            .setContentText("Xabarlarni kutmoqda")
            .setPriority(NotificationCompat.PRIORITY_MIN)
            .setOngoing(true)
            .setContentIntent(openApp(this))
            .build()
        if (Build.VERSION.SDK_INT >= 29) startForeground(1, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC) else startForeground(1, n)
        if (job?.isActive != true) job = scope.launch { loop() }
        return START_STICKY
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    private suspend fun loop() {
        val prefs = getSharedPreferences("notify_state", MODE_PRIVATE)
        val settings = getSharedPreferences("settings", MODE_PRIVATE)
        var last = -1L
        var since = 0
        var firstRun = true
        while (scope.isActive) {
            val power = settings.getBoolean("power", false)
            val seq = RustCore.tgUpdateSeq()
            if (firstRun || seq != last || since >= (if (power) 30 else 60)) {
                last = seq
                since = 0
                scan(prefs, settings, silent = firstRun || AppVisible.foreground)
                firstRun = false
            }
            since++
            delay(if (power) 6_000 else 2_500)
        }
    }

    private fun scan(prefs: android.content.SharedPreferences, settings: android.content.SharedPreferences, silent: Boolean) {
        val j = RustCore.tgDialogs(30)
        if (j.optString("error").isNotEmpty()) return
        val arr = j.optJSONArray("dialogs") ?: return
        val nm = getSystemService(NOTIFICATION_SERVICE) as NotificationManager
        val edit = prefs.edit()
        for (i in 0 until arr.length()) {
            val o = arr.getJSONObject(i)
            val id = o.optLong("id")
            val top = o.optInt("top")
            val key = "top_$id"
            val known = prefs.getInt(key, -1)
            if (top > known) {
                edit.putInt(key, top)
                val unread = o.optInt("unread")
                val group = o.optString("kind") != "user"
                val wanted = !silent && known != -1 && unread > 0 && !o.optBoolean("muted") && !o.optBoolean("out") &&
                    settings.getBoolean("notify", true) && (settings.getBoolean("notify_groups", true) || !group)
                if (wanted) {
                    val text = if (settings.getBoolean("notify_preview", true)) o.optString("text").ifEmpty { "Yangi xabar" } else "Yangi xabar"
                    val n = NotificationCompat.Builder(this, CH_MESSAGES)
                        .setSmallIcon(android.R.drawable.stat_notify_chat)
                        .setContentTitle(o.optString("name"))
                        .setContentText(text)
                        .setNumber(unread)
                        .setStyle(NotificationCompat.BigTextStyle().bigText(text))
                        .setAutoCancel(true)
                        .setCategory(Notification.CATEGORY_MESSAGE)
                        .setPriority(NotificationCompat.PRIORITY_HIGH)
                        .setContentIntent(openApp(this))
                        .also { if (!settings.getBoolean("notify_vibrate", true)) it.setVibrate(longArrayOf(0)) }
                        .build()
                    runCatching { nm.notify(id.hashCode(), n) }
                }
            }
        }
        edit.apply()
    }

    companion object {
        const val CH_SERVICE = "arugram_service"
        const val CH_MESSAGES = "arugram_messages"

        fun createChannels(ctx: Context) {
            if (Build.VERSION.SDK_INT < 26) return
            val nm = ctx.getSystemService(NOTIFICATION_SERVICE) as NotificationManager
            nm.createNotificationChannel(NotificationChannel(CH_SERVICE, "Fon ulanishi", NotificationManager.IMPORTANCE_MIN))
            nm.createNotificationChannel(NotificationChannel(CH_MESSAGES, "Xabarlar", NotificationManager.IMPORTANCE_HIGH))
        }

        private fun openApp(ctx: Context): PendingIntent =
            PendingIntent.getActivity(ctx, 0, Intent(ctx, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)

        fun start(ctx: Context) {
            runCatching { androidx.core.content.ContextCompat.startForegroundService(ctx, Intent(ctx, NotifyService::class.java)) }
        }

        fun stop(ctx: Context) {
            runCatching { ctx.stopService(Intent(ctx, NotifyService::class.java)) }
        }
    }
}

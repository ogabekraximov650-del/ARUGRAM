package uz.arutube.app

import kotlinx.serialization.Serializable
import java.text.SimpleDateFormat
import java.util.Locale
import java.util.TimeZone

@Serializable
data class Video(
    val code: String = "",
    val title: String = "",
    val duration: Long = 0,
    val uploaded: String = "",
    val views: Long = 0,
    val thumb: String = "",
    val embed: String = "",
    val canplay: Int = 1,
)

@Serializable
data class VideoPage(
    val videos: List<Video> = emptyList(),
    val page: Int = 1,
    val pages: Int = 1,
    val total: Int = 0,
    val error: String? = null,
)

@Serializable
data class Ticket(val url: String = "", val key: String = "", val error: String? = null)

@Serializable
data class Remote(val ok: Boolean = false, val code: String = "", val error: String? = null)

@Serializable
data class DoodFile(val filecode: String = "", val download_url: String = "")

@Serializable
data class UploadResult(
    val status: Int = 0,
    val msg: String = "",
    val result: List<DoodFile> = emptyList(),
    val error: String? = null,
)

object Format {
    /** 6001 -> "1:40:01", 75 -> "1:15". */
    fun duration(sec: Long): String {
        if (sec <= 0) return "0:00"
        val h = sec / 3600
        val m = (sec % 3600) / 60
        val s = sec % 60
        return if (h > 0) String.format(Locale.US, "%d:%02d:%02d", h, m, s) else String.format(Locale.US, "%d:%02d", m, s)
    }

    /** 1200 -> "1.2K", 3_400_000 -> "3.4M". */
    fun count(n: Long): String = when {
        n >= 1_000_000 -> String.format(Locale.US, "%.1fM", n / 1_000_000.0)
        n >= 1_000 -> String.format(Locale.US, "%.1fK", n / 1_000.0)
        else -> n.toString()
    }

    fun size(bytes: Long): String = when {
        bytes >= 1L shl 30 -> String.format(Locale.US, "%.2f GB", bytes / (1L shl 30).toDouble())
        bytes >= 1L shl 20 -> String.format(Locale.US, "%.1f MB", bytes / (1L shl 20).toDouble())
        bytes >= 1L shl 10 -> String.format(Locale.US, "%.0f KB", bytes / (1L shl 10).toDouble())
        else -> "$bytes B"
    }

    /** "2026-11-06 07:54:34" (UTC) -> "2 kun oldin"; tushunarsiz bo'lsa "". */
    fun ago(uploaded: String, now: Long = System.currentTimeMillis()): String {
        val t = try {
            SimpleDateFormat("yyyy-MM-dd HH:mm:ss", Locale.US).apply { timeZone = TimeZone.getTimeZone("UTC") }.parse(uploaded)?.time
        } catch (e: Exception) {
            null
        } ?: return ""
        val s = (now - t) / 1000
        return when {
            s < 60 -> "hozirgina"
            s < 3600 -> "${s / 60} daqiqa oldin"
            s < 86400 -> "${s / 3600} soat oldin"
            s < 86400L * 30 -> "${s / 86400} kun oldin"
            s < 86400L * 365 -> "${s / (86400L * 30)} oy oldin"
            else -> "${s / (86400L * 365)} yil oldin"
        }
    }

    /** "2026-11-06 07:54:34" -> "2026-11-06". */
    fun date(s: String): String = s.substringBefore(' ')
}

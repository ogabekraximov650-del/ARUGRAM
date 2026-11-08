package uz.arudrive.app.data

import kotlinx.serialization.Serializable

/** Ro'yxatdagi bitta fayl (`GET /api/bd/list`). `state`: ready | uploaded | storing. */
@Serializable
data class DriveFile(
    val id: String,
    val name: String = "",
    val kind: String = "file",
    val mime: String = "",
    val size: Long = 0,
    val w: Int = 0,
    val h: Int = 0,
    val dur: Long = 0,
    val state: String = "",
    val created: Long = 0,
    /** Telegram bo'laklari: jami va hozirgacha saqlangani (fayl saqlanayotgan paytda ham birinchi bo'lak tayyor bo'lsa video ochiladi). */
    val chunks: Int = 0,
    val stored: Int = 0,
) {
    val isVideo get() = kind == "video"
    val ready get() = state == "ready"
    /** Birinchi bo'lak saqlangan bo'lsa — saqlash tugamagan bo'lsa ham ijro etiladi. */
    val playable get() = ready || stored > 0
}

@Serializable
data class DriveList(val files: List<DriveFile> = emptyList())

/** Kartochka nisbati (eni/bo'yi): video o'lchamiga qarab yotiq yoki tik, o'lcham noma'lum bo'lsa 16:9. */
fun cardRatio(w: Int, h: Int): Float =
    if (w <= 0 || h <= 0) 16f / 9f else (w.toFloat() / h).coerceIn(9f / 16f, 16f / 9f)

fun humanSize(b: Long): String = when {
    b >= 1L shl 30 -> String.format(java.util.Locale.US, "%.2f GB", b / (1L shl 30).toDouble())
    b >= 1L shl 20 -> String.format(java.util.Locale.US, "%.1f MB", b / (1L shl 20).toDouble())
    b >= 1L shl 10 -> String.format(java.util.Locale.US, "%.0f KB", b / 1024.0)
    else -> "$b B"
}

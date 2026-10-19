package uz.arugram.app.ui.main

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.webkit.MimeTypeMap
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextDecoration
import androidx.core.content.FileProvider
import java.io.File
import java.util.Calendar

/** Fayl hajmi: `1,5 MB`. */
fun formatSize(bytes: Long): String = when {
    bytes < 1024 -> "$bytes B"
    bytes < 1024 * 1024 -> "%.1f KB".format(bytes / 1024.0).replace('.', ',')
    bytes < 1024L * 1024 * 1024 -> "%.1f MB".format(bytes / 1048576.0).replace('.', ',')
    else -> "%.2f GB".format(bytes / 1073741824.0).replace('.', ',')
}

/** Davomiylik: `0:07` / `1:02:03`. */
fun formatDuration(seconds: Double): String {
    val t = seconds.toInt().coerceAtLeast(0)
    val h = t / 3600
    val m = (t % 3600) / 60
    val s = t % 60
    return if (h > 0) "%d:%02d:%02d".format(h, m, s) else "%d:%02d".format(m, s)
}

private val UrlRegex = Regex("""(https?://[^\s<>"]+|www\.[^\s<>"]+|t\.me/[^\s<>"]+)""", RegexOption.IGNORE_CASE)
private val MentionRegex = Regex("""(^|\s)(@[A-Za-z0-9_]{4,32})""")

/** Matndagi havola va @foydalanuvchi nomlarini ajratadi (Telegram'dagidek ko'k, tagi chizilmagan). `url` izohi bilan bosilganda ochiladi. */
fun linkify(text: String, linkColor: Color): AnnotatedString = buildAnnotatedString {
    append(text)
    UrlRegex.findAll(text).forEach { m ->
        addStyle(SpanStyle(color = linkColor, textDecoration = TextDecoration.None), m.range.first, m.range.last + 1)
        val raw = m.value.trimEnd('.', ',', ')', '!', '?', ';', ':')
        val url = if (raw.startsWith("http", true)) raw else "https://$raw"
        addStringAnnotation("url", url, m.range.first, m.range.first + raw.length)
    }
    MentionRegex.findAll(text).forEach { m ->
        val g = m.groups[2]!!
        addStyle(SpanStyle(color = linkColor), g.range.first, g.range.last + 1)
        addStringAnnotation("url", "https://t.me/" + g.value.removePrefix("@"), g.range.first, g.range.last + 1)
    }
}

fun openUrl(context: Context, url: String) {
    runCatching {
        context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }
}

fun copyText(context: Context, text: String) {
    val cm = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    cm.setPrimaryClip(ClipData.newPlainText("ARUGRAM", text))
}

private fun mimeFor(file: File, hint: String): String {
    if (hint.isNotEmpty()) return hint
    val ext = file.extension.lowercase()
    return MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext) ?: "*/*"
}

/** Yuklangan faylni tizim ilovasi bilan ochadi. */
fun openFile(context: Context, file: File, mime: String = "") {
    runCatching {
        val uri = FileProvider.getUriForFile(context, context.packageName + ".files", file)
        context.startActivity(
            Intent(Intent.ACTION_VIEW).setDataAndType(uri, mimeFor(file, mime))
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK),
        )
    }
}

/** Faylni boshqa ilovaga ulashadi. */
fun shareFile(context: Context, file: File, mime: String = "") {
    runCatching {
        val uri = FileProvider.getUriForFile(context, context.packageName + ".files", file)
        val send = Intent(Intent.ACTION_SEND).setType(mimeFor(file, mime)).putExtra(Intent.EXTRA_STREAM, uri)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        context.startActivity(Intent.createChooser(send, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }
}

/** Galereyadan tanlangan `content://` faylni kesh papkaga nusxalaydi (Rust yadrosi oddiy fayl yo'lini kutadi). */
fun copyUriToCache(context: Context, uri: Uri): File? = runCatching {
    var name = "fayl"
    context.contentResolver.query(uri, null, null, null, null)?.use { c ->
        val i = c.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
        if (i >= 0 && c.moveToFirst()) name = c.getString(i) ?: name
    }
    val dir = File(context.cacheDir, "send").apply { mkdirs() }
    val out = File(dir, "${System.currentTimeMillis()}_${name.replace('/', '_')}")
    context.contentResolver.openInputStream(uri)?.use { input -> out.outputStream().use { input.copyTo(it) } }
    out.takeIf { it.length() > 0 }
}.getOrNull()

/** Kun kaliti (mahalliy vaqt bo'yicha). */
fun dayKey(unix: Long): Long = (unix * 1000 + java.util.TimeZone.getDefault().getOffset(unix * 1000)) / 86_400_000L

private val Months = arrayOf("yanvar", "fevral", "mart", "aprel", "may", "iyun", "iyul", "avgust", "sentyabr", "oktyabr", "noyabr", "dekabr")

/** "Bugun" / "7 mart" / "7 mart 2020". */
fun dateLabel(unix: Long): String {
    val now = Calendar.getInstance()
    val d = Calendar.getInstance().apply { timeInMillis = unix * 1000 }
    return when {
        dayKey(unix) == dayKey(now.timeInMillis / 1000) -> "Bugun"
        now.get(Calendar.YEAR) == d.get(Calendar.YEAR) -> "${d.get(Calendar.DAY_OF_MONTH)} ${Months[d.get(Calendar.MONTH)]}"
        else -> "${d.get(Calendar.DAY_OF_MONTH)} ${Months[d.get(Calendar.MONTH)]} ${d.get(Calendar.YEAR)}"
    }
}

/** Soat:daqiqa. */
fun clock(unix: Long): String {
    val d = Calendar.getInstance().apply { timeInMillis = unix * 1000 }
    return "%02d:%02d".format(d.get(Calendar.HOUR_OF_DAY), d.get(Calendar.MINUTE))
}

/** Foydalanuvchi holati: "onlayn", "bugun 12:30 da onlayn edi" ... (`@<unix>` — Rust yuborgan oxirgi onlayn vaqt). */
fun statusText(raw: String): String {
    if (!raw.startsWith("@")) return raw
    val ts = raw.drop(1).toLongOrNull() ?: return "uzoq vaqt oldin onlayn edi"
    val now = Calendar.getInstance()
    val d = Calendar.getInstance().apply { timeInMillis = ts * 1000 }
    return when {
        dayKey(ts) == dayKey(now.timeInMillis / 1000) -> "bugun ${clock(ts)} da onlayn edi"
        dayKey(ts) == dayKey(now.timeInMillis / 1000) - 1 -> "kecha ${clock(ts)} da onlayn edi"
        else -> "%02d.%02d.%02d da onlayn edi".format(d.get(Calendar.DAY_OF_MONTH), d.get(Calendar.MONTH) + 1, d.get(Calendar.YEAR) % 100)
    }
}

/** Matndagi havolani ilovaning o'zida ochishga urinadi (t.me/<nom> — suhbat). `true` — ishlandi, `false` — brauzerga beriladi. */
val LocalOpenLink = androidx.compose.runtime.staticCompositionLocalOf<(String) -> Boolean> { { false } }

/** `https://t.me/<nom>` dan nomni ajratadi (taklif havolalari va ichki yo'llar uchun null). */
fun tmeUsername(url: String): String? {
    val m = Regex("""^https?://(?:www\.)?t\.me/([A-Za-z0-9_]{4,32})/?(?:\?.*)?$""").find(url) ?: return null
    val n = m.groupValues[1]
    return if (n.equals("joinchat", true) || n.equals("addstickers", true) || n.equals("share", true) || n.equals("c", true)) null else n
}

/** Taklif havolasi (`t.me/+hash`, `t.me/joinchat/hash`) hash'i. */
fun tmeInvite(url: String): String? =
    Regex("""^https?://(?:www\.)?t\.me/(?:\+|joinchat/)([A-Za-z0-9_-]{8,})/?$""").find(url)?.groupValues?.get(1)

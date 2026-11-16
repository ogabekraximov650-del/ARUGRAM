package uz.arumediatv.com.data

import android.content.Context
import android.content.Intent
import android.net.Uri
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * UZ: Havola orqali ochish (YouTube/Instagram kabi). `https://arumediatvuz.pages.dev/s/<anime>/<bo'lim>/<qism>`.
 * Ilova o'rnatilgan bo'lsa to'g'ridan-to'g'ri shu qism ochiladi; yo'q bo'lsa sayt sahifasi ochiladi (`link-web/`).
 * EN: share/open links. Installed app opens the exact episode; otherwise the web landing page opens.
 * RU: ssylki: pri ustanovlennom prilozhenii otkryvaetsya imenno eta seriya, inache — veb-stranitsa.
 */
object DeepLink {
    const val HOST = "arumediatvuz.pages.dev"

    data class Target(val animeId: Long, val seasonId: Long, val episodeId: Long)

    fun url(animeId: Long, seasonId: Long, episodeId: Long = 0): String =
        "https://$HOST/s/$animeId/$seasonId" + if (episodeId > 0) "/$episodeId" else ""

    /** Sof tahlil (testlanadi): `/s/12/3/45` -> Target(12, 3, 45); noto'g'ri bo'lsa `null`. */
    fun parsePath(path: String?): Target? {
        val p = path.orEmpty().trim('/').split('/')
        if (p.size < 3 || p.size > 4 || p[0] != "s") return null
        val a = p[1].toLongOrNull() ?: return null
        val s = p[2].toLongOrNull() ?: return null
        val e = if (p.size == 4) (p[3].toLongOrNull() ?: return null) else 0L
        if (a <= 0 || s < 0 || e < 0) return null
        return Target(a, s, e)
    }

    fun parse(uri: Uri?): Target? =
        if (uri != null && uri.scheme in setOf("https", "http") && uri.host.equals(HOST, ignoreCase = true)) parsePath(uri.path) else null

    private val _pending = MutableStateFlow<Target?>(null)
    /** Hali ochilmagan havola (ilova yuklanishini, kirishni kutadi). */
    val pending: StateFlow<Target?> = _pending.asStateFlow()

    fun offer(intent: Intent?) {
        if (intent?.action != Intent.ACTION_VIEW) return
        parse(intent.data)?.let { _pending.value = it }
    }

    fun consume() { _pending.value = null }

    /** "Ulashish" oynasi: matn + havola. */
    fun share(ctx: Context, title: String, episodeNo: Int, animeId: Long, seasonId: Long, episodeId: Long) {
        val link = url(animeId, seasonId, episodeId)
        val text = "$title · $episodeNo-qism\n$link"
        val send = Intent(Intent.ACTION_SEND).apply {
            type = "text/plain"
            putExtra(Intent.EXTRA_TEXT, text)
        }
        ctx.startActivity(Intent.createChooser(send, "Ulashish").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }
}

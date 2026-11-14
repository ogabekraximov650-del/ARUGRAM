package uz.arumediatv.com.ui.admin

import android.annotation.SuppressLint
import android.content.Intent
import android.webkit.CookieManager
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.FileProvider
import uz.arumediatv.com.ui.profile.OverlayScaffold
import uz.arumediatv.com.ui.theme.AruColors
import java.io.File

/**
 * UZ: YouTube cookie'larini olish: WebView'da alohida akkaunt bilan kirasiz, "Cookie olish" `cookies.txt` (Netscape) yasaydi va Telegram'ga (ytbot) ulashadi.
 * EN: Export YouTube cookies: sign in inside the WebView, "Cookie olish" builds `cookies.txt` (Netscape) and shares it to Telegram (ytbot).
 */
object CookieExport {
    /** UZ: Kirish belgisi bo'ladigan cookie'lar (hech biri bo'lmasa — akkauntga kirilmagan). EN: auth cookies; none present means not signed in. */
    val AUTH_NAMES = setOf("SAPISID", "__Secure-3PAPISID", "SID", "__Secure-3PSID", "LOGIN_INFO")

    /** `("a=b; c=d")` ni (nom, qiymat) juftlariga ajratadi. */
    fun parse(header: String?): List<Pair<String, String>> =
        header.orEmpty().split(';').mapNotNull { p ->
            val i = p.indexOf('=')
            if (i <= 0) null else p.substring(0, i).trim() to p.substring(i + 1).trim()
        }.filter { it.first.isNotEmpty() }

    /** (domen, nom, qiymat) ro'yxatidan Netscape `cookies.txt` matni. Takrorlar (domen+nom) tashlanadi. */
    fun netscape(cookies: List<Triple<String, String, String>>, nowSec: Long): String {
        val expiry = nowSec + 365L * 24 * 3600
        val seen = HashSet<String>()
        val sb = StringBuilder("# Netscape HTTP Cookie File\n# ARUMEDIATV: YouTube cookie\n\n")
        for ((domain, name, value) in cookies) {
            if (!seen.add("$domain|$name")) continue
            // __Host- cookie'lari domen bilan bog'lanmaydi (subdomainsiz), qolganlari ".domen".
            val sub = if (name.startsWith("__Host-")) "FALSE" else "TRUE"
            sb.append("$domain\t$sub\t/\tTRUE\t$expiry\t$name\t$value\n")
        }
        return sb.toString()
    }

    fun signedIn(cookies: List<Triple<String, String, String>>): Boolean = cookies.any { it.second in AUTH_NAMES }

    private val SOURCES = listOf(
        "https://www.youtube.com" to ".youtube.com",
        "https://youtube.com" to ".youtube.com",
        "https://accounts.google.com" to ".google.com",
        "https://www.google.com" to ".google.com",
        "https://google.com" to ".google.com",
    )

    fun collect(): List<Triple<String, String, String>> {
        val cm = CookieManager.getInstance()
        cm.flush()
        return SOURCES.flatMap { (url, domain) -> parse(cm.getCookie(url)).map { (n, v) -> Triple(domain, n, v) } }
    }

    /** Chrome (Android) ko'rinishidagi User-Agent: "; wv" belgisiz, Google kirishni kamroq bloklaydi. */
    const val UA = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36"
    const val START = "https://accounts.google.com/ServiceLogin?service=youtube&hl=en&continue=https%3A%2F%2Fwww.youtube.com%2F"
}

@SuppressLint("SetJavaScriptEnabled")
@Composable
fun AdminCookieScreen(onBack: () -> Unit) {
    val ctx = LocalContext.current
    var msg by remember { mutableStateOf("Alohida YouTube akkaunt bilan kiring (asosiy akkauntingiz bilan EMAS), so'ng pastdagi tugmani bosing.") }
    // Har safar toza sessiya: eski cookie'lar aralashmasin; chiqqanda ham tozalanadi (serverda sessiya o'chmaydi, fayl yaroqli qoladi).
    val web = remember {
        CookieManager.getInstance().removeAllCookies(null)
        WebView(ctx).apply {
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.userAgentString = CookieExport.UA
            settings.mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
            CookieManager.getInstance().setAcceptCookie(true)
            CookieManager.getInstance().setAcceptThirdPartyCookies(this, true)
            webViewClient = WebViewClient()
            loadUrl(CookieExport.START)
        }
    }
    DisposableEffect(Unit) {
        onDispose {
            web.stopLoading(); web.destroy()
            CookieManager.getInstance().removeAllCookies(null)
        }
    }
    OverlayScaffold("YouTube cookie", onBack) {
        Column(Modifier.fillMaxSize()) {
            Box(Modifier.weight(1f).fillMaxWidth()) { AndroidView(factory = { web }, modifier = Modifier.fillMaxSize()) }
            Text(msg, color = Color.White.copy(alpha = 0.7f), fontSize = 12.sp, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
            val shape = RoundedCornerShape(16.dp)
            Box(
                Modifier.padding(start = 16.dp, end = 16.dp, bottom = 16.dp).fillMaxWidth().clip(shape).background(AruColors.Accent2)
                    .clickable {
                        val cookies = CookieExport.collect()
                        if (!CookieExport.signedIn(cookies)) {
                            msg = "Akkauntga kirilmagan ko'rinadi (kirish cookie'lari yo'q). Avval YouTube'ga kiring."
                            return@clickable
                        }
                        try {
                            val dir = File(ctx.cacheDir, "cookies").apply { mkdirs() }
                            val f = File(dir, "cookies.txt")
                            f.writeText(CookieExport.netscape(cookies, System.currentTimeMillis() / 1000))
                            val uri = FileProvider.getUriForFile(ctx, "${ctx.packageName}.files", f)
                            val send = Intent(Intent.ACTION_SEND).apply {
                                type = "text/plain"
                                putExtra(Intent.EXTRA_STREAM, uri)
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                            }
                            ctx.startActivity(Intent.createChooser(send, "cookies.txt ni @ARUdownlodbot ga yuboring").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                            msg = "${cookies.size} ta cookie olindi. Telegram'ni tanlab, @ARUdownlodbot chatiga yuboring."
                        } catch (e: Exception) {
                            msg = "Xato: ${e.message}"
                        }
                    }.padding(vertical = 16.dp),
                contentAlignment = Alignment.Center,
            ) { Text("🍪 Cookie olish va yuborish", color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.Bold) }
        }
    }
}

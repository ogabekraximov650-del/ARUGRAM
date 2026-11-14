package uz.arumediatv.com.ui.admin

import android.annotation.SuppressLint
import android.content.Intent
import android.graphics.Bitmap
import android.webkit.CookieManager
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
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
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.FileProvider
import uz.arumediatv.com.ui.profile.OverlayScaffold
import uz.arumediatv.com.ui.theme.AruColors
import java.io.File
import java.net.URLEncoder

/**
 * UZ: Brauzer: nom yoki havola bilan istalgan saytni ochasiz (Chrome kabi), kirasiz va "Cookie olish" shu sayt uchun `cookies.txt` (Netscape) yasab Telegram'ga (ytbot) ulashadi.
 * EN: Browser: open any site by name or URL (like Chrome), sign in, and "Cookie olish" builds `cookies.txt` (Netscape) for that site and shares it to Telegram (ytbot).
 */
object CookieExport {
    /** UZ: Kirish belgisi bo'ladigan cookie'lar (xabar uchun; export ularsiz ham ishlaydi). EN: auth cookies (only used for the hint). */
    val AUTH_NAMES = setOf("SAPISID", "__Secure-3PAPISID", "SID", "__Secure-3PSID", "LOGIN_INFO", "sessionid", "ds_user_id", "auth_token", "c_user", "xs")

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
        val sb = StringBuilder("# Netscape HTTP Cookie File\n# ARUMEDIATV: cookie\n\n")
        for ((domain, name, value) in cookies) {
            if (!seen.add("$domain|$name")) continue
            // __Host- cookie'lari faqat o'sha xostga tegishli: domen nuqtasiz va subdomen FALSE (yt-dlp: nuqta bor <=> TRUE bo'lishi shart).
            val host = name.startsWith("__Host-")
            sb.append("${if (host) domain.trimStart('.') else domain}\t${if (host) "FALSE" else "TRUE"}\t/\tTRUE\t$expiry\t$name\t$value\n")
        }
        return sb.toString()
    }

    fun signedIn(cookies: List<Triple<String, String, String>>): Boolean = cookies.any { it.second in AUTH_NAMES }

    /** UZ: Chrome kabi: havola bo'lsa ochadi, aks holda Google'dan qidiradi. EN: like Chrome: a URL is opened, anything else is searched on Google. */
    fun resolveInput(input: String): String {
        val s = input.trim()
        if (s.isEmpty()) return "https://www.google.com"
        if (s.startsWith("http://", true) || s.startsWith("https://", true)) return s
        val looksLikeHost = !s.contains(' ') && s.contains('.') && !s.startsWith(".") && !s.endsWith(".")
        return if (looksLikeHost) "https://$s" else "https://www.google.com/search?q=" + URLEncoder.encode(s, "UTF-8")
    }

    /** UZ: `www.m.instagram.com` -> `instagram.com`; `x.co.uk` -> `x.co.uk`. EN: registrable domain of a host. */
    fun baseDomain(host: String): String {
        val p = host.lowercase().trim('.').split('.')
        if (p.size <= 2) return p.joinToString(".")
        val second = setOf("co", "com", "org", "net", "gov", "edu", "ac")
        return if (p.last().length == 2 && p[p.size - 2] in second) p.takeLast(3).joinToString(".") else p.takeLast(2).joinToString(".")
    }

    /** UZ: shu sayt kirishi uchun kerak bo'lgan domenlar (YouTube kirishi Google cookie'larini ham talab qiladi). EN: domains needed for the site's login. */
    fun relatedDomains(host: String): List<String> {
        val b = baseDomain(host)
        return when {
            b == "youtube.com" || b == "youtu.be" || b == "google.com" -> listOf("youtube.com", "google.com")
            else -> listOf(b)
        }.filter { it.contains('.') }
    }

    fun collect(host: String): List<Triple<String, String, String>> {
        val cm = CookieManager.getInstance()
        cm.flush()
        return relatedDomains(host).flatMap { d ->
            val urls = buildList {
                add("https://www.$d"); add("https://$d")
                if (d == "google.com") add("https://accounts.google.com")
                if (baseDomain(host) == d && host != "www.$d" && host != d) add("https://$host")
            }
            urls.flatMap { u -> parse(cm.getCookie(u)).map { (n, v) -> Triple(".$d", n, v) } }
        }
    }

    /** Chrome (Android) ko'rinishidagi User-Agent: "; wv" belgisiz, Google kirishni kamroq bloklaydi. */
    const val UA = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36"
}

@SuppressLint("SetJavaScriptEnabled")
@Composable
fun AdminCookieScreen(onBack: () -> Unit) {
    val ctx = LocalContext.current
    var url by remember { mutableStateOf("") }
    var input by remember { mutableStateOf("") }
    var editing by remember { mutableStateOf(false) }
    var msg by remember { mutableStateOf("Sayt nomi yoki havolasini yozing, alohida akkaunt bilan kiring (asosiy akkauntingiz bilan EMAS), so'ng pastdagi tugmani bosing.") }
    // Toza sessiya: eski cookie'lar aralashmasin; chiqqanda ham tozalanadi (serverda sessiya o'chmaydi, fayl yaroqli qoladi).
    val web = remember {
        CookieManager.getInstance().removeAllCookies(null)
        WebView(ctx).apply {
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.userAgentString = CookieExport.UA
            settings.mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
            CookieManager.getInstance().setAcceptCookie(true)
            CookieManager.getInstance().setAcceptThirdPartyCookies(this, true)
            webViewClient = object : WebViewClient() {
                override fun onPageStarted(v: WebView, u: String?, f: Bitmap?) { if (!u.isNullOrEmpty()) url = u }
                override fun doUpdateVisitedHistory(v: WebView, u: String?, isReload: Boolean) { if (!u.isNullOrEmpty()) url = u }
                // intent://, market:// kabi o'zga ilovaga ochuvchi havolalar o'tkazilmaydi: faqat brauzer ichida.
                override fun shouldOverrideUrlLoading(v: WebView, r: android.webkit.WebResourceRequest): Boolean =
                    r.url.scheme != "http" && r.url.scheme != "https"
            }
            loadUrl("https://www.google.com")
        }
    }
    DisposableEffect(Unit) {
        onDispose {
            web.stopLoading(); web.destroy()
            CookieManager.getInstance().removeAllCookies(null)
        }
    }
    // Orqaga: avval sahifa tarixida, tarix tugagach ekrandan chiqadi.
    BackHandler { if (web.canGoBack()) web.goBack() else onBack() }
    OverlayScaffold("Brauzer", onBack) {
        Column(Modifier.fillMaxSize()) {
            Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                val btn = RoundedCornerShape(12.dp)
                Box(Modifier.clip(btn).background(AruColors.CardAlt).clickable { if (web.canGoBack()) web.goBack() }.padding(horizontal = 12.dp, vertical = 10.dp)) { Text("◀", color = Color.White, fontSize = 14.sp) }
                Box(Modifier.clip(btn).background(AruColors.CardAlt).clickable { if (web.canGoForward()) web.goForward() }.padding(horizontal = 12.dp, vertical = 10.dp)) { Text("▶", color = Color.White, fontSize = 14.sp) }
                Box(Modifier.weight(1f).clip(btn).background(AruColors.CardAlt).padding(horizontal = 12.dp, vertical = 10.dp)) {
                    BasicTextField(
                        value = if (editing) input else url,
                        onValueChange = { input = it; editing = true },
                        singleLine = true,
                        textStyle = TextStyle(color = Color.White, fontSize = 14.sp),
                        cursorBrush = SolidColor(Color.White),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go),
                        keyboardActions = KeyboardActions(onGo = { editing = false; web.loadUrl(CookieExport.resolveInput(input)) }),
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                Box(Modifier.clip(btn).background(AruColors.CardAlt).clickable { web.reload() }.padding(horizontal = 12.dp, vertical = 10.dp)) { Text("⟳", color = Color.White, fontSize = 14.sp) }
            }
            Box(Modifier.weight(1f).fillMaxWidth()) { AndroidView(factory = { web }, modifier = Modifier.fillMaxSize()) }
            Text(msg, color = Color.White.copy(alpha = 0.7f), fontSize = 12.sp, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
            val shape = RoundedCornerShape(16.dp)
            Box(
                Modifier.padding(start = 16.dp, end = 16.dp, bottom = 16.dp).fillMaxWidth().clip(shape).background(AruColors.Accent2)
                    .clickable {
                        val host = try { android.net.Uri.parse(web.url ?: url).host.orEmpty() } catch (e: Exception) { "" }
                        if (host.isEmpty()) { msg = "Avval sayt oching."; return@clickable }
                        val cookies = CookieExport.collect(host)
                        if (cookies.isEmpty()) {
                            msg = "$host uchun cookie yo'q. Avval saytga kiring."
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
                            val auth = if (CookieExport.signedIn(cookies)) "kirilgan" else "kirish cookie'lari topilmadi (kirmagan bo'lishingiz mumkin)"
                            msg = "$host: ${cookies.size} ta cookie ($auth). Telegram'ni tanlab, @ARUdownlodbot chatiga yuboring."
                        } catch (e: Exception) {
                            msg = "Xato: ${e.message}"
                        }
                    }.padding(vertical = 16.dp),
                contentAlignment = Alignment.Center,
            ) { Text("🍪 Cookie olish va yuborish", color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.Bold) }
        }
    }
}

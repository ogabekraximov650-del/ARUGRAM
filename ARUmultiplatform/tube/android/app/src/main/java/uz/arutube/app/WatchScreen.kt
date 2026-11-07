package uz.arutube.app

import android.app.Activity
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.Share
import androidx.compose.material.icons.filled.ThumbUp
import androidx.compose.foundation.layout.size
import androidx.core.view.WindowCompat
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.activity.compose.BackHandler
import android.content.Intent
import android.content.pm.ActivityInfo
import android.graphics.Color as AColor
import android.net.Uri
import android.view.View
import android.view.ViewGroup
import android.webkit.CookieManager
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView

/** To'liq ekran: pleyer tugmasi bosilganda Activity ustiga chiqariladi (gorizontal). */
private class FsChrome(private val act: Activity) : WebChromeClient() {
    private var view: View? = null
    private var cb: WebChromeClient.CustomViewCallback? = null

    override fun onShowCustomView(v: View, c: WebChromeClient.CustomViewCallback) {
        if (view != null) { c.onCustomViewHidden(); return }
        view = v
        cb = c
        (act.window.decorView as FrameLayout).addView(v, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        act.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
    }

    override fun onHideCustomView() {
        view?.let { (act.window.decorView as FrameLayout).removeView(it) }
        view = null
        cb?.onCustomViewHidden()
        cb = null
        act.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
    }
}

/** Pleyer rejimi: IFRAME — `<iframe src=embed>` (so'ralgan usul); DIRECT — embed sahifasi WebView'ning o'zida. Biri ishlamasa ikkinchisiga o'tish mumkin. */
enum class PlayerMode(val label: String) { NATIVE("Pleyer: reklamasiz"), IFRAME("Pleyer: iframe"), DIRECT("Pleyer: to'g'ridan") }

/**
 * DoodStream pleyeri. IFRAME rejimida sahifa bazaviy manzili embed domeni (Referer/origin to'g'ri), uchinchi tomon cookie'lari yoqilgan
 * (Cloudflare Turnstile iframe ichida shuni talab qiladi — avvalgi versiyada qora ekranning sababi shu bo'lishi mumkin).
 * Boshqa saytga o'tishga urinish (reklama) bloklanadi.
 */
@Composable
fun EmbedPlayer(embed: String, mode: PlayerMode, modifier: Modifier = Modifier, adBlock: Boolean = true) {
    val ctx = LocalContext.current
    var loading by remember(embed, mode, adBlock) { mutableStateOf(true) }
    Box(modifier.background(Color.Black).aspectRatio(16f / 9f)) {
        androidx.compose.runtime.key(embed, mode, adBlock) {
            AndroidView(
                modifier = Modifier.fillMaxSize(),
                factory = {
                    WebView(ctx).apply {
                        settings.javaScriptEnabled = true
                        settings.domStorageEnabled = true
                        settings.mediaPlaybackRequiresUserGesture = false
                        settings.javaScriptCanOpenWindowsAutomatically = false
                        settings.setSupportMultipleWindows(false)
                        // "wv" belgisi olib tashlanadi: sayt oddiy Chrome deb biladi.
                        settings.userAgentString = settings.userAgentString.replace("; wv", "")
                        CookieManager.getInstance().setAcceptCookie(true)
                        CookieManager.getInstance().setAcceptThirdPartyCookies(this, true)
                        setBackgroundColor(AColor.BLACK)
                        webChromeClient = FsChrome(ctx as Activity)
                        val host = Uri.parse(embed).host.orEmpty()
                        webViewClient = object : WebViewClient() {
                            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean =
                                request.isForMainFrame && (mode == PlayerMode.IFRAME || request.url.host != host)

                            override fun onPageFinished(view: WebView, url: String) {
                                loading = false
                            }

                            override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? =
                                if (adBlock && AdBlock.isAd(request.url)) AdBlock.empty() else null
                        }
                        if (adBlock && WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
                            WebViewCompat.addDocumentStartJavaScript(this, AdBlock.SCRIPT, setOf("*"))
                        }
                        if (mode == PlayerMode.IFRAME) {
                            val html = """<!doctype html><html><head><meta name="viewport" content="width=device-width, initial-scale=1">
<style>html,body{margin:0;height:100%;background:#000;overflow:hidden}iframe{position:fixed;left:0;top:0;width:100%;height:100%;border:0}</style></head>
<body><iframe src="$embed" referrerpolicy="origin" allowfullscreen allow="autoplay; fullscreen; encrypted-media; picture-in-picture"></iframe></body></html>"""
                            loadDataWithBaseURL("https://$host/", html, "text/html", "utf-8", null)
                        } else {
                            loadUrl(embed, mapOf("Referer" to "https://$host/"))
                        }
                    }
                },
                onRelease = { it.destroy() },
            )
        }
        if (loading) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                CircularProgressIndicator(color = TubeRed)
            }
        }
    }
}

/** YouTube'dagi doira shaklidagi harakat tugmasi: belgi + yozuv. */
@Composable
private fun ActionPill(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, active: Boolean, onClick: () -> Unit) {
    Row(
        Modifier.clip(RoundedCornerShape(20.dp)).background(if (active) Color(0xFF3F3F3F) else TubeChip).clickable(onClick = onClick).padding(horizontal = 14.dp, vertical = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        androidx.compose.material3.Icon(icon, contentDescription = null, tint = Color.White, modifier = Modifier.size(18.dp))
        Text(label, color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.Medium, modifier = Modifier.padding(start = 8.dp))
    }
}

@Composable
private fun ActionChip(label: String, onClick: () -> Unit) {
    Text(
        label,
        color = Color.White,
        fontSize = 13.sp,
        fontWeight = FontWeight.Medium,
        modifier = Modifier.clip(RoundedCornerShape(18.dp)).background(TubeChip).clickable(onClick = onClick).padding(horizontal = 14.dp, vertical = 8.dp),
    )
}

@Composable
fun WatchScreen(video: Video, holder: PlayerHolder, onMinimize: () -> Unit, onOpen: (Video) -> Unit) {
    val ctx = LocalContext.current
    val act = ctx as Activity
    val clip = LocalClipboardManager.current
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    var others by remember { mutableStateOf(listOf<Video>()) }
    var copied by remember(video.code) { mutableStateOf(false) }
    var mode by remember(video.code) { mutableStateOf(PlayerMode.NATIVE) }
    var note by remember(video.code) { mutableStateOf("") }
    var fs by remember { mutableStateOf(false) }
    var adBlock by remember { mutableStateOf(true) }

    LaunchedEffect(video.code) {
        val r = Api.videos(1)
        if (r.error == null) {
            others = r.videos.filter { it.code != video.code }
            Repo.merge(r.videos)
            // Joriy video ochilgach, "keyingi videolar"ning birinchi ikkitasi uchun havola tayyorlanadi.
            LinkCache.prefetch(scope, act, others, 2)
        }
    }
    // Reklamasiz rejim: pleyer holder'da (mini pleyer bilan umumiy); topilmasa iframe'ga o'tiladi.
    LaunchedEffect(video.code, mode) {
        if (mode == PlayerMode.NATIVE) {
            note = ""
            holder.open(video, act)
        } else {
            holder.close()
        }
    }
    LaunchedEffect(holder.status) {
        if (holder.status == "failed" && mode == PlayerMode.NATIVE && holder.video?.code == video.code) {
            note = "Reklamasiz havola topilmadi (captcha yoki tarmoq) — iframe ochildi."
            mode = PlayerMode.IFRAME
        }
    }
    // To'liq ekran: gorizontal + tizim panellari yashirin.
    LaunchedEffect(fs) {
        val c = WindowCompat.getInsetsController(act.window, act.window.decorView)
        if (fs) {
            act.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
            c.hide(WindowInsetsCompat.Type.systemBars())
            c.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        } else {
            act.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
            c.show(WindowInsetsCompat.Type.systemBars())
        }
    }
    BackHandler(enabled = fs) { fs = false }

    val exo = if (holder.video?.code == video.code) holder.exo else null
    if (fs && mode == PlayerMode.NATIVE && exo != null) {
        PlayerSurface(exo, { fs = it }, Modifier.fillMaxSize())
        return
    }

    // Pleyer tepada qotib turadi, pastda ma'lumot va keyingi videolar aylanadi.
    Column(Modifier.statusBarsPadding()) {
        if (mode == PlayerMode.NATIVE) {
            if (exo == null) {
                Box(Modifier.fillMaxWidth().aspectRatio(16f / 9f).background(Color.Black), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator(color = TubeRed)
                }
            } else {
                PlayerSurface(exo, { fs = it }, Modifier.fillMaxWidth().aspectRatio(16f / 9f))
            }
        } else {
            EmbedPlayer(video.embed, mode, Modifier.fillMaxWidth(), adBlock)
        }
        LazyColumn(Modifier.fillMaxWidth()) {
            item {
                Column(Modifier.padding(12.dp)) {
                    Text(video.title.ifBlank { video.code }, fontSize = 18.sp, lineHeight = 24.sp, fontWeight = FontWeight.SemiBold)
                    val ago = Format.ago(video.uploaded)
                    Text(
                        "${Format.count(video.views)} ko'rish" + (if (ago.isNotEmpty()) " · $ago" else "") + " · ${Format.duration(video.duration)}",
                        fontSize = 13.sp,
                        color = TubeGray,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                    if (note.isNotEmpty()) Text(note, fontSize = 12.sp, color = Color(0xFFFFB74D), modifier = Modifier.padding(top = 6.dp))
                    // Asosiy harakatlar (YouTube'dagidek): yoqdi, saqlash, ulashish, yuklab olish.
                    Row(Modifier.padding(top = 12.dp).horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        val liked = LocalStore.isLiked(video.code)
                        ActionPill(Icons.Filled.ThumbUp, if (liked) "Yoqdi ✓" else "Yoqdi", liked) { LocalStore.toggleLike(video.code) }
                        val saved = LocalStore.isLater(video.code)
                        ActionPill(if (saved) Icons.Filled.Check else Icons.Filled.Add, if (saved) "Saqlandi" else "Saqlash", saved) { LocalStore.toggleLater(video) }
                        ActionPill(Icons.Filled.Share, "Ulashish", false) { shareVideo(ctx, video) }
                        ActionPill(Icons.Filled.KeyboardArrowDown, "Yuklab olish", false) {
                            ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(video.downloadUrl)))
                        }
                    }
                    // Qo'shimcha sozlamalar.
                    Row(Modifier.padding(top = 8.dp).horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        ActionChip("⌄ Kichraytirish", onMinimize)
                        ActionChip(mode.label) {
                            val all = PlayerMode.values()
                            mode = all[(mode.ordinal + 1) % all.size]
                        }
                        ActionChip(if (adBlock) "Reklama bloki: yoqilgan" else "Reklama bloki: o'chiq") { adBlock = !adBlock }
                        ActionChip(if (copied) "✓ Nusxalandi" else "Havolani nusxalash") {
                            clip.setText(AnnotatedString(video.embed))
                            copied = true
                        }
                        ActionChip("Brauzerda ochish") { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(video.embed))) }
                    }
                }
                Spacer(Modifier.height(4.dp))
            }
            items(others, key = { it.code }) { v -> VideoCard(v, progress = LocalStore.progress(v.code)?.fraction ?: 0f) { onOpen(v) } }
        }
    }
}

package uz.arutube.app

import android.app.Activity
import android.content.Intent
import android.content.pm.ActivityInfo
import android.graphics.Color as AColor
import android.net.Uri
import android.view.View
import android.view.ViewGroup
import android.webkit.CookieManager
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
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

/**
 * DoodStream pleyeri: embed sahifasi (`<host>/e/<kod>`) WebView'ning O'ZIDA ochiladi (Cloudflare tekshiruvi va `X-Frame-Options`
 * tufayli boshqa sahifa ichidagi iframe qora qolardi). Boshqa saytga o'tishga urinish (reklama) bloklanadi.
 */
@Composable
fun EmbedPlayer(embed: String, modifier: Modifier = Modifier) {
    val ctx = LocalContext.current
    var loading by remember(embed) { mutableStateOf(true) }
    Box(modifier.background(Color.Black)) {
        AndroidView(
            modifier = Modifier.fillMaxWidth().aspectRatio(16f / 9f),
            factory = {
                WebView(ctx).apply {
                    settings.javaScriptEnabled = true
                    settings.domStorageEnabled = true
                    settings.mediaPlaybackRequiresUserGesture = false
                    settings.javaScriptCanOpenWindowsAutomatically = false
                    // "wv" belgisi olib tashlanadi: sayt oddiy Chrome deb biladi.
                    settings.userAgentString = settings.userAgentString.replace("; wv", "")
                    CookieManager.getInstance().setAcceptThirdPartyCookies(this, true)
                    setBackgroundColor(AColor.BLACK)
                    webChromeClient = FsChrome(ctx as Activity)
                    val host = Uri.parse(embed).host.orEmpty()
                    webViewClient = object : WebViewClient() {
                        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean =
                            request.isForMainFrame && request.url.host != host

                        override fun onPageFinished(view: WebView, url: String) {
                            loading = false
                        }
                    }
                    loadUrl(embed, mapOf("Referer" to "https://$host/"))
                }
            },
            onRelease = { it.destroy() },
        )
        if (loading) {
            Box(Modifier.fillMaxWidth().aspectRatio(16f / 9f), contentAlignment = Alignment.Center) {
                CircularProgressIndicator(color = TubeRed)
            }
        }
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
fun WatchScreen(video: Video, onBack: () -> Unit, onOpen: (Video) -> Unit) {
    val ctx = LocalContext.current
    val clip = LocalClipboardManager.current
    var others by remember { mutableStateOf(listOf<Video>()) }
    var copied by remember(video.code) { mutableStateOf(false) }
    LaunchedEffect(video.code) {
        val r = Api.videos(1)
        if (r.error == null) others = r.videos.filter { it.code != video.code }
    }
    // Pleyer tepada qotib turadi, pastda ma'lumot va keyingi videolar aylanadi.
    Column(Modifier.statusBarsPadding()) {
        EmbedPlayer(video.embed, Modifier.fillMaxWidth())
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
                    Row(Modifier.padding(top = 12.dp).horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        ActionChip("← Orqaga", onBack)
                        ActionChip(if (copied) "✓ Nusxalandi" else "Havolani nusxalash") {
                            clip.setText(AnnotatedString(video.embed))
                            copied = true
                        }
                        ActionChip("Ulashish") {
                            val send = Intent(Intent.ACTION_SEND).apply {
                                type = "text/plain"
                                putExtra(Intent.EXTRA_TEXT, video.embed)
                            }
                            ctx.startActivity(Intent.createChooser(send, null))
                        }
                        ActionChip("Brauzerda ochish") { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(video.embed))) }
                    }
                }
                Spacer(Modifier.height(4.dp))
            }
            items(others, key = { it.code }) { v -> VideoCard(v) { onOpen(v) } }
        }
    }
}

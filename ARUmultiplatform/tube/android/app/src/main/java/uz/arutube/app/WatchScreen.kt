package uz.arutube.app

import android.app.Activity
import android.content.pm.ActivityInfo
import android.graphics.Color as AColor
import android.view.View
import android.view.ViewGroup
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView

/** Iframe orqali DoodStream pleyeri (`<iframe src=embed>`); to'liq ekran Activity ustiga chiqariladi. */
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

@Composable
fun EmbedPlayer(embed: String, modifier: Modifier = Modifier) {
    val ctx = LocalContext.current
    val html = """<html><body style="margin:0;background:#000"><iframe src="$embed" style="border:0;width:100%;height:100vh" allowfullscreen allow="autoplay; fullscreen; encrypted-media; picture-in-picture"></iframe></body></html>"""
    AndroidView(
        modifier = modifier,
        factory = {
            WebView(ctx).apply {
                settings.javaScriptEnabled = true
                settings.domStorageEnabled = true
                settings.mediaPlaybackRequiresUserGesture = false
                setBackgroundColor(AColor.BLACK)
                webChromeClient = FsChrome(ctx as Activity)
                // Reklama/yo'naltirishlar: asosiy sahifani almashtirishga urinish bloklanadi.
                webViewClient = object : WebViewClient() {
                    override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean = request.isForMainFrame
                }
                loadDataWithBaseURL("https://playmogo.com/", html, "text/html", "utf-8", null)
            }
        },
        onRelease = { it.destroy() },
    )
}

@Composable
fun WatchScreen(video: Video, onBack: () -> Unit, onOpen: (Video) -> Unit) {
    var others by remember { mutableStateOf(listOf<Video>()) }
    LaunchedEffect(video.code) {
        val r = Api.videos(1)
        if (r.error == null) others = r.videos.filter { it.code != video.code }
    }
    LazyColumn(Modifier.fillMaxWidth().statusBarsPadding()) {
        item {
            EmbedPlayer(video.embed, Modifier.fillMaxWidth().aspectRatio(16f / 9f))
            Column(Modifier.padding(12.dp)) {
                Text(video.title.ifBlank { video.code }, fontSize = 18.sp, fontWeight = FontWeight.SemiBold)
                Text(
                    "${Format.count(video.views)} ko'rish · ${Format.date(video.uploaded)} · ${Format.duration(video.duration)}",
                    fontSize = 12.sp,
                    color = Color(0xFFAAAAAA),
                    modifier = Modifier.padding(top = 4.dp),
                )
                TextButton(onClick = onBack) { Text("← Orqaga") }
            }
        }
        items(others, key = { it.code }) { v -> VideoCard(v) { onOpen(v) } }
    }
}

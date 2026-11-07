package uz.arutube.app

import android.app.Activity
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.webkit.CookieManager
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeoutOrNull
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.coroutines.resume

/** Pleyerga beriladigan to'g'ridan-to'g'ri video havolasi (IP va sarlavhalarga bog'liq — shu qurilmada ishlatiladi). */
data class Direct(val url: String, val userAgent: String, val referer: String)

/**
 * Yashirin WebView (2x2 piksel, deyarli shaffof) DoodStream embed sahifasini ochadi: haqiqiy brauzer dvigateli va telefonning IP'si
 * Cloudflare tekshiruvidan o'tadi. Sahifadagi `<video>` manzili (yoki Range sarlavhali media so'rovi) topilgach, WebView yopiladi
 * va havola o'z pleyerimizga beriladi — DoodStream reklamalari ko'rsatilmaydi (ular yashirin sahifada qoladi).
 * Asosiy oqimdan chaqiring. Topilmasa (captcha, tarmoq, muddat) `null`.
 */
object DirectLink {
    private val staticAsset = Regex("""\.(css|js|png|jpe?g|gif|webp|svg|ico|woff2?|ttf|json|html?)(\?|$)""", RegexOption.IGNORE_CASE)

    suspend fun resolve(activity: Activity, embed: String, timeoutMs: Long = 35_000): Direct? {
        val host = Uri.parse(embed).host.orEmpty()
        val root = activity.window.decorView as FrameLayout
        val handler = Handler(Looper.getMainLooper())
        var wv: WebView? = null
        var poll: Runnable? = null
        try {
            return withTimeoutOrNull(timeoutMs) {
                suspendCancellableCoroutine<Direct?> { cont ->
                    val w = WebView(activity)
                    wv = w
                    val found = AtomicBoolean(false)
                    fun done(u: String) {
                        if (found.compareAndSet(false, true) && cont.isActive) {
                            cont.resume(Direct(u, w.settings.userAgentString, "https://$host/"))
                        }
                    }
                    w.settings.javaScriptEnabled = true
                    w.settings.domStorageEnabled = true
                    w.settings.mediaPlaybackRequiresUserGesture = false
                    w.settings.javaScriptCanOpenWindowsAutomatically = false
                    w.settings.setSupportMultipleWindows(false)
                    w.settings.userAgentString = w.settings.userAgentString.replace("; wv", "")
                    CookieManager.getInstance().setAcceptCookie(true)
                    CookieManager.getInstance().setAcceptThirdPartyCookies(w, true)
                    w.webViewClient = object : WebViewClient() {
                        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean =
                            request.isForMainFrame && request.url.host != host

                        override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? {
                            // `<video>` so'rovlari `Range` sarlavhasi bilan boradi; statik fayllar (css/js/rasm) hisobga olinmaydi.
                            if (AdBlock.isAd(request.url)) return AdBlock.empty()
                            val u = request.url.toString()
                            if (request.requestHeaders.keys.any { it.equals("Range", true) } && !staticAsset.containsMatchIn(u) && request.url.host != host) {
                                done(u)
                            }
                            return null
                        }
                    }
                    root.addView(w, FrameLayout.LayoutParams(2, 2))
                    w.alpha = 0.01f
                    w.loadUrl(embed, mapOf("Referer" to "https://$host/"))

                    val r = object : Runnable {
                        override fun run() {
                            if (found.get() || !cont.isActive) return
                            w.evaluateJavascript(
                                "(function(){var v=document.querySelector('video');if(!v)return '';" +
                                    "if(v.paused){try{v.muted=true;v.play()}catch(e){}}return v.currentSrc||v.src||''})()",
                            ) { raw ->
                                val s = (raw ?: "").trim('"').replace("\\/", "/")
                                if (s.startsWith("http")) done(s)
                            }
                            handler.postDelayed(this, 700)
                        }
                    }
                    poll = r
                    handler.postDelayed(r, 1500)
                }
            }
        } finally {
            poll?.let { handler.removeCallbacks(it) }
            wv?.let {
                root.removeView(it)
                it.destroy()
            }
        }
    }
}

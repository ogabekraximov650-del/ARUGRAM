package uz.arugram.app.ui.main

import android.annotation.SuppressLint
import android.webkit.JavascriptInterface
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.text.BasicText
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import uz.arugram.app.core.RustCore
import uz.arugram.app.ui.components.CloseIcon
import uz.arugram.app.ui.theme.LocalTg

/** Telegram `Telegram.WebApp` mavzu ranglari (JSON) — sahifa ilova mavzusiga moslashadi. */
private fun themeJson(dark: Boolean): String = JSONObject().apply {
    put("bg_color", if (dark) "#1c2229" else "#ffffff")
    put("text_color", if (dark) "#ffffff" else "#000000")
    put("hint_color", if (dark) "#8a8a8a" else "#999999")
    put("link_color", "#3390ec")
    put("button_color", "#3390ec")
    put("button_text_color", "#ffffff")
    put("secondary_bg_color", if (dark) "#0f1419" else "#f0f0f0")
    put("header_bg_color", if (dark) "#1c2229" else "#ffffff")
    put("accent_text_color", "#3390ec")
    put("section_bg_color", if (dark) "#1c2229" else "#ffffff")
    put("subtitle_text_color", if (dark) "#8a8a8a" else "#999999")
    put("destructive_text_color", "#e53935")
}.toString()

/**
 * Bot mini-ilovasi (DrKLO `BotWebViewSheet`): to'liq ekran WebView. Telegram `TelegramWebviewProxy.postEvent` ko'prigi:
 * `web_app_ready` / `web_app_expand` / `web_app_request_viewport` / `web_app_request_theme` javob beriladi, `web_app_close` oynani yopadi.
 * [mode]: `menu` (bot menyu tugmasi), `button` (klaviatura tugmasi), `simple`, `main`.
 */
@SuppressLint("SetJavaScriptEnabled")
@Composable
fun WebAppDialog(spec: String, bot: String, title: String, url: String, mode: String, onClose: () -> Unit) {
    val c = LocalTg.current
    var loaded by remember { mutableStateOf<String?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var web by remember { mutableStateOf<WebView?>(null) }
    LaunchedEffect(spec, bot, url, mode) {
        val r = withContext(Dispatchers.IO) { RustCore.tgWebView(spec, bot, url, mode, "", themeJson(c.dark)) }
        val u = r.optString("url")
        if (u.isEmpty()) error = r.optString("error").ifEmpty { "Mini-ilova ochilmadi" } else loaded = u
    }
    BackHandler(onBack = onClose)
    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        Column(Modifier.fillMaxSize().background(c.bar).statusBarsPadding()) {
            Row(Modifier.fillMaxWidth().height(56.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(56.dp).clickable(onClick = onClose), contentAlignment = Alignment.Center) { CloseIcon(c.text) }
                BasicText(title, style = TextStyle(color = c.text, fontSize = 18.sp, fontWeight = FontWeight.Medium), maxLines = 1)
            }
            Box(Modifier.weight(1f).fillMaxWidth()) {
                val u = loaded
                when {
                    error != null -> BasicText(error.orEmpty(), Modifier.align(Alignment.Center), style = TextStyle(color = c.red, fontSize = 15.sp))
                    u == null -> CircularProgressIndicator(Modifier.align(Alignment.Center).size(36.dp), color = c.inputActive, strokeWidth = 3.dp)
                    else -> AndroidView(
                        modifier = Modifier.fillMaxSize(),
                        factory = { ctx ->
                            WebView(ctx).apply {
                                settings.javaScriptEnabled = true
                                settings.domStorageEnabled = true
                                settings.mediaPlaybackRequiresUserGesture = false
                                webViewClient = WebViewClient()
                                addJavascriptInterface(object {
                                    @JavascriptInterface
                                    fun postEvent(eventType: String, eventData: String?) {
                                        post {
                                            when (eventType) {
                                                "web_app_close" -> onClose()
                                                "web_app_request_theme" -> evaluateJavascript("window.Telegram&&Telegram.WebView.receiveEvent('theme_changed',{theme_params:${themeJson(c.dark)}})", null)
                                                "web_app_request_viewport", "web_app_expand" ->
                                                    evaluateJavascript("window.Telegram&&Telegram.WebView.receiveEvent('viewport_changed',{height:window.innerHeight,is_state_stable:true,is_expanded:true})", null)
                                                "web_app_open_link" -> runCatching { openUrl(ctx, JSONObject(eventData.orEmpty()).optString("url")) }
                                                "web_app_open_tg_link" -> runCatching { openUrl(ctx, "https://t.me" + JSONObject(eventData.orEmpty()).optString("path_full")) }
                                            }
                                        }
                                    }
                                }, "TelegramWebviewProxy")
                                loadUrl(u)
                                web = this
                            }
                        },
                        onRelease = { it.destroy() },
                    )
                }
            }
        }
    }
}

package uz.arumediatv.com.ui.login

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.geometry.Offset
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.GoogleSignIn
import uz.arumediatv.com.ui.theme.AruColors

/**
 * "Account qo'shish": yangi hisobga qanday kirishni tanlash. Telegram — bot orqali (Telegram ilovasida «Start»), Google — Google akkaunt tanlash.
 * Bekor qilinsa avvalgi hisobga qaytiladi. Hisob almashtirish ilova ichida bo'ladi (qayta ochilmaydi).
 */
@Composable
fun AddAccountScreen(
    onTelegram: () -> Unit,
    onDone: () -> Unit,
    onBack: () -> Unit,
    title: String = "Account qo'shish",
    subtitle: String = "Yangi account uchun kirish usulini tanlang",
) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    BackHandler(enabled = !busy, onBack = onBack)

    Column(
        Modifier.fillMaxSize().background(AruColors.Bg).padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center,
    ) {
        Text(title, color = Color.White, fontSize = 22.sp, fontWeight = FontWeight.Bold)
        Spacer(Modifier.height(8.dp))
        Text(subtitle, color = Color.White.copy(alpha = 0.55f), fontSize = 14.sp, textAlign = TextAlign.Center)
        Spacer(Modifier.height(28.dp))
        if (busy) {
            CircularProgressIndicator(color = AruColors.Accent, strokeWidth = 2.dp, modifier = Modifier.padding(4.dp))
        } else {
            Choice("Telegram orqali", { TelegramLogo() }, onTelegram)
            Spacer(Modifier.height(12.dp))
            Choice("Google orqali", { GoogleLogo() }) {
                error = null
                busy = true
                scope.launch {
                    val r = GoogleSignIn.idToken(ctx)
                    val err = r.second ?: AppServices.auth.loginWithGoogle(r.first.orEmpty())
                    busy = false
                    if (err == null) onDone() else if (err.isNotEmpty()) error = err
                }
            }
            Spacer(Modifier.height(20.dp))
            Text(
                "Orqaga", color = AruColors.Accent2, fontSize = 14.sp, fontWeight = FontWeight.SemiBold,
                modifier = Modifier.clip(RoundedCornerShape(12.dp)).clickable(onClick = onBack).padding(horizontal = 16.dp, vertical = 10.dp),
            )
        }
        error?.let {
            Spacer(Modifier.height(16.dp))
            Text(it, color = Color(0xFFF87171), fontSize = 13.sp, textAlign = TextAlign.Center)
        }
    }
}

/** Kapsula tugma: chetlari to'liq yarim aylana, chap chetida dumaloq logotip, matn o'rtada. */
@Composable
private fun Choice(title: String, logo: @Composable () -> Unit, onClick: () -> Unit) {
    Box(
        Modifier.fillMaxWidth().height(58.dp).clip(RoundedCornerShape(29.dp)).background(AruColors.Accent).clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Box(Modifier.align(Alignment.CenterStart).padding(start = 7.dp)) { logo() }
        Text(title, color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, textAlign = TextAlign.Center)
    }
}

@Composable
private fun LogoCircle(bg: Color, content: @Composable () -> Unit) {
    Box(Modifier.size(44.dp).clip(CircleShape).background(bg), contentAlignment = Alignment.Center) { content() }
}

/** SVG yo'llarini (rasmiy logotip shakllari) Canvas'da chizadi; `viewBox` — asl kenglik. */
@Composable
private fun SvgPaths(viewBox: Float, size: Dp, paths: List<Pair<String, Color>>) {
    val parsed = remember(paths) { paths.map { PathParser().parsePathString(it.first).toPath() to it.second } }
    Canvas(Modifier.size(size)) {
        val k = this.size.width / viewBox
        scale(k, k, pivot = Offset.Zero) { parsed.forEach { (p, c) -> drawPath(p, c) } }
    }
}

@Composable
private fun TelegramLogo() = LogoCircle(Color(0xFF229ED9)) {
    SvgPaths(24f, 26.dp, listOf("M9.04 15.6l-.38 4.1c.55 0 .79-.24 1.08-.52l2.6-2.48 5.38 3.94c.99.55 1.69.26 1.96-.91l3.55-16.6c.32-1.46-.53-2.03-1.5-1.67L1.2 10.6c-1.42.55-1.4 1.35-.24 1.7l5.35 1.67 12.42-7.83c.58-.38 1.12-.17.68.21z" to Color.White))
}

@Composable
private fun GoogleLogo() = LogoCircle(Color.White) {
    SvgPaths(
        48f, 26.dp,
        listOf(
            "M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z" to Color(0xFFEA4335),
            "M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z" to Color(0xFF4285F4),
            "M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z" to Color(0xFFFBBC05),
            "M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z" to Color(0xFF34A853),
        ),
    )
}

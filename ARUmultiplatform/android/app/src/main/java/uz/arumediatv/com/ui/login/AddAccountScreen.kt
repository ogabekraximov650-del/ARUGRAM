package uz.arumediatv.com.ui.login

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
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
fun AddAccountScreen(onTelegram: () -> Unit, onDone: () -> Unit, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    BackHandler(enabled = !busy, onBack = onBack)

    Column(
        Modifier.fillMaxSize().background(AruColors.Bg).padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center,
    ) {
        Text("Account qo'shish", color = Color.White, fontSize = 22.sp, fontWeight = FontWeight.Bold)
        Spacer(Modifier.height(8.dp))
        Text("Yangi account uchun kirish usulini tanlang", color = Color.White.copy(alpha = 0.55f), fontSize = 14.sp, textAlign = TextAlign.Center)
        Spacer(Modifier.height(28.dp))
        if (busy) {
            CircularProgressIndicator(color = AruColors.Accent, strokeWidth = 2.dp, modifier = Modifier.padding(4.dp))
        } else {
            Choice("Telegram orqali", onTelegram)
            Spacer(Modifier.height(12.dp))
            Choice("Google orqali") {
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

@Composable
private fun Choice(title: String, onClick: () -> Unit) {
    Text(
        title, color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, textAlign = TextAlign.Center,
        modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(AruColors.Accent).clickable(onClick = onClick).padding(vertical = 16.dp),
    )
}

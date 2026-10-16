package uz.arumediatv.com.ui.login

import android.content.Intent
import android.net.Uri
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
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
import kotlinx.coroutines.delay
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.LoginRequest
import uz.arumediatv.com.data.LoginStatus
import uz.arumediatv.com.ui.theme.AruColors

private enum class Stage { PREPARING, WAITING, EXPIRED, ERROR }

/**
 * Bot orqali kirish (Flutter `TelegramLoginScreen`): bir martalik havola Telegram'da botni ochadi, bot hisobni tasdiqlaydi,
 * ilova holatni har 0,8 soniyada so'raydi. Telefon raqami ulanmaydi — rasm/video uchun keyin Profil > Telegram > Ulash.
 */
@Composable
fun BotLoginScreen(onDone: () -> Unit, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val auth = AppServices.auth
    var stage by remember { mutableStateOf(Stage.PREPARING) }
    var message by remember { mutableStateOf("") }
    var req by remember { mutableStateOf<LoginRequest?>(null) }
    var left by remember { mutableIntStateOf(0) }
    var attempt by remember { mutableIntStateOf(0) }
    BackHandler(onBack = onBack)

    LaunchedEffect(attempt) {
        stage = Stage.PREPARING
        val r = auth.start()
        if (r == null) {
            stage = Stage.ERROR
            message = "Serverga ulanib bo'lmadi. Internetni tekshirib, qaytadan urinib ko'ring."
            return@LaunchedEffect
        }
        req = r
        left = r.expiresIn
        stage = Stage.WAITING
        var tick = 0
        while (left > 0) {
            when (auth.check(r.token)) {
                LoginStatus.OK -> { onDone(); return@LaunchedEffect }
                LoginStatus.BANNED -> { stage = Stage.ERROR; message = auth.banMessage.ifEmpty { "Hisobingiz bloklangan." }; return@LaunchedEffect }
                LoginStatus.EXPIRED -> break
                else -> {}
            }
            delay(800)
            if (++tick % 1 == 0) left = (left - 1).coerceAtLeast(0)
        }
        stage = Stage.EXPIRED
    }

    fun openTelegram() {
        val link = req?.deepLink ?: return
        try {
            ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(link)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        } catch (_: Exception) {
            stage = Stage.ERROR
            message = "Telegram ochilmadi. Telegram ilovasi o'rnatilganini tekshiring."
        }
    }

    Column(
        Modifier.fillMaxSize().background(AruColors.Bg).padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center,
    ) {
        Text("Bot orqali kirish", color = Color.White, fontSize = 22.sp, fontWeight = FontWeight.Bold)
        Spacer(Modifier.height(10.dp))
        when (stage) {
            Stage.PREPARING -> CircularProgressIndicator(color = AruColors.Accent, strokeWidth = 2.dp, modifier = Modifier.size(32.dp))
            Stage.WAITING -> {
                Text("Telegram'ni oching, botda «Start» tugmasini bosing. Tasdiqlangach ilova o'zi kiradi.", color = AruColors.TextDim, fontSize = 14.sp, textAlign = TextAlign.Center)
                Spacer(Modifier.height(20.dp))
                Text(
                    "Telegram'ni ochish", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 15.sp,
                    modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(AruColors.Accent).clickable(onClick = ::openTelegram).padding(vertical = 15.dp),
                    textAlign = TextAlign.Center,
                )
                Spacer(Modifier.height(10.dp))
                Text("Havola muddati: $left s", color = AruColors.TextFaint, fontSize = 12.sp)
            }
            Stage.EXPIRED -> {
                Text("Havola muddati tugadi", color = AruColors.TextDim, fontSize = 14.sp)
                Spacer(Modifier.height(14.dp))
                Retry { attempt++ }
            }
            Stage.ERROR -> {
                Text(message, color = Color(0xFFF87171), fontSize = 14.sp, textAlign = TextAlign.Center)
                Spacer(Modifier.height(14.dp))
                Retry { attempt++ }
            }
        }
        Spacer(Modifier.height(24.dp))
        Text("Orqaga", color = AruColors.TextDim, fontSize = 14.sp, modifier = Modifier.clickable(onClick = onBack).padding(10.dp))
    }
}

@Composable
private fun Retry(onClick: () -> Unit) {
    Text(
        "Qayta urinish", color = Color.White, fontWeight = FontWeight.SemiBold,
        modifier = Modifier.clip(RoundedCornerShape(14.dp)).background(AruColors.Accent).clickable(onClick = onClick).padding(horizontal = 24.dp, vertical = 12.dp),
    )
}

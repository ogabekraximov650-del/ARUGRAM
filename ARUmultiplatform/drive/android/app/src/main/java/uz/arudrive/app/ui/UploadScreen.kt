package uz.arudrive.app.ui

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import uz.arudrive.app.data.Picked
import uz.arudrive.app.data.Upload
import uz.arudrive.app.data.humanSize

/** Yuklash sahifasi: video yoki fayl tanlanadi -> worker orqali R2 ga -> ikkinchi akkaunt Telegramga bo'laklab saqlaydi. */
@Composable
fun UploadScreen(onUploaded: () -> Unit) {
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val scope = rememberCoroutineScope()
    var picked by remember { mutableStateOf<Picked?>(null) }
    var sent by remember { mutableLongStateOf(0L) }
    var busy by remember { mutableStateOf(false) }
    var message by remember { mutableStateOf<String?>(null) }
    var failed by remember { mutableStateOf(false) }

    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
        if (uri != null) {
            val p = Upload.pick(ctx, uri)
            if (p.size <= 0) { message = "Fayl hajmi aniqlanmadi"; failed = true } else { picked = p; message = null; failed = false; sent = 0 }
        }
    }

    Column(Modifier.fillMaxSize().background(AruColors.Bg).padding(20.dp)) {
        Text("Yuklash", color = AruColors.Text, fontSize = 22.sp, fontWeight = FontWeight.Bold)
        Text("Fayl 18 MB bo'laklarga bo'linib, worker orqali Telegram kanaliga saqlanadi.", color = AruColors.TextDim, fontSize = 13.sp, modifier = Modifier.padding(top = 6.dp, bottom = 20.dp))

        Box(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(AruColors.Card)
                .clickable(enabled = !busy) { launcher.launch(arrayOf("video/*", "*/*")) }.padding(22.dp),
            contentAlignment = Alignment.Center,
        ) {
            Text(picked?.name ?: "Video yoki fayl tanlash", color = AruColors.Text, fontWeight = FontWeight.SemiBold)
        }
        picked?.let { p ->
            Text(humanSize(p.size) + if (p.isVideo) " · video" else " · fayl", color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.padding(top = 8.dp))
            if (busy || sent > 0) {
                LinearProgressIndicator(
                    progress = { (sent.toFloat() / p.size).coerceIn(0f, 1f) },
                    color = AruColors.Accent, trackColor = AruColors.CardAlt,
                    modifier = Modifier.fillMaxWidth().padding(top = 14.dp),
                )
                Text("${humanSize(sent)} / ${humanSize(p.size)}", color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.padding(top = 6.dp))
            }
            Text(
                if (busy) "Yuborilmoqda…" else "Yuklash", color = Color.White, fontWeight = FontWeight.SemiBold,
                modifier = Modifier.padding(top = 18.dp).clip(RoundedCornerShape(12.dp))
                    .background(if (busy) AruColors.CardAlt else AruColors.Accent)
                    .clickable(enabled = !busy) {
                        busy = true; failed = false; message = null; sent = 0
                        scope.launch {
                            runCatching { Upload.run(ctx, p) { sent = it } }
                                .onSuccess { message = "Yuklandi — bosh sahifada ko'rinadi."; picked = null; sent = 0; onUploaded() }
                                .onFailure { message = it.message ?: "Xato"; failed = true }
                            busy = false
                        }
                    }.padding(horizontal = 26.dp, vertical = 12.dp),
            )
        }
        message?.let { Text(it, color = if (failed) Color(0xFFFF6B6B) else AruColors.TextDim, fontSize = 13.sp, modifier = Modifier.padding(top = 16.dp)) }
    }
}

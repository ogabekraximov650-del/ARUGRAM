package uz.arumediatv.com.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
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
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.ui.theme.AruColors
import java.io.File

/** Xotira: yuklab olingan videolar, rasmlar keshi, ilova ma'lumotlari — qancha joy egallagani va tozalash. */
@Composable
fun StorageScreen(onBack: () -> Unit) {
    val scope = rememberCoroutineScope()
    var videos by remember { mutableLongStateOf(0) }
    var images by remember { mutableLongStateOf(0) }
    var data by remember { mutableLongStateOf(0) }
    var pending by remember { mutableLongStateOf(0) }
    var confirm by remember { mutableStateOf(false) }

    suspend fun measure() = withContext(Dispatchers.IO) {
        videos = AppServices.videoCache.diskBytes()
        images = AppServices.images.diskBytes()
        val ctx = AppServices.appContext
        data = File(ctx.filesDir, "cache").walkTopDown().filter { it.isFile }.sumOf { it.length() } +
            File(ctx.filesDir, "tg").walkTopDown().filter { it.isFile }.sumOf { it.length() }
        pending = AppServices.sync.pendingCount.toLong()
    }
    LaunchedEffect(Unit) { measure() }

    OverlayScaffold("Xotira", onBack) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            val total = videos + images + data
            Text(formatBytes(total), color = AruColors.Accent2, fontSize = 30.sp, fontWeight = FontWeight.Black)
            Text("ilova egallagan joy", color = AruColors.TextDim, fontSize = 13.sp)
            Spacer(Modifier.height(14.dp))
            Bar(videos, images, data, total)
            Spacer(Modifier.height(18.dp))
            Item("Yuklab olingan videolar", videos, AruColors.Accent2, "O'chirish") { confirm = true }
            Item("Rasmlar keshi", images, AruColors.Gold, "Tozalash") {
                AppServices.images.clearDisk()
                scope.launch { measure() }
            }
            Item("Ro'yxatlar va sessiya", data, Color(0xFF60A5FA), null) {}
            if (pending > 0) {
                Spacer(Modifier.height(10.dp))
                Item("Serverga yuborilmagan o'zgarishlar", pending, Color(0xFF4ADE80), "Yuborish", countOnly = true) {
                    AppServices.sync.flushAsync(force = true)
                    scope.launch { kotlinx.coroutines.delay(1500); measure() }
                }
            }
            Spacer(Modifier.height(40.dp))
        }
    }
    if (confirm) {
        AlertDialog(
            onDismissRequest = { confirm = false },
            title = { Text("Videolarni o'chirish") },
            text = { Text("Yuklab olingan barcha videolar telefondan o'chiriladi.") },
            confirmButton = { TextButton(onClick = { confirm = false; AppServices.downloads.wipeAll(); scope.launch { measure() } }) { Text("O'chirish", color = Color(0xFFF87171)) } },
            dismissButton = { TextButton(onClick = { confirm = false }) { Text("Bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
}

@Composable
private fun Bar(a: Long, b: Long, c: Long, total: Long) {
    val sum = total.coerceAtLeast(1).toFloat()
    Row(Modifier.fillMaxWidth().height(10.dp).clip(RoundedCornerShape(5.dp)).background(AruColors.CardAlt)) {
        listOf(a to AruColors.Accent2, b to AruColors.Gold, c to Color(0xFF60A5FA)).forEach { (v, col) ->
            if (v > 0) Box(Modifier.weight((v / sum).coerceAtLeast(0.01f)).height(10.dp).background(col))
        }
    }
}

@Composable
private fun Item(title: String, bytes: Long, color: Color, action: String?, countOnly: Boolean = false, onAction: () -> Unit) {
    val shape = RoundedCornerShape(16.dp)
    Row(
        Modifier.fillMaxWidth().padding(vertical = 5.dp).clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).padding(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.padding(end = 12.dp).height(34.dp).background(color, RoundedCornerShape(3.dp)).padding(horizontal = 3.dp))
        Column(Modifier.weight(1f)) {
            Text(title, color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
            Text(if (countOnly) "$bytes ta yozuv" else formatBytes(bytes), color = AruColors.TextDim, fontSize = 12.sp)
        }
        if (action != null) {
            Text(
                action, color = if (action == "Yuborish") Color.White else Color(0xFFF87171), fontSize = 12.sp, fontWeight = FontWeight.SemiBold,
                modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(if (action == "Yuborish") AruColors.Accent else Color(0x22F87171)).clickable(onClick = onAction).padding(horizontal = 12.dp, vertical = 8.dp),
            )
        }
    }
}

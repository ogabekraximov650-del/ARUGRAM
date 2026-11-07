package uz.arutube.app

import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material3.Button
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private data class Picked(val uri: Uri, val name: String, val size: Long)

@Composable
fun UploadScreen(onBack: () -> Unit, onUploaded: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var picked by remember { mutableStateOf<Picked?>(null) }
    var progress by remember { mutableFloatStateOf(0f) }
    var busy by remember { mutableStateOf(false) }
    var status by remember { mutableStateOf("") }
    var link by remember { mutableStateOf("") }
    var title by remember { mutableStateOf("") }

    val pick = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) {
            var name = "video.mp4"
            var size = 0L
            ctx.contentResolver.query(uri, null, null, null, null)?.use { c ->
                if (c.moveToFirst()) {
                    val ni = c.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    val si = c.getColumnIndex(OpenableColumns.SIZE)
                    if (ni >= 0) name = c.getString(ni) ?: name
                    if (si >= 0) size = c.getLong(si)
                }
            }
            picked = Picked(uri, name, size)
            status = ""
        }
    }

    fun upload() {
        val p = picked ?: return
        busy = true
        progress = 0f
        status = "Tayyorlanmoqda…"
        scope.launch {
            val t = Api.ticket()
            if (t.error != null || t.url.isBlank()) {
                status = "❌ ${t.error ?: "yuklash manzili olinmadi"}"
                busy = false
                return@launch
            }
            status = "Yuklanmoqda…"
            // Qayta chaqiruv havolasi yuklash tugaguncha saqlanadi (JNA uni GC qilmasligi uchun).
            val cb = object : ProgressCb {
                override fun invoke(done: Long, total: Long) {
                    progress = if (total > 0) (done.toFloat() / total).coerceIn(0f, 1f) else 0f
                }
            }
            val text = withContext(Dispatchers.IO) {
                val pfd = ctx.contentResolver.openFileDescriptor(p.uri, "r")
                if (pfd == null) "{\"error\":\"fayl ochilmadi\"}" else TubeCore.upload(t.url, t.key, pfd.detachFd(), p.name, cb)
            }
            val r = Api.parseUpload(text)
            if (r.error != null) {
                status = "❌ ${r.error}"
            } else if (r.status == 200 && r.result.isNotEmpty()) {
                progress = 1f
                status = "✅ Yuklandi: ${r.result[0].filecode} (DoodStream qayta ishlashi biroz vaqt oladi)"
                picked = null
                onUploaded()
            } else {
                status = "❌ DoodStream: ${r.msg.ifBlank { "noma'lum javob" }}"
            }
            busy = false
        }
    }

    fun uploadLink() {
        busy = true
        status = "Havola yuborilmoqda…"
        scope.launch {
            val r = Api.remote(link.trim(), title.trim())
            if (r.error != null) {
                status = "❌ ${r.error}"
            } else {
                status = "✅ Navbatga qo'yildi: ${r.code} (DoodStream videoni o'zi yuklab oladi)"
                link = ""
                onUploaded()
            }
            busy = false
        }
    }

    Column(Modifier.fillMaxWidth().statusBarsPadding().padding(16.dp)) {
        Text("Video yuklash", fontSize = 20.sp, fontWeight = FontWeight.Bold)
        Text("1) Telefondan fayl", fontSize = 14.sp, color = Color(0xFFAAAAAA), modifier = Modifier.padding(top = 16.dp))
        Button(onClick = { pick.launch(arrayOf("video/*")) }, enabled = !busy, modifier = Modifier.padding(top = 8.dp)) { Text("Fayl tanlash") }
        picked?.let {
            Text("${it.name} · ${Format.size(it.size)}", modifier = Modifier.padding(top = 8.dp))
            Button(onClick = { upload() }, enabled = !busy, modifier = Modifier.padding(top = 8.dp)) { Text("Yuklash") }
        }
        if (busy && picked != null) {
            LinearProgressIndicator(progress = { progress }, modifier = Modifier.fillMaxWidth().padding(top = 12.dp))
            Text("${(progress * 100).toInt()}%", fontSize = 12.sp)
        }

        Text("2) Havola bilan", fontSize = 14.sp, color = Color(0xFFAAAAAA), modifier = Modifier.padding(top = 24.dp))
        OutlinedTextField(link, { link = it }, singleLine = true, placeholder = { Text("https://…/video.mp4") }, modifier = Modifier.fillMaxWidth())
        OutlinedTextField(title, { title = it }, singleLine = true, placeholder = { Text("Nom (ixtiyoriy)") }, modifier = Modifier.fillMaxWidth())
        Button(onClick = { uploadLink() }, enabled = !busy && link.startsWith("http"), modifier = Modifier.padding(top = 8.dp)) { Text("Havoladan yuklash") }

        if (status.isNotBlank()) Text(status, modifier = Modifier.padding(top = 16.dp))
        TextButton(onClick = onBack, modifier = Modifier.padding(top = 8.dp)) { Text("← Orqaga") }
    }
}

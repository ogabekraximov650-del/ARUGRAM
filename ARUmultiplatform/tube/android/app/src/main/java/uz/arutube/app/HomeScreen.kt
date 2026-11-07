package uz.arutube.app

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil.compose.AsyncImage
import kotlinx.coroutines.launch

/** YouTube'dagidek karta: 16:9 muqova, davomiylik belgisi, nom, "ko'rishlar · sana". */
@Composable
fun VideoCard(v: Video, onClick: () -> Unit) {
    Column(Modifier.fillMaxWidth().clickable(onClick = onClick).padding(bottom = 18.dp)) {
        Box(Modifier.fillMaxWidth().aspectRatio(16f / 9f).background(Color(0xFF222222))) {
            if (v.thumb.isNotBlank()) {
                AsyncImage(
                    model = v.thumb,
                    contentDescription = v.title,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxWidth().aspectRatio(16f / 9f),
                )
            }
            Text(
                Format.duration(v.duration),
                color = Color.White,
                fontSize = 12.sp,
                fontWeight = FontWeight.Medium,
                modifier = Modifier
                    .align(Alignment.BottomEnd)
                    .padding(8.dp)
                    .clip(RoundedCornerShape(4.dp))
                    .background(Color(0xCC000000))
                    .padding(horizontal = 5.dp, vertical = 2.dp),
            )
        }
        Column(Modifier.padding(horizontal = 12.dp, vertical = 10.dp)) {
            Text(v.title.ifBlank { v.code }, fontSize = 15.sp, fontWeight = FontWeight.Medium, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Text(
                "${Format.count(v.views)} ko'rish · ${Format.date(v.uploaded)}",
                fontSize = 12.sp,
                color = Color(0xFFAAAAAA),
                modifier = Modifier.padding(top = 2.dp),
            )
        }
    }
}

@Composable
fun HomeScreen(refreshKey: Int, onOpen: (Video) -> Unit, onUpload: () -> Unit) {
    val scope = rememberCoroutineScope()
    var videos by remember { mutableStateOf(listOf<Video>()) }
    var page by remember { mutableIntStateOf(1) }
    var pages by remember { mutableIntStateOf(1) }
    var loading by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var query by remember { mutableStateOf("") }

    suspend fun load(p: Int) {
        loading = true
        val r = Api.videos(p)
        if (r.error != null) {
            error = r.error
        } else {
            error = null
            videos = if (p == 1) r.videos else videos + r.videos
            page = p
            pages = r.pages
        }
        loading = false
    }

    LaunchedEffect(refreshKey) { load(1) }

    val shown = if (query.isBlank()) videos else videos.filter { it.title.contains(query, ignoreCase = true) }

    LazyColumn(Modifier.fillMaxWidth().statusBarsPadding()) {
        item {
            Row(Modifier.fillMaxWidth().padding(12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.SpaceBetween) {
                Text("ARUTUBE", fontSize = 22.sp, fontWeight = FontWeight.Bold, color = MaterialTheme.colorScheme.primary)
                Row {
                    TextButton(onClick = { scope.launch { load(1) } }) { Text("Yangilash") }
                    Button(onClick = onUpload) { Text("Yuklash") }
                }
            }
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                singleLine = true,
                placeholder = { Text("Qidirish") },
                modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp),
            )
        }
        items(shown, key = { it.code }) { v -> VideoCard(v) { onOpen(v) } }
        item {
            Column(Modifier.fillMaxWidth().padding(16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                when {
                    loading -> CircularProgressIndicator()
                    error != null -> {
                        Text("❌ $error", color = Color(0xFFFF8080))
                        TextButton(onClick = { scope.launch { load(if (videos.isEmpty()) 1 else page + 1) } }) { Text("Qayta urinish") }
                    }
                    shown.isEmpty() -> Text("Video yo'q", color = Color(0xFFAAAAAA))
                    page < pages && query.isBlank() -> TextButton(onClick = { scope.launch { load(page + 1) } }) { Text("Yana yuklash") }
                }
            }
        }
    }
}

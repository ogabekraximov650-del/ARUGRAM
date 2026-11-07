package uz.arutube.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** Kutubxona: davom ettirish, "keyin ko'rish" va tarix (hammasi qurilmada saqlanadi). */
@Composable
fun LibraryScreen(onOpen: (Video) -> Unit) {
    val history = LocalStore.history
    val cont = history.filter { it.inProgress }
    val later = LocalStore.later
    LazyColumn(Modifier.fillMaxWidth().statusBarsPadding()) {
        item { Text("Kutubxona", fontSize = 22.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(16.dp)) }
        if (cont.isNotEmpty()) {
            item { SectionTitle("Davom ettirish") }
            items(cont, key = { "c" + it.video.code }) { s -> VideoCard(s.video, progress = s.fraction) { onOpen(s.video) } }
        }
        if (later.isNotEmpty()) {
            item { SectionTitle("Keyin ko'rish") }
            items(later, key = { "l" + it.code }) { v -> VideoCard(v, progress = LocalStore.progress(v.code)?.fraction ?: 0f) { onOpen(v) } }
        }
        item {
            Row(Modifier.fillMaxWidth().padding(end = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.SpaceBetween) {
                SectionTitle("Tarix")
                if (history.isNotEmpty()) TextButton(onClick = { LocalStore.clearHistory() }) { Text("Tozalash") }
            }
        }
        if (history.isEmpty()) {
            item { Text("Hali hech narsa ko'rilmagan", color = TubeGray, modifier = Modifier.padding(16.dp)) }
        }
        items(history, key = { "h" + it.video.code }) { s -> VideoCard(s.video, progress = s.fraction) { onOpen(s.video) } }
    }
}

@Composable
fun SectionTitle(text: String) {
    Text(text, fontSize = 18.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
}

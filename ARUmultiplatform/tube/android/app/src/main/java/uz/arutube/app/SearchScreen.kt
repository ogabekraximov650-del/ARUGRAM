package uz.arutube.app

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** Qidiruv: barcha sahifalar yig'iladi, yozilganda takliflar (oxirgi qidiruvlar va sarlavhalar) va natijalar chiqadi. */
@Composable
fun SearchScreen(onOpen: (Video) -> Unit) {
    var q by remember { mutableStateOf("") }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) {
        runCatching { focus.requestFocus() }
        Repo.ensureLoaded()
    }
    val results = if (q.isBlank()) emptyList() else Repo.all.filter { it.title.contains(q.trim(), ignoreCase = true) }
    val suggestions = if (q.isBlank()) LocalStore.queries else
        (LocalStore.queries.filter { it.contains(q.trim(), ignoreCase = true) && !it.equals(q.trim(), true) } +
            Repo.all.map { it.title }.filter { it.contains(q.trim(), ignoreCase = true) && !it.equals(q.trim(), true) }).distinct().take(5)

    LazyColumn(Modifier.fillMaxWidth().statusBarsPadding()) {
        item {
            Row(Modifier.fillMaxWidth().padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.weight(1f).clip(RoundedCornerShape(22.dp)).background(TubeChip).padding(horizontal = 14.dp, vertical = 11.dp)) {
                    if (q.isEmpty()) Text("Videolardan qidirish", color = TubeGray, fontSize = 15.sp)
                    BasicTextField(
                        value = q,
                        onValueChange = { q = it },
                        singleLine = true,
                        textStyle = TextStyle(color = Color.White, fontSize = 15.sp),
                        cursorBrush = SolidColor(Color.White),
                        modifier = Modifier.fillMaxWidth().focusRequester(focus),
                    )
                }
                if (q.isNotEmpty()) {
                    Icon(
                        Icons.Filled.Close,
                        contentDescription = "Tozalash",
                        tint = Color.White,
                        modifier = Modifier.padding(start = 6.dp).size(40.dp).clip(CircleShape).clickable { q = "" }.padding(10.dp),
                    )
                }
            }
        }
        items(suggestions, key = { "s$it" }) { s ->
            Row(Modifier.fillMaxWidth().clickable { q = s }.padding(horizontal = 16.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Filled.Search, contentDescription = null, tint = TubeGray, modifier = Modifier.size(20.dp))
                Text(s, fontSize = 15.sp, modifier = Modifier.padding(start = 14.dp), maxLines = 1)
            }
        }
        if (q.isBlank() && LocalStore.queries.isNotEmpty()) {
            item { TextButton(onClick = { LocalStore.clearQueries() }, modifier = Modifier.padding(horizontal = 8.dp)) { Text("Qidiruv tarixini tozalash") } }
        }
        if (q.isNotBlank()) {
            if (Repo.loading && results.isEmpty()) {
                item { Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(color = TubeRed) } }
            } else if (results.isEmpty()) {
                item { Text("Hech narsa topilmadi", color = TubeGray, modifier = Modifier.padding(16.dp)) }
            }
            items(results, key = { "r" + it.code }) { v ->
                VideoCard(v, progress = LocalStore.progress(v.code)?.fraction ?: 0f) {
                    LocalStore.addQuery(q)
                    onOpen(v)
                }
            }
        }
    }
}

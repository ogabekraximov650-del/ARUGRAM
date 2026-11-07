package uz.arutube.app

import android.app.Activity
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
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
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil.compose.AsyncImage
import kotlinx.coroutines.launch

private val AvatarColors = listOf(0xFFE53935, 0xFF8E24AA, 0xFF3949AB, 0xFF00897B, 0xFFF4511E, 0xFF6D4C41, 0xFF039BE5, 0xFF7CB342)

/** Kanal belgisi o'rnida: nomning birinchi harfi, rangi nomdan olinadi. */
@Composable
fun Avatar(seed: String, size: Int = 36) {
    val c = Color(AvatarColors[(seed.hashCode() and 0x7fffffff) % AvatarColors.size])
    Box(Modifier.size(size.dp).clip(CircleShape).background(c), contentAlignment = Alignment.Center) {
        Text(seed.trim().take(1).uppercase().ifEmpty { "A" }, color = Color.White, fontWeight = FontWeight.Bold, fontSize = (size * 0.45f).sp)
    }
}

/** YouTube'dagidek karta: to'liq kenglikdagi 16:9 muqova, davomiylik belgisi, kanal belgisi, nom, "ko'rish · vaqt". */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun VideoCard(v: Video, progress: Float = 0f, onClick: () -> Unit) {
    val quick = LocalQuickActions.current
    Column(Modifier.fillMaxWidth().combinedClickable(onClick = onClick, onLongClick = { quick(v) }).padding(bottom = 20.dp)) {
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
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier
                    .align(Alignment.BottomEnd)
                    .padding(8.dp)
                    .clip(RoundedCornerShape(4.dp))
                    .background(Color(0xCC000000))
                    .padding(horizontal = 5.dp, vertical = 2.dp),
            )
            if (progress > 0f) {
                Box(Modifier.align(Alignment.BottomStart).fillMaxWidth(progress.coerceIn(0f, 1f)).height(3.dp).background(TubeRed))
            }
        }
        Row(Modifier.padding(start = 12.dp, end = 12.dp, top = 12.dp), verticalAlignment = Alignment.Top) {
            Avatar("ARUTUBE")
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(v.title.ifBlank { v.code }, fontSize = 15.sp, lineHeight = 20.sp, fontWeight = FontWeight.Medium, maxLines = 2, overflow = TextOverflow.Ellipsis)
                val ago = Format.ago(v.uploaded)
                Text(
                    "ARUTUBE · ${Format.count(v.views)} ko'rish" + if (ago.isNotEmpty()) " · $ago" else "",
                    fontSize = 12.sp,
                    color = TubeGray,
                    modifier = Modifier.padding(top = 2.dp),
                )
            }
            Icon(
                Icons.Filled.MoreVert,
                contentDescription = "Amallar",
                tint = TubeGray,
                modifier = Modifier.size(36.dp).clip(CircleShape).clickable { quick(v) }.padding(6.dp),
            )
        }
    }
}

/** "Davom ettirish" qatoridagi karta: gorizontal ro'yxatda, ostida ko'rish chizig'i. */
@Composable
fun ContinueCard(sv: Saved, onClick: () -> Unit) {
    Column(Modifier.width(220.dp).clickable(onClick = onClick)) {
        Box(Modifier.fillMaxWidth().aspectRatio(16f / 9f).clip(RoundedCornerShape(10.dp)).background(Color(0xFF222222))) {
            if (sv.video.thumb.isNotBlank()) {
                AsyncImage(model = sv.video.thumb, contentDescription = sv.video.title, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
            }
            Box(Modifier.align(Alignment.BottomStart).fillMaxWidth(sv.fraction).height(3.dp).background(TubeRed))
        }
        Text(sv.video.title.ifBlank { sv.video.code }, fontSize = 13.sp, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 6.dp))
    }
}

/** Shorts kartasi: vertikal 9:16, pastda nom (YouTube Shorts javonidagidek). */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ShortCard(v: Video, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val quick = LocalQuickActions.current
    Box(modifier.aspectRatio(9f / 16f).clip(RoundedCornerShape(12.dp)).background(Color(0xFF222222)).combinedClickable(onClick = onClick, onLongClick = { quick(v) })) {
        if (v.thumb.isNotBlank()) {
            AsyncImage(model = v.thumb, contentDescription = v.title, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
        }
        Box(Modifier.align(Alignment.BottomStart).fillMaxWidth().background(androidx.compose.ui.graphics.Brush.verticalGradient(listOf(Color.Transparent, Color(0xCC000000))))) {
            Column(Modifier.padding(10.dp)) {
                Text(v.title.ifBlank { v.code }, fontSize = 14.sp, lineHeight = 18.sp, fontWeight = FontWeight.SemiBold, maxLines = 2, overflow = TextOverflow.Ellipsis, color = Color.White)
                Text("${Format.count(v.views)} ko'rish", fontSize = 11.sp, color = Color(0xFFDDDDDD), modifier = Modifier.padding(top = 2.dp))
            }
        }
    }
}

/** Yuklanish paytida kartalar o'rnida miltillovchi joy. */
@Composable
fun SkeletonCard() {
    val alpha by rememberInfiniteTransition(label = "sk").animateFloat(
        initialValue = 0.35f,
        targetValue = 0.75f,
        animationSpec = infiniteRepeatable(tween(800), RepeatMode.Reverse),
        label = "a",
    )
    val c = Color(0xFF2A2A2A).copy(alpha = alpha)
    Column(Modifier.fillMaxWidth().padding(bottom = 20.dp)) {
        Box(Modifier.fillMaxWidth().aspectRatio(16f / 9f).background(c))
        Row(Modifier.padding(12.dp)) {
            Box(Modifier.size(36.dp).clip(CircleShape).background(c))
            Spacer(Modifier.width(12.dp))
            Column {
                Box(Modifier.fillMaxWidth(0.8f).height(14.dp).clip(RoundedCornerShape(4.dp)).background(c))
                Spacer(Modifier.height(8.dp))
                Box(Modifier.fillMaxWidth(0.5f).height(12.dp).clip(RoundedCornerShape(4.dp)).background(c))
            }
        }
    }
}

private enum class Chip(val label: String) {
    ALL("Hammasi"), NEW("Yangi"), POPULAR("Mashhur");

    fun apply(list: List<Video>): List<Video> = when (this) {
        ALL -> list
        NEW -> list.sortedByDescending { it.uploaded }
        POPULAR -> list.sortedByDescending { it.views }
    }
}

@Composable
fun HomeScreen(refreshKey: Int, onOpen: (Video) -> Unit, onOpenShort: (List<Video>, Int) -> Unit, onSearch: () -> Unit) {
    val scope = rememberCoroutineScope()
    var videos by remember { mutableStateOf(listOf<Video>()) }
    var page by remember { mutableIntStateOf(1) }
    var pages by remember { mutableIntStateOf(1) }
    var loading by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var query by remember { mutableStateOf("") }
    var searching by remember { mutableStateOf(false) }
    var chip by remember { mutableStateOf(Chip.ALL) }

    suspend fun load(p: Int) {
        loading = true
        val r = Api.videos(p)
        if (r.error != null) {
            error = r.error
        } else {
            error = null
            videos = if (p == 1) r.videos else videos + r.videos
            Repo.merge(r.videos)
            page = p
            pages = r.pages
        }
        loading = false
    }

    LaunchedEffect(refreshKey) { load(1) }

    // Reklamasiz havolalar oldindan tayyorlanadi: avval davom ettirilayotgan, keyin birinchi uzun videolar va Shorts.
    val act = LocalContext.current as Activity
    LaunchedEffect(videos.firstOrNull()?.code) {
        if (videos.isNotEmpty()) {
            val cont = LocalStore.history.filter { it.inProgress }.map { it.video }
            LinkCache.prefetch(scope, act, cont + videos.filter { !it.isShort } + videos.filter { it.isShort }, 4)
        }
    }

    val byQuery = if (query.isBlank()) videos else videos.filter { it.title.contains(query, ignoreCase = true) }
    val shown = chip.apply(byQuery)
    val shorts = shown.filter { it.isShort }
    val longs = shown.filter { !it.isShort }

    Column(Modifier.statusBarsPadding()) {
        // Yuqori panel: logotip (qizil "play" belgisi + nom), qidiruv va yangilash.
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            if (searching) {
                Box(Modifier.weight(1f).clip(RoundedCornerShape(20.dp)).background(TubeChip).padding(horizontal = 14.dp, vertical = 10.dp)) {
                    if (query.isEmpty()) Text("Qidirish", color = TubeGray, fontSize = 15.sp)
                    BasicTextField(
                        value = query,
                        onValueChange = { query = it },
                        singleLine = true,
                        textStyle = TextStyle(color = Color.White, fontSize = 15.sp),
                        cursorBrush = SolidColor(Color.White),
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                Icon(
                    Icons.Filled.Close,
                    contentDescription = "Yopish",
                    tint = Color.White,
                    modifier = Modifier.padding(start = 8.dp).size(36.dp).clip(CircleShape).clickable { searching = false; query = "" }.padding(6.dp),
                )
            } else {
                Box(Modifier.size(width = 32.dp, height = 22.dp).clip(RoundedCornerShape(6.dp)).background(TubeRed), contentAlignment = Alignment.Center) {
                    Icon(Icons.Filled.PlayArrow, contentDescription = null, tint = Color.White, modifier = Modifier.size(18.dp))
                }
                Text("ARUTUBE", fontSize = 20.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(start = 6.dp).weight(1f))
                Icon(
                    Icons.Filled.Refresh,
                    contentDescription = "Yangilash",
                    tint = Color.White,
                    modifier = Modifier.size(40.dp).clip(CircleShape).clickable { scope.launch { load(1) } }.padding(8.dp),
                )
                Icon(
                    Icons.Filled.Search,
                    contentDescription = "Qidirish",
                    tint = Color.White,
                    modifier = Modifier.size(40.dp).clip(CircleShape).clickable { onSearch() }.padding(8.dp),
                )
            }
        }
        // Toifa tugmalari (YouTube'dagi "chip"lar).
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Chip.values().forEach { c ->
                val sel = c == chip
                Text(
                    c.label,
                    color = if (sel) Color.Black else Color.White,
                    fontSize = 14.sp,
                    fontWeight = FontWeight.Medium,
                    modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(if (sel) Color.White else TubeChip).clickable { chip = c }.padding(horizontal = 12.dp, vertical = 7.dp),
                )
            }
        }
        LazyColumn(Modifier.fillMaxWidth().padding(top = 8.dp)) {
            if (loading && videos.isEmpty()) {
                items(3) { SkeletonCard() }
            }
            val cont = LocalStore.history.filter { it.inProgress }
            if (cont.isNotEmpty() && query.isBlank()) {
                item(key = "continue") {
                    Column(Modifier.padding(bottom = 16.dp)) {
                        Text("Davom ettirish", fontSize = 18.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp))
                        androidx.compose.foundation.lazy.LazyRow(contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            items(cont, key = { it.video.code }) { sv -> ContinueCard(sv) { onOpen(sv.video) } }
                        }
                    }
                }
            }
            if (shorts.isNotEmpty()) {
                item(key = "shorts-header") {
                    Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.size(28.dp).clip(RoundedCornerShape(8.dp)).background(TubeRed), contentAlignment = Alignment.Center) {
                            Icon(Icons.Filled.PlayArrow, contentDescription = null, tint = Color.White, modifier = Modifier.size(18.dp))
                        }
                        Text("Shorts", fontSize = 20.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(start = 10.dp))
                    }
                }
                // Ikki ustunli to'r (YouTube Shorts javoni kabi): har qatorda 2 ta vertikal karta.
                items(shorts.chunked(2), key = { row -> "s" + row.first().code }) { row ->
                    Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp).padding(bottom = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        row.forEach { v ->
                            ShortCard(v, Modifier.weight(1f)) { onOpenShort(shorts, shorts.indexOf(v)) }
                        }
                        if (row.size == 1) Spacer(Modifier.weight(1f))
                    }
                }
                item(key = "shorts-gap") { Spacer(Modifier.height(12.dp)) }
            }
            items(longs, key = { it.code }) { v -> VideoCard(v) { onOpen(v) } }
            item {
                Column(Modifier.fillMaxWidth().padding(16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    when {
                        loading && videos.isNotEmpty() -> CircularProgressIndicator(color = TubeRed)
                        error != null -> {
                            Text("❌ $error", color = Color(0xFFFF8080))
                            TextButton(onClick = { scope.launch { load(if (videos.isEmpty()) 1 else page + 1) } }) { Text("Qayta urinish") }
                        }
                        !loading && shorts.isEmpty() && longs.isEmpty() -> Text("Video yo'q", color = TubeGray)
                        page < pages && query.isBlank() -> TextButton(onClick = { scope.launch { load(page + 1) } }) { Text("Yana yuklash") }
                    }
                }
            }
        }
    }
}

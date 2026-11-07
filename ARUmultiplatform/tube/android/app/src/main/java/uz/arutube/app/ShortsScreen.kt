package uz.arutube.app

import android.app.Activity
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.pager.VerticalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Share
import androidx.compose.material.icons.filled.ThumbUp
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** Vertikal Shorts: tepaga/pastga surish keyingi/oldingi videoga o'tkazadi; ekranga bosish — pauza. */
@Composable
fun ShortsScreen(list: List<Video>, start: Int, onBack: () -> Unit) {
    val pager = rememberPagerState(initialPage = start.coerceIn(0, (list.size - 1).coerceAtLeast(0)), pageCount = { list.size })
    Box(Modifier.fillMaxSize().background(Color.Black)) {
        VerticalPager(state = pager, modifier = Modifier.fillMaxSize()) { page ->
            ShortPage(list[page], next = list.getOrNull(page + 1), active = pager.currentPage == page)
        }
        androidx.compose.foundation.layout.Row(Modifier.statusBarsPadding().padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(
                Icons.AutoMirrored.Filled.ArrowBack,
                contentDescription = "Orqaga",
                tint = Color.White,
                modifier = Modifier.size(44.dp).clip(CircleShape).clickable(onClick = onBack).padding(10.dp),
            )
            Text("Shorts", color = Color.White, fontSize = 20.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(start = 6.dp))
        }
    }
}

@Composable
private fun ShortPage(v: Video, next: Video?, active: Boolean) {
    val act = LocalContext.current as Activity
    val scope = rememberCoroutineScope()
    var direct by remember(v.code) { mutableStateOf<Direct?>(null) }
    var failed by remember(v.code) { mutableStateOf(false) }
    var paused by remember(v.code) { mutableStateOf(false) }

    // Faqat ko'rinib turgan sahifa uchun reklamasiz havola olinadi.
    LaunchedEffect(v.code, active) {
        if (active && direct == null && !failed) {
            val d = LinkCache.resolve(scope, act, v)
            if (d != null) direct = d else failed = true
        }
        // Keyingi Short uchun havola oldindan tayyorlanadi: surilganda darhol ochiladi.
        if (active && next != null) LinkCache.prefetch(scope, act, listOf(next), 1)
    }

    Box(Modifier.fillMaxSize().background(Color.Black)) {
        val d = direct
        when {
            d != null -> {
                NativePlayer(
                    d, false, {}, { failed = true; direct = null },
                    Modifier.fillMaxSize().clickable { paused = !paused },
                    controls = false, loop = true, playing = active && !paused,
                )
            }
            failed -> EmbedPlayer(v.embed, PlayerMode.IFRAME, Modifier.fillMaxSize())
            else -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator(color = TubeRed) }
        }
        // O'ng tomondagi harakatlar (Shorts'dagidek): yoqdi, saqlash, ulashish, ko'proq.
        val ctx2 = LocalContext.current
        val quick = LocalQuickActions.current
        Column(Modifier.align(Alignment.CenterEnd).padding(end = 8.dp, bottom = 80.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            val liked = LocalStore.isLiked(v.code)
            SideButton(Icons.Filled.ThumbUp, if (liked) "Yoqdi ✓" else "Yoqdi", liked) { LocalStore.toggleLike(v.code) }
            val saved = LocalStore.isLater(v.code)
            SideButton(if (saved) Icons.Filled.Check else Icons.Filled.Add, if (saved) "Saqlandi" else "Saqlash", saved) { LocalStore.toggleLater(v) }
            SideButton(Icons.Filled.Share, "Ulashish", false) { shareVideo(ctx2, v) }
            SideButton(Icons.Filled.MoreVert, "Yana", false) { quick(v) }
        }
        Box(Modifier.align(Alignment.BottomStart).fillMaxWidth().background(Brush.verticalGradient(listOf(Color.Transparent, Color(0xAA000000))))) {
            Column(Modifier.padding(16.dp)) {
                Text(v.title.ifBlank { v.code }, color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, maxLines = 2, overflow = TextOverflow.Ellipsis)
                Text("${Format.count(v.views)} ko'rish · ARUTUBE", color = Color(0xFFDDDDDD), fontSize = 12.sp, modifier = Modifier.padding(top = 2.dp))
            }
        }
    }
}

@Composable
private fun SideButton(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, active: Boolean, onClick: () -> Unit) {
    Column(Modifier.padding(vertical = 10.dp).clickable(onClick = onClick), horizontalAlignment = Alignment.CenterHorizontally) {
        Icon(
            icon,
            contentDescription = label,
            tint = if (active) TubeRed else Color.White,
            modifier = Modifier.size(46.dp).clip(CircleShape).background(Color(0x66000000)).padding(11.dp),
        )
        Text(label, color = Color.White, fontSize = 11.sp, modifier = Modifier.padding(top = 3.dp))
    }
}

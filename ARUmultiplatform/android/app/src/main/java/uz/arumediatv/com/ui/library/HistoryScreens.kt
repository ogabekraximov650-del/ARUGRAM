package uz.arumediatv.com.ui.library

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.geometry.Offset
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.HistoryItem
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LocalBarPad
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.theme.AruColors
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

private fun fmtDate(ms: Long, pattern: String): String {
    val f = SimpleDateFormat(pattern, Locale.US).apply { timeZone = TimeZone.getTimeZone("GMT+5") } // Toshkent
    return f.format(Date(if (ms < 100_000_000_000L) ms * 1000 else ms))
}

/** `12:34` — daqiqa va soniya (soat ajratilmaydi, Flutter bilan bir xil). */
private fun clock(ms: Long): String {
    val s = (ms / 1000).coerceAtLeast(0)
    return String.format(Locale.US, "%02d:%02d", s / 60, s % 60)
}

private fun percent(p: Float): String = String.format(Locale("uz"), "%.2f%%", p * 100).replace('.', ',')

/** Anime bo'yicha ro'yxat: har bir BO'LIM bitta karta (eng oxirgi ko'rilgani). */
fun groupByAnime(items: List<HistoryItem>): List<HistoryItem> {
    val seen = HashSet<String>()
    return items.sortedByDescending { it.updatedAt }.filter { seen.add("${it.animeId}:${it.seasonId}") }
}

@Composable
fun HistoryAnimeList(items: List<HistoryItem>, pad: Dp, onOpenAnime: (HistoryItem) -> Unit) {
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = pad),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        items(groupByAnime(items), key = { "${it.animeId}:${it.seasonId}" }) { h -> AnimeRow(h) { onOpenAnime(h) } }
    }
}

/** To'xtagan joydagi kadr; tayyor bo'lguncha (yoki yasalmasa) bo'lim posteri turadi. */
@Composable
fun HistoryFrame(item: HistoryItem, modifier: Modifier = Modifier) {
    // Kalit — qism shaxsi (joy yoki video manzili o'zgarganda kadr miltillamaydi/yo'qolmaydi).
    var bmp by remember(item.animeId, item.seasonId, item.episodeNo, item.videoUrl) { mutableStateOf(AppServices.thumbs.peek(item)) }
    androidx.compose.runtime.LaunchedEffect(item.animeId, item.seasonId, item.episodeNo, item.positionMs, item.videoUrl) {
        if (bmp == null) bmp = AppServices.thumbs.cached(item)
        AppServices.thumbs.load(item)?.let { bmp = it }
    }
    Box(modifier) {
        TgImage(item.photoUrl, Modifier.fillMaxSize(), maxPx = 720)
        bmp?.let { b ->
            val img = remember(b) { b.asImageBitmap() }
            androidx.compose.foundation.Image(img, null, contentScale = androidx.compose.ui.layout.ContentScale.Crop, modifier = Modifier.fillMaxSize())
        }
    }
}

/** Anime kartasi: 16:9 kadr, tagida nom, "Oxirgi marta N-bo'lim M-qismni ko'rdingiz" va sana. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun AnimeRow(item: HistoryItem, onClick: () -> Unit) {
    val shape = RoundedCornerShape(18.dp)
    Column(
        Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape)
            .combinedClickable(onClick = onClick).padding(10.dp),
    ) {
        HistoryFrame(item, Modifier.fillMaxWidth().aspectRatio(16f / 9f).clip(RoundedCornerShape(12.dp)).background(AruColors.CardAlt))
        Text(item.title, color = Color.White, fontSize = 15.5.sp, fontWeight = FontWeight.Bold, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 10.dp))
        Text(
            "Oxirgi marta ${item.sectionNo}-bo'lim ${item.episodeNo}-qismni ko'rdingiz",
            color = Color.White.copy(alpha = 0.68f), fontSize = 12.5.sp, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 6.dp),
        )
        if (item.updatedAt > 0) Text("Sana: ${fmtDate(item.updatedAt, "HH:mm/dd/MM/yyyy")}", color = Color.White.copy(alpha = 0.52f), fontSize = 12.sp, modifier = Modifier.padding(top = 2.dp))
    }
}

private val shadowStyle = TextStyle(
    shadow = Shadow(Color.Black, Offset(0f, 2f), 6f),
    color = Color.White,
)

@Composable
private fun ShadowText(text: String, size: Float, weight: FontWeight = FontWeight.Medium, align: TextAlign = TextAlign.Start, alpha: Float = 1f, modifier: Modifier = Modifier) {
    if (text.isEmpty()) return
    Text(text, style = shadowStyle.copy(color = Color.White.copy(alpha = alpha)), fontSize = size.sp, fontWeight = weight, textAlign = align, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = modifier)
}

/** Qism kartasi: 16:9 kadr, ustida yozuvlar, pastida progress chizig'i. Bossa — shu joydan ochiladi, uzoq bossa — o'chirish. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun EpisodeRow(item: HistoryItem, onOpen: () -> Unit, onLongPress: (() -> Unit)? = null) {
    Box(
        Modifier.fillMaxWidth().aspectRatio(16f / 9f).clip(RoundedCornerShape(16.dp)).background(AruColors.CardAlt)
            .combinedClickable(onClick = onOpen, onLongClick = onLongPress),
    ) {
        HistoryFrame(item, Modifier.fillMaxSize())
        Row(Modifier.align(Alignment.BottomStart).fillMaxWidth().padding(start = 8.dp, end = 8.dp, bottom = 10.dp), verticalAlignment = Alignment.Bottom) {
            Column(Modifier.weight(1f)) {
                ShadowText(item.title, 11.5f, FontWeight.Bold)
                ShadowText("${item.sectionNo}-bo'lim ${item.episodeNo}-qism", 10.5f)
                if (item.updatedAt > 0) ShadowText("sana: ${fmtDate(item.updatedAt, "HH:mm/dd/MM/yyyy")}", 10f, alpha = 0.85f)
            }
            Spacer(Modifier.width(8.dp))
            ShadowText("${percent(item.progress)} | ${clock(item.positionMs)}/${clock(item.durationMs)}", 10.5f, FontWeight.SemiBold, TextAlign.End)
        }
        Box(Modifier.align(Alignment.BottomStart).fillMaxWidth().height(3.5.dp).background(Color.White.copy(alpha = 0.24f))) {
            Box(Modifier.fillMaxWidth(item.progress).height(3.5.dp).background(Brush.horizontalGradient(listOf(AruColors.Accent, AruColors.Accent2))))
        }
    }
}

/** Bitta animening (bo'limning) ko'rilgan qismlari, oxirgisi tepada. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun AnimeHistoryScreen(animeId: Long, seasonId: Long, title: String, onBack: () -> Unit, onOpenEpisode: (HistoryItem) -> Unit) {
    BackHandler(onBack = onBack)
    val state by AppServices.library.state.collectAsStateWithLifecycle()
    val rows = state.history.filter { it.animeId == animeId && it.seasonId == seasonId }.sortedByDescending { it.updatedAt }
    var removing by remember { mutableStateOf<HistoryItem?>(null) }
    Column(Modifier.fillMaxSize().background(AruColors.Bg).statusBarsPadding()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier.clip(RoundedCornerShape(14.dp)).background(AruColors.Card).border(1.dp, AruColors.Border, RoundedCornerShape(14.dp))
                    .combinedClickable(onClick = onBack).padding(8.dp),
            ) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Orqaga", tint = Color.White, modifier = Modifier.size(24.dp)) }
            Spacer(Modifier.width(14.dp))
            Text(title.ifEmpty { "Tomosha tarixi" }, color = Color.White, fontSize = 18.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        if (rows.isEmpty()) {
            EmptyState(Icons.Filled.PlayArrow, "Hali hech narsa ko'rilmagan")
        } else {
            LazyColumn(
                Modifier.fillMaxSize(),
                contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 40.dp + LocalBarPad.current.calculateBottomPadding()),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                items(rows, key = { it.key }) { h -> EpisodeRow(h, onOpen = { onOpenEpisode(h) }, onLongPress = { removing = h }) }
            }
        }
    }
    removing?.let { h ->
        AlertDialog(
            onDismissRequest = { removing = null },
            title = { Text("Tarixdan o'chirish") },
            text = { Text("Rostdan ham bu tarixni o'chirib tashlaysizmi?") },
            confirmButton = { TextButton(onClick = { AppServices.library.removeHistory(h); removing = null }) { Text("O'chirish", color = Color(0xFFF87171)) } },
            dismissButton = { TextButton(onClick = { removing = null }) { Text("Bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
}

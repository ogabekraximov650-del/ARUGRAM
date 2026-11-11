package uz.arumediatv.com.ui.library

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
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
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.DownloadItem
import uz.arumediatv.com.data.Episode
import uz.arumediatv.com.data.HistoryItem
import uz.arumediatv.com.data.QUALITIES
import uz.arumediatv.com.data.Season
import uz.arumediatv.com.data.VideoStat
import uz.arumediatv.com.ui.common.AruIcons
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.profile.formatBytes
import uz.arumediatv.com.ui.theme.AruColors
import java.util.Locale

private fun percent(r: Float) = String.format(Locale.US, "%.2f%%", r * 100).replace('.', ',')

private fun bytes(b: Long) = formatBytes(b).replace('.', ',')

private fun historyOf(animeId: Long, seasonId: Long, episodeId: Long): HistoryItem? =
    AppServices.library.state.value.history.firstOrNull { it.animeId == animeId && it.seasonId == seasonId && it.epizodId == episodeId }

private fun seasonOf(d: DownloadItem, known: List<Season>): Season =
    known.firstOrNull { it.animeId == d.animeId && it.seasonId == d.seasonId }
        ?: Season(d.animeId, d.seasonId, d.title, d.sectionNo, 0, d.poster, 0.0, 0, 0, 0, emptyList(), "", null)

/** Diskdagi yuklanmalar (har bir (qism, sifat) alohida qator) — jonli holat bilan. */
@Composable
private fun rememberDownloads(refreshMs: Long): Pair<List<DownloadItem>?, Map<String, VideoStat>> {
    val seasonsState by AppServices.seasons.state.collectAsStateWithLifecycle()
    val live by AppServices.downloads.stats.collectAsStateWithLifecycle()
    var items by remember { mutableStateOf<List<DownloadItem>?>(null) }
    LaunchedEffect(seasonsState.items) {
        while (true) {
            items = AppServices.downloadsIndex.collect(seasonsState.items)
            delay(refreshMs)
        }
    }
    val owner = remember { Any() }
    DisposableEffect(items) {
        items?.let { list -> AppServices.downloads.watch(owner, list.map { it.url }.toSet()) }
        onDispose { AppServices.downloads.unwatch(owner) }
    }
    return items to live
}

/** Kutubxona → Yuklanmalar: har bir bo'lim bitta karta (kadr, nom, "N ta qism to'liq yuklangan", hajm). */
@Composable
fun DownloadsList(pad: Dp, onOpenAnime: (DownloadItem) -> Unit) {
    val (items, live) = rememberDownloads(10_000)
    var confirmAll by remember { mutableStateOf(false) }
    val list = items
    when {
        list == null -> LoadingBox()
        list.isEmpty() -> EmptyState(Icons.Filled.PlayArrow, "Yuklab olingan qism yo'q\nQismlar ro'yxatida ▾ ni bosib yuklab oling")
        else -> {
            val groups = list.groupBy { "${it.animeId}/${it.seasonId}" }.values.toList()
            LazyColumn(
                Modifier.fillMaxSize(),
                contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = pad),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                item {
                    val total = list.sumOf { (live[it.url] ?: it.stat).downloaded }
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        Text("${list.size} ta fayl · ${bytes(total)}", color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.weight(1f))
                        Text("Hammasini o'chirish", color = Color(0xFFF87171), fontSize = 12.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.clickable { confirmAll = true })
                    }
                }
                items(groups, key = { "${it.first().animeId}/${it.first().seasonId}" }) { g -> AnimeDownloadCard(g, live) { onOpenAnime(g.first()) } }
            }
        }
    }
    if (confirmAll) {
        AlertDialog(
            onDismissRequest = { confirmAll = false },
            title = { Text("Hammasini o'chirish") },
            text = { Text("Yuklab olingan barcha videolar telefondan o'chiriladi.") },
            confirmButton = { TextButton(onClick = { confirmAll = false; AppServices.downloads.wipeAll() }) { Text("O'chirish", color = Color(0xFFF87171)) } },
            dismissButton = { TextButton(onClick = { confirmAll = false }) { Text("Bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
}

@Composable
private fun Frame(first: DownloadItem, modifier: Modifier) {
    val h = historyOf(first.animeId, first.seasonId, first.episodeId)
    if (h != null) HistoryFrame(h, modifier) else TgImage(first.poster, modifier.background(AruColors.CardAlt), maxPx = 720)
}

@Composable
private fun AnimeDownloadCard(items: List<DownloadItem>, live: Map<String, VideoStat>, onClick: () -> Unit) {
    val first = items.first()
    val perEp = items.groupBy { it.episodeId }
    val full = perEp.count { (_, qs) -> qs.any { (live[it.url] ?: it.stat).complete } }
    val onDisk = items.sumOf { (live[it.url] ?: it.stat).downloaded }
    val shape = RoundedCornerShape(18.dp)
    Column(Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).clickable(onClick = onClick).padding(10.dp)) {
        Frame(first, Modifier.fillMaxWidth().aspectRatio(16f / 9f).clip(RoundedCornerShape(12.dp)))
        Text(first.title.ifEmpty { "Anime" }, color = Color.White, fontSize = 15.5.sp, fontWeight = FontWeight.Bold, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 10.dp))
        Text("${first.sectionNo}-bo'lim", color = Color.White.copy(alpha = 0.7f), fontSize = 13.sp, modifier = Modifier.padding(top = 4.dp))
        Text("$full ta qism to'liq yuklangan", color = Color.White.copy(alpha = 0.68f), fontSize = 12.5.sp, modifier = Modifier.padding(top = 2.dp))
        Text("Hajmi: ${bytes(onDisk)}", color = Color.White.copy(alpha = 0.52f), fontSize = 12.sp, modifier = Modifier.padding(top = 2.dp))
    }
}

/** Bitta bo'limning yuklangan qismlari: kadr, nom, ⬇ / 🗑 tugmalari va sifat kataklari (progress bilan). */
@Composable
fun DownloadsAnimeScreen(
    animeId: Long, seasonId: Long, title: String,
    onBack: () -> Unit, onOpen: (Season, Long, String) -> Unit,
) {
    BackHandler(onBack = onBack)
    val (items, live) = rememberDownloads(3_000)
    val seasonsState by AppServices.seasons.state.collectAsStateWithLifecycle()
    val rows = items?.filter { it.animeId == animeId && (seasonId == 0L || it.seasonId == seasonId) }.orEmpty()
        .groupBy { it.episodeId }.values.sortedWith(compareBy({ it.first().sectionNo }, { it.first().episodeNo }))
    var askFull by remember { mutableStateOf<DownloadItem?>(null) }
    var qualitiesFor by remember { mutableStateOf<DownloadItem?>(null) }
    var clearFor by remember { mutableStateOf<List<DownloadItem>?>(null) }

    Column(Modifier.fillMaxSize().background(AruColors.Bg).statusBarsPadding()) {
        Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            val bs = RoundedCornerShape(14.dp)
            Box(Modifier.clip(bs).background(AruColors.Card).border(1.dp, AruColors.Border, bs).clickable(onClick = onBack).padding(8.dp)) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, "Orqaga", tint = Color.White, modifier = Modifier.size(24.dp))
            }
            Spacer(Modifier.width(14.dp))
            Text(title.ifEmpty { "Yuklanmalar" }, color = Color.White, fontSize = 18.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        when {
            items == null -> LoadingBox()
            rows.isEmpty() -> EmptyState(Icons.Filled.PlayArrow, "Yuklab olingan qism yo'q")
            else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 40.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                items(rows, key = { it.first().episodeId }) { qs ->
                    EpisodeDownloadRow(
                        qs, live,
                        onOpenFrame = {
                            val best = qs.filter { (live[it.url] ?: it.stat).complete }.minByOrNull { QUALITIES.indexOf(it.quality) }
                            if (best != null) onOpen(seasonOf(best, seasonsState.items), best.episodeId, best.quality) else askFull = qs.first()
                        },
                        onDownload = { qualitiesFor = qs.first() },
                        onDelete = { clearFor = qs },
                    )
                }
            }
        }
    }

    askFull?.let { d ->
        val st = live[d.url] ?: d.stat
        AlertDialog(
            onDismissRequest = { askFull = null },
            title = { Text("Bu qism to'liq yuklab olinmagan") },
            text = { Text("Ko'rish uchun avval to'liq yuklab olinsinmi?\nHozir ${percent(st.ratio)} yuklangan.") },
            confirmButton = { TextButton(onClick = { AppServices.downloads.download(d.url); askFull = null }) { Text("Yuklab olish") } },
            dismissButton = { TextButton(onClick = { askFull = null }) { Text("Yo'q") } },
            containerColor = AruColors.Surface,
        )
    }
    qualitiesFor?.let { d -> QualitySheet(d, live) { qualitiesFor = null } }
    clearFor?.let { qs ->
        AlertDialog(
            onDismissRequest = { clearFor = null },
            title = { Text("Tozalash") },
            text = { Text("${qs.first().episodeNo}-qismning yuklab olingan barcha sifatlari telefondan o'chiriladi.") },
            confirmButton = { TextButton(onClick = { qs.forEach { AppServices.downloads.delete(it.url) }; clearFor = null }) { Text("O'chirish", color = Color(0xFFF87171)) } },
            dismissButton = { TextButton(onClick = { clearFor = null }) { Text("Bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
}

@Composable
private fun EpisodeDownloadRow(
    qs: List<DownloadItem>, live: Map<String, VideoStat>,
    onOpenFrame: () -> Unit, onDownload: () -> Unit, onDelete: () -> Unit,
) {
    val first = qs.first()
    val h = historyOf(first.animeId, first.seasonId, first.episodeId)
    val sorted = qs.sortedBy { it.quality.filter { c -> c.isDigit() }.toIntOrNull() ?: Int.MAX_VALUE }
    val speed = qs.sumOf { (live[it.url] ?: it.stat).speed }
    val active = qs.any { val s = live[it.url] ?: it.stat; s.downloading || s.queued }
    val shape = RoundedCornerShape(16.dp)
    Column(Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).padding(10.dp)) {
        Row(verticalAlignment = Alignment.Top) {
            Box(Modifier.weight(5f).aspectRatio(16f / 9f).clip(RoundedCornerShape(12.dp)).clickable(onClick = onOpenFrame)) {
                Frame(first, Modifier.fillMaxSize())
                if (h != null) Text(
                    "${clock(h.positionMs)}/${clock(h.durationMs)}", color = Color.White, fontSize = 10.sp, fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.align(Alignment.BottomEnd).padding(start = 6.dp, end = 6.dp, bottom = 7.dp),
                )
                Box(Modifier.align(Alignment.BottomStart).fillMaxWidth().height(3.5.dp).background(Color.White.copy(alpha = 0.24f))) {
                    Box(Modifier.fillMaxWidth(h?.progress ?: 0f).height(3.5.dp).background(Brush.horizontalGradient(listOf(AruColors.Accent, AruColors.Accent2))))
                }
            }
            Spacer(Modifier.width(10.dp))
            Column(Modifier.weight(4f)) {
                Text(first.title.ifEmpty { "Anime" }, color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.Bold, maxLines = 2, overflow = TextOverflow.Ellipsis)
                Text("${first.sectionNo}-bo'lim ${first.episodeNo}-qism", color = Color.White.copy(alpha = 0.65f), fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 3.dp))
                Row(Modifier.padding(top = 6.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    IconBtn(AruIcons.Download, Color.White.copy(alpha = 0.7f), onDownload)
                    IconBtn(AruIcons.DeleteOutline, Color(0xFFEF4444), onDelete)
                }
                val label = if (speed > 0) VideoStat(speed = speed).speedLabel else if (active) "kutilmoqda..." else ""
                if (label.isNotEmpty()) Row(Modifier.padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Icon(AruIcons.Speed, null, tint = AruColors.Accent.copy(alpha = 0.9f), modifier = Modifier.size(14.dp))
                    Spacer(Modifier.width(4.dp))
                    Text(label, color = Color.White.copy(alpha = 0.75f), fontSize = 11.5.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
                }
            }
        }
        if (sorted.isNotEmpty()) {
            Spacer(Modifier.height(10.dp))
            sorted.chunked(2).forEachIndexed { i, pair ->
                if (i > 0) Spacer(Modifier.height(6.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    pair.forEach { d -> Box(Modifier.weight(1f)) { QualityBox(d, live[d.url] ?: d.stat) } }
                    if (pair.size == 1) Spacer(Modifier.weight(1f))
                }
            }
        }
    }
}

private fun clock(ms: Long): String {
    val s = (ms / 1000).coerceAtLeast(0)
    return String.format(Locale.US, "%02d:%02d", s / 60, s % 60)
}

@Composable
private fun IconBtn(icon: androidx.compose.ui.graphics.vector.ImageVector, tint: Color, onClick: () -> Unit) {
    Box(Modifier.size(44.dp).clickable(onClick = onClick), contentAlignment = Alignment.Center) { Icon(icon, null, tint = tint, modifier = Modifier.size(26.dp)) }
}

/** `480p  18,22%` + `9,00 MB / 49,4 MB`; foiz bo'yicha to'lib boradi, to'liq bo'lsa urg'u rangli chegara. */
@Composable
private fun QualityBox(d: DownloadItem, st: VideoStat) {
    val done = st.complete
    val waiting = !done && (st.queued || (st.downloading && st.total <= 0))
    val shape = RoundedCornerShape(10.dp)
    Box(
        Modifier.fillMaxWidth().height(48.dp).clip(shape).background(AruColors.CardAlt)
            .border(1.dp, if (done) AruColors.Accent else AruColors.Border, shape)
            .clickable(enabled = !done) { if (st.downloading) AppServices.downloads.pause(d.url) else AppServices.downloads.download(d.url) },
        contentAlignment = Alignment.Center,
    ) {
        if (waiting) Box(Modifier.fillMaxSize().background(AruColors.Accent.copy(alpha = 0.25f)))
        Box(Modifier.align(Alignment.CenterStart).fillMaxHeight().fillMaxWidth(st.ratio).background(Brush.horizontalGradient(listOf(AruColors.Accent.copy(alpha = 0.85f), AruColors.Accent2.copy(alpha = 0.85f)))))
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text("${d.qualityTitle.ifEmpty { "?" }}  ${percent(st.ratio)}", color = Color.White, fontSize = 12.sp, fontWeight = FontWeight.ExtraBold, maxLines = 1)
            Text(if (st.total > 0) "${bytes(st.downloaded)} / ${bytes(st.total)}" else bytes(st.downloaded), color = Color.White.copy(alpha = 0.8f), fontSize = 10.5.sp, maxLines = 1)
        }
    }
}

/** ⬇ bosilganda: shu qismning barcha sifatlari (hajmi bilan) — bosilsa yuklash boshlanadi / to'xtaydi. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun QualitySheet(d: DownloadItem, live: Map<String, VideoStat>, onDismiss: () -> Unit) {
    var eps by remember { mutableStateOf<List<Episode>?>(null) }
    LaunchedEffect(d.animeId, d.seasonId) { eps = AppServices.seasonData.cachedEpisodes(d.animeId, d.seasonId) }
    val ep = eps?.firstOrNull { it.id == d.episodeId }
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = AruColors.Surface) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 20.dp).padding(bottom = 28.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("${d.episodeNo}-qism · sifatni tanlang", color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.Bold)
            if (ep == null) LoadingBox(Modifier.height(80.dp))
            else ep.qualities.forEach { q ->
                val url = ep.urls[q].orEmpty()
                val st = live[url] ?: VideoStat()
                val size = ep.sizes[q] ?: st.total
                Row(
                    Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(AruColors.Card).clickable {
                        if (st.complete) Unit else if (st.downloading) AppServices.downloads.pause(url) else AppServices.downloads.download(url)
                    }.padding(horizontal = 14.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(ep.qualityTitle(q), color = Color.White, fontWeight = FontWeight.Bold, fontSize = 15.sp, modifier = Modifier.width(90.dp))
                    Text(if (size > 0) bytes(size) else "", color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.weight(1f))
                    Text(
                        when { st.complete -> "Yuklangan"; st.downloading -> "${percent(st.ratio)} · Pauza"; st.downloaded > 0 -> "${percent(st.ratio)} · Davom"; else -> "Yuklab olish" },
                        color = if (st.complete) Color(0xFF4ADE80) else AruColors.Accent2, fontSize = 12.sp, fontWeight = FontWeight.SemiBold,
                    )
                }
            }
        }
    }
}

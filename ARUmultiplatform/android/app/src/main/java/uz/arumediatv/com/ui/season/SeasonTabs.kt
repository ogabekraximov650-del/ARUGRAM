package uz.arumediatv.com.ui.season

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import kotlinx.coroutines.launch
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.filled.Check
import androidx.compose.ui.draw.alpha
import androidx.compose.foundation.layout.heightIn
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import uz.arumediatv.com.data.VideoStat
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
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.Episode
import uz.arumediatv.com.data.Season
import uz.arumediatv.com.data.SeasonInfo
import uz.arumediatv.com.ui.common.AruIcons
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.profile.formatBytes
import uz.arumediatv.com.ui.theme.AruColors
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

/** `1.284.512` — mingliklar nuqta bilan (Flutter `formatCount`). */
fun formatCount(n: Long): String {
    val d = Math.abs(n).toString()
    val sb = StringBuilder()
    for (i in d.indices) {
        if (i > 0 && (d.length - i) % 3 == 0) sb.append('.')
        sb.append(d[i])
    }
    return if (n < 0) "-$sb" else sb.toString()
}

/** `12:05` — soat:daqiqa (Flutter `formatHours`). */
fun formatHours(ms: Long): String {
    val total = ms / 1000
    return "${formatCount(total / 3600)}:${((total % 3600) / 60).toString().padStart(2, '0')}"
}

fun formatYmd(ms: Long): String =
    SimpleDateFormat("dd.MM.yyyy", Locale.US).apply { timeZone = TimeZone.getTimeZone("GMT+5") }.format(Date(if (ms < 100_000_000_000L) ms * 1000 else ms))

/** `2026/10/16/09:44` (Flutter `formatMomentYmd`), Toshkent vaqti. */
fun formatMomentYmd(ms: Long): String =
    if (ms <= 0) "" else SimpleDateFormat("yyyy/MM/dd/HH:mm", Locale.US).apply { timeZone = TimeZone.getTimeZone("GMT+5") }.format(Date(if (ms < 100_000_000_000L) ms * 1000 else ms))

/** `10.00` (Flutter `formatRating`). */
fun formatRating(v: Double): String = String.format(Locale.US, "%05.2f", v.coerceIn(0.0, 10.0))

private val cardShape = RoundedCornerShape(18.dp)

@Composable
private fun CardBox(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    Box(modifier.fillMaxWidth().clip(cardShape).background(AruColors.Card).border(1.dp, AruColors.Border, cardShape)) { content() }
}

// ══════════════════════════ MA'LUMOT ══════════════════════════

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun InfoTab(season: Season, info: SeasonInfo?, onRate: (Int) -> Unit, onFavorite: () -> Unit) {
    var rating by remember { mutableStateOf(false) }
    fun s(k: String, fallback: String = ""): String = info?.str(k)?.takeIf { it.isNotEmpty() } ?: fallback
    val ratingCount = info?.ratingCount ?: season.ratingCount.toLong()
    val myStars = info?.myStars ?: 0
    val isFav = info?.isFav == true

    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            ActionTile(if (myStars > 0) Icons.Filled.Star else AruIcons.StarBorder, AruColors.Gold, if (myStars > 0) "Bahoyingiz: $myStars" else "Baholash", Modifier.weight(1f)) { rating = true }
            ActionTile(if (isFav) Icons.Filled.Favorite else AruIcons.FavoriteBorder, AruColors.Accent2, if (isFav) "Sevimlilarda" else "Sevimlilarga qo'shish", Modifier.weight(1f), onFavorite)
        }
        Spacer(Modifier.height(12.dp))
        CardBox {
            Column(Modifier.padding(16.dp)) {
                StatLine(AruIcons.Visibility, "Ko'rishlar", formatCount(info?.views ?: season.views))
                StatLine(AruIcons.Schedule, "Tomosha vaqti", "${formatHours(info?.watchMs ?: 0)} soat")
                StatLine(AruIcons.FavoriteBorder, "Sevimlilarga qo'shilgan", formatCount(info?.favCount ?: 0))
                StatLine(
                    AruIcons.StarBorder, "Reyting",
                    if (ratingCount > 0) "${formatRating(info?.rating ?: season.ratingAvg)}  (${formatCount(ratingCount)} ta baho)" else "hali baholanmagan",
                )
                val bolim = (info?.long("bolim_id")?.toInt() ?: season.sectionNo).takeIf { it > 0 } ?: 1
                StatLine(AruIcons.Movie, "Bo'lim", "$bolim-bo'lim · ${formatCount(info?.long("epizod_count") ?: season.episodes.toLong())} qism")
                val created = info?.long("created_at") ?: 0
                if (created > 0) StatLine(AruIcons.EventAvailable, "Qo'shilgan sana", formatMomentYmd(created))
            }
        }
        Spacer(Modifier.height(12.dp))
        CardBox {
            Column(Modifier.padding(16.dp)) {
                InfoRow("Turi", s("turi", season.kind))
                InfoRow("Yili", s("yili", season.year))
                InfoRow("Janri", s("janri", season.genres.joinToString(", ")))
                InfoRow("Studio", s("studio"))
                InfoRow("Tarjimon", s("tarjimon"))
                InfoRow("Holati", s("holati", season.status))
                val tavsif = s("tavsif")
                if (tavsif.isNotEmpty()) {
                    Spacer(Modifier.height(10.dp))
                    Text("Tavsif", color = AruColors.TextDim, fontSize = 12.sp, fontWeight = FontWeight.Medium)
                    Spacer(Modifier.height(6.dp))
                    Text(tavsif, color = Color.White.copy(alpha = 0.82f), fontSize = 14.sp, lineHeight = 22.sp)
                }
            }
        }
        Spacer(Modifier.height(24.dp))
    }

    if (rating) {
        AlertDialog(
            onDismissRequest = { rating = false },
            title = { Text("Baholang (1–10)") },
            text = {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    (1..10).forEach { n ->
                        val on = n == myStars
                        Box(
                            Modifier.size(46.dp).clip(CircleShape)
                                .background(if (on) AruColors.Accent else AruColors.CardAlt)
                                .clickable { rating = false; onRate(n) },
                            contentAlignment = Alignment.Center,
                        ) { Text("$n", color = Color.White, fontWeight = FontWeight.Bold) }
                    }
                }
            },
            confirmButton = {},
            dismissButton = { TextButton(onClick = { rating = false }) { Text("Bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
}

@Composable
private fun ActionTile(icon: androidx.compose.ui.graphics.vector.ImageVector, tint: Color, label: String, modifier: Modifier, onClick: () -> Unit) {
    Row(
        modifier.clip(cardShape).background(AruColors.Card).border(1.dp, AruColors.Border, cardShape).clickable(onClick = onClick).padding(horizontal = 14.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(icon, null, tint = tint, modifier = Modifier.size(22.dp))
        Spacer(Modifier.width(8.dp))
        Text(label, color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, maxLines = 2, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
private fun StatLine(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, value: String) {
    Row(Modifier.fillMaxWidth().padding(vertical = 5.dp), verticalAlignment = Alignment.CenterVertically) {
        Icon(icon, null, tint = Color.White.copy(alpha = 0.45f), modifier = Modifier.size(16.dp))
        Spacer(Modifier.width(9.dp))
        Text("$label:", color = AruColors.TextDim, fontSize = 13.sp)
        Spacer(Modifier.width(8.dp))
        Text(value, color = Color.White, fontSize = 13.5.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f), textAlign = androidx.compose.ui.text.style.TextAlign.End)
    }
}

@Composable
private fun InfoRow(label: String, value: String) {
    if (value.isEmpty()) return
    Row(Modifier.fillMaxWidth().padding(bottom = 8.dp)) {
        Text(label, color = AruColors.TextDim, fontSize = 13.sp, modifier = Modifier.width(90.dp))
        Text(value, color = Color.White, fontSize = 13.sp, modifier = Modifier.weight(1f))
    }
}

// ══════════════════════════ QISMLAR ══════════════════════════

private const val RANGE = 50

private fun clockMs(ms: Long): String {
    val t = (ms / 1000).coerceAtLeast(0)
    val h = t / 3600
    val m = (t % 3600) / 60
    val sec = t % 60
    return if (h > 0) String.format(java.util.Locale.US, "%d:%02d:%02d", h, m, sec) else String.format(java.util.Locale.US, "%d:%02d", m, sec)
}

@Composable
fun EpisodesTab(
    canWatch: Boolean,
    loading: Boolean,
    episodes: List<Episode>,
    currentId: Long?,
    currentQuality: String?,
    onPlay: (Episode, String?) -> Unit,
    onSubscribe: () -> Unit,
    onDeleted: (Episode, String) -> Unit = { _, _ -> },
) {
    when {
        !canWatch -> EmptyState(Icons.Filled.PlayArrow, "Qismlarni ko'rish uchun obuna kerak")
        loading && episodes.isEmpty() -> LoadingBox()
        episodes.isEmpty() -> EmptyState(Icons.Filled.PlayArrow, "Qismlar topilmadi")
        else -> {
            val asc = remember(episodes) { episodes.sortedBy { it.number } }
            var newestFirst by rememberSaveable { mutableStateOf(true) }
            val chunks = remember(asc) { asc.chunked(RANGE) }
            val curChunk = remember(asc, currentId) { chunks.indexOfFirst { c -> c.any { it.id == currentId } }.coerceAtLeast(0) }
            var chunk by rememberSaveable { mutableStateOf(-1) }
            // Qism almashganda (pastdagi tugmalar yoki avto) tegishli oraliq o'zi tanlanadi.
            androidx.compose.runtime.LaunchedEffect(currentId, chunks.size) { chunk = curChunk }
            val sel = if (chunks.size > 1) chunk.coerceIn(0, chunks.size - 1) else 0
            val shown = remember(chunks, sel, newestFirst) {
                val l = chunks.getOrElse(sel) { asc }
                if (newestFirst) l.asReversed() else l
            }
            val ls = androidx.compose.foundation.lazy.rememberLazyListState()
            androidx.compose.runtime.LaunchedEffect(currentId, shown) {
                val i = shown.indexOfFirst { it.id == currentId }
                if (i >= 0) ls.animateScrollToItem((i - 1).coerceAtLeast(0))
            }
            Column(Modifier.fillMaxSize()) {
                androidx.compose.foundation.lazy.LazyRow(
                    contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.padding(bottom = 8.dp),
                ) {
                    if (chunks.size > 1) {
                        items(chunks.size) { i ->
                            val c = chunks[i]
                            ChipBtn("${c.first().number}–${c.last().number}", i == sel) { chunk = i }
                        }
                    }
                    item { ChipBtn(if (newestFirst) "⇅ Yangi" else "⇅ Eski", false) { newestFirst = !newestFirst } }
                }
                LazyColumn(Modifier.fillMaxSize(), state = ls, contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(shown, key = { it.id }) { e -> EpisodeTile(e, e.id == currentId, onPlay, onDeleted) }
                }
            }
        }
    }
}

@Composable
private fun ChipBtn(text: String, on: Boolean, onClick: () -> Unit) {
    val sh = RoundedCornerShape(20.dp)
    Box(
        Modifier.height(40.dp).clip(sh)
            .background(if (on) AruColors.AccentTint else AruColors.Card)
            .border(1.dp, if (on) AruColors.Accent else AruColors.Border, sh)
            .clickable(onClick = onClick).padding(horizontal = 14.dp),
        contentAlignment = Alignment.Center,
    ) { Text(text, color = if (on) AruColors.Accent2 else Color.White.copy(alpha = 0.8f), fontSize = 13.sp, fontWeight = FontWeight.SemiBold, maxLines = 1) }
}

@Composable
private fun EpisodeTile(e: Episode, isCurrent: Boolean, onPlay: (Episode, String?) -> Unit, onDeleted: (Episode, String) -> Unit) {
    val shape = RoundedCornerShape(16.dp)
    var expanded by remember { mutableStateOf(false) }
    val dm = AppServices.downloads
    DisposableEffect(expanded) {
        if (expanded) dm.watch(e, e.urls.values.toSet())
        onDispose { dm.unwatch(e) }
    }
    // Ko'rish holati: eng ko'p ko'rilgan sifat bo'yicha.
    val snap = remember(e.id, isCurrent) { e.urls.values.filter { it.isNotEmpty() }.mapNotNull { AppServices.progress.snapshot(it) }.maxByOrNull { it.first } }
    val frac = snap?.let { if (it.second > 0) (it.first.toFloat() / it.second).coerceIn(0f, 1f) else 0f } ?: 0f
    val seen = !isCurrent && snap != null && snap.first > snap.second - 5000
    val status = when {
        snap != null && snap.first >= 3000 && snap.first <= snap.second - 5000 -> "${clockMs(snap.first)} dan davom etasiz"
        seen -> "Ko'rildi ✓"
        isCurrent -> "Hozir ijro etilmoqda"
        else -> "Ko'rilmagan"
    }
    // Yuklash holati (barcha sifatlar bo'yicha): to'liq / yuklanmoqda (foiz) / yo'q.
    val all by dm.stats.collectAsStateWithLifecycle()
    var done = false
    var active = false
    var pct = 0
    e.urls.values.filter { it.isNotEmpty() }.forEach { u ->
        val st = all[u]
        if (st != null) {
            if (st.complete) done = true
            if (st.downloading || st.queued) { active = true; pct = maxOf(pct, st.percent) }
        }
    }
    if (!done && !active) done = e.urls.values.any { it.isNotEmpty() && AppServices.videoCache.isComplete(it) }
    Column(
        Modifier.fillMaxWidth().alpha(if (seen) 0.6f else 1f).clip(shape)
            .background(if (isCurrent) AruColors.AccentTint else AruColors.Card)
            .border(1.dp, if (isCurrent) AruColors.Accent.copy(alpha = 0.5f) else AruColors.Border, shape),
    ) {
        Row(Modifier.fillMaxWidth().heightIn(min = 72.dp), verticalAlignment = Alignment.CenterVertically) {
            Row(
                Modifier.weight(1f).clickable { onPlay(e, null) }.padding(start = 12.dp, top = 12.dp, bottom = 12.dp).heightIn(min = 48.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(
                    Modifier.size(48.dp).clip(RoundedCornerShape(13.dp)).background(if (isCurrent) AruColors.Accent else AruColors.CardAlt),
                    contentAlignment = Alignment.Center,
                ) {
                    if (isCurrent) Icon(Icons.Filled.PlayArrow, null, tint = Color.White, modifier = Modifier.size(26.dp))
                    else Text("${e.number}", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 16.sp)
                }
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text("${e.number}-qism", color = if (isCurrent) AruColors.Accent2 else Color.White, fontSize = 15.5.sp, fontWeight = FontWeight.Bold, maxLines = 1)
                    Text(
                        if (e.name.isNotBlank()) "$status · ${e.name}" else status,
                        color = AruColors.TextDim, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            // Yuklash holati: bosilsa sifatlar ochiladi (adashib katta yuklash boshlanmasin).
            Box(Modifier.size(48.dp).clip(CircleShape).clickable { expanded = !expanded }, contentAlignment = Alignment.Center) {
                when {
                    done -> Icon(Icons.Filled.Check, null, tint = Color(0xFF4ADE80), modifier = Modifier.size(24.dp))
                    active -> Box(Modifier.size(38.dp), contentAlignment = Alignment.Center) {
                        androidx.compose.material3.CircularProgressIndicator(
                            progress = { pct / 100f }, modifier = Modifier.fillMaxSize(), color = AruColors.Accent2,
                            trackColor = Color.White.copy(alpha = 0.12f), strokeWidth = 3.dp,
                        )
                        Text("$pct", color = AruColors.Accent2, fontSize = 10.sp, fontWeight = FontWeight.Bold)
                    }
                    else -> Icon(AruIcons.Download, null, tint = Color.White.copy(alpha = 0.6f), modifier = Modifier.size(24.dp))
                }
            }
            Box(Modifier.padding(end = 4.dp).size(48.dp).clip(CircleShape).clickable { expanded = !expanded }, contentAlignment = Alignment.Center) {
                Text(if (expanded) "▴" else "▾", color = AruColors.TextDim, fontSize = 22.sp)
            }
        }
        if (frac > 0f && !seen) {
            Box(Modifier.padding(horizontal = 12.dp).fillMaxWidth().height(3.dp).clip(RoundedCornerShape(2.dp)).background(Color.White.copy(alpha = 0.1f))) {
                Box(Modifier.fillMaxWidth(frac).fillMaxHeight().background(Brush.horizontalGradient(listOf(AruColors.Accent, AruColors.Accent2))))
            }
            Spacer(Modifier.height(10.dp))
        }
        if (expanded) {
            Column(Modifier.padding(start = 12.dp, end = 12.dp, bottom = 10.dp)) {
                e.qualities.forEach { q -> QualityRow(e, q, { onPlay(e, q) }, { onDeleted(e, q) }) }
            }
        }
    }
}

/** Sifat qatori (Flutter `_QualityRow`): sifat, foiz, hajm, tezlik/navbat belgilari, chiziq, yuklash/pauza va o'chirish (tasdiq bilan). */
@Composable
private fun QualityRow(e: Episode, q: String, onPlay: () -> Unit, onDeleted: (String) -> Unit) {
    val dm = AppServices.downloads
    val all by dm.stats.collectAsStateWithLifecycle()
    val url = e.urls[q].orEmpty()
    val st = all[url] ?: VideoStat()
    val size = if (st.total > 0) st.total else e.sizes[q] ?: 0
    var confirm by remember { mutableStateOf(false) }
    Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    e.qualityTitle(q), color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.ExtraBold,
                    modifier = Modifier.clip(RoundedCornerShape(6.dp)).clickable(onClick = onPlay),
                )
                Spacer(Modifier.width(8.dp))
                Text("${st.percent}%", color = if (st.complete) AruColors.Accent else Color.White, fontSize = 13.sp, fontWeight = FontWeight.ExtraBold)
                Spacer(Modifier.width(8.dp))
                Text(
                    "${formatBytes(st.downloaded)} / ${if (size > 0) formatBytes(size) else "—"}",
                    color = Color.White.copy(alpha = 0.65f), fontSize = 11.5.sp, fontWeight = FontWeight.SemiBold, maxLines = 1,
                )
            }
            val speed = if (st.downloading && !st.queued) st.speedLabel else ""
            if (speed.isNotEmpty() || st.retrying || st.queued) {
                Spacer(Modifier.height(3.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    if (st.queued) DlChip("navbatda", Color.White.copy(alpha = 0.12f), Color.White.copy(alpha = 0.7f))
                    if (speed.isNotEmpty()) DlChip(speed, AruColors.Accent.copy(alpha = 0.18f), AruColors.Accent)
                    if (st.retrying) DlChip("qayta urinilmoqda", Color(0x2DFF9800), Color(0xFFFF9800))
                }
            }
            Spacer(Modifier.height(5.dp))
            Box(Modifier.padding(end = 12.dp).fillMaxWidth().height(4.dp).clip(RoundedCornerShape(3.dp)).background(Color.White.copy(alpha = 0.10f))) {
                Box(Modifier.fillMaxWidth(st.ratio).fillMaxHeight().background(if (st.complete) Color.White else AruColors.Accent))
            }
        }
        if (!st.complete) {
            SquareBtn(
                if (st.downloading) AruIcons.Pause else AruIcons.Download,
                if (st.downloading) AruColors.Accent else Color.White,
                if (st.downloading) AruColors.Accent.copy(alpha = 0.22f) else Color.White.copy(alpha = 0.08f),
                if (st.downloading) AruColors.Accent.copy(alpha = 0.4f) else Color.White.copy(alpha = 0.15f),
            ) { if (st.downloading) dm.pause(url) else dm.download(url) }
        }
        Spacer(Modifier.width(5.dp))
        SquareBtn(AruIcons.DeleteOutline, Color.White.copy(alpha = 0.7f), Color.White.copy(alpha = 0.08f), Color.Transparent) { confirm = true }
    }
    if (confirm) {
        AlertDialog(
            onDismissRequest = { confirm = false },
            containerColor = AruColors.Card,
            shape = RoundedCornerShape(20.dp),
            title = { Text("Tozalash", color = Color.White, fontSize = 17.sp, fontWeight = FontWeight.Bold) },
            text = { Text("Rostanham ${e.qualityTitle(q)} sifatidagi videoni tozalab tashlaysizmi?", color = Color.White.copy(alpha = 0.7f), fontSize = 14.sp) },
            dismissButton = { TextButton(onClick = { confirm = false }) { Text("Bekor qilish", color = Color.White.copy(alpha = 0.54f)) } },
            confirmButton = {
                TextButton(onClick = {
                    confirm = false
                    dm.delete(url)
                    AppServices.progress.forget(url)
                    onDeleted(q)
                }) { Text("Ha", color = AruColors.Accent, fontWeight = FontWeight.Bold) }
            },
        )
    }
}

@Composable
private fun DlChip(text: String, bg: Color, fg: Color) {
    Text(text, color = fg, fontSize = 11.sp, fontWeight = FontWeight.Bold, modifier = Modifier.clip(RoundedCornerShape(5.dp)).background(bg).padding(horizontal = 6.dp, vertical = 2.dp))
}

@Composable
private fun SquareBtn(icon: androidx.compose.ui.graphics.vector.ImageVector, tint: Color, bg: Color, border: Color, onClick: () -> Unit) {
    val sh = RoundedCornerShape(12.dp)
    Box(
        Modifier.size(44.dp).clip(sh).background(bg).border(1.dp, border, sh).clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { Icon(icon, null, tint = tint, modifier = Modifier.size(23.dp)) }
}

@Composable
private fun MiniBtn(text: String, danger: Boolean = false, onClick: () -> Unit) {
    Text(
        text, color = if (danger) Color(0xFFF87171) else Color.White, fontSize = 11.sp, fontWeight = FontWeight.SemiBold,
        modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(if (danger) Color(0x22F87171) else AruColors.Accent).clickable(onClick = onClick).padding(horizontal = 10.dp, vertical = 6.dp),
    )
}

// ══════════════════════════ BO'LIMLAR ══════════════════════════

@Composable
fun SeasonsTab(seasons: List<Season>, current: Season, onOpen: (Season) -> Unit) {
    if (seasons.isEmpty()) {
        EmptyState(Icons.Filled.PlayArrow, "Bo'limlar topilmadi")
        return
    }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        items(seasons, key = { it.key }) { s ->
            val isCur = s.seasonId == current.seasonId
            val shape = RoundedCornerShape(16.dp)
            Row(
                Modifier.fillMaxWidth().clip(shape).background(if (isCur) AruColors.AccentTint else AruColors.Card)
                    .border(1.dp, if (isCur) AruColors.Accent.copy(alpha = 0.5f) else AruColors.Border, shape)
                    .clickable(enabled = !isCur) { onOpen(s) }.padding(10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TgImage(s.photoUrl, Modifier.size(54.dp).clip(RoundedCornerShape(10.dp)).background(AruColors.CardAlt), maxPx = 200)
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(s.name.ifEmpty { "${s.sectionNo}-bo'lim" }, color = if (isCur) AruColors.Accent2 else Color.White, fontSize = 14.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text("${s.sectionNo}-bo'lim · ${s.episodes} qism", color = AruColors.TextDim, fontSize = 12.sp)
                }
                if (s.free == false) Text("Pullik", color = AruColors.Gold, fontSize = 11.sp, fontWeight = FontWeight.Bold)
                if (isCur) Icon(Icons.Filled.PlayArrow, null, tint = AruColors.Accent2)
            }
        }
    }
}

// ══════════════════════════ PLEYER SOZLAMALARI ══════════════════════════

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun PlayerSettingsSheet(
    episode: Episode?,
    currentUrl: String,
    speed: Float,
    autoNext: Boolean,
    autoSkipIntro: Boolean,
    sleepEndAt: Long,
    onQuality: (String) -> Unit,
    onSpeed: (Float) -> Unit,
    onAutoNext: (Boolean) -> Unit,
    onAutoSkip: (Boolean) -> Unit,
    onSleep: (Int) -> Unit,
    onDismiss: () -> Unit,
) {
    PlayerSheetHost("Sozlamalar", onDismiss) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp).padding(bottom = 28.dp)) {
            if (episode != null && episode.qualities.isNotEmpty()) {
                SheetTitle("Sifat")
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    val cur = episode.qualityOf(currentUrl)
                    episode.qualities.forEach { q ->
                        val size = episode.sizes[q] ?: 0
                        Chip("${episode.qualityTitle(q)}${if (size > 0) " · ${formatBytes(size)}" else ""}", q == cur) { onQuality(q); onDismiss() }
                    }
                }
            }
            SheetTitle("Tezlik")
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf(0.5f, 0.75f, 1f, 1.25f, 1.5f, 2f).forEach { v -> Chip(if (v == 1f) "Oddiy" else "${v}x", speed == v) { onSpeed(v) } }
            }
            SheetTitle("Avtomatik")
            ToggleRow("Keyingi qismni avto ochish", autoNext, onAutoNext)
            ToggleRow("Introni avto o'tkazish", autoSkipIntro, onAutoSkip)
            SheetTitle("Uyqu taymeri" + if (sleepEndAt > 0) " · ${((sleepEndAt - System.currentTimeMillis()) / 60_000 + 1).coerceAtLeast(1)} daqiqa qoldi" else "")
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Chip("O'chiq", sleepEndAt <= 0) { onSleep(0) }
                listOf(15, 30, 45, 60, 90).forEach { m -> Chip("$m daqiqa", false) { onSleep(m); onDismiss() } }
            }
        }
    }
}

/**
 * Pleyer oynalari (sozlamalar, qismlar ro'yxati) uchun qulay joylashuv: gorizontal (to'liq ekran) holatda o'ng tomondagi
 * to'liq balandlikdagi panel (bosh barmoq yetadi, kontent siljiydi), tik holatda esa to'liq ochiladigan pastki oyna.
 * Tashqarini bossangiz yopiladi.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun PlayerSheetHost(title: String, onDismiss: () -> Unit, content: @Composable () -> Unit) {
    val landscape = androidx.compose.ui.platform.LocalConfiguration.current.orientation == android.content.res.Configuration.ORIENTATION_LANDSCAPE
    if (landscape) {
        // O'ngdan suzib chiqadi va yopilganda o'ngga suzib ketadi.
        var shown by remember { mutableStateOf(false) }
        val scope = androidx.compose.runtime.rememberCoroutineScope()
        androidx.compose.runtime.LaunchedEffect(Unit) { shown = true }
        val close: () -> Unit = { if (shown) { shown = false; scope.launch { kotlinx.coroutines.delay(240); onDismiss() } } }
        androidx.compose.ui.window.Dialog(
            onDismissRequest = close,
            properties = androidx.compose.ui.window.DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false),
        ) {
            // Video usti QORAYTIRILMAYDI (xiralashtiruvchi parda yo'q); tashqarini bossangiz panel yopiladi.
            Box(
                Modifier.fillMaxSize()
                    .clickable(interactionSource = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }, indication = null, onClick = close),
            ) {
                androidx.compose.animation.AnimatedVisibility(
                    visible = shown,
                    modifier = Modifier.align(Alignment.CenterEnd),
                    enter = androidx.compose.animation.slideInHorizontally(androidx.compose.animation.core.tween(280, easing = androidx.compose.animation.core.FastOutSlowInEasing)) { it } + androidx.compose.animation.fadeIn(androidx.compose.animation.core.tween(200)),
                    exit = androidx.compose.animation.slideOutHorizontally(androidx.compose.animation.core.tween(220)) { it } + androidx.compose.animation.fadeOut(androidx.compose.animation.core.tween(200)),
                ) {
                    Column(
                        Modifier.fillMaxHeight().widthIn(max = 560.dp).fillMaxWidth(0.66f)
                            .clip(RoundedCornerShape(topStart = 22.dp, bottomStart = 22.dp)).background(AruColors.Surface)
                            .clickable(interactionSource = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }, indication = null) {}
                            .statusBarsPadding().navigationBarsPadding(),
                    ) {
                        Row(Modifier.fillMaxWidth().padding(start = 20.dp, end = 8.dp, top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                            Text(title, color = Color.White, fontSize = 17.sp, fontWeight = FontWeight.Bold, modifier = Modifier.weight(1f))
                            Box(Modifier.size(48.dp).clip(CircleShape).clickable(onClick = close), contentAlignment = Alignment.Center) {
                                Text("✕", color = Color.White.copy(alpha = 0.8f), fontSize = 18.sp)
                            }
                        }
                        content()
                    }
                }
            }
        }
    } else {
        ModalBottomSheet(onDismissRequest = onDismiss, containerColor = AruColors.Surface, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) { content() }
    }
}

@Composable
private fun SheetTitle(t: String) {
    Text(t, color = AruColors.Accent2, fontSize = 12.sp, fontWeight = FontWeight.Bold, letterSpacing = 1.sp, modifier = Modifier.padding(top = 16.dp, bottom = 8.dp))
}

@Composable
private fun Chip(text: String, selected: Boolean, onClick: () -> Unit) {
    val shape = RoundedCornerShape(12.dp)
    Text(
        text, color = if (selected) Color.White else Color.White.copy(alpha = 0.75f), fontSize = 13.sp, fontWeight = FontWeight.SemiBold, maxLines = 1,
        modifier = Modifier.clip(shape).defaultMinSize(minHeight = 40.dp)
            .then(if (selected) Modifier.background(Brush.linearGradient(listOf(AruColors.Accent, AruColors.Accent2))) else Modifier.background(AruColors.Card).border(1.dp, AruColors.Border, shape))
            .clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 11.dp),
    )
}

@Composable
private fun ToggleRow(label: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth().defaultMinSize(minHeight = 52.dp).clickable { onChange(!checked) }, verticalAlignment = Alignment.CenterVertically) {
        Text(label, color = Color.White, fontSize = 15.sp, modifier = Modifier.weight(1f))
        Switch(checked = checked, onCheckedChange = onChange, colors = SwitchDefaults.colors(checkedThumbColor = Color.White, checkedTrackColor = AruColors.Accent))
    }
}

/** To'liq ekrandagi "qismlar ro'yxati": bossa shu qism ochiladi. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EpisodeListSheet(episodes: List<Episode>, currentId: Long?, onPick: (Episode) -> Unit, onDismiss: () -> Unit) {
    PlayerSheetHost("Qismlar", onDismiss) {
        val desc = remember(episodes) { episodes.sortedByDescending { it.number } }
        val ls = androidx.compose.foundation.lazy.rememberLazyListState()
        // Ochilganda joriy qism ko'rinadigan joyga suriladi.
        androidx.compose.runtime.LaunchedEffect(Unit) {
            val i = desc.indexOfFirst { it.id == currentId }
            if (i > 1) ls.scrollToItem(i - 1)
        }
        LazyColumn(Modifier.fillMaxWidth(), state = ls, contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(desc, key = { it.id }) { e ->
                val cur = e.id == currentId
                Row(
                    Modifier.fillMaxWidth().defaultMinSize(minHeight = 54.dp).clip(RoundedCornerShape(14.dp)).background(if (cur) AruColors.AccentTint else AruColors.Card)
                        .clickable { onPick(e); onDismiss() }.padding(horizontal = 16.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text("${e.number}-qism", color = if (cur) AruColors.Accent2 else Color.White, fontWeight = FontWeight.Bold, fontSize = 16.sp)
                    if (e.name.isNotBlank()) Text("  ${e.name}", color = AruColors.TextDim, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        }    }
}

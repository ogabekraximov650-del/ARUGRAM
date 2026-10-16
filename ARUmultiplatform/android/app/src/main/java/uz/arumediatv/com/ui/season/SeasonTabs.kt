package uz.arumediatv.com.ui.season

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.profile.formatBytes
import uz.arumediatv.com.ui.theme.AruColors
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

fun formatCount(n: Long): String = when {
    n >= 1_000_000 -> String.format(Locale.US, "%.1fM", n / 1_000_000.0)
    n >= 10_000 -> String.format(Locale.US, "%.1fK", n / 1_000.0)
    else -> n.toString()
}

fun formatHours(ms: Long): String = String.format(Locale.US, "%.1f", ms / 3_600_000.0)

fun formatYmd(ms: Long): String =
    SimpleDateFormat("dd.MM.yyyy", Locale.US).apply { timeZone = TimeZone.getTimeZone("GMT+5") }.format(Date(if (ms < 100_000_000_000L) ms * 1000 else ms))

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
            ActionTile(Icons.Filled.Star, AruColors.Gold, if (myStars > 0) "Bahoyingiz: $myStars" else "Baholash", Modifier.weight(1f)) { rating = true }
            ActionTile(Icons.Filled.Favorite, AruColors.Accent2, if (isFav) "Sevimlilarda" else "Sevimlilarga qo'shish", Modifier.weight(1f), onFavorite)
        }
        Spacer(Modifier.height(12.dp))
        CardBox {
            Column(Modifier.padding(16.dp)) {
                StatLine("Ko'rishlar", formatCount(info?.views ?: season.views))
                StatLine("Tomosha vaqti", "${formatHours(info?.watchMs ?: 0)} soat")
                StatLine("Sevimlilarga qo'shilgan", formatCount(info?.favCount ?: 0))
                StatLine(
                    "Reyting",
                    if (ratingCount > 0) "${String.format(Locale.US, "%.1f", info?.rating ?: season.ratingAvg)}  ($ratingCount ta baho)" else "hali baholanmagan",
                )
                val bolim = (info?.long("bolim_id")?.toInt() ?: season.sectionNo).takeIf { it > 0 } ?: 1
                StatLine("Bo'lim", "$bolim-bo'lim · ${info?.long("epizod_count") ?: season.episodes} qism")
                val created = info?.long("created_at") ?: 0
                if (created > 0) StatLine("Qo'shilgan sana", formatYmd(created))
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
private fun StatLine(label: String, value: String) {
    Row(Modifier.fillMaxWidth().padding(vertical = 5.dp)) {
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

@Composable
fun EpisodesTab(
    canWatch: Boolean,
    loading: Boolean,
    episodes: List<Episode>,
    currentId: Long?,
    currentQuality: String?,
    onPlay: (Episode, String?) -> Unit,
    onSubscribe: () -> Unit,
) {
    when {
        !canWatch -> EmptyState(Icons.Filled.PlayArrow, "Qismlarni ko'rish uchun obuna kerak")
        loading && episodes.isEmpty() -> LoadingBox()
        episodes.isEmpty() -> EmptyState(Icons.Filled.PlayArrow, "Qismlar topilmadi")
        else -> {
            val desc = remember(episodes) { episodes.sortedByDescending { it.number } }
            LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                items(desc, key = { it.id }) { e -> EpisodeTile(e, e.id == currentId, onPlay) }
            }
        }
    }
}

@Composable
private fun EpisodeTile(e: Episode, isCurrent: Boolean, onPlay: (Episode, String?) -> Unit) {
    val shape = RoundedCornerShape(16.dp)
    val frac = AppServices.progress.fractionOf(e.urlFor(null))
    Column(
        Modifier.fillMaxWidth().clip(shape)
            .background(if (isCurrent) AruColors.AccentTint else AruColors.Card)
            .border(1.dp, if (isCurrent) AruColors.Accent.copy(alpha = 0.5f) else AruColors.Border, shape)
            .clickable { onPlay(e, null) }.padding(12.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier.size(42.dp).clip(RoundedCornerShape(12.dp)).background(if (isCurrent) AruColors.Accent else AruColors.CardAlt),
                contentAlignment = Alignment.Center,
            ) {
                if (isCurrent) Icon(Icons.Filled.PlayArrow, null, tint = Color.White, modifier = Modifier.size(24.dp))
                else Text("${e.number}", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 15.sp)
            }
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text("${e.number}-qism", color = if (isCurrent) AruColors.Accent2 else Color.White, fontSize = 15.sp, fontWeight = FontWeight.Bold)
                if (e.name.isNotBlank()) Text(e.name, color = AruColors.TextDim, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            if (e.views > 0) Text("${formatCount(e.views)} ko'rish", color = AruColors.TextFaint, fontSize = 11.sp)
        }
        if (frac > 0f) {
            Spacer(Modifier.height(8.dp))
            Box(Modifier.fillMaxWidth().height(3.dp).clip(RoundedCornerShape(2.dp)).background(Color.White.copy(alpha = 0.1f))) {
                Box(Modifier.fillMaxWidth(frac).fillMaxHeight().background(Brush.horizontalGradient(listOf(AruColors.Accent, AruColors.Accent2))))
            }
        }
        if (e.qualities.size > 1 || isCurrent) {
            Spacer(Modifier.height(8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                e.qualities.forEach { q ->
                    Text(
                        e.qualityTitle(q), color = Color.White.copy(alpha = 0.8f), fontSize = 11.sp, fontWeight = FontWeight.SemiBold,
                        modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(Color.White.copy(alpha = 0.08f)).clickable { onPlay(e, q) }.padding(horizontal = 9.dp, vertical = 4.dp),
                    )
                }
            }
        }
    }
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
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = AruColors.Surface) {
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

@Composable
private fun SheetTitle(t: String) {
    Text(t, color = AruColors.Accent2, fontSize = 12.sp, fontWeight = FontWeight.Bold, letterSpacing = 1.sp, modifier = Modifier.padding(top = 16.dp, bottom = 8.dp))
}

@Composable
private fun Chip(text: String, selected: Boolean, onClick: () -> Unit) {
    val shape = RoundedCornerShape(12.dp)
    Text(
        text, color = if (selected) Color.White else Color.White.copy(alpha = 0.75f), fontSize = 13.sp, fontWeight = FontWeight.SemiBold,
        modifier = Modifier.clip(shape)
            .then(if (selected) Modifier.background(Brush.linearGradient(listOf(AruColors.Accent, AruColors.Accent2))) else Modifier.background(AruColors.Card).border(1.dp, AruColors.Border, shape))
            .clickable(onClick = onClick).padding(horizontal = 14.dp, vertical = 9.dp),
    )
}

@Composable
private fun ToggleRow(label: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(label, color = Color.White, fontSize = 14.sp, modifier = Modifier.weight(1f))
        Switch(checked = checked, onCheckedChange = onChange, colors = SwitchDefaults.colors(checkedThumbColor = Color.White, checkedTrackColor = AruColors.Accent))
    }
}

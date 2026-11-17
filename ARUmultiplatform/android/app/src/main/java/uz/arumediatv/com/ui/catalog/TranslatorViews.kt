package uz.arumediatv.com.ui.catalog

import androidx.compose.foundation.Canvas
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
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uz.arumediatv.com.data.Season
import uz.arumediatv.com.data.Translator
import uz.arumediatv.com.data.TranslatorPage
import uz.arumediatv.com.data.TranslatorRepo
import uz.arumediatv.com.data.TranslatorStats
import uz.arumediatv.com.data.latestAnime
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.common.LocalBarPad
import uz.arumediatv.com.ui.common.SeasonGrid
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.common.fullSpan
import uz.arumediatv.com.ui.home.SeasonCard
import uz.arumediatv.com.ui.season.formatCount
import uz.arumediatv.com.ui.theme.AruColors

/** Dumaloq tarjimon logotipi; rasm yo'q yoki yuklanmagan bo'lsa nomning bosh harflari. */
@Composable
fun TranslatorLogo(t: Translator, size: Dp) {
    Box(
        Modifier.size(size).clip(CircleShape).background(AruColors.AccentTint).border(1.dp, AruColors.BorderBright, CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        val initials = t.name.split(' ', '.', '-').filter { it.isNotBlank() }.take(2).joinToString("") { it.take(1).uppercase() }.ifEmpty { "?" }
        TgImage(
            url = t.photoUrl,
            modifier = Modifier.fillMaxSize(),
            maxPx = 256,
            placeholder = {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text(initials, color = AruColors.Accent2, fontSize = (size.value / 2.4f).sp, fontWeight = FontWeight.Bold)
                }
            },
        )
    }
}

/** O'ngga qaragan chiziqli uchburchak (strelka): "Accountlarim"dagi kabi, lekin uchi o'ngga. */
@Composable
private fun ChevronRight() {
    Canvas(Modifier.size(width = 9.dp, height = 16.dp)) {
        val sw = 2.2.dp.toPx()
        val w = size.width
        val h = size.height
        drawLine(Color.White, Offset(sw / 2, sw / 2), Offset(w - sw / 2, h / 2), sw, StrokeCap.Round)
        drawLine(Color.White, Offset(w - sw / 2, h / 2), Offset(sw / 2, h - sw / 2), sw, StrokeCap.Round)
    }
}

/**
 * Katalogning "Tarjimonlar" bo'limi: har tarjimon uchun qator (dumaloq logotip, nom, o'ngda strelka) va ostida oxirgi qo'shilgan 2 ta anime.
 * Qatorga (logotip, nom, strelka) bosilsa tarjimon sahifasi ochiladi.
 */
@Composable
fun TranslatorsTab(seasons: List<Season>, onOpenTranslator: (Translator) -> Unit, onOpenSeason: (Season) -> Unit) {
    var list by remember { mutableStateOf<List<Translator>?>(null) }
    LaunchedEffect(Unit) { list = TranslatorRepo.list() }
    val l = list
    // Animesi bor tarjimonlargina ko'rsatiladi.
    val shown = remember(l, seasons) { l.orEmpty().filter { t -> t.animeCount > 0 || seasons.any { it.trId == t.id } } }
    when {
        l == null -> LoadingBox()
        shown.isEmpty() -> EmptyState(Icons.Filled.Menu, "Hozircha tarjimonlar yo'q")
        else -> LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 24.dp + LocalBarPad.current.calculateBottomPadding()),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            items(shown, key = { it.id }) { t ->
                Column {
                    Row(
                        Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).clickable { onOpenTranslator(t) }.padding(vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        TranslatorLogo(t, 40.dp)
                        Spacer(Modifier.width(10.dp))
                        Text(t.name, color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                        Spacer(Modifier.width(8.dp))
                        ChevronRight()
                        Spacer(Modifier.width(6.dp))
                    }
                    Spacer(Modifier.height(8.dp))
                    val latest = latestAnime(seasons, t.id)
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                        latest.forEach { s -> SeasonCard(season = s, onClick = { onOpenSeason(s) }, modifier = Modifier.weight(1f).aspectRatio(0.65f)) }
                        if (latest.size < 2) Spacer(Modifier.weight((2 - latest.size).toFloat()))
                    }
                }
            }
        }
    }
}

@Composable
private fun StatTile(label: String, value: String, modifier: Modifier) {
    val shape = RoundedCornerShape(14.dp)
    Column(
        modifier.clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).padding(horizontal = 4.dp, vertical = 10.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(Modifier.height(26.dp), contentAlignment = Alignment.Center) {
            Text(label, color = Color.White.copy(alpha = 0.55f), fontSize = 10.5.sp, lineHeight = 12.sp, maxLines = 2, textAlign = TextAlign.Center)
        }
        Text(value, color = Color.White, fontSize = 17.sp, fontWeight = FontWeight.ExtraBold, maxLines = 1, modifier = Modifier.padding(top = 4.dp))
    }
}

/**
 * Tarjimon sahifasi: tepada o'rtada dumaloq logotip, nom, tavsif; to'rtta oyna (jami anime, qismlar, ko'rishlar, o'rtacha baho); pastda animelari.
 */
@Composable
fun TranslatorScreen(translator: Translator, onBack: () -> Unit, onOpenSeason: (Season) -> Unit) {
    var page by remember { mutableStateOf<TranslatorPage?>(null) }
    var failed by remember { mutableStateOf(false) }
    LaunchedEffect(translator.id) {
        // Avval oxirgi saqlangan nusxa (darhol), keyin serverdan yangisi (jimgina almashadi).
        TranslatorRepo.cachedPage(translator.id)?.let { page = it }
        val fresh = TranslatorRepo.page(translator.id)
        if (fresh != null) page = fresh
        failed = page == null
    }
    val t = page?.translator ?: translator
    val stats = page?.stats ?: TranslatorStats()
    Box(Modifier.fillMaxSize()) {
        SeasonGrid(
            items = page?.seasons.orEmpty(),
            onOpen = onOpenSeason,
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 56.dp, bottom = 24.dp + LocalBarPad.current.calculateBottomPadding()),
            header = {
                fullSpan {
                    Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
                        TranslatorLogo(t, 96.dp)
                        Spacer(Modifier.height(12.dp))
                        Text(t.name, color = Color.White, fontSize = 21.sp, fontWeight = FontWeight.ExtraBold, textAlign = TextAlign.Center)
                        if (t.bio.isNotEmpty()) {
                            Spacer(Modifier.height(8.dp))
                            Text(t.bio, color = AruColors.TextDim, fontSize = 13.sp, lineHeight = 18.sp, textAlign = TextAlign.Center, modifier = Modifier.padding(horizontal = 12.dp))
                        }
                    }
                }
                fullSpan {
                    Row(Modifier.fillMaxWidth().padding(top = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        val ready = page != null
                        StatTile("Jami anime", if (ready) stats.anime.toString() else "—", Modifier.weight(1f))
                        StatTile("Jami qismlar", if (ready) stats.episodes.toString() else "—", Modifier.weight(1f))
                        StatTile("Ko'rishlar", if (ready) formatCount(stats.views) else "—", Modifier.weight(1f))
                        StatTile("O'rtacha baho", if (ready && stats.rating > 0) "★ " + "%.1f".format(java.util.Locale.US, stats.rating) else "—", Modifier.weight(1f))
                    }
                }
                fullSpan { Text("Animelar", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(top = 4.dp)) }
            },
            emptyContent = {
                fullSpan {
                    when {
                        page == null && !failed -> LoadingBox()
                        failed -> EmptyState(Icons.Filled.Menu, "Ma'lumot yuklanmadi")
                        else -> EmptyState(Icons.Filled.Menu, "Bu tarjimonning animelari hali yo'q")
                    }
                }
            },
        )
        Icon(
            Icons.AutoMirrored.Filled.ArrowBack, "Orqaga", tint = Color.White,
            modifier = Modifier.padding(start = 8.dp, top = 8.dp).size(44.dp).clip(CircleShape).clickable(onClick = onBack).padding(10.dp),
        )
    }
}
